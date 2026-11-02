//! The item mall (`docs/original/item-mall.md`): points, purchases, gifts, the item-mall storage (`0x26xx`) and the billing storage
//! (`0xB101`, `0xB102`). The enter burst's `0x2601` is here too; `0xB103` is `game::billing_info`.

use crate::chat::{NAME_LEN, name_field};
use crate::frame::encode;

/// Server: the points and the purchase log at world entry (`TP_POINT_AND_LOG`).
pub const OP_POINT_AND_LOG: u16 = 0x2601;
/// Client: buy (`char code[21]`, `TPC_POINT_BUY`). Server: the answer (`TP_POINT_BUY`).
pub const OP_BUY: u16 = 0x2602;
/// Client: a gift (`char code[21], char target[21]`, `TPC_POINT_GIFT_SEND`). Server: the answer, always 57 bytes.
pub const OP_GIFT: u16 = 0x2603;
/// Server: to the recipient, a gift came (`TP_POINT_GIFT_RECV`); with no rows, a notice of one that came while away.
pub const OP_GIFT_RECEIVED: u16 = 0x2604;
/// Client: the points again. Server: the points (`TP_POINT_RELOAD`).
pub const OP_RELOAD: u16 = 0x2605;
/// Server: the item-mall storage at world entry (`TP_POINT_STORED_ITEMLIST`).
pub const OP_STORED_LIST: u16 = 0x2606;
/// Client: take a stored item (`u8 slot`). Server: `u8 slot, u8 bag, u8 slot` (bag 0: nothing taken).
pub const OP_STORED_GET: u16 = 0x2607;
/// Server: the billing storage in the enter burst (`TP_BILLING_ITEMLIST`).
pub const OP_BILLING_LIST: u16 = 0xB101;
/// Client: take an item of the billing storage (`u8 slot`). Server: as `0x2607`.
pub const OP_BILLING_GET: u16 = 0xB102;

/// Product codes are `char[21]`.
pub const CODE_LEN: usize = 21;
/// Rows in a log (`TP_POINTLOG_UNIT[10]`), items in a purchase (`TP_POINTITEM_UNIT[24]`), slots of a storage.
pub const LOG_ROWS: usize = 10;
pub const BUY_ITEMS: usize = 24;
pub const STORAGE_SLOTS: u8 = 240;

/// `0x2602` results (section 3).
pub const BUY_OK: u8 = 0;
pub const BUY_NO_PRODUCT: u8 = 1;
pub const BUY_NO_POINTS: u8 = 2;
pub const BUY_NO_ROOM: u8 = 3;
pub const BUY_BUSY: u8 = 4;

/// `0x2603` results (section 4).
pub const GIFT_OK: u8 = 0;
pub const GIFT_NO_CHARACTER: u8 = 1;
pub const GIFT_OWN_ACCOUNT: u8 = 2;
pub const GIFT_STORAGE_FULL: u8 = 3;
pub const GIFT_NO_PRODUCT: u8 = 6;
pub const GIFT_FAILED: u8 = 7;
pub const GIFT_BUSY: u8 = 9;
pub const GIFT_NO_POINTS: u8 = 10;

/// A purchase log row (`TP_POINTLOG_UNIT`): the code, when (packed local time), the points used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRow {
    pub code: Vec<u8>,
    pub time: u32,
    pub points: u32,
}

/// An item placed by a purchase (`TP_POINTITEM_UNIT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub bag: u8,
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
    pub count: u8,
}

/// A storage row (`POINT_STORED_ITEMUNIT`, `BILLING_ITEMUNIT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stored {
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
    pub count: u8,
}

/// A `char[21]` field up to its first zero, the last byte a zero whatever was sent (the original writes one there).
fn field(bytes: &[u8]) -> Vec<u8> {
    bytes[..bytes.len() - 1].iter().copied().take_while(|&b| b != 0).collect()
}

/// `0x2602`: the product code.
pub fn parse_buy(payload: &[u8]) -> Option<Vec<u8>> {
    payload.get(..CODE_LEN).map(field)
}

/// `0x2603`: the product code and the recipient's name.
pub fn parse_gift(payload: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let code = payload.get(..CODE_LEN)?;
    let name = payload.get(CODE_LEN..CODE_LEN + NAME_LEN)?;
    Some((field(code), field(name)))
}

/// `0x2607`, `0xB102`: the storage slot.
pub fn parse_slot(payload: &[u8]) -> Option<u8> {
    payload.first().copied()
}

fn code_field(code: &[u8]) -> [u8; CODE_LEN] {
    name_field(code)
}

fn put_log(p: &mut Vec<u8>, row: &LogRow) {
    p.extend_from_slice(&code_field(&row.code));
    p.extend_from_slice(&row.time.to_le_bytes());
    p.extend_from_slice(&row.points.to_le_bytes());
}

/// `0x2601`: `u32 points, u8 n`, n × (`char code[21], u32 time, u32 points used`), at most 10 rows.
pub fn point_and_log(points: u32, log: &[LogRow]) -> Vec<u8> {
    let log = &log[..log.len().min(LOG_ROWS)];
    let mut p = points.to_le_bytes().to_vec();
    p.push(log.len() as u8);
    for row in log {
        put_log(&mut p, row);
    }
    encode(OP_POINT_AND_LOG, &p)
}

/// `0x2602` refused: `u8 result`.
pub fn buy_refused(result: u8) -> Vec<u8> {
    encode(OP_BUY, &[result])
}

/// `0x2602` 0: `u8 0, u32 points left, TP_POINTLOG_UNIT, u8 n`, n × `TP_POINTITEM_UNIT` (37 + 5n bytes), at most 24 items.
pub fn bought(points_left: u32, log: &LogRow, items: &[Placed]) -> Vec<u8> {
    let items = &items[..items.len().min(BUY_ITEMS)];
    let mut p = vec![BUY_OK];
    p.extend_from_slice(&points_left.to_le_bytes());
    put_log(&mut p, log);
    p.push(items.len() as u8);
    for i in items {
        p.extend_from_slice(&[i.bag, i.slot, i.item_type, i.type_id, i.count]);
    }
    encode(OP_BUY, &p)
}

/// `0x2603`: `u8 result, char target[21], u32 points, TP_POINTLOG_UNIT`, always 57 bytes (the log row zeros when refused).
pub fn gift_answer(result: u8, target: &[u8], points: u32, log: Option<&LogRow>) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&name_field(target));
    p.extend_from_slice(&points.to_le_bytes());
    match log {
        Some(row) => put_log(&mut p, row),
        None => p.extend_from_slice(&[0; CODE_LEN + 8]),
    }
    encode(OP_GIFT, &p)
}

fn put_stored(p: &mut Vec<u8>, rows: &[Stored]) {
    for r in rows {
        p.extend_from_slice(&[r.slot, r.item_type, r.type_id, r.count]);
    }
}

/// `0x2604`: `char sender[21], char code[21], u8 n`, n × storage rows (45 + 4n); no rows is the notice of a gift that came while away.
pub fn gift_received(sender: &[u8], code: &[u8], rows: &[Stored]) -> Vec<u8> {
    let rows = &rows[..rows.len().min(BUY_ITEMS)];
    let mut p = name_field(sender).to_vec();
    p.extend_from_slice(&code_field(code));
    p.push(rows.len() as u8);
    put_stored(&mut p, rows);
    encode(OP_GIFT_RECEIVED, &p)
}

/// `0x2605`: `u32 points`.
pub fn reload(points: u32) -> Vec<u8> {
    encode(OP_RELOAD, &points.to_le_bytes())
}

/// `0x2606` or `0xB101`: `u8 n`, n × storage rows; `None` for an empty storage (neither is sent then).
fn storage_list(opcode: u16, rows: &[Stored]) -> Option<Vec<u8>> {
    if rows.is_empty() {
        return None;
    }
    let rows = &rows[..rows.len().min(usize::from(STORAGE_SLOTS))];
    let mut p = vec![rows.len() as u8];
    put_stored(&mut p, rows);
    Some(encode(opcode, &p))
}

pub fn stored_list(rows: &[Stored]) -> Option<Vec<u8>> {
    storage_list(OP_STORED_LIST, rows)
}

pub fn billing_list(rows: &[Stored]) -> Option<Vec<u8>> {
    storage_list(OP_BILLING_LIST, rows)
}

/// `0x2607` or `0xB102`: `u8 slot, u8 bag, u8 slot`; `None` place = nothing taken (bag and slot 0).
pub fn taken(opcode: u16, slot: u8, place: Option<(u8, u8)>) -> Vec<u8> {
    let (bag, at) = place.unwrap_or((0, 0));
    encode(opcode, &[slot, bag, at])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> LogRow {
        LogRow { code: b"A1_Code".to_vec(), time: 0x1234_5678, points: 250 }
    }

    #[test]
    fn sizes_are_the_originals() {
        assert_eq!(point_and_log(5, &[]).len(), 2 + 7);
        assert_eq!(point_and_log(5, &vec![row(); 12]).len(), 2 + 7 + 29 * 10, "ten rows at most");
        assert_eq!(buy_refused(BUY_NO_ROOM).len(), 2 + 3);
        let placed = Placed { bag: 1, slot: 4, item_type: 100, type_id: 42, count: 1 };
        assert_eq!(bought(9, &row(), &[placed; 2]).len(), 2 + 37 + 5 * 2);
        assert_eq!(gift_answer(GIFT_OK, b"Someone", 9, Some(&row())).len(), 2 + 57);
        assert_eq!(gift_answer(GIFT_NO_POINTS, b"Someone", 9, None).len(), 2 + 57);
        let stored = Stored { slot: 3, item_type: 100, type_id: 42, count: 2 };
        assert_eq!(gift_received(b"Me", b"A1_Code", &[stored]).len(), 2 + 45 + 4);
        assert_eq!(gift_received(b"Me", b"A1_Code", &[]).len(), 2 + 45);
        assert_eq!(reload(1).len(), 2 + 6);
        assert_eq!(stored_list(&[stored]).map(|p| p.len()), Some(2 + 3 + 4));
        assert_eq!(billing_list(&[]), None);
        assert_eq!(taken(OP_STORED_GET, 3, None).len(), 2 + 5);
    }

    #[test]
    fn layouts() {
        let p = bought(1000, &row(), &[Placed { bag: 1, slot: 4, item_type: 100, type_id: 42, count: 3 }]);
        assert_eq!(&p[4..9], &[0, 0xe8, 0x03, 0, 0]);
        assert_eq!(&p[9..16], b"A1_Code");
        assert_eq!(&p[30..34], &0x1234_5678u32.to_le_bytes());
        assert_eq!(&p[34..38], &250u32.to_le_bytes());
        assert_eq!(&p[38..], &[1, 1, 4, 100, 42, 3]);
        let g = gift_answer(GIFT_OK, b"Target", 7, Some(&row()));
        assert_eq!((g[4], &g[5..11], &g[26..30]), (0, &b"Target"[..], &7u32.to_le_bytes()[..]));
        assert_eq!(&g[30..37], b"A1_Code");
        assert_eq!(taken(OP_BILLING_GET, 9, Some((2, 5))), encode(OP_BILLING_GET, &[9, 2, 5]));
    }

    #[test]
    fn requests() {
        let mut buy = [0u8; 23];
        buy[..4].copy_from_slice(b"Code");
        assert_eq!(parse_buy(&buy), Some(b"Code".to_vec()));
        assert_eq!(parse_buy(&[b'A'; 21]), Some(vec![b'A'; 20]), "the last byte is a zero whatever was sent");
        assert_eq!(parse_buy(&[0; 5]), None);
        let mut gift = [0u8; 42];
        gift[..4].copy_from_slice(b"Code");
        gift[21..25].copy_from_slice(b"Name");
        assert_eq!(parse_gift(&gift), Some((b"Code".to_vec(), b"Name".to_vec())));
        assert_eq!(parse_slot(&[7]), Some(7));
    }
}
