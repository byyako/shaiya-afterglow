//! Guild wars (`docs/original/guild-war.md`).

use crate::frame::encode;

/// Client: ask a player's guild (`u32 id`). Server: to both, `u32 requester, u32 target` (`TP_PVPG_REQ`).
pub const OP_REQUEST: u16 = 0x2501;
/// Client: the answer (`u8` 0 refuse, anything else accept). Server: `u8 result, u32 id` (`TP_PVPG_ANS`).
pub const OP_ANSWER: u16 = 0x2502;
/// Server: the war is on (`f32 x, f32 z, u32 enemy guild`).
pub const OP_READY: u16 = 0x2503;
/// Server: the fight begins.
pub const OP_START: u16 = 0x2504;
/// Server: the result (`u8` 1 the receiver's guild won, 2 the enemy guild won).
pub const OP_RESULT: u16 = 0x2505;
/// Server: a player left the war (`u8 event, u32 id`).
pub const OP_LEFT: u16 = 0x2506;

/// `0x2502` from the client.
pub const ANSWER_REFUSE: u8 = 0;

/// `0x2502` results.
pub const REFUSED: u8 = 0;
pub const ACCEPTED: u8 = 1;
pub const NOT_POSSIBLE: u8 = 2;
pub const NO_ANSWER: u8 = 3;

/// `0x2505` results.
pub const WON: u8 = 1;
pub const LOST: u8 = 2;

/// `0x2506` events.
pub const LEFT_WORLD: u8 = 0;
pub const OUT_OF_AREA: u8 = 1;
pub const DEFEATED: u8 = 3;

pub use crate::trade::{parse_byte, parse_id};

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

/// `0x2503`: the centre's x and z and the other guild's id.
pub fn ready(x: f32, z: f32, enemy: u32) -> Vec<u8> {
    let mut p = x.to_le_bytes().to_vec();
    p.extend_from_slice(&z.to_le_bytes());
    p.extend_from_slice(&enemy.to_le_bytes());
    encode(OP_READY, &p)
}

pub fn start() -> Vec<u8> {
    encode(OP_START, &[])
}

pub fn result(result: u8) -> Vec<u8> {
    encode(OP_RESULT, &[result])
}

pub fn left(event: u8, id: u32) -> Vec<u8> {
    let mut p = vec![event];
    p.extend_from_slice(&id.to_le_bytes());
    encode(OP_LEFT, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn sizes_match_the_original() {
        // Opcode included, as the PDB sizes count it.
        let len = |p: &[u8]| payload(p).len() + 2;
        assert_eq!(len(&request(1, 2)), 10);
        assert_eq!(len(&answer(NO_ANSWER, 2)), 7);
        assert_eq!(len(&ready(1.0, 2.0, 3)), 14);
        assert_eq!(len(&start()), 2);
        assert_eq!(len(&result(WON)), 3);
        assert_eq!(len(&left(DEFEATED, 2)), 7);
    }

    #[test]
    fn fields_in_order() {
        assert_eq!(payload(&request(0x0102_0304, 9)), [4, 3, 2, 1, 9, 0, 0, 0]);
        assert_eq!(payload(&answer(ACCEPTED, 7)), [1, 7, 0, 0, 0]);
        assert_eq!(payload(&ready(1.0, -1.0, 5)), [0, 0, 0x80, 0x3f, 0, 0, 0x80, 0xbf, 5, 0, 0, 0]);
        assert_eq!(payload(&left(OUT_OF_AREA, 7)), [1, 7, 0, 0, 0]);
    }
}
