//! Mounts and the two-seat ride (`docs/original/vehicles.md`).

use crate::frame::encode;

/// Client: summon the mount, or get off (no payload). Server: the result (`TP_USEVEHICLE`: `u8 success, u8 status`).
pub const OP_USE: u16 = 0x0216;
/// Server: someone summons its mount (`TP_USEVEHICLEREADY`: `u32 id`).
pub const OP_READY: u16 = 0x0217;
/// Client: the passenger gets off (no payload). Server: someone rides behind someone, or no longer (`TP_VEHICLE_USEVEHICLE2`).
pub const OP_BEHIND: u16 = 0x021C;
/// Client: ask someone to ride behind (`u32 target`). Server: someone asks (`TP_VEHICLE_REQUEST`: `u32 asker`).
pub const OP_ASK: u16 = 0x021D;
/// Client: the answer (`u8`, 0 = yes). Server: the answer, or 2 when the asking failed (`TP_VEHICLE_ANSWER`).
pub const OP_ANSWER: u16 = 0x021E;

/// `0x0216` results: (success, status).
pub const SUMMON_REFUSED: (u8, u8) = (0, 1);
pub const TOO_SOON: (u8, u8) = (0, 0);
pub const MOUNTED: (u8, u8) = (1, 1);
pub const GOT_OFF: (u8, u8) = (1, 0);
/// `0x021E`: yes, and the asking failed.
pub const ANSWER_YES: u8 = 0;
pub const ASK_FAILED: u8 = 2;

/// `0x0216`.
pub fn used((success, status): (u8, u8)) -> Vec<u8> {
    encode(OP_USE, &[success, status])
}

/// `0x0217`.
pub fn ready(id: u32) -> Vec<u8> {
    encode(OP_READY, &id.to_le_bytes())
}

/// `0x021C`: `u32 passenger, u32 driver` (0 when it got off).
pub fn behind(passenger: u32, driver: u32) -> Vec<u8> {
    let mut p = passenger.to_le_bytes().to_vec();
    p.extend_from_slice(&driver.to_le_bytes());
    encode(OP_BEHIND, &p)
}

/// `0x021D` to the one asked: the asker.
pub fn ask(asker: u32) -> Vec<u8> {
    encode(OP_ASK, &asker.to_le_bytes())
}

/// `0x021E`.
pub fn answer(answer: u8) -> Vec<u8> {
    encode(OP_ANSWER, &[answer])
}

/// `0x021D` from the client: the player asked.
pub fn parse_ask(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

/// `0x021E` from the client: the answer.
pub fn parse_answer(payload: &[u8]) -> Option<u8> {
    let &[answer] = payload else { return None };
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&used(MOUNTED)), 4);
        assert_eq!(len(&ready(1)), 6);
        assert_eq!(len(&behind(1, 2)), 10);
        assert_eq!(len(&ask(1)), 6);
        assert_eq!(len(&answer(ASK_FAILED)), 3);
        assert_eq!(parse_ask(&7u32.to_le_bytes()), Some(7));
        assert_eq!(parse_answer(&[0]), Some(0));
        assert_eq!(parse_answer(&[]), None);
    }
}
