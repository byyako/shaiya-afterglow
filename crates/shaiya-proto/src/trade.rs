//! Trade between two players (`docs/original/trade.md`).

use crate::chat::NAME_LEN;
use crate::frame::encode;
use crate::world::ItemUnit;

/// Client: request a trade (`u32 id`). Server: someone requests one (`u32 requester id`).
pub const OP_REQUEST: u16 = 0x0A01;
/// Client: the answer to a request (`u8` 0 yes, anything else no).
pub const OP_ANSWER: u16 = 0x0A02;
/// Server: the trade starts or did not (`u32 id, u8 result`).
pub const OP_START: u16 = 0x0A03;
/// Server: the trade is over (`u8 result`).
pub const OP_END: u16 = 0x0A04;
/// Client: ready (`u8` 0 ready, 1 not ready, 2 cancel). Server: who is ready (`u8 who, u8` 0 ready, 1 not ready).
pub const OP_READY: u16 = 0x0A05;
/// Client: offer an item (`u8 bag, slot, count, trade slot`). Server: the same, echoed.
pub const OP_ADD: u16 = 0x0A06;
/// Client: take an item back (`u8 trade slot`). Server: a trade slot emptied (`u8 who, u8 trade slot`).
pub const OP_REMOVE: u16 = 0x0A07;
/// Client: the money offered (`u32`). Server: `u8 who, u32 money`.
pub const OP_MONEY: u16 = 0x0A08;
/// Server: the partner offered an item (`TP_EXC_ADD_DEST`).
pub const OP_ADD_PARTNER: u16 = 0x0A09;

/// Item slots on each side of the window.
pub const SLOTS: u8 = 8;

/// `0x0A03` results.
pub const START_OK: u8 = 0;
pub const START_REFUSED: u8 = 1;
pub const START_BUSY: u8 = 2;

/// `0x0A04` results.
pub const END_DONE: u8 = 0;
pub const END_CANCELLED: u8 = 2;
pub const END_CANCELLED_BY_PARTNER: u8 = 3;
pub const END_NO_ROOM: u8 = 4;
pub const END_PARTNER_NO_ROOM: u8 = 5;
pub const END_TOO_MUCH_GOLD: u8 = 6;
pub const END_PARTNER_TOO_MUCH_GOLD: u8 = 7;

/// `byWho` of `0x0A05`, `0x0A07`, `0x0A08`.
pub const WHO_ME: u8 = 1;
pub const WHO_PARTNER: u8 = 2;
pub const WHO_BOTH: u8 = 3;

/// `0x0A05` from the client.
pub const READY: u8 = 0;
pub const NOT_READY: u8 = 1;
pub const CANCEL: u8 = 2;

/// `0x0A02` yes.
pub const ANSWER_YES: u8 = 0;

/// What `0x0A06` asks: `count` of the item at `bag`/`slot` into trade slot `trade_slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub bag: u8,
    pub slot: u8,
    pub count: u8,
    pub trade_slot: u8,
}

pub fn parse_id(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

/// `0x0A02` and `0x0A05` (`u8`), `0x0A07` (`u8 trade slot`).
pub fn parse_byte(payload: &[u8]) -> Option<u8> {
    match payload {
        [b] => Some(*b),
        _ => None,
    }
}

pub fn parse_offer(payload: &[u8]) -> Option<Offer> {
    match payload {
        &[bag, slot, count, trade_slot] => Some(Offer { bag, slot, count, trade_slot }),
        _ => None,
    }
}

pub fn parse_money(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

pub fn request(requester: u32) -> Vec<u8> {
    encode(OP_REQUEST, &requester.to_le_bytes())
}

pub fn start(id: u32, result: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(result);
    encode(OP_START, &p)
}

pub fn end(result: u8) -> Vec<u8> {
    encode(OP_END, &[result])
}

/// `0x0A05`: `not_ready` false for ready.
pub fn ready(who: u8, not_ready: bool) -> Vec<u8> {
    encode(OP_READY, &[who, u8::from(not_ready)])
}

pub fn added(offer: &Offer) -> Vec<u8> {
    encode(OP_ADD, &[offer.bag, offer.slot, offer.count, offer.trade_slot])
}

pub fn removed(who: u8, trade_slot: u8) -> Vec<u8> {
    encode(OP_REMOVE, &[who, trade_slot])
}

pub fn money(who: u8, money: u32) -> Vec<u8> {
    let mut p = vec![who];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_MONEY, &p)
}

/// `0x0A09`: `u8 trade slot, type, type id, count, u16 quality, u8 gems[6], char craft_name[21]` (35 bytes with the opcode), the count
/// being the one offered.
pub fn partner_added(trade_slot: u8, item: &ItemUnit, count: u8) -> Vec<u8> {
    let mut p = vec![trade_slot, item.item_type, item.type_id, count];
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    let mut name = [0u8; NAME_LEN];
    let len = item.craft_name.len().min(NAME_LEN - 1);
    name[..len].copy_from_slice(&item.craft_name[..len]);
    p.extend_from_slice(&name);
    encode(OP_ADD_PARTNER, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    #[test]
    fn sizes_match_the_original() {
        // Opcode included, as the PDB sizes count it.
        let len = |p: &[u8]| payload(p).len() + 2;
        assert_eq!(len(&request(7)), 6);
        assert_eq!(len(&start(7, START_BUSY)), 7);
        assert_eq!(len(&end(END_DONE)), 3);
        assert_eq!(len(&ready(WHO_BOTH, true)), 4);
        assert_eq!(len(&added(&Offer { bag: 1, slot: 2, count: 3, trade_slot: 4 })), 6);
        assert_eq!(len(&removed(WHO_ME, 4)), 4);
        assert_eq!(len(&money(WHO_PARTNER, 5)), 7);
        let sword = ItemUnit {
            item_type: 1,
            type_id: 2,
            quality: 300,
            gems: [1, 2, 3, 4, 5, 6],
            count: 1,
            craft_name: b"0123".to_vec(),
            ..Default::default()
        };
        let p = partner_added(3, &sword, 1);
        assert_eq!(opcode(&p), OP_ADD_PARTNER);
        assert_eq!(len(&p), 35);
        assert_eq!(&payload(&p)[..12], &[3, 1, 2, 1, 44, 1, 1, 2, 3, 4, 5, 6]);
        assert_eq!(&payload(&p)[12..17], b"0123\0");
    }

    #[test]
    fn requests_parse() {
        assert_eq!(parse_id(&[1, 0, 0, 0]), Some(1));
        assert_eq!(parse_id(&[1, 0, 0]), None);
        assert_eq!(parse_byte(&[2]), Some(2));
        assert_eq!(parse_byte(&[]), None);
        assert_eq!(parse_offer(&[1, 2, 3, 4]), Some(Offer { bag: 1, slot: 2, count: 3, trade_slot: 4 }));
        assert_eq!(parse_money(&[0, 1, 0, 0]), Some(256));
    }
}
