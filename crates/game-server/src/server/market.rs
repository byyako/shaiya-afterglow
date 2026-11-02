//! The auction house on the connection's side (`docs/original/auction.md`): the `0x1Cxx` requests, answered only on map 42, and the
//! end pass. The rules are `crate::market`'s; each change is planned under the house's turn, written in one transaction, then applied
//! in memory, answered and told to the other characters it concerns.

use super::*;
use crate::backend::MarketOp;
use crate::market::{self as mk, Planned, Refusal, Requester};
use crate::sync::Relock;
use shaiya_proto::market as mp;

/// A `0x1Cxx` request (`CUser::PacketMarket`). Off map 42 nothing answers (section 1); unknown opcodes of the family are ignored.
pub(super) async fn packet<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
    payload: &[u8],
) -> Result<()> {
    let id = character.id;
    if crate::world::map_of(state.map) != mk::AUCTION_MAP {
        tracing::debug!(id, map = state.map, opcode = format_args!("{opcode:#06x}"), "auction request ignored: not in the Auction House");
        return Ok(());
    }
    let now = mk::now();
    let me = Requester { id, name: &character.name, gold: state.money };
    let defs = &ctx.data.items;
    match opcode {
        mp::OP_SEARCH | mp::OP_SAME_ITEM => {
            let mode = if opcode == mp::OP_SEARCH {
                let Some(filter) = mp::parse_search(payload) else { return Ok(()) };
                mk::Mode::Normal(filter)
            } else {
                let Some((item_type, type_id)) = mp::parse_same_item(payload) else { return Ok(()) };
                mk::Mode::Item(u32::from(item_type) * 1000 + u32::from(type_id))
            };
            let mut search = mk::Search::new(mode);
            let page = ctx.market.state.guard().first(&mut search, defs);
            state.market_search = Some(search);
            conn.send(page).await?;
        }
        mp::OP_PAGE => {
            let Some(action) = mp::parse_page(payload) else { return Ok(()) };
            let Some(search) = state.market_search.as_mut() else { return Ok(()) };
            let page = ctx.market.state.guard().page(search, action, defs);
            if let Some(page) = page {
                conn.send(page).await?;
            }
        }
        mp::OP_SELL_LIST | mp::OP_BID_LIST | mp::OP_END_ITEMS | mp::OP_END_MONEYS => {
            let packets = {
                let market = ctx.market.state.guard();
                match opcode {
                    mp::OP_SELL_LIST => market.sell_list(id),
                    mp::OP_BID_LIST => market.bid_list(id),
                    mp::OP_END_ITEMS => market.end_items(id, now),
                    _ => market.end_moneys(id, now),
                }
            };
            for packet in packets {
                conn.send(packet).await?;
            }
        }
        mp::OP_INTEREST_LIST => {
            let list = ctx.market.state.guard().interest_list(state.interest.as_deref().unwrap_or_default());
            conn.send(list).await?;
        }
        mp::OP_INTEREST_ADD | mp::OP_INTEREST_REMOVE | mp::OP_INTEREST_CLEAR => {
            let market_id = mp::parse_id(payload);
            let (answer, changed) = match (opcode, market_id) {
                (mp::OP_INTEREST_ADD, Some(market_id)) => ctx.market.state.guard().interest_add(&mut state.interest, market_id),
                (mp::OP_INTEREST_REMOVE, Some(market_id)) => mk::interest_remove(&mut state.interest, market_id),
                (mp::OP_INTEREST_CLEAR, _) => mk::interest_clear(&mut state.interest),
                _ => return Ok(()),
            };
            if changed {
                ctx.store.save_interest(id, state.interest.as_deref().unwrap_or_default()).await?;
            }
            conn.send(answer).await?;
        }
        mp::OP_REGISTER => {
            let Some(register) = mp::parse_register(payload) else { return Ok(()) };
            // The trade ends and the personal shop closes first (section 5.3 step 1), whatever follows.
            ctx.zones.cancel_actions(id);
            let packets = {
                let _turn = ctx.market.turn.lock().await;
                let planned = ctx.market.state.guard().register(me, &state.inventory, defs, register, now);
                finish(ctx, character, state, planned).await?
            };
            send_all(conn, packets).await?;
        }
        mp::OP_CANCEL | mp::OP_BUY | mp::OP_GET_ITEM | mp::OP_GET_MONEY => {
            let Some(market_id) = mp::parse_id(payload) else { return Ok(()) };
            let packets = {
                let _turn = ctx.market.turn.lock().await;
                let planned = {
                    let market = ctx.market.state.guard();
                    match opcode {
                        mp::OP_CANCEL => market.cancel(id, market_id, now),
                        mp::OP_BUY => market.buy(me, market_id, now),
                        mp::OP_GET_ITEM => market.collect_item(id, &state.inventory, market_id, now),
                        _ => market.collect_money(me, market_id, now),
                    }
                };
                finish(ctx, character, state, planned).await?
            };
            send_all(conn, packets).await?;
        }
        mp::OP_BID => {
            let Some((market_id, price)) = mp::parse_bid(payload) else { return Ok(()) };
            let packets = {
                let _turn = ctx.market.turn.lock().await;
                let planned = ctx.market.state.guard().bid(me, market_id, price, now);
                finish(ctx, character, state, planned).await?
            };
            send_all(conn, packets).await?;
        }
        _ => tracing::debug!(id, opcode = format_args!("{opcode:#06x}"), "auction request ignored"),
    }
    Ok(())
}

/// The packets of a request, sent one after the other once the turn is dropped: a turn never waits on a client (`docs/server/audit.md`
/// R8).
async fn send_all(conn: &mut Connection, packets: Vec<Vec<u8>>) -> Result<()> {
    for packet in packets {
        conn.send(packet).await?;
    }
    Ok(())
}

/// A plan carried out under the turn: refused (silently or with its answer), or written, applied and told to the other characters.
/// The answers for the requester come back to be sent after the turn. The store refusing what the plan checked ends the connection,
/// as the guild bank does.
async fn finish<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    planned: Result<Planned, Refusal>,
) -> Result<Vec<Vec<u8>>> {
    let id = character.id;
    let planned = match planned {
        Err(Refusal::Ignore) => {
            tracing::debug!(id, "auction request ignored");
            return Ok(Vec::new());
        }
        Err(Refusal::Answer(answer)) => return Ok(vec![answer]),
        Ok(planned) => planned,
    };
    let view = ctx.market.state.guard().log_view(&planned);
    let Some(written) = ctx.store.market(&planned.write).await? else {
        bail!("the database refused a market change the server allowed: {:?}", planned.write);
    };
    let applied = ctx.market.state.guard().apply(&planned, &written);
    if let Some(gold) = planned.gold {
        state.money = gold;
    }
    if let Some(change) = &planned.bag_change {
        state.inventory.apply(change);
    }
    if let Some(item) = planned.received.clone() {
        state.inventory.receive(item);
    }
    log(id, &planned, &written);
    records(ctx, Some((character, state)), &planned, &written, &view);
    notify(ctx, applied.notices);
    Ok(applied.answer)
}

/// Notices go to characters wherever they are on this server (`CWorld::FindUser`); an offline one sees the row in its list later.
fn notify<S: Backend>(ctx: &Context<S>, notices: Vec<(u32, Vec<u8>)>) {
    for (character, packet) in notices {
        ctx.zones.send_to_char(character, &packet);
    }
}

fn log(id: u32, planned: &Planned, written: &crate::backend::MarketWritten) {
    use mk::Then;
    let action = match planned.then {
        Then::Register { .. } => "register",
        Then::Cancel { .. } => "cancel",
        Then::Buy { .. } => "buy",
        Then::Bid { .. } => "bid",
        Then::CollectItem { .. } | Then::CollectMoney { .. } => "collect",
        Then::Unsold { .. } | Then::Sold { .. } => "end",
        Then::Purge { .. } => "purge",
        Then::Recall { .. } => "recall",
    };
    tracing::info!(
        character = id,
        then = ?planned.then,
        money = planned.write.money,
        auctions = ?written.auction_ids,
        money_rows = ?written.money_ids,
        "market {action}"
    );
}

/// The end pass (section 10, deviation 1), every `PASS_EVERY`: the auctions past their end, one write each, then the rows past
/// their deadline.
pub async fn end_pass<S: Backend>(ctx: &Context<S>) {
    let _turn = ctx.market.turn.lock().await;
    let now = mk::now();
    let plans = {
        let market = ctx.market.state.guard();
        let mut plans = market.expire(now);
        plans.extend(market.purge(now));
        plans
    };
    for planned in plans {
        let view = ctx.market.state.guard().log_view(&planned);
        match ctx.store.market(&planned.write).await {
            Ok(Some(written)) => {
                let applied = ctx.market.state.guard().apply(&planned, &written);
                log(0, &planned, &written);
                records(ctx, None, &planned, &written, &view);
                notify(ctx, applied.notices);
            }
            Ok(None) => tracing::warn!(write = ?planned.write, "the database refused a market end; tried again next pass"),
            Err(error) => tracing::warn!(%error, write = ?planned.write, "a market end failed; tried again next pass"),
        }
    }
}

/// `FA1A` `/auctionsearch` (section 12.2): a seller search, its first page as `0x1C02`.
pub(super) fn seller_search<S: Backend>(ctx: &Context<S>, state: &mut WorldState, name: Vec<u8>) -> Vec<u8> {
    let mut search = mk::Search::new(mk::Mode::Seller(name));
    let page = ctx.market.state.guard().first(&mut search, &ctx.data.items);
    state.market_search = Some(search);
    page
}

/// `FA19` `/auctionrecall` (section 12.1, deviation 3), after the seller was disconnected: the items to the GM, written and applied;
/// the `0x1C06` packets for the GM, or `None` when nothing was recalled.
pub(super) async fn recall<S: Backend>(ctx: &Context<S>, gm: u32, name: &[u8], market_id: u32) -> Result<Option<Vec<Vec<u8>>>> {
    let _turn = ctx.market.turn.lock().await;
    let planned = ctx.market.state.guard().recall(gm, name, market_id, mk::now());
    let Some(planned) = planned else { return Ok(None) };
    let view = ctx.market.state.guard().log_view(&planned);
    let Some(written) = ctx.store.market(&planned.write).await? else {
        bail!("the database refused a market recall the server allowed: {:?}", planned.write);
    };
    let applied = ctx.market.state.guard().apply(&planned, &written);
    log(gm, &planned, &written);
    records(ctx, None, &planned, &written, &view);
    notify(ctx, applied.notices);
    Ok(Some(applied.answer))
}

/// An auction's item as the records name it, with the row the write left for it.
fn auction_item<S>(ctx: &Context<S>, item: &crate::backend::MarketItem, row: Option<crate::backend::Stored>) -> crate::game_log::ItemPart {
    let unit = ItemUnit {
        item_type: item.item_type,
        type_id: item.type_id,
        quality: item.quality,
        gems: item.gems,
        count: item.count,
        craft_name: item.craft_name.clone(),
        ..Default::default()
    };
    ctx.game_log.item(&unit, row)
}

/// The reasons a bidder's gold comes back (`0x0A0C` V5; the original's numbers were not read, these are this server's).
const REFUND_OUTBID: u8 = 1;
const REFUND_CANCELLED: u8 = 2;
const REFUND_BOUGHT: u8 = 3;
const REFUND_RECALLED: u8 = 4;

/// The game log's records of a market write (section 3.2 and 3.6 of `docs/original/game-log.md`), once it is written. `actor` is the
/// character whose request it was (none for the end pass and a recall, which write only the keep records). The items are named by the
/// rows the write left (`MarketWritten::ops`, `placed`); the auctions are read as they were before it (`LogView`).
fn records<S>(
    ctx: &Context<S>,
    actor: Option<(&PlayCharacter, &WorldState)>,
    planned: &Planned,
    written: &crate::backend::MarketWritten,
    view: &mk::LogView,
) {
    use crate::game_log as gl;
    use mk::Then;
    let header = || actor.map(|(character, state)| record::header(character, state));
    let money_rows = &written.money_ids;
    let name = |name: &str| name.trim_end_matches('\0').to_owned();
    // The auction an op ended or bid on, with the row it had.
    let ended = |id: u32| {
        let index = planned
            .write
            .ops
            .iter()
            .position(|op| matches!(op, MarketOp::EndAuction { id: i, .. } | MarketOp::SetBid { id: i, .. } if *i == id))?;
        let a = view.auctions.get(&id)?;
        Some((a, auction_item(ctx, &a.item, written.ops.get(index).copied().flatten())))
    };
    // A gold row an op made: its place among the new ones.
    let money_id = |nth: usize| money_rows.get(nth).copied().unwrap_or(0);
    match &planned.then {
        Then::Register { bag, slot, count, gold } => {
            let (Some(h), Some(MarketOp::NewAuction(n))) = (header(), planned.write.ops.first()) else { return };
            let item = auction_item(ctx, &n.item, written.ops.first().copied().flatten());
            let registered = gl::Registered {
                market_id: written.auction_ids.first().copied().unwrap_or(0),
                bag: *bag,
                slot: *slot,
                count: *count,
                min_bid: n.min,
                direct: n.direct,
                deposit: n.deposit,
                money_after: *gold,
                hours: mk::HOURS[usize::from(n.period.min(3))] as u8,
            };
            let end = gl::time_text(n.ends_at).unwrap_or_default();
            ctx.game_log.action(gl::market_register(h, &item, &registered, &end));
        }
        Then::Cancel { id, refund, .. } => {
            let (Some(h), Some((a, item))) = (header(), ended(*id)) else { return };
            let bidder = a.bidder.as_ref().map(|(b, n)| (*b, name(n))).unwrap_or_default();
            ctx.game_log.action(gl::market_cancel(h, &item, *id, bidder.0, &bidder.1));
            if refund.is_some() {
                ctx.game_log.action(gl::market_refund(&item, *id, money_id(0), REFUND_CANCELLED, a.seller, &name(&a.seller_name)));
            }
        }
        Then::Buy { id, gold, seller, refund } => {
            let (Some(h), Some((a, item))) = (header(), ended(*id)) else { return };
            ctx.game_log.action(gl::market_buy(h.clone(), &item, *id, a.direct, *gold, *seller, &name(&a.seller_name)));
            // The seller's side: the gold row the sale made.
            if let Some(MarketOp::NewMoneyRow(m)) = planned.write.ops.get(1) {
                let sale = gl::Sale {
                    market_id: *id,
                    money_id: money_id(0),
                    money: m.money,
                    deposit: m.deposit,
                    returned: m.return_money,
                    buyer_id: h.character_id,
                    buyer_name: h.character_name.clone(),
                };
                ctx.game_log.action(gl::market_sold(*seller, &name(&a.seller_name), &item, &sale));
            }
            if refund.is_some() {
                ctx.game_log.action(gl::market_refund(&item, *id, money_id(1), REFUND_BOUGHT, a.seller, &name(&a.seller_name)));
            }
        }
        Then::Bid { id, gold, seller, refund } => {
            let (Some(h), Some((a, item))) = (header(), ended(*id)) else { return };
            let Some(MarketOp::SetBid { price, .. }) = planned.write.ops.first() else { return };
            ctx.game_log.action(gl::market_bid(h.clone(), &item, *id, *price, *gold, *seller, refund.unwrap_or(0), &name(&a.seller_name)));
            ctx.game_log.action(gl::market_bid_seller(&item, *id, h.character_id, &h.character_name));
            if refund.is_some() {
                ctx.game_log.action(gl::market_refund(&item, *id, money_id(0), REFUND_OUTBID, a.seller, &name(&a.seller_name)));
            }
        }
        Then::CollectItem { id, bag, slot } => {
            let (Some(h), Some(placed)) = (header(), actor.and_then(|(_, state)| state.inventory.get(*bag, *slot))) else { return };
            let item = ctx.game_log.item(placed, written.placed);
            ctx.game_log.action(gl::market_item_get(h, &item, *id, *bag, *slot, placed.count));
        }
        Then::CollectMoney { id, gold, gained } => {
            let (Some(h), Some(row)) = (header(), view.money.as_ref()) else { return };
            ctx.game_log.action(gl::market_money_get(h, row.market_id, *id, row.money, row.deposit, *gained, *gold));
        }
        Then::Unsold { id, .. } => {
            if let Some((a, item)) = ended(*id) {
                ctx.game_log.action(gl::market_unsold(&item, *id, a.bidder.as_ref().map_or(0, |b| b.0), "TimeOut"));
            }
        }
        Then::Sold { id, .. } => {
            if let Some((a, item)) = ended(*id) {
                ctx.game_log.action(gl::market_won(&item, *id, a.seller, &name(&a.seller_name)));
            }
        }
        Then::Purge { .. } => {}
        Then::Recall { gm, ids, refunds } => {
            // Per auction: its refund (if any), then the seller's deposit row.
            let gm_name = ctx.zones.name_of(*gm).unwrap_or_default();
            let mut at = 0;
            for (id, bidder) in ids.iter().zip(refunds) {
                if let Some((a, item)) = ended(*id) {
                    ctx.game_log.action(gl::market_unsold(&item, *id, a.bidder.as_ref().map_or(0, |b| b.0), &format!("GM({gm_name})")));
                    if bidder.is_some() {
                        ctx.game_log.action(gl::market_refund(&item, *id, money_id(at), REFUND_RECALLED, a.seller, &name(&a.seller_name)));
                    }
                }
                at += usize::from(bidder.is_some()) + 1;
            }
        }
    }
}
