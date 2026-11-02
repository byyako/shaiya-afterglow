//! The auction house (`docs/original/auction.md`, the client's side in `docs/client/auction-house.md`): searches and pages, the four
//! lists, registering, cancelling, buying now, bidding, collecting, the notices and the interest list. Sizes in the comments count the
//! opcode, as the PDB sizes do.

use crate::frame::encode;

/// Client: search (`TPC_MARKET_SEARCH`, 39). Server: the first page (`TP_MARKET_SEARCH`, 5 + 48n).
pub const OP_SEARCH: u16 = 0x1C01;
/// Client: "search this item" (`u8 type, u8 type id`). Server: the first page of an item or seller search.
pub const OP_SAME_ITEM: u16 = 0x1C02;
/// Client: a page (`u8` 0 refresh, 1 previous, 2 next). Server: that page.
pub const OP_PAGE: u16 = 0x1C03;
/// Client: my auctions. Server: `u8 n, n × TP_MARKET_SELLITEM`, in chunks of 36.
pub const OP_SELL_LIST: u16 = 0x1C04;
/// Client: my bids. Server: `u8 n, n × TP_MARKET_ITEM`, in chunks of 40.
pub const OP_BID_LIST: u16 = 0x1C05;
/// Client: items to collect. Server: `u8 n, n × TP_MARKET_ENDITEM`, in chunks of 46.
pub const OP_END_ITEMS: u16 = 0x1C06;
/// Client: gold to collect. Server: `u8 n, n × TP_MARKET_ENDMONEY`, in chunks of 36.
pub const OP_END_MONEYS: u16 = 0x1C07;
/// Client: register (`TPC_MARKET_ITEM_REGISTER`, 14). Server: `u8 result`, with the new auction when 0 (62).
pub const OP_REGISTER: u16 = 0x1C08;
/// Client: cancel (`u32 market id`). Server: `u8 result`, with the item row when 0 (44).
pub const OP_CANCEL: u16 = 0x1C09;
/// Client: buy now (`u32 market id`). Server: `u8 result`, with the gold and the item row when 0 (48).
pub const OP_BUY: u16 = 0x1C0A;
/// Client: bid (`u32 market id, u32 price`). Server: `u32 market id, u8 result` and what the result carries.
pub const OP_BID: u16 = 0x1C0B;
/// Client: collect an item (`u32 market id`). Server: `u8 result`, with `TP_MARKET_GET_ITEM` when 0 (41).
pub const OP_GET_ITEM: u16 = 0x1C0C;
/// Client: collect gold (`u32 money id`). Server: `u8 result`, with `u32 money id, u32 gold` when 0 (11).
pub const OP_GET_MONEY: u16 = 0x1C0D;
/// Server: to the seller, sold (`TP_MARKET_INFO_SELL_OK`: the gold row, 55).
pub const OP_SOLD: u16 = 0x1C0E;
/// Server: to the seller, ended unsold (`INFO_SELL_FAIL`: the item row, 43).
pub const OP_UNSOLD: u16 = 0x1C0F;
/// Server: to the seller, a bid (`INFO_TENDER`: market id, price, type, type id, count; 13).
pub const OP_BID_ON: u16 = 0x1C10;
/// Server: to the top bidder, won (`INFO_TENDER_OK`: the item row, 43).
pub const OP_WON: u16 = 0x1C11;
/// Server: to a bidder, outbid or the auction gone (`INFO_TENDER_FAIL`: the refund's gold row, 55).
pub const OP_OUTBID: u16 = 0x1C12;
/// Server: the market's state (`TP_MARKET_INFO`, `u8`); the new server never sends it (spec section 15).
pub const OP_INFO: u16 = 0x1C13;
/// Client: the interest list. Server: `u8 n, n × TP_MARKET_ITEM`.
pub const OP_INTEREST_LIST: u16 = 0x1C14;
/// Client: add to it (`u32 market id`). Server: `u8 result`, with the row when 0 (51).
pub const OP_INTEREST_ADD: u16 = 0x1C15;
/// Client: remove from it (`u32 market id`). Server: `u32 market id, u8 result` (7).
pub const OP_INTEREST_REMOVE: u16 = 0x1C16;
/// Client: clear it. Server: `u8 result` (3).
pub const OP_INTEREST_CLEAR: u16 = 0x1C17;

/// Rows per packet of the lists (the PDB's arrays).
pub const SELL_CHUNK: usize = 36;
pub const BID_CHUNK: usize = 40;
pub const END_ITEM_CHUNK: usize = 46;
pub const END_MONEY_CHUNK: usize = 36;

/// The craft name's field in the rows, its zero included.
const CRAFT_NAME_FIELD: usize = 21;
/// The name field of a search, of which the server uses 31 bytes (it sets the last to 0).
const SEARCH_NAME_FIELD: usize = 32;

/// `0x1C01`: the filters as the client sends them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    pub country: u8,
    pub level_min: u8,
    pub level_max: u8,
    pub category: u8,
    pub grade: u8,
    /// Up to its first zero within 31 bytes.
    pub name: Vec<u8>,
}

/// `0x1C08`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Register {
    pub bag: u8,
    pub slot: u8,
    pub count: u8,
    pub period: u8,
    pub min: u32,
    pub direct: u32,
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

pub fn parse_search(payload: &[u8]) -> Option<Search> {
    let p = payload.get(..5 + SEARCH_NAME_FIELD)?;
    let name = &p[5..5 + SEARCH_NAME_FIELD - 1];
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    Some(Search { country: p[0], level_min: p[1], level_max: p[2], category: p[3], grade: p[4], name: name[..end].to_vec() })
}

/// `0x1C02`: `u8 type, u8 type id`.
pub fn parse_same_item(payload: &[u8]) -> Option<(u8, u8)> {
    match payload {
        [item_type, type_id, ..] => Some((*item_type, *type_id)),
        _ => None,
    }
}

/// `0x1C03`: `u8 action`.
pub fn parse_page(payload: &[u8]) -> Option<u8> {
    payload.first().copied()
}

pub fn parse_register(payload: &[u8]) -> Option<Register> {
    let p = payload.get(..12)?;
    Some(Register { bag: p[0], slot: p[1], count: p[2], period: p[3], min: u32_at(p, 4), direct: u32_at(p, 8) })
}

/// `0x1C09`, `0x1C0A`, `0x1C0C`, `0x1C0D`, `0x1C15`, `0x1C16`: `u32 id`.
pub fn parse_id(payload: &[u8]) -> Option<u32> {
    payload.get(..4).map(|p| u32_at(p, 0))
}

/// `0x1C0B`: `u32 market id, u32 price`.
pub fn parse_bid(payload: &[u8]) -> Option<(u32, u32)> {
    payload.get(..8).map(|p| (u32_at(p, 0), u32_at(p, 4)))
}

/// The item as the rows show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemRow {
    pub item_type: u8,
    pub type_id: u8,
    pub quality: u16,
    pub gems: [u8; 6],
    pub count: u8,
    /// Without its padding; cut at 20 bytes when written.
    pub craft_name: Vec<u8>,
}

/// `TP_MARKET_ITEM`: an auction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketRow {
    pub id: u32,
    /// The current price: the minimum until the first bid, then the top bid.
    pub price: u32,
    pub direct: u32,
    /// The end date, packed.
    pub end: u32,
    pub item: ItemRow,
}

/// `TP_MARKET_SELLITEM`: an auction of mine, with its deposit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SellRow {
    pub row: MarketRow,
    pub deposit: u32,
}

/// `TP_MARKET_ENDITEM`: an item to collect.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EndItem {
    pub id: u32,
    pub result: u8,
    /// The collection deadline, packed.
    pub deadline: u32,
    pub item: ItemRow,
}

/// `TP_MARKET_ENDMONEY`: gold to collect.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EndMoney {
    pub money_id: u32,
    pub market_id: u32,
    pub result: u8,
    /// What collecting pays.
    pub return_money: u32,
    /// The price paid.
    pub money: u32,
    pub deadline: u32,
    pub item: ItemRow,
}

/// The craft name, zero-padded to its field (at most 20 bytes and a zero).
fn craft_name(p: &mut Vec<u8>, name: &[u8]) {
    let mut field = [0u8; CRAFT_NAME_FIELD];
    let len = name.len().min(CRAFT_NAME_FIELD - 1);
    field[..len].copy_from_slice(&name[..len]);
    p.extend_from_slice(&field);
}

/// Type, type id, quality, gems, count, craft name (32 bytes).
fn item(p: &mut Vec<u8>, item: &ItemRow) {
    p.extend_from_slice(&[item.item_type, item.type_id]);
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    p.push(item.count);
    craft_name(p, &item.craft_name);
}

/// `TP_MARKET_ITEM` (48).
pub fn market_item(p: &mut Vec<u8>, row: &MarketRow) {
    for v in [row.id, row.price, row.direct, row.end] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    item(p, &row.item);
}

/// `TP_MARKET_SELLITEM` (52).
pub fn sell_item(p: &mut Vec<u8>, row: &SellRow) {
    market_item(p, &row.row);
    p.extend_from_slice(&row.deposit.to_le_bytes());
}

/// `TP_MARKET_ENDITEM` (41).
pub fn end_item(p: &mut Vec<u8>, row: &EndItem) {
    p.extend_from_slice(&row.id.to_le_bytes());
    p.push(row.result);
    p.extend_from_slice(&row.deadline.to_le_bytes());
    item(p, &row.item);
}

/// `TP_MARKET_ENDMONEY` (53).
pub fn end_money(p: &mut Vec<u8>, row: &EndMoney) {
    p.extend_from_slice(&row.money_id.to_le_bytes());
    p.extend_from_slice(&row.market_id.to_le_bytes());
    p.push(row.result);
    for v in [row.return_money, row.money, row.deadline] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    item(p, &row.item);
}

/// `0x1C01` to `0x1C03`: `u8 has previous, u8 has next, u8 n`, the rows (5 + 48n).
pub fn search(op: u16, prev: bool, next: bool, rows: &[MarketRow]) -> Vec<u8> {
    let mut p = vec![u8::from(prev), u8::from(next), rows.len() as u8];
    for row in rows {
        market_item(&mut p, row);
    }
    encode(op, &p)
}

/// A list in chunks (spec section 4): every full chunk its own packet, then one with the rest, sent even when nothing is left.
fn chunked<T>(op: u16, rows: &[T], per: usize, write: fn(&mut Vec<u8>, &T)) -> Vec<Vec<u8>> {
    let full = rows.len() / per;
    let mut packets: Vec<Vec<u8>> = rows.chunks(per).take(full).map(|chunk| list(op, chunk, write)).collect();
    packets.push(list(op, &rows[full * per..], write));
    packets
}

fn list<T>(op: u16, rows: &[T], write: fn(&mut Vec<u8>, &T)) -> Vec<u8> {
    let mut p = vec![rows.len() as u8];
    for row in rows {
        write(&mut p, row);
    }
    encode(op, &p)
}

/// `0x1C04` (3 + 52n per packet).
pub fn sell_list(rows: &[SellRow]) -> Vec<Vec<u8>> {
    chunked(OP_SELL_LIST, rows, SELL_CHUNK, sell_item)
}

/// `0x1C05` (3 + 48n per packet).
pub fn bid_list(rows: &[MarketRow]) -> Vec<Vec<u8>> {
    chunked(OP_BID_LIST, rows, BID_CHUNK, market_item)
}

/// `0x1C06` (3 + 41n per packet).
pub fn end_items(rows: &[EndItem]) -> Vec<Vec<u8>> {
    chunked(OP_END_ITEMS, rows, END_ITEM_CHUNK, end_item)
}

/// `0x1C07` (3 + 53n per packet).
pub fn end_moneys(rows: &[EndMoney]) -> Vec<Vec<u8>> {
    chunked(OP_END_MONEYS, rows, END_MONEY_CHUNK, end_money)
}

/// `0x1C14` (3 + 48n; ten at most).
pub fn interest_list(rows: &[MarketRow]) -> Vec<u8> {
    list(OP_INTEREST_LIST, rows, market_item)
}

/// `0x1C08` result 0 (62): the place and the count taken, the gold after the fee, the new auction.
pub fn register_ok(bag: u8, slot: u8, count: u8, gold: u32, row: &SellRow) -> Vec<u8> {
    let mut p = vec![0, bag, slot, count];
    p.extend_from_slice(&gold.to_le_bytes());
    sell_item(&mut p, row);
    encode(OP_REGISTER, &p)
}

/// `0x1C08` result 1 (3).
pub fn register_failed() -> Vec<u8> {
    encode(OP_REGISTER, &[1])
}

/// `0x1C09`: result 0 with the item row (44), or result 1 (3).
pub fn cancel(row: Option<&EndItem>) -> Vec<u8> {
    match row {
        Some(row) => {
            let mut p = vec![0];
            end_item(&mut p, row);
            encode(OP_CANCEL, &p)
        }
        None => encode(OP_CANCEL, &[1]),
    }
}

/// `0x1C0A`: result 0 with the gold after and the item row (48), or the refusal's result (3).
pub fn buy(result: Result<(u32, &EndItem), u8>) -> Vec<u8> {
    match result {
        Ok((gold, row)) => {
            let mut p = vec![0];
            p.extend_from_slice(&gold.to_le_bytes());
            end_item(&mut p, row);
            encode(OP_BUY, &p)
        }
        Err(result) => encode(OP_BUY, &[result]),
    }
}

/// `0x1C0B` result 0 (59): the gold after and the auction as it is now.
pub fn bid_ok(id: u32, gold: u32, row: &MarketRow) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(0);
    p.extend_from_slice(&gold.to_le_bytes());
    market_item(&mut p, row);
    encode(OP_BID, &p)
}

/// `0x1C0B` results 1 to 4 (7).
pub fn bid_refused(id: u32, result: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(result);
    encode(OP_BID, &p)
}

/// `0x1C0B` result 5 (11): the lowest bid now.
pub fn bid_too_low(id: u32, min: u32) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(5);
    p.extend_from_slice(&min.to_le_bytes());
    encode(OP_BID, &p)
}

/// `0x1C0C`: result 0 (41, `TP_MARKET_GET_ITEM`: market id, bag, slot, type, type id, **count, quality**, gems, craft name), or
/// result 1 (3).
pub fn got_item(got: Option<(u32, u8, u8, &ItemRow)>) -> Vec<u8> {
    let Some((id, bag, slot, row)) = got else { return encode(OP_GET_ITEM, &[1]) };
    let mut p = vec![0];
    p.extend_from_slice(&id.to_le_bytes());
    p.extend_from_slice(&[bag, slot, row.item_type, row.type_id, row.count]);
    p.extend_from_slice(&row.quality.to_le_bytes());
    p.extend_from_slice(&row.gems);
    craft_name(&mut p, &row.craft_name);
    encode(OP_GET_ITEM, &p)
}

/// `0x1C0D`: result 0 with the money id and the gold after (11), or result 1 (3).
pub fn got_money(got: Option<(u32, u32)>) -> Vec<u8> {
    let Some((money_id, gold)) = got else { return encode(OP_GET_MONEY, &[1]) };
    let mut p = vec![0];
    p.extend_from_slice(&money_id.to_le_bytes());
    p.extend_from_slice(&gold.to_le_bytes());
    encode(OP_GET_MONEY, &p)
}

fn end_money_packet(op: u16, row: &EndMoney) -> Vec<u8> {
    let mut p = Vec::new();
    end_money(&mut p, row);
    encode(op, &p)
}

fn end_item_packet(op: u16, row: &EndItem) -> Vec<u8> {
    let mut p = Vec::new();
    end_item(&mut p, row);
    encode(op, &p)
}

/// `0x1C0E` (55).
pub fn sold(row: &EndMoney) -> Vec<u8> {
    end_money_packet(OP_SOLD, row)
}

/// `0x1C0F` (43).
pub fn unsold(row: &EndItem) -> Vec<u8> {
    end_item_packet(OP_UNSOLD, row)
}

/// `0x1C10` (13).
pub fn bid_on(id: u32, price: u32, item_type: u8, type_id: u8, count: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&price.to_le_bytes());
    p.extend_from_slice(&[item_type, type_id, count]);
    encode(OP_BID_ON, &p)
}

/// `0x1C11` (43).
pub fn won(row: &EndItem) -> Vec<u8> {
    end_item_packet(OP_WON, row)
}

/// `0x1C12` (55).
pub fn outbid(row: &EndMoney) -> Vec<u8> {
    end_money_packet(OP_OUTBID, row)
}

/// `0x1C15`: result 0 with the row (51), or the refusal's result (3).
pub fn interest_added(result: Result<&MarketRow, u8>) -> Vec<u8> {
    match result {
        Ok(row) => {
            let mut p = vec![0];
            market_item(&mut p, row);
            encode(OP_INTEREST_ADD, &p)
        }
        Err(result) => encode(OP_INTEREST_ADD, &[result]),
    }
}

/// `0x1C16` (7).
pub fn interest_removed(id: u32, result: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(result);
    encode(OP_INTEREST_REMOVE, &p)
}

/// `0x1C17` (3).
pub fn interest_cleared(result: u8) -> Vec<u8> {
    encode(OP_INTEREST_CLEAR, &[result])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    fn len(p: &[u8]) -> usize {
        payload(p).len() + 2
    }

    fn item_row() -> ItemRow {
        ItemRow { item_type: 100, type_id: 1, quality: 0x0203, gems: [1, 2, 3, 4, 5, 6], count: 7, craft_name: b"0102".to_vec() }
    }

    fn row(id: u32) -> MarketRow {
        MarketRow { id, price: 1000, direct: 5000, end: 9, item: item_row() }
    }

    fn sell(id: u32) -> SellRow {
        SellRow { row: row(id), deposit: 120 }
    }

    fn end(id: u32) -> EndItem {
        EndItem { id, result: 1, deadline: 9, item: item_row() }
    }

    fn money(id: u32) -> EndMoney {
        EndMoney { money_id: id, market_id: 4, result: 0, return_money: 5020, money: 5000, deadline: 9, item: item_row() }
    }

    #[test]
    fn sizes_match_the_original() {
        assert_eq!(len(&search(OP_SEARCH, false, false, &[])), 5);
        assert_eq!(len(&search(OP_SEARCH, false, true, &vec![row(1); 17])), 821);
        assert_eq!(len(&sell_list(&[sell(1)])[0]), 3 + 52);
        assert_eq!(len(&bid_list(&[row(1)])[0]), 3 + 48);
        assert_eq!(len(&end_items(&[end(1)])[0]), 3 + 41);
        assert_eq!(len(&end_moneys(&[money(1)])[0]), 3 + 53);
        assert_eq!(len(&interest_list(&[row(1), row(2)])), 3 + 96);
        assert_eq!(len(&register_ok(1, 2, 3, 4, &sell(1))), 62);
        assert_eq!(len(&register_failed()), 3);
        assert_eq!((len(&cancel(Some(&end(1)))), len(&cancel(None))), (44, 3));
        assert_eq!((len(&buy(Ok((1, &end(1))))), len(&buy(Err(3)))), (48, 3));
        assert_eq!(len(&bid_ok(1, 2, &row(1))), 59);
        assert_eq!(len(&bid_refused(1, 3)), 7);
        assert_eq!(len(&bid_too_low(1, 1050)), 11);
        assert_eq!((len(&got_item(Some((1, 1, 0, &item_row())))), len(&got_item(None))), (41, 3));
        assert_eq!((len(&got_money(Some((1, 2)))), len(&got_money(None))), (11, 3));
        assert_eq!(len(&sold(&money(1))), 55);
        assert_eq!(len(&unsold(&end(1))), 43);
        assert_eq!(len(&bid_on(1, 2, 3, 4, 5)), 13);
        assert_eq!(len(&won(&end(1))), 43);
        assert_eq!(len(&outbid(&money(1))), 55);
        assert_eq!((len(&interest_added(Ok(&row(1)))), len(&interest_added(Err(2)))), (51, 3));
        assert_eq!(len(&interest_removed(1, 0)), 7);
        assert_eq!(len(&interest_cleared(0)), 3);
    }

    #[test]
    fn lists_come_in_chunks_with_a_last_packet_always() {
        let sizes = |packets: Vec<Vec<u8>>| packets.iter().map(|p| len(p)).collect::<Vec<_>>();
        assert_eq!(sizes(sell_list(&[])), [3]);
        assert_eq!(sizes(sell_list(&vec![sell(1); 36])), [1875, 3]);
        assert_eq!(sizes(sell_list(&vec![sell(1); 37])), [1875, 55]);
        assert_eq!(sizes(bid_list(&vec![row(1); 41])), [3 + 40 * 48, 3 + 48]);
        assert_eq!(sizes(end_items(&vec![end(1); 46])), [3 + 46 * 41, 3]);
        assert_eq!(sizes(end_moneys(&vec![money(1); 72])), [1911, 1911, 3]);
        assert!(sell_list(&vec![sell(1); 37]).iter().all(|p| opcode(p) == OP_SELL_LIST));
    }

    #[test]
    fn rows_are_where_the_client_reads_them() {
        let p = search(OP_PAGE, true, false, &[row(0x0102_0304)]);
        let b = payload(&p);
        assert_eq!(&b[..3], &[1, 0, 1]);
        assert_eq!(u32::from_le_bytes(b[3..7].try_into().unwrap()), 0x0102_0304);
        assert_eq!(u32::from_le_bytes(b[7..11].try_into().unwrap()), 1000, "the current price");
        assert_eq!(&b[19..23], &[100, 1, 3, 2], "type, type id, quality");
        assert_eq!(&b[23..29], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(b[29], 7, "count");
        assert_eq!(&b[30..51], b"0102\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0", "21 bytes, zero-padded");

        let long = ItemRow { craft_name: vec![b'9'; 30], ..item_row() };
        let p = got_item(Some((5, 2, 3, &long)));
        let b = payload(&p);
        assert_eq!(&b[..1], &[0]);
        assert_eq!(&b[5..10], &[2, 3, 100, 1, 7], "bag, slot, type, type id, then the count before the quality");
        assert_eq!(&b[10..12], &[3, 2]);
        assert_eq!(&b[18..38], &[b'9'; 20]);
        assert_eq!(b[38], 0, "cut to 20 and its zero");

        let p = sold(&money(8));
        let b = payload(&p);
        assert_eq!(u32::from_le_bytes(b[0..4].try_into().unwrap()), 8, "money id");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 4, "market id");
        assert_eq!(u32::from_le_bytes(b[9..13].try_into().unwrap()), 5020, "the return before the money");
        assert_eq!(u32::from_le_bytes(b[13..17].try_into().unwrap()), 5000);

        let p = register_ok(1, 4, 3, 880, &sell(6));
        let b = payload(&p);
        assert_eq!(&b[..4], &[0, 1, 4, 3]);
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 880);
        assert_eq!(u32::from_le_bytes(b[56..60].try_into().unwrap()), 120, "the deposit last");

        assert_eq!(payload(&bid_too_low(6, 1050)), &[6, 0, 0, 0, 5, 0x1A, 4, 0, 0]);
    }

    #[test]
    fn requests_parse() {
        let mut search = vec![1, 2, 30, 4, 255];
        search.extend_from_slice(b"Potion");
        search.resize(5 + 32, 0);
        let parsed = parse_search(&search).unwrap();
        assert_eq!((parsed.country, parsed.level_min, parsed.level_max, parsed.category, parsed.grade), (1, 2, 30, 4, 255));
        assert_eq!(parsed.name, b"Potion");
        let full: Vec<u8> = [0; 5].into_iter().chain([b'x'; 32]).collect();
        assert_eq!(parse_search(&full).unwrap().name.len(), 31, "the last byte is made 0");
        assert_eq!(parse_search(&search[..36]), None);

        assert_eq!(parse_same_item(&[30, 2]), Some((30, 2)));
        assert_eq!(parse_same_item(&[30]), None);
        assert_eq!(parse_page(&[2]), Some(2));
        assert_eq!(parse_page(&[]), None);
        let register = [1, 4, 3, 1, 0xE8, 3, 0, 0, 0x88, 0x13, 0, 0];
        assert_eq!(parse_register(&register), Some(Register { bag: 1, slot: 4, count: 3, period: 1, min: 1000, direct: 5000 }));
        assert_eq!(parse_register(&register[..11]), None);
        assert_eq!(parse_id(&[7, 0, 0, 0]), Some(7));
        assert_eq!(parse_id(&[7, 0, 0]), None);
        assert_eq!(parse_bid(&[7, 0, 0, 0, 0x1A, 4, 0, 0]), Some((7, 1050)));
        assert_eq!(parse_bid(&[7, 0, 0, 0]), None);
    }
}
