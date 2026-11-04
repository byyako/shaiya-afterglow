//! Personal shops on the connection's side (`docs/original/personal-shop.md`): the `0x23xx` requests, the checks against the bags,
//! a purchase waiting for the seller's connection, and the sales a seller's connection carries out. The zone does what touches the
//! other player.

use super::*;
use crate::my_shop::{self as ms, Bought, Purchase};
use shaiya_proto::my_shop as mp;

/// The `0x23xx` requests (`CUser::PacketMyShop`). Each first ends a trade (not the shop); unknown ones are ignored, as the original.
pub(super) async fn packet<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    ctx.zones.trade_end(id);
    match opcode {
        mp::OP_BEGIN => {
            // Sitting (`m_byMotion` 1), on a map no `deny personal_shop` rule closes (deviation 1; the `market_maps` setting makes
            // the rule that closes every other map).
            let refusal = ctx.zones.rules().denied(crate::rules::Point::PersonalShop, map, &rules_who(character, country));
            if state.motion != w::MOTION_SIT || refusal.is_some() {
                match &refusal {
                    Some(refusal) if !refusal.name.starts_with("setting.") => {
                        tracing::debug!(id, map, point = "personal_shop", rule = &*refusal.name, "refused by a rule");
                    }
                    _ => tracing::debug!(id, map, motion = state.motion, "personal shop refused: not sitting or not a market map"),
                }
                if state.motion == w::MOTION_SIT && let Some(refusal) = refusal {
                    super::refused::tell_refused(conn, state, &refusal).await?;
                }
                return Ok(());
            }
            ctx.zones.shop_begin(map, id);
        }
        mp::OP_END => ctx.zones.shop_finish(map, id),
        mp::OP_START => {
            let Some((len, title)) = mp::parse_start(payload) else { return Ok(()) };
            ctx.zones.shop_open(map, id, len, title);
        }
        mp::OP_STOP => ctx.zones.shop_close(map, id),
        mp::OP_ADD => {
            let Some(add) = mp::parse_add(payload) else { return Ok(()) };
            match placeable(state, &ctx.data.items, &add) {
                Some(item) => {
                    if !ctx.zones.shop_place(map, id, add, item.clone()) {
                        tracing::debug!(id, ?add, "personal shop item ignored");
                    }
                }
                None => tracing::debug!(id, ?add, "personal shop item refused: no such item, a quest or bound item, or too dear"),
            }
        }
        mp::OP_REMOVE => {
            let Some(shop_slot) = mp::parse_byte(payload) else { return Ok(()) };
            ctx.zones.shop_take_off(map, id, shop_slot);
        }
        mp::OP_SET_PRICE => {
            let Some((shop_slot, price)) = mp::parse_set_price(payload) else { return Ok(()) };
            ctx.zones.shop_set_price(map, id, shop_slot, price);
        }
        mp::OP_VISIT => {
            let Some(seller) = mp::parse_id(payload) else { return Ok(()) };
            ctx.zones.shop_visit(map, id, seller);
        }
        mp::OP_LEAVE => ctx.zones.shop_leave(map, id),
        mp::OP_BUY => {
            let Some((shop_slot, count)) = mp::parse_buy(payload) else { return Ok(()) };
            buy(ctx, character, state, shop_slot, count).await;
        }
        _ => tracing::debug!(id, opcode = format_args!("{opcode:#06x}"), "personal shop request ignored"),
    }
    Ok(())
}

/// The item a `0x2305` names, when it may be placed (section 3.3, deviation 2): an inventory bag in use, an item there that a trade
/// could take, all of it at that price within 32 bits.
fn placeable<'a>(state: &'a WorldState, items: &ItemDefs, add: &mp::Add) -> Option<&'a ItemUnit> {
    if add.bag == 0 || add.bag > state.inventory.max_bag() {
        return None;
    }
    let item = state.inventory.get(add.bag, add.slot)?;
    ms::placeable(item, items.get(item.item_type, item.type_id), add.price).then_some(item)
}

/// `0x230D` (section 5): the buyer's checks (the listing and its count, the money, an empty slot), then the purchase waits for the
/// seller's connection, which writes it and sends the buyer its `0x230D 1`; the item and the money follow here. Nothing else happens
/// on this connection meanwhile, so its bags and money stay as checked.
async fn buy<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, shop_slot: u8, count: u8) {
    let id = character.id;
    if count == 0 {
        return;
    }
    let Some((seller, listed)) = ctx.zones.shop_listing(state.map, id, shop_slot) else { return };
    let failed = |state: &WorldState, code: u8| {
        let _ = state.outbox.send(mp::buy_failed(code));
    };
    let Some(seen) = listed.filter(|l| count <= l.item.count) else { return failed(state, mp::BUY_FAILED) };
    // Deviation 3: a total past 32 bits is refused, not wrapped.
    let Some(total) = ms::total(count, seen.price) else { return failed(state, mp::BUY_FAILED) };
    if state.money < total {
        return failed(state, mp::BUY_NO_GOLD);
    }
    let Some(&to) = state.inventory.free_slots().first() else { return failed(state, mp::BUY_FAILED) };
    let (reply, mut answer) = tokio::sync::oneshot::channel();
    let purchase = Purchase { buyer: id, buyer_money: state.money, shop_slot, count, seen, total, to, reply };
    if ctx.zones.shop_queue(state.map, seller, purchase).is_err() {
        return failed(state, mp::BUY_FAILED);
    }
    let outcome = match timeout(ms::SALE_WAIT, &mut answer).await {
        Ok(outcome) => outcome.ok(),
        Err(_) if ctx.zones.shop_withdraw(seller, id) => {
            tracing::warn!(id, seller, "purchase withdrawn: the seller's connection did not take it");
            None
        }
        // The seller's connection has it: its answer comes.
        Err(_) => answer.await.ok(),
    };
    match outcome {
        Some(Ok(Bought { item, total, seller, row })) => {
            // `0x0405` for the buyer (`CUser::PacketMyShop`): the item got, with the seller and what was paid.
            let part = ctx.game_log.item(&item, row);
            let name = ctx.zones.name_of(seller).unwrap_or_default();
            ctx.game_log.action(crate::game_log::trade_receive(record::header(character, state), &part, seller, item.count, total, &name));
            state.inventory.receive(item);
            state.money -= total;
        }
        Some(Err(code)) => failed(state, code),
        // The shop ended meanwhile, its purchases dropped.
        None => failed(state, mp::BUY_FAILED),
    }
}

/// Purchases wait for this seller (`ZoneEvent::Sale`): each is carried out in turn.
pub(super) async fn sales<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    while let Some(purchase) = ctx.zones.shop_next_sale(character.id) {
        sell(ctx, character, state, purchase).await?;
    }
    Ok(())
}

/// One purchase on the seller's side (section 5, deviations 7 and 8): the shop slot and the bag still hold what the buyer saw, the
/// money fits; both characters' money and the item in one transaction; then `0x230D 1` to the buyer, `0x230E` to the visitors and
/// `0x2308` here, and the buyer's connection gets its share.
async fn sell<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, purchase: Purchase) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let listed = ctx.zones.shop_slot(map, id, purchase.shop_slot);
    let bag_item = listed.as_ref().and_then(|l| state.inventory.get(l.bag, l.slot));
    let sale = match ms::plan(&purchase, id, listed.as_ref(), bag_item, state.money, map) {
        Ok(sale) => sale,
        Err(code) => {
            tracing::debug!(seller = id, buyer = purchase.buyer, code, "purchase refused");
            let _ = purchase.reply.send(Err(code));
            return Ok(());
        }
    };
    let money = [(id, i64::from(sale.gain)), (purchase.buyer, -i64::from(purchase.total))];
    let saved = ctx.store.exchange(money, std::slice::from_ref(&sale.handover)).await;
    let Ok(Some(rows)) = saved else {
        let _ = purchase.reply.send(Err(mp::BUY_FAILED));
        saved?;
        bail!("the database refused a sale the server allowed: {:?}", sale.handover);
    };
    let handed = rows.first().copied().unwrap_or_default();
    // `0x0405` for the seller: the item sold (the row it keeps the rest in, or the one that moved), the buyer and what it paid.
    if let Some(unit) = bag_item {
        let part = ctx.game_log.item(&ItemUnit { count: purchase.count, ..unit.clone() }, handed.giver.or(handed.receiver));
        let name = ctx.zones.name_of(purchase.buyer).unwrap_or_default();
        ctx.game_log.action(crate::game_log::trade_receive(
            record::header(character, state),
            &part,
            purchase.buyer,
            purchase.count,
            sale.gain,
            &name,
        ));
    }
    state.inventory.apply(&sale.change);
    state.money += sale.gain;
    let bought = mp::bought(purchase.buyer_money - purchase.total, purchase.shop_slot, sale.left, purchase.to, &sale.got, purchase.count);
    ctx.zones.shop_sold(map, id, purchase.shop_slot, sale.left, purchase.buyer, &bought);
    let _ = state.outbox.send(mp::sold(purchase.shop_slot, sale.left, state.money));
    tracing::info!(
        seller = id,
        buyer = purchase.buyer,
        count = purchase.count,
        total = purchase.total,
        gain = sale.gain,
        "sold from a personal shop"
    );
    let _ = purchase.reply.send(Ok(Bought { item: sale.got, total: purchase.total, seller: id, row: handed.receiver }));
    Ok(())
}
