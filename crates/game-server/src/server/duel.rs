//! Duels on the connection's side (`docs/original/duel.md`): the `0x24xx` requests, the checks against the bags, taking the stakes
//! out for the fight and putting them back, and victories and defeats. The zone does what touches the other player.

use super::*;
use crate::duel::{self as du, Held};
use shaiya_proto::duel as dp;

/// `m_dwKillCount[2]` and `[3]`.
const VICTORIES: usize = 2;
const DEFEATS: usize = 3;

/// The `0x24xx` requests (`CUser::PacketPVP`). The server's own opcodes and anything past `0x240C` close the connection, as in the
/// original.
pub(super) async fn packet<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    match opcode {
        dp::OP_REQUEST => {
            let Some(target) = dp::parse_id(payload) else { return Ok(()) };
            ctx.zones.duel_request(map, id, target, now);
        }
        dp::OP_ANSWER => {
            let Some(answer) = dp::parse_byte(payload) else { return Ok(()) };
            ctx.zones.duel_answer(map, id, answer != dp::ANSWER_REFUSE);
        }
        dp::OP_CANCEL => ctx.zones.duel_give_up(map, id, now),
        dp::OP_STAKE_READY => {
            let Some(request) = dp::parse_byte(payload) else { return Ok(()) };
            if ctx.zones.duel_ready(map, id, request) {
                take_stakes(ctx, character, state).await?;
            }
        }
        dp::OP_STAKE_ADD => {
            let Some(offer) = dp::parse_offer(payload) else { return Ok(()) };
            // As for trades: bags 1 up, enough of it, no quest, bound or type 99 item (deviation 4).
            match tradeable(state, &ctx.data.items, offer) {
                Some(item) => {
                    let item = item.clone();
                    if !ctx.zones.duel_stake(map, id, offer, &item) {
                        tracing::debug!(id, ?offer, "duel stake ignored");
                    }
                }
                None => tracing::debug!(id, ?offer, "duel stake refused: no such item, too many, a quest item or not tradeable"),
            }
        }
        dp::OP_STAKE_REMOVE => {
            let Some(stake_slot) = dp::parse_byte(payload) else { return Ok(()) };
            ctx.zones.duel_unstake(map, id, stake_slot);
        }
        dp::OP_STAKE_MONEY => {
            let Some(money) = dp::parse_money(payload) else { return Ok(()) };
            if money <= state.money {
                ctx.zones.duel_money(map, id, money);
            }
        }
        _ => bail!("unexpected duel opcode {opcode:#06x}"),
    }
    Ok(())
}

/// Both are ready (`CUser::ExchangePvP`, section 4): this side's stakes come out of its bags in one transaction (the money it has, each
/// item still as staked; otherwise the duel is cancelled for both and nothing is taken, deviation 5); `0x0213`, a `0x0206` for each
/// and `0x2408 0` to the player, then the zone holds them for the end of the fight.
pub(super) async fn take_stakes<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let Some((offers, money)) = ctx.zones.duel_offers(map, id) else { return Ok(()) };
    let gives = du::prepare(&offers, &state.inventory).filter(|_| money <= state.money);
    let Some(gives) = gives else {
        tracing::info!(id, "duel cancelled: a stake is gone, smaller or no longer paid for");
        ctx.zones.duel_stakes_failed(map, id, Instant::now());
        return Ok(());
    };
    let Some(taken) = ctx.store.take_stakes(id, money, &du::taken(&gives)).await? else {
        bail!("the database refused duel stakes the server allowed: {gives:?}");
    };
    let rows: Vec<crate::backend::Stored> = taken.iter().map(|&(_, row)| row).collect();
    let kept: Vec<Kept> = taken.into_iter().map(|(kept, _)| kept).collect();
    state.money -= money;
    // `0x0805` and `0x040F` (`CUser::ExchangePvP`): the stakes handed over, with the duel partner; the money first.
    let (other_id, other_name) = ctx.zones.duel_partner(map, id).unwrap_or_default();
    let header = record::header(character, state);
    if money > 0 {
        ctx.game_log.action(crate::game_log::stake_money(header.clone(), money, state.money, other_id, &other_name));
    }
    for ((item, count), row) in gives.iter().zip(&rows) {
        let part = ctx.game_log.item(&ItemUnit { count: *count, ..item.clone() }, Some(*row));
        ctx.game_log.action(crate::game_log::stake_give(header.clone(), &part, other_id, *count, 0, &other_name, item.bag, item.slot));
    }
    if money > 0 {
        let _ = state.outbox.send(w::set_money(state.money));
    }
    for (item, count) in &gives {
        let (bag, slot) = (item.bag, item.slot);
        let change = if *count == item.count {
            SlotChange::Remove { bag, slot }
        } else {
            SlotChange::SetCount { bag, slot, count: item.count - count }
        };
        state.inventory.apply(&change);
        let _ = state.outbox.send(gp::item_left(bag, slot, state.inventory.get(bag, slot)));
    }
    let _ = state.outbox.send(dp::window_closed(dp::WINDOW_DONE));
    tracing::info!(id, money, items = gives.len(), "duel stakes taken");
    if let Some(held) = ctx.zones.duel_staked(map, id, du::held(money, &gives, kept), Instant::now()) {
        let (at, mut out) = (state.position, Vec::new());
        put_back(ctx, id, state, map, at, held, &mut out).await?;
        for packet in out {
            let _ = state.outbox.send(packet);
        }
    }
    Ok(())
}

/// Stakes a player took with it out of its map, back into its money and bags (section 6.7, deviation 3); what does not fit goes on
/// the ground of `map` at `at`, its own. Its packets go into `out`. The gold and every item are planned on a copy of the bags and
/// saved in one transaction; on a failure everything goes on the ground for its owner before the error is returned (audit R15).
pub(super) async fn put_back<S: Backend>(
    ctx: &Context<S>,
    id: u32,
    state: &mut WorldState,
    map: u16,
    at: [f32; 3],
    held: Held,
    out: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let now = Instant::now();
    let money = held.money.min(u32::MAX - state.money);
    let ground = |loot: Loot| ctx.zones.drop_on_ground(map, loot, at, id, 0, now, gp::KIND_PLAYER_DROP, crate::zone::Dropped::death(id));
    // What does not fit goes on the ground as today; the items that do are planned one after the other on a scratch copy.
    let mut scratch = state.inventory.clone();
    let mut changes = Vec::new();
    let mut plans = Vec::new();
    let mut left_over = Vec::new();
    for loot in &held.items {
        let Loot::Item { item_type, type_id, count, quality, kept } = *loot else { continue };
        let Some(def) = ctx.data.items.get(item_type, type_id) else {
            left_over.push(*loot);
            continue;
        };
        let pick = scratch.pick_up(def, count, quality, kept);
        for change in &pick.changes {
            scratch.apply(change);
        }
        if pick.left > 0 {
            left_over.push(Loot::Item { item_type, type_id, count: pick.left, quality, kept });
        }
        plans.push((def, changes.len()..changes.len() + pick.changes.len()));
        changes.extend(pick.changes);
    }
    if money > 0 || !changes.is_empty() {
        let saved = ctx.store.trade(id, i64::from(money), &changes).await;
        let Ok(Some(rows)) = saved else {
            // Nothing was saved: all of it, the gold included, lies at the player's feet for the player.
            for loot in held.loot() {
                ground(loot);
            }
            saved?;
            bail!("the database refused returned duel stakes the server allowed: {changes:?}");
        };
        if money > 0 {
            state.money += money;
            out.push(w::set_money(state.money));
        }
        for (def, range) in plans {
            picked_up(ctx, state, def, &changes[range.clone()], &rows[range], None, out);
        }
    }
    if money < held.money {
        ground(Loot::Gold(held.money - money));
    }
    for loot in left_over {
        ground(loot);
    }
    tracing::info!(id, money = held.money, "duel stakes back in the bags");
    Ok(())
}

/// What a player takes out of its zone (`Zones::leave`): HP, MP, SP and applied skills kept; duel stakes back into its bags, their
/// packets returned for the caller to send or drop.
pub(super) async fn left_zone<S: Backend>(
    ctx: &Context<S>,
    id: u32,
    state: &mut WorldState,
    left: crate::zone::Left,
) -> Result<Vec<Vec<u8>>> {
    state.health = left.health;
    state.buffs = left.buffs;
    let mut out = Vec::new();
    if let Some(held) = left.stakes {
        let (map, at) = (state.map, state.position);
        put_back(ctx, id, state, map, at, held, &mut out).await?;
    }
    Ok(out)
}

/// A duel won or lost (`CUser::PvPChkLoseDeath`, section 6.1 step 3): a victory or a defeat, below 65535, saved; no packet.
pub(super) async fn result<S: Backend>(
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &WorldState,
    won: bool,
    opponent: (u32, &[u8]),
) -> Result<()> {
    // `0x0E01` for the winner, `0x0E02` for the loser.
    let (other_id, other_name) = (opponent.0, String::from_utf8_lossy(opponent.1).trim_end_matches('\0').to_owned());
    let header = record::header(character, state);
    ctx.game_log.action(if won {
        crate::game_log::duel_win(header, other_id, &other_name)
    } else {
        crate::game_log::duel_lose(header, other_id, &other_name)
    });
    let i = if won { VICTORIES } else { DEFEATS };
    let Some(count) = du::counted(character.kills[i]) else { return Ok(()) };
    character.kills[i] = count;
    ctx.store.save_kills(character.id, character.kills).await?;
    tracing::info!(id = character.id, won, count, "duel result");
    Ok(())
}

/// A duel ended without a winner (`PvPChkLeaveBy*`, `0x0E03`): written for each side, with the cancel reason.
pub(super) fn cancelled<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, reason: u8, opponent: (u32, &[u8])) {
    let name = String::from_utf8_lossy(opponent.1).trim_end_matches('\0').to_owned();
    ctx.game_log.action(crate::game_log::duel_cancel(record::header(character, state), opponent.0, &name, reason));
}
