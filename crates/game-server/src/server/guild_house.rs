//! Guild houses (`docs/original/guild-house.md`): returning Etin (`0x0D35`), NPC upgrades (`0x0D25`), buying the house (`0x0D34`),
//! and the house NPCs' rates and refusals (`0x0D28`) for the shop, gatekeeper, repair and lapis requests. The rules are
//! `crate::guild_house`'s; the guild's state is `crate::guild`'s.

use super::*;
use crate::guild_house::{self as gh, House};

/// The player's guild and house when the player stands on a house map; `None` elsewhere, `Some(None)` there without a guild.
fn in_house<S>(ctx: &Context<S>, map: u16, id: u32) -> Option<Option<(u8, House)>> {
    (ctx.data.world.create_type(map) == crate::world::create_type::GUILD_HOUSE)
        .then(|| ctx.zones.guild_house_of(id).map(|(_, country, house)| (country, house)))
}

/// A merchant's rate on a house map (section 9): `None` elsewhere, the rate, or the `0x0D28` to send.
pub(super) fn shop_rate<S>(ctx: &Context<S>, map: u16, id: u32, npc_type_id: u16) -> Option<Result<f64, Vec<u8>>> {
    let guild = in_house(ctx, map, id)?;
    let Some((country, house)) = guild else { return Some(Err(gd::house_refused(gh::REFUSED_RANKING, gh::ANY_RANKING))) };
    let shop_type = ctx.data.world.shop_type(npc_type_id);
    Some(house.shop_rate(ctx.data.world.guild_house(), country, npc_type_id, shop_type).map_err(|(r, n)| gd::house_refused(r, n)))
}

/// A gatekeeper's rate on a house map: `None` elsewhere, the rate, or the `0x0D28` to send.
pub(super) fn gate_rate<S>(ctx: &Context<S>, map: u16, id: u32, npc_type_id: u16) -> Option<Result<f64, Vec<u8>>> {
    let guild = in_house(ctx, map, id)?;
    let Some((country, house)) = guild else { return Some(Err(gd::house_refused(gh::REFUSED_RANKING, gh::ANY_RANKING))) };
    Some(house.gate_rate(ctx.data.world.guild_house(), country, npc_type_id).map_err(|level| gd::house_refused(gh::REFUSED_LEVEL, level)))
}

/// The blacksmith's house: `None` elsewhere, the house when the guild's ranking is 14 or better, or the `0x0D28` to send.
fn smith<S>(ctx: &Context<S>, map: u16, id: u32) -> Option<Result<House, Vec<u8>>> {
    let guild = in_house(ctx, map, id)?;
    Some(match guild {
        Some((_, house)) if house.ranking <= gh::SMITH_RANKING => Ok(house),
        _ => Err(gd::house_refused(gh::REFUSED_RANKING, gh::SMITH_RANKING)),
    })
}

/// The repair rate (1 elsewhere), or the `0x0D28` to send.
pub(super) fn repair_rate<S>(ctx: &Context<S>, map: u16, id: u32) -> Result<f64, Vec<u8>> {
    smith(ctx, map, id).map_or(Ok(1.0), |house| house.map(|h| h.repair_rate(ctx.data.world.guild_house())))
}

/// The lapis rates (none elsewhere), or the `0x0D28` to send; `link` takes the bonus too.
pub(super) fn lapis_rates<S>(ctx: &Context<S>, map: u16, id: u32, link: bool) -> Result<crate::lapis::HouseRates, Vec<u8>> {
    let rules = ctx.data.world.guild_house();
    smith(ctx, map, id).map_or(Ok(crate::lapis::NO_HOUSE), |house| {
        house.map(|h| crate::lapis::HouseRates { price: h.mix_rate(rules), bonus: if link { h.mix_bonus(rules) } else { 0 } })
    })
}

/// `0x0D34` and `0x0D35` come from an NPC's menu: the NPC must be in view (deviation 2).
fn npc_near<S>(ctx: &Context<S>, map: u16, id: u32, npc_id: u32) -> bool {
    let near = ctx.zones.npc_in_view(map, id, npc_id).is_some();
    if !near {
        tracing::debug!(id, npc_id, "guild house request without the NPC in view ignored");
    }
    near
}

/// `0x0D35` (section 3): every Etin in the bags that is worth something taken out, the places to the player, the Etin to the guild. The
/// items and the guild's Etin are one transaction that holds only while the player is still a member (audit R16); Etin items worth 0
/// stay in the bags.
pub(super) async fn return_etin<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    npc_id: u32,
) -> Result<()> {
    let id = character.id;
    if !npc_near(ctx, state.map, id, npc_id) {
        return Ok(());
    }
    // Nothing is taken out of the bags of a player without a guild.
    let Some((guild, _, _)) = ctx.zones.guild_house_of(id) else { return Ok(()) };
    // The items leave the bags: a trade or a duel stakes window that held one ends (audit R3).
    ctx.zones.cancel_actions(id);
    let mut places = Vec::new();
    let mut sum = 0u32;
    for item in state.inventory.usable_items().filter(|i| i.bag != 0 && i.item_type == gh::ETIN_TYPE) {
        let special = ctx.data.items.get(item.item_type, item.type_id).map_or(0, |d| d.special);
        let worth = gh::etin_of(special, item.count);
        if worth > 0 {
            sum = sum.saturating_add(worth);
            places.push((item.bag, item.slot));
        }
    }
    let changes: Vec<SlotChange> = places.iter().map(|&(bag, slot)| SlotChange::Remove { bag, slot }).collect();
    if !changes.is_empty() {
        let Some(rows) = ctx.store.guild_return_etin(id, guild, sum, &changes).await? else {
            tracing::info!(id, guild, "Etin return refused: no longer a member of the guild, or the bags changed");
            return Ok(());
        };
        // `0x0415` (`CUser::PacketGuild`): each Etin item handed in, named by its row.
        for (&(bag, slot), row) in places.iter().zip(&rows) {
            if let Some(unit) = state.inventory.get(bag, slot) {
                let item = ctx.game_log.item(unit, row.row);
                ctx.game_log.action(crate::game_log::etin_item(record::header(character, state), &item, bag, slot));
            }
        }
        for change in &changes {
            state.inventory.apply(change);
        }
        ctx.zones.guild_return_etin(guild, id, sum);
    }
    conn.send(gd::etin_returned(&places)).await
}

/// `0x0D34` (section 5): the master pays `HouseBuyMoney`. The money and the house are one transaction that holds only while the player
/// is the guild's master and the house is not bought (audit R16).
pub(super) async fn buy_house<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    id: u32,
    state: &mut WorldState,
    npc_id: u32,
) -> Result<()> {
    if !npc_near(ctx, state.map, id, npc_id) {
        return Ok(());
    }
    let rules = ctx.data.world.guild_house();
    let guild = match ctx.zones.guild_house_purchase(id, state.money, rules) {
        Ok(guild) => guild,
        Err(result) => return conn.send(gd::house_bought(result, state.money)).await,
    };
    if !ctx.store.buy_guild_house(id, guild, rules.buy_money, rules.keep_etin).await? {
        // The guild was disbanded or the master changed while the request was on its way: nothing was paid.
        tracing::info!(id, guild, "guild house purchase refused: not the master any more, bought already or the guild is gone");
        return conn.send(gd::house_bought(gh::buy::NOT_MASTER, state.money)).await;
    }
    state.money -= rules.buy_money;
    ctx.zones.guild_house_bought(guild, rules);
    tracing::info!(id, guild, price = rules.buy_money, money = state.money, "guild house purchase");
    conn.send(gd::house_bought(gh::buy::BOUGHT, state.money)).await
}
