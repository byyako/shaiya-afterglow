//! The guild bank (`docs/original/guild-bank.md`): the item move `0x0204` with bag 255 on one side. The rules are
//! `crate::guild_bank`'s; the guild's bank and the packets to its members are `crate::guild`'s.

use super::*;
use crate::world::create_type;

/// A move into, out of or inside the guild bank (section 5). Deviation 1: it needs a guild house map and a warehouse keeper in
/// view; otherwise it is ignored like the original's other refusals. Moves run one at a time, from the plan to the apply.
pub(super) async fn item_move<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    map: u16,
    character: &PlayCharacter,
    state: &mut WorldState,
    from: (u8, u8),
    to: (u8, u8),
) -> Result<()> {
    let id = character.id;
    if ctx.data.world.create_type(map) != create_type::GUILD_HOUSE || !ctx.zones.npc_kind_in_view(map, id, WAREHOUSE_KEEPER) {
        tracing::debug!(id, ?from, ?to, "guild bank move ignored: no house warehouse keeper");
        return Ok(());
    }
    // Planned, written and applied under the turn; what is answered is sent after it is dropped (`docs/server/audit.md` R8).
    let answer = {
        let _turn = ctx.zones.guild_bank_turn().await;
        moved(ctx, character, state, from, to).await?
    };
    if let Some(answer) = answer {
        conn.send(answer).await?;
    }
    Ok(())
}

/// `item_move` under the turn: the packet to answer with, if any.
async fn moved<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    from: (u8, u8),
    to: (u8, u8),
) -> Result<Option<Vec<u8>>> {
    let id = character.id;
    let rules = ctx.data.world.guild_house();
    let planned = ctx.zones.guild_bank_plan(id, &state.inventory, &ctx.data.items, rules, state.money, from, to);
    let (guild, planned) = match planned {
        None => {
            tracing::debug!(id, ?from, ?to, "guild bank move without a guild");
            return Ok(None);
        }
        Some((_, Err(None))) => {
            tracing::debug!(id, ?from, ?to, "guild bank move ignored");
            return Ok(None);
        }
        Some((_, Err(Some((kind, needed))))) => return Ok(Some(gd::house_refused(kind, needed))),
        Some((guild, Ok(planned))) => (guild, planned),
    };
    let Some(rows) = ctx.store.guild_bank(id, guild, -i64::from(planned.fee), &planned.changes).await? else {
        bail!("the database refused a guild bank move the server allowed: {:?}", planned.changes);
    };
    // `0x0413` for an item into the bank, `0x0414` for one out of it (a swap does both); a move inside the bank is not logged. The
    // items are named by the rows the write left, as they were before the move is applied.
    if planned.kind != crate::guild_bank::Kind::Rearrange
        && let [change] = &planned.changes[..]
        && let [changed] = &rows[..]
    {
        let (bag, bank_slot) = if from.0 == w::GUILD_BANK { (to, from.1) } else { (from, to.1) };
        let (guild_name, bank_unit) = ctx.zones.guild_bank_view(guild, bank_slot).unwrap_or_default();
        let bag_unit = state.inventory.get(bag.0, bag.1).cloned();
        let header = record::header(character, state);
        // The row of the item now in the bag place and of the one now in the bank slot (a swap names `a` then `b`).
        let (bag_now, bank_now) = match change {
            SlotChange::Swap { a, .. } if a.0 == w::GUILD_BANK => (changed.swapped, changed.row),
            SlotChange::Swap { .. } => (changed.row, changed.swapped),
            _ => (changed.row, changed.row),
        };
        // What was in the bag place goes into the bank (`0x0413`); what was in the bank slot comes out (`0x0414`).
        if let Some(unit) = &bag_unit {
            let part = ctx.game_log.item(unit, bank_now);
            ctx.game_log.action(crate::game_log::guild_bank_in(
                header.clone(),
                &part,
                unit.count,
                bank_slot,
                bag.0,
                bag.1,
                guild,
                &guild_name,
            ));
        }
        if let Some(unit) = &bank_unit {
            let part = ctx.game_log.item(unit, bag_now);
            ctx.game_log.action(crate::game_log::guild_bank_out(header, &part, unit.count, bank_slot, bag.0, bag.1, guild, &guild_name));
        }
    }
    state.money -= planned.fee;
    tracing::info!(character = id, guild, kind = ?planned.kind, ?from, ?to, fee = planned.fee, money = state.money, "guild bank move");
    Ok(ctx.zones.guild_bank_apply(guild, id, &mut state.inventory, &planned, from, to, state.money))
}
