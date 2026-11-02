//! Duels (`docs/original/duel.md`). The stakes window is the trade window (`crate::trade`) with these opcodes.

use crate::frame::encode;
use crate::trade::{self, Offer};
use crate::world::ItemUnit;

/// Client: ask a player (`u32 id`). Server: to both, `u32 requester, u32 target` (`TP_PVP_REQ`).
pub const OP_REQUEST: u16 = 0x2401;
/// Client: the answer (`u8` 1 accept, 0 refuse). Server: `u8 result, u32 id` (`TP_PVP_ANS`).
pub const OP_ANSWER: u16 = 0x2402;
/// Server: the countdown begins at the area's centre (`f32 x, f32 z`, `TP_PVP_READY`).
pub const OP_READY: u16 = 0x2403;
/// Server: the fight begins (`TP_PVP_START`).
pub const OP_START: u16 = 0x2404;
/// Server: the result (`u8` 1 won, 2 lost; `TP_PVP_RESULT`).
pub const OP_RESULT: u16 = 0x2405;
/// Client: give up (`/defeat`). Server: ended without a winner (`u8 reason, u32 id`, `TP_PVP_CANCLE`).
pub const OP_CANCEL: u16 = 0x2406;
/// Server: the stakes window opens (`u32 partner, u8 0`).
pub const OP_WINDOW: u16 = 0x2407;
/// Server: the stakes window closes (`u8`).
pub const OP_WINDOW_CLOSED: u16 = 0x2408;
/// Client: stakes ready (`u8` 0 ready, 1 not ready, 2 close). Server: `u8 who, u8 not_ready`, as `0x0A05`.
pub const OP_STAKE_READY: u16 = 0x2409;
/// Client: stake an item (`u8 bag, slot, count, stake slot`). Server: the same, echoed.
pub const OP_STAKE_ADD: u16 = 0x240A;
/// Client: take an item back (`u8 stake slot`). Server: `u8 who, u8 stake slot`.
pub const OP_STAKE_REMOVE: u16 = 0x240B;
/// Client: the money staked (`u32`). Server: `u8 who, u32 money`.
pub const OP_STAKE_MONEY: u16 = 0x240C;
/// Server: the partner staked an item (`TP_EXC_ADD_DEST`, as `0x0A09`).
pub const OP_STAKE_PARTNER: u16 = 0x240D;

/// The family's last opcode (`CUser::PacketPVP`'s table); beyond it, and the server's own opcodes, close the connection.
pub const OP_LAST: u16 = OP_STAKE_PARTNER - 1;

/// `0x2402` from the client.
pub const ANSWER_REFUSE: u8 = 0;

/// `0x2402` results.
pub const REFUSED: u8 = 0;
pub const ACCEPTED: u8 = 1;
pub const NOT_POSSIBLE: u8 = 2;
pub const NO_ANSWER: u8 = 3;
pub const DEBUFFED: u8 = 4;

/// `0x2405` results.
pub const WON: u8 = 1;
pub const LOST: u8 = 2;

/// `0x2406` reasons.
pub const CANCEL_LEFT: u8 = 0;
pub const CANCEL_OUT_OF_AREA: u8 = 1;
pub const CANCEL_GAVE_UP: u8 = 2;
pub const CANCEL_INTERRUPTED: u8 = 3;

/// `0x2408` values.
pub const WINDOW_DONE: u8 = 0;
pub const WINDOW_CLOSED_BY_ME: u8 = 2;
pub const WINDOW_CLOSED_BY_PARTNER: u8 = 3;

/// `0x2409` from the client.
pub const READY: u8 = 0;
pub const NOT_READY: u8 = 1;
pub const CLOSE: u8 = 2;

pub use trade::{SLOTS, WHO_BOTH, WHO_ME, WHO_PARTNER, parse_byte, parse_id, parse_money, parse_offer};

pub fn request(requester: u32, target: u32) -> Vec<u8> {
    let mut p = requester.to_le_bytes().to_vec();
    p.extend_from_slice(&target.to_le_bytes());
    encode(OP_REQUEST, &p)
}

pub fn answer(result: u8, id: u32) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&id.to_le_bytes());
    encode(OP_ANSWER, &p)
}

/// `0x2403`: the centre's x and z.
pub fn ready(x: f32, z: f32) -> Vec<u8> {
    let mut p = x.to_le_bytes().to_vec();
    p.extend_from_slice(&z.to_le_bytes());
    encode(OP_READY, &p)
}

pub fn start() -> Vec<u8> {
    encode(OP_START, &[])
}

pub fn result(result: u8) -> Vec<u8> {
    encode(OP_RESULT, &[result])
}

pub fn cancel(reason: u8, id: u32) -> Vec<u8> {
    let mut p = vec![reason];
    p.extend_from_slice(&id.to_le_bytes());
    encode(OP_CANCEL, &p)
}

pub fn window(partner: u32) -> Vec<u8> {
    let mut p = partner.to_le_bytes().to_vec();
    p.push(0);
    encode(OP_WINDOW, &p)
}

pub fn window_closed(value: u8) -> Vec<u8> {
    encode(OP_WINDOW_CLOSED, &[value])
}

/// `0x2409`: `not_ready` false for ready.
pub fn stake_ready(who: u8, not_ready: bool) -> Vec<u8> {
    encode(OP_STAKE_READY, &[who, u8::from(not_ready)])
}

pub fn staked(offer: &Offer) -> Vec<u8> {
    encode(OP_STAKE_ADD, &[offer.bag, offer.slot, offer.count, offer.trade_slot])
}

pub fn unstaked(who: u8, stake_slot: u8) -> Vec<u8> {
    encode(OP_STAKE_REMOVE, &[who, stake_slot])
}

pub fn stake_money(who: u8, money: u32) -> Vec<u8> {
    let mut p = vec![who];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_STAKE_MONEY, &p)
}

/// `0x240D`: as `0x0A09`, the count being the one staked.
pub fn partner_staked(stake_slot: u8, item: &ItemUnit, count: u8) -> Vec<u8> {
    encode(OP_STAKE_PARTNER, crate::frame::payload(&trade::partner_added(stake_slot, item, count)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    #[test]
    fn sizes_match_the_original() {
        // Opcode included, as the PDB sizes count it.
        let len = |p: &[u8]| payload(p).len() + 2;
        assert_eq!(len(&request(1, 2)), 10);
        assert_eq!(len(&answer(DEBUFFED, 2)), 7);
        assert_eq!(len(&ready(1.0, 2.0)), 10);
        assert_eq!(len(&start()), 2);
        assert_eq!(len(&result(WON)), 3);
        assert_eq!(len(&cancel(CANCEL_GAVE_UP, 2)), 7);
        assert_eq!(len(&window(2)), 7);
        assert_eq!(len(&window_closed(WINDOW_DONE)), 3);
        assert_eq!(len(&stake_ready(WHO_BOTH, true)), 4);
        assert_eq!(len(&staked(&Offer { bag: 1, slot: 2, count: 3, trade_slot: 4 })), 6);
        assert_eq!(len(&unstaked(WHO_ME, 4)), 4);
        assert_eq!(len(&stake_money(WHO_PARTNER, 5)), 7);
        let sword = ItemUnit { item_type: 1, type_id: 2, quality: 300, count: 1, ..Default::default() };
        let partner = partner_staked(3, &sword, 1);
        assert_eq!(len(&partner), 35);
        assert_eq!(opcode(&partner), OP_STAKE_PARTNER);
        assert_eq!(payload(&partner), payload(&trade::partner_added(3, &sword, 1)));
    }

    #[test]
    fn fields_in_order() {
        assert_eq!(payload(&request(0x0102_0304, 9)), [4, 3, 2, 1, 9, 0, 0, 0]);
        assert_eq!(payload(&answer(ACCEPTED, 7)), [1, 7, 0, 0, 0]);
        assert_eq!(payload(&cancel(CANCEL_OUT_OF_AREA, 7)), [1, 7, 0, 0, 0]);
        assert_eq!(payload(&window(7)), [7, 0, 0, 0, 0]);
        assert_eq!(payload(&ready(1.0, -1.0)), [0, 0, 0x80, 0x3f, 0, 0, 0x80, 0xbf]);
    }
}
