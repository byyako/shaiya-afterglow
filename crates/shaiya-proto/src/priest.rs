//! The Priest's paid reading (`0x0Fxx`, `CUser::PacketPriest`, `docs/original/small-items.md` section 12).

use crate::frame::encode;

/// Client: the reading (no payload). Server: `u8 1` and the five hidden numbers, or `u8 0` (not enough gold).
pub const OP_PRIEST_START: u16 = 0x0F01;
/// Client: the next reading (no payload). Server: `u8 1` or `u8 0`.
pub const OP_PRIEST_NEXT: u16 = 0x0F02;

/// The character's five hidden numbers as the reading shows them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    /// The honour.
    pub hg: u16,
    pub vg: i16,
    pub cg: u8,
    pub og: u8,
    pub ig: u8,
}

/// `0x0F01` (`TP_PRIEST_START`): `u8 1, u16 HG, i16 VG, u8 CG, u8 OG, u8 IG` (10 bytes with the opcode), or `u8 0` (3 bytes).
pub fn priest_start(reading: Option<Reading>) -> Vec<u8> {
    let Some(r) = reading else { return encode(OP_PRIEST_START, &[0]) };
    let mut p = vec![1];
    p.extend_from_slice(&r.hg.to_le_bytes());
    p.extend_from_slice(&r.vg.to_le_bytes());
    p.extend_from_slice(&[r.cg, r.og, r.ig]);
    encode(OP_PRIEST_START, &p)
}

/// `0x0F02`: `u8 1` (paid) or `u8 0`.
pub fn priest_next(paid: bool) -> Vec<u8> {
    encode(OP_PRIEST_NEXT, &[u8::from(paid)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn answers_have_the_original_sizes() {
        let r = priest_start(Some(Reading { hg: 300, vg: -2, cg: 3, og: 4, ig: 5 }));
        assert_eq!(r.len() - 2, 10);
        assert_eq!(payload(&r), [1, 0x2C, 0x01, 0xFE, 0xFF, 3, 4, 5]);
        assert_eq!(priest_start(None).len() - 2, 3);
        assert_eq!(payload(&priest_start(None)), [0]);
        assert_eq!(priest_next(true), [5, 0, 0x02, 0x0F, 1]);
        assert_eq!(priest_next(false).len() - 2, 3);
    }
}
