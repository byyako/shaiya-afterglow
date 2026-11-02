//! The item mall on the connection's side (`docs/original/item-mall.md`): purchases, gifts, the point refresh, the two storages,
//! and what the mall has for the account while it is in the world. The rules are `crate::mall`'s; every change is one transaction
//! (deviation 1).

use super::*;
use crate::backend::{Bought, GiftNotice, GiftOrder, GiftOutcome, Purchase, Storage, StoredPointItem};
use crate::mall as ml;
use shaiya_proto::point as pp;

/// The connection's copy of the mall: the account's points as last read, when `0x2605` may read them again, the two storages.
#[derive(Debug, Default)]
pub(super) struct MallState {
    pub points: u32,
    pub reload_after: Option<Instant>,
    pub stored: Vec<StoredPointItem>,
    pub billing: Vec<StoredPointItem>,
}

impl MallState {
    pub(super) fn new(mall: &crate::backend::Mall) -> Self {
        Self { points: mall.points, reload_after: None, stored: mall.stored.clone(), billing: mall.billing.clone() }
    }
}

/// After the enter burst (deviation 6): the account is reachable for gifts, and the gifts that came while it was away are told, one
/// `0x2604` without rows each.
pub(super) async fn enter<S: Backend>(conn: &mut Connection, ctx: &Context<S>, user_id: u32, state: &WorldState) -> Result<()> {
    ctx.online.enter_world(user_id, state.events.clone());
    for notice in ctx.store.take_gift_notices(user_id, ctx.config.server_id).await? {
        conn.send(pp::gift_received(notice.sender.as_bytes(), notice.code.as_bytes(), &[])).await?;
    }
    Ok(())
}

/// Leaving the world: gifts the character was not told of become notices for its next entry.
pub(super) async fn leave<S: Backend>(ctx: &Context<S>, user_id: u32) {
    for gift in ctx.online.leave_world(user_id) {
        let notice = GiftNotice { sender: gift.sender, code: gift.code };
        if let Err(error) = ctx.store.add_gift_notice(user_id, ctx.config.server_id, &notice).await {
            tracing::warn!(%error, user_id, "could not keep a gift notice");
        }
    }
}

/// `ZoneEvent::Mall`: gifts that came (into the storage copy, `0x2604` with their rows) and the points again after a change.
pub(super) async fn inbox<S: Backend>(conn: &mut Connection, ctx: &Context<S>, user_id: u32, state: &mut WorldState) -> Result<()> {
    let inbox = ctx.online.take_mall(user_id);
    for gift in inbox.gifts {
        // A character staying after its connection ended sees nothing: a notice for its next entry instead.
        if conn.closed {
            ctx.store.add_gift_notice(user_id, ctx.config.server_id, &GiftNotice { sender: gift.sender, code: gift.code }).await?;
            continue;
        }
        let units: Vec<_> = gift.rows.iter().map(ml::stored_unit).collect();
        state.mall.stored.extend(gift.rows);
        conn.send(pp::gift_received(gift.sender.as_bytes(), gift.code.as_bytes(), &units)).await?;
    }
    if inbox.points {
        state.mall.points = ctx.store.points(user_id).await?;
        conn.send(pp::reload(state.mall.points)).await?;
    }
    Ok(())
}

/// A `0x26xx` request in the world (`CUser::PacketPoint`) or `0xB102` (`CUser::PacketBilling`); other opcodes of the families are
/// ignored.
pub(super) async fn packet<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    match opcode {
        // The console's `shop silent` (`g_bDisableShop` 2): no answer at all to a purchase or a gift
        // (`docs/original/operator-console.md` section 2).
        pp::OP_BUY | pp::OP_GIFT if ctx.switches.shop() == switches::SHOP_SILENT => {
            tracing::info!(id = character.id, opcode = format_args!("{opcode:#06x}"), "item mall request dropped: the mall is silent");
            Ok(())
        }
        pp::OP_BUY => {
            let Some(code) = pp::parse_buy(payload) else { return Ok(()) };
            let answer = buy(ctx, player.user_id, character, state, &code).await?;
            conn.send(answer).await
        }
        pp::OP_GIFT => {
            let Some((code, target)) = pp::parse_gift(payload) else { return Ok(()) };
            let answer = gift(ctx, player.user_id, character, state, &code, &target).await?;
            conn.send(answer).await
        }
        pp::OP_RELOAD => {
            let now = Instant::now();
            if state.mall.reload_after.is_none_or(|after| now >= after) {
                state.mall.points = ctx.store.points(player.user_id).await?;
                state.mall.reload_after = Some(now + ml::RELOAD_EVERY);
            }
            conn.send(pp::reload(state.mall.points)).await
        }
        pp::OP_STORED_GET | pp::OP_BILLING_GET => {
            let Some(slot) = pp::parse_slot(payload) else { return Ok(()) };
            let storage = if opcode == pp::OP_STORED_GET { Storage::Mall } else { Storage::Billing };
            let place = take(ctx, character, state, storage, slot).await?;
            conn.send(pp::taken(opcode, slot, place)).await
        }
        opcode => {
            tracing::debug!(id = character.id, opcode = format_args!("{opcode:#06x}"), "item mall request ignored");
            Ok(())
        }
    }
}

/// The log row a purchase or gift answers with: the code, now, the price.
fn log_row(code: &[u8], price: u32) -> pp::LogRow {
    pp::LogRow { code: code.to_vec(), time: local_packed_time(), points: price }
}

/// `0x2602` (section 3): the answer.
async fn buy<S: Backend>(
    ctx: &Context<S>,
    user_id: u32,
    character: &PlayCharacter,
    state: &mut WorldState,
    code: &[u8],
) -> Result<Vec<u8>> {
    let id = character.id;
    let product = ctx.data.products.get(code).filter(|_| ctx.switches.shop() == switches::SHOP_ON);
    let Some(product) = product else {
        tracing::info!(id, code = %String::from_utf8_lossy(code), "item mall purchase refused: no such product (or the mall is off)");
        return Ok(pp::buy_refused(pp::BUY_NO_PRODUCT));
    };
    if state.mall.points < product.price {
        return Ok(pp::buy_refused(pp::BUY_NO_POINTS));
    }
    let items = match ml::plan_buy(product, &ctx.data.items, &state.inventory.free_slots()) {
        Ok(items) => items,
        Err(result) => return Ok(pp::buy_refused(result)),
    };
    let purchase = Purchase { user_id, character_id: id, code: product.code.clone(), price: product.price, items };
    match ctx.store.buy(&purchase).await? {
        Bought::Done(left, rows) => {
            for (item, row) in purchase.items.iter().zip(&rows) {
                state.inventory.apply(&SlotChange::Insert(*item));
                // `0x0410` (`CUser::PacketPoint`): each item bought, named by the row it got.
                if let Some(part) = record::item_in(ctx, state, (item.bag, item.slot), Some(*row)) {
                    ctx.game_log.action(crate::game_log::mall_buy(record::header(character, state), &part, item.count));
                }
            }
            state.mall.points = left;
            tracing::info!(id, code = %product.code, price = product.price, left, items = purchase.items.len(), "item mall purchase");
            let placed: Vec<_> = purchase
                .items
                .iter()
                .map(|i| pp::Placed { bag: i.bag, slot: i.slot, item_type: i.item_type, type_id: i.type_id, count: i.count })
                .collect();
            Ok(pp::bought(left, &log_row(code, product.price), &placed))
        }
        Bought::NoPoints => {
            // Spent elsewhere meanwhile (another server, the operator tool).
            state.mall.points = ctx.store.points(user_id).await?;
            Ok(pp::buy_refused(pp::BUY_NO_POINTS))
        }
        Bought::Refused => {
            tracing::warn!(id, code = %product.code, "the database refused an item mall purchase the server allowed");
            Ok(pp::buy_refused(pp::BUY_NO_ROOM))
        }
    }
}

/// `0x2603` (section 4): the answer to the buyer; the recipient is told, or gets a notice.
async fn gift<S: Backend>(
    ctx: &Context<S>,
    user_id: u32,
    character: &PlayCharacter,
    state: &mut WorldState,
    code: &[u8],
    target: &[u8],
) -> Result<Vec<u8>> {
    let id = character.id;
    let refused = |result: u8, points: u32| pp::gift_answer(result, target, points, None);
    let Some(product) = ctx.data.products.get(code).filter(|_| ctx.switches.shop() == switches::SHOP_ON) else {
        return Ok(refused(pp::GIFT_NO_PRODUCT, state.mall.points));
    };
    if state.mall.points < product.price {
        return Ok(refused(pp::GIFT_NO_POINTS, state.mall.points));
    }
    if target.is_empty() {
        return Ok(refused(pp::GIFT_NO_CHARACTER, state.mall.points));
    }
    // A product with an unusable item answers 6 and leaves nothing set (deviation 2).
    let rows = match ml::gift_rows(product, &ctx.data.items) {
        Ok(rows) => rows,
        Err(result) => return Ok(refused(result, state.mall.points)),
    };
    let target_name = String::from_utf8_lossy(target).into_owned();
    let order = GiftOrder {
        user_id,
        server_id: ctx.config.server_id,
        character_id: id,
        target: target_name.clone(),
        code: product.code.clone(),
        price: product.price,
        rows,
    };
    let (target_user, rows) = match ctx.store.gift(&order).await? {
        GiftOutcome::Sent { points_left, target_user, rows } => {
            state.mall.points = points_left;
            (target_user, rows)
        }
        other => {
            let result = match other {
                GiftOutcome::NoCharacter => pp::GIFT_NO_CHARACTER,
                GiftOutcome::OwnAccount => pp::GIFT_OWN_ACCOUNT,
                GiftOutcome::StorageFull => pp::GIFT_STORAGE_FULL,
                _ => {
                    state.mall.points = ctx.store.points(user_id).await?;
                    pp::GIFT_NO_POINTS
                }
            };
            tracing::info!(id, code = %product.code, target = %target_name, result, "item mall gift refused");
            return Ok(refused(result, state.mall.points));
        }
    };
    tracing::info!(id, code = %product.code, target = %target_name, target_user, rows = rows.len(), left = state.mall.points, "item mall gift");
    let arrived = ArrivedGift { sender: character.name.clone(), code: product.code.clone(), rows };
    if let Err(gift) = ctx.online.deliver_gift(target_user, arrived) {
        ctx.store.add_gift_notice(target_user, ctx.config.server_id, &GiftNotice { sender: gift.sender, code: gift.code }).await?;
    }
    Ok(pp::gift_answer(pp::GIFT_OK, target, state.mall.points, Some(&log_row(code, product.price))))
}

/// `0x2607`, `0xB102` (sections 5 and 6): the row's item into the first empty place; that place, `None` when nothing was taken.
async fn take<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    storage: Storage,
    slot: u8,
) -> Result<Option<(u8, u8)>> {
    let id = character.id;
    let rows = match storage {
        Storage::Mall => &state.mall.stored,
        Storage::Billing => &state.mall.billing,
    };
    let Some(&row) = rows.iter().find(|r| r.slot == slot && slot < pp::STORAGE_SLOTS) else { return Ok(None) };
    let Some(def) = ml::def_of(&ctx.data.items, row.item_id) else { return Ok(None) };
    let Some(&place) = state.inventory.free_slots().first() else { return Ok(None) };
    let make_type = if storage == Storage::Mall { ml::MALL_MAKE_TYPE } else { ml::BILLING_MAKE_TYPE };
    let item = ml::new_item(def, row.count, place, make_type);
    let Some(stored) = ctx.store.take_stored(id, storage, row, &item).await? else {
        tracing::warn!(id, ?storage, ?row, "the database refused a storage take");
        return Ok(None);
    };
    match storage {
        Storage::Mall => state.mall.stored.retain(|r| r.slot != slot),
        Storage::Billing => state.mall.billing.retain(|r| r.slot != slot),
    }
    state.inventory.apply(&SlotChange::Insert(item));
    // `0x0411` for the mall's storage, `0x040D` for the billing one; the billing take's NPC id has no source here (NULL).
    if let Some(part) = record::item_in(ctx, state, place, Some(stored)) {
        let header = record::header(character, state);
        ctx.game_log.action(match storage {
            Storage::Mall => crate::game_log::mall_stored_take(header, &part, item.count),
            Storage::Billing => crate::game_log::billing_take(header, &part, None, item.count),
        });
    }
    tracing::info!(id, ?storage, slot, item = row.item_id, count = row.count, ?place, "storage item taken");
    Ok(Some(place))
}
