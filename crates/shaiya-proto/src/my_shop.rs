//! Personal shops (`docs/original/personal-shop.md`): the seller's setup and shop, visits and purchases, and the sign over the seller.

use crate::chat::NAME_LEN;
use crate::frame::encode;
use crate::world::ItemUnit;

/// Client: begin setting up. Server: the setup window opens (`TP_MYSHOP_BEGIN`).
pub const OP_BEGIN: u16 = 0x2301;
/// Client: end. Server: the setup window closes (`TP_MYSHOP_END`).
pub const OP_END: u16 = 0x2302;
/// Client: open the shop (`u8 len, char title[]`, `TPC_MYSHOP_START`). Server: opened (`TP_MYSHOP_START`).
pub const OP_START: u16 = 0x2303;
/// Client: close it. Server: closed (`TP_MYSHOP_STOP`).
pub const OP_STOP: u16 = 0x2304;
/// Client: place an item (`u8 bag, u8 slot, u8 shop slot, u32 price`, `TP_MYSHOP_ADD`). Server: the same, echoed.
pub const OP_ADD: u16 = 0x2305;
/// Client: take it off (`u8 shop slot`, `TP_MYSHOP_REMOVE`). Server: the same, echoed.
pub const OP_REMOVE: u16 = 0x2306;
/// Client: a new price (`u8 shop slot, u32 price`, `TP_MYSHOP_SETMONEY`; the US client never sends it).
pub const OP_SET_PRICE: u16 = 0x2307;
/// Server: to the seller, sold (`u8 shop slot, u8 count left, u32 money`, `TP_MYSHOP_SELLED`).
pub const OP_SOLD: u16 = 0x2308;
/// Client: visit (`u32 seller id`). Server: `u8 ok, u32 seller id` (`TP_MYSHOP_BUY_START`; the client ignores `ok`).
pub const OP_VISIT: u16 = 0x2309;
/// Client: stop visiting. Server: the visit window closes (`TP_MYSHOP_BUY_END`).
pub const OP_LEAVE: u16 = 0x230A;
/// Server: the shop's items (`TP_MYSHOP_BUY_LIST`).
pub const OP_LIST: u16 = 0x230B;
/// Server: to a visitor, the seller closed the shop (`TP_MYSHOP_BUY_STOPPED`).
pub const OP_STOPPED: u16 = 0x230C;
/// Client: buy (`u8 shop slot, u8 count`). Server: `u8 ok`, with the purchase when 1 (`TP_MYSHOP_BUY_BUYITEM`).
pub const OP_BUY: u16 = 0x230D;
/// Server: to a visitor, a shop slot's count (`u8 shop slot, u8 count left`, `TP_MYSHOP_BUY_SLOTSTAT`).
pub const OP_SLOT: u16 = 0x230E;
/// Server: the sign over a seller (`u32 id, u8 open, u8 len, char title[]`, `TP_PC_MYSHOPINFO`).
pub const OP_SIGN: u16 = 0x0523;

/// Shop slots (`stMyShop`'s arrays).
pub const SLOTS: u8 = 12;
/// The longest title kept (`CUser::PacketMyShop` cuts `len` to 120 and terminates the title there).
pub const TITLE_MAX: usize = 120;

/// `0x230D` results.
pub const BUY_FAILED: u8 = 0;
pub const BUY_OK: u8 = 1;
pub const BUY_NO_GOLD: u8 = 2;
pub const BUY_SELLER_TOO_MUCH_GOLD: u8 = 3;

/// `0x2305` and `0x2307`: an item placed or a new price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Add {
    pub bag: u8,
    pub slot: u8,
    pub shop_slot: u8,
    pub price: u32,
}

/// `0x2303`: the length the client gave (cut to [`TITLE_MAX`]) and the title up to its first zero, within that length's buffer. A title
/// holding a control byte or a colour marker that would run past its text is refused, as the same text would be in chat
/// (`docs/server/audit.md` R21).
pub fn parse_start(payload: &[u8]) -> Option<(u8, Vec<u8>)> {
    let (&len, rest) = payload.split_first()?;
    let len = len.min(TITLE_MAX as u8);
    let buffer = &rest[..rest.len().min(TITLE_MAX)];
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    let title = &buffer[..end];
    (title.iter().all(|&b| b >= 0x20) && crate::chat::markup_is_safe(title)).then(|| (len, title.to_vec()))
}

pub fn parse_add(payload: &[u8]) -> Option<Add> {
    match payload {
        &[bag, slot, shop_slot, a, b, c, d] => Some(Add { bag, slot, shop_slot, price: u32::from_le_bytes([a, b, c, d]) }),
        _ => None,
    }
}

/// `0x2307`: `u8 shop slot, u32 price`.
pub fn parse_set_price(payload: &[u8]) -> Option<(u8, u32)> {
    match payload {
        &[shop_slot, a, b, c, d] => Some((shop_slot, u32::from_le_bytes([a, b, c, d]))),
        _ => None,
    }
}

/// `0x230D`: `u8 shop slot, u8 count`.
pub fn parse_buy(payload: &[u8]) -> Option<(u8, u8)> {
    match payload {
        &[shop_slot, count] => Some((shop_slot, count)),
        _ => None,
    }
}

pub use crate::trade::{parse_byte, parse_id};

pub fn begin() -> Vec<u8> {
    encode(OP_BEGIN, &[])
}

pub fn end() -> Vec<u8> {
    encode(OP_END, &[])
}

pub fn started() -> Vec<u8> {
    encode(OP_START, &[])
}

pub fn stopped() -> Vec<u8> {
    encode(OP_STOP, &[])
}

pub fn added(add: &Add) -> Vec<u8> {
    let mut p = vec![add.bag, add.slot, add.shop_slot];
    p.extend_from_slice(&add.price.to_le_bytes());
    encode(OP_ADD, &p)
}

pub fn removed(shop_slot: u8) -> Vec<u8> {
    encode(OP_REMOVE, &[shop_slot])
}

pub fn sold(shop_slot: u8, left: u8, money: u32) -> Vec<u8> {
    let mut p = vec![shop_slot, left];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_SOLD, &p)
}

/// `0x2309` with ok 1 (the only one the new server sends: the client opens the window for any).
pub fn visiting(seller: u32) -> Vec<u8> {
    let mut p = vec![1];
    p.extend_from_slice(&seller.to_le_bytes());
    encode(OP_VISIT, &p)
}

pub fn left() -> Vec<u8> {
    encode(OP_LEAVE, &[])
}

/// The type, type id, count, quality, gems and craft name (21 bytes, zero-padded) of an item, as the shop packets carry them.
fn item_fields(p: &mut Vec<u8>, item: &ItemUnit, count: u8) {
    p.extend_from_slice(&[item.item_type, item.type_id, count]);
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    let mut name = [0u8; NAME_LEN];
    let len = item.craft_name.len().min(NAME_LEN - 1);
    name[..len].copy_from_slice(&item.craft_name[..len]);
    p.extend_from_slice(&name);
}

/// `0x230B`: each shop slot shown, in slot order, with its price of one (`MYSHOP_UNIT`, 37 bytes).
pub fn list(units: &[(u8, u32, &ItemUnit)]) -> Vec<u8> {
    let mut p = vec![units.len() as u8];
    for &(shop_slot, price, item) in units {
        p.push(shop_slot);
        p.extend_from_slice(&price.to_le_bytes());
        item_fields(&mut p, item, item.count);
    }
    encode(OP_LIST, &p)
}

pub fn closed_to_visitor() -> Vec<u8> {
    encode(OP_STOPPED, &[])
}

/// `0x230D` with a failure's result alone (3 bytes).
pub fn buy_failed(result: u8) -> Vec<u8> {
    encode(OP_BUY, &[result])
}

/// `0x230D 1`: the buyer's money now, the shop slot and what is left in it, then where the item went and what it is (`item`'s
/// type, type id, quality, gems and craft name with the count bought).
pub fn bought(money: u32, shop_slot: u8, left: u8, (bag, slot): (u8, u8), item: &ItemUnit, count: u8) -> Vec<u8> {
    let mut p = vec![BUY_OK];
    p.extend_from_slice(&money.to_le_bytes());
    p.extend_from_slice(&[shop_slot, left, bag, slot]);
    item_fields(&mut p, item, count);
    encode(OP_BUY, &p)
}

pub fn slot_count(shop_slot: u8, left: u8) -> Vec<u8> {
    encode(OP_SLOT, &[shop_slot, left])
}

/// `0x0523` open: `len + 9` bytes as the original sends it, the title cut at `len` and zero-padded (its zero and one byte more).
pub fn sign(id: u32, len: u8, title: &[u8]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&[1, len]);
    let mut text = vec![0u8; usize::from(len) + 1];
    let n = title.len().min(usize::from(len));
    text[..n].copy_from_slice(&title[..n]);
    p.extend_from_slice(&text);
    encode(OP_SIGN, &p)
}

/// `0x0523` closed (7 bytes).
pub fn sign_down(id: u32) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(0);
    encode(OP_SIGN, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    fn item() -> ItemUnit {
        ItemUnit { bag: 1, slot: 4, item_type: 25, type_id: 2, quality: 0, gems: [0; 6], count: 7, craft_name: b"0102".to_vec() }
    }

    #[test]
    fn sizes_match_the_original() {
        // Opcode included, as the PDB sizes count it.
        let len = |p: &[u8]| payload(p).len() + 2;
        let add = Add { bag: 1, slot: 2, shop_slot: 3, price: 500 };
        for p in [begin(), end(), started(), stopped(), left(), closed_to_visitor()] {
            assert_eq!(len(&p), 2);
        }
        assert_eq!(len(&added(&add)), 9);
        assert_eq!(len(&removed(3)), 3);
        assert_eq!(len(&sold(3, 0, 10)), 8);
        assert_eq!(len(&visiting(9)), 7);
        assert_eq!(len(&list(&[(0, 5, &item()), (4, 6, &item())])), 3 + 2 * 37);
        assert_eq!(len(&buy_failed(BUY_NO_GOLD)), 3);
        assert_eq!(len(&bought(1, 2, 3, (1, 5), &item(), 2)), 43);
        assert_eq!(len(&slot_count(2, 0)), 4);
        assert_eq!(len(&sign(9, 5, b"hello")), 5 + 9);
        assert_eq!(len(&sign(9, 120, &[b'a'; 120])), 129, "TP_PC_MYSHOPINFO");
        assert_eq!(len(&sign_down(9)), 7);
    }

    #[test]
    fn fields_are_where_the_client_reads_them() {
        let p = bought(0x0102_0304, 2, 5, (1, 9), &item(), 2);
        assert_eq!(opcode(&p), OP_BUY);
        let b = payload(&p);
        assert_eq!(b[0], BUY_OK);
        assert_eq!(u32::from_le_bytes(b[1..5].try_into().unwrap()), 0x0102_0304);
        assert_eq!(&b[5..12], &[2, 5, 1, 9, 25, 2, 2], "shop slot, left, bag, slot, type, type id, count bought");
        assert_eq!(&b[20..24], b"0102");

        let l = list(&[(4, 70, &item())]);
        let b = payload(&l);
        assert_eq!((b[0], b[1]), (1, 4));
        assert_eq!(u32::from_le_bytes(b[2..6].try_into().unwrap()), 70);
        assert_eq!(&b[6..9], &[25, 2, 7], "the item's count now");

        let s = sign(9, 3, b"abcdef");
        assert_eq!(payload(&s), &[9, 0, 0, 0, 1, 3, b'a', b'b', b'c', 0]);
    }

    #[test]
    fn requests_parse() {
        assert_eq!(parse_add(&[1, 2, 3, 0xF4, 1, 0, 0]), Some(Add { bag: 1, slot: 2, shop_slot: 3, price: 500 }));
        assert_eq!(parse_add(&[1, 2, 3]), None);
        assert_eq!(parse_buy(&[3, 2]), Some((3, 2)));
        assert_eq!(parse_set_price(&[3, 1, 0, 0, 0]), Some((3, 1)));
        assert_eq!(parse_start(&[6, b'h', b'e', b'l', b'l', b'o', 0]), Some((6, b"hello".to_vec())));
        let long: Vec<u8> = [200].into_iter().chain([b'x'; 130]).collect();
        let (len, title) = parse_start(&long).unwrap();
        assert_eq!((len, title.len()), (120, 120), "cut at 120");
        assert_eq!(parse_start(&[]), None);
        assert_eq!(parse_start(&[6, b'<', b'<', b'9', b'9', b'9', b'c', b'0', b'1', 0]), None, "a colour marker running past the text");
        assert_eq!(parse_start(&[3, b'a', 7, b'b', 0]), None, "a control byte");
        assert_eq!(parse_start(&[3, b'a', 0x1F, b'b', 0]), None);
        assert_eq!(parse_start(&[3, b'a', 0x20, b'b', 0]), Some((3, b"a b".to_vec())));
    }
}
