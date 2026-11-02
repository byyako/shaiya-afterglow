//! The gambling house: the slot machine and the card double-up (`docs/original/mini-game.md` section 2). The client's requests carry
//! at most one byte; each builder is the original's answer (`CUser::PacketMiniGame`).

use crate::frame::encode;

/// Client: sit at a table (`u8 table`, 0 to 6). Server: `u8 ok`.
pub const OP_SIT: u16 = 0x3101;
/// Client: bet one more line, or take back the re-bet. Server: `u8 result, u32 bet, u32 money`.
pub const OP_BET: u16 = 0x3102;
/// Client: spin. Server: `u8 ok, u8 cells[9], u8 won, u32 winnings, u8 short`.
pub const OP_SPIN: u16 = 0x3103;
/// Client: double up with the win. Server: `u8 dealer`.
pub const OP_DOUBLE: u16 = 0x3104;
/// Client: take the win and play again. Server: no payload.
pub const OP_AGAIN: u16 = 0x3105;
/// Client: stand up. Server: `u32 money`.
pub const OP_STAND: u16 = 0x3106;
/// Client: the guess (`u8`, 0 low, else high). Server: `u8 card, u8 result, u32 amount`.
pub const OP_GUESS: u16 = 0x3107;
/// Client: the next card. Server: `u8 dealer`.
pub const OP_NEXT: u16 = 0x3108;
/// Client: stop and take the stake. Server: `u32 money`.
pub const OP_STOP: u16 = 0x3109;

/// `0x3102` results: refused, a line added, the bet handed back.
pub const BET_REFUSED: u8 = 0;
pub const BET_LINE: u8 = 1;
pub const BET_BACK: u8 = 2;

/// `0x3107` results: lost, won (go on or stop), won the fifth round.
pub const GUESS_LOST: u8 = 0;
pub const GUESS_WON: u8 = 1;
pub const GUESS_WON_LAST: u8 = 2;

/// `0x3101`: `u8 table` from the client.
pub fn parse_sit(payload: &[u8]) -> Option<u8> {
    payload.first().copied()
}

/// `0x3107`: `u8 guess` from the client; true for high.
pub fn parse_guess(payload: &[u8]) -> Option<bool> {
    payload.first().map(|&g| g != 0)
}

/// `0x3101`.
pub fn sat(ok: bool) -> Vec<u8> {
    encode(OP_SIT, &[u8::from(ok)])
}

/// `0x3102`: the result, the bet and the money after.
pub fn bet(result: u8, bet: u32, money: u32) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&bet.to_le_bytes());
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_BET, &p)
}

/// `0x3103`: `ok` 0 with the rest zero when the spin was refused; else the nine cells (rows of three), whether a line won, the
/// winnings and whether the next spin could not be paid for (`short`).
pub fn spin(ok: bool, cells: [u8; 9], won: bool, winnings: u32, short: bool) -> Vec<u8> {
    let mut p = vec![u8::from(ok)];
    p.extend_from_slice(&cells);
    p.push(u8::from(won));
    p.extend_from_slice(&winnings.to_le_bytes());
    p.push(u8::from(short));
    encode(OP_SPIN, &p)
}

/// `0x3104` (the double-up starts) and `0x3108` (the next card): the dealer's card.
pub fn dealer(opcode: u16, card: u8) -> Vec<u8> {
    encode(opcode, &[card])
}

/// `0x3105`.
pub fn again() -> Vec<u8> {
    encode(OP_AGAIN, &[])
}

/// `0x3106`: the money, after `0x0213`.
pub fn stood(money: u32) -> Vec<u8> {
    encode(OP_STAND, &money.to_le_bytes())
}

/// `0x3107`: the player's card, the result and the amount (the stake won, or the fifth that came back).
pub fn guessed(card: u8, result: u8, amount: u32) -> Vec<u8> {
    let mut p = vec![card, result];
    p.extend_from_slice(&amount.to_le_bytes());
    encode(OP_GUESS, &p)
}

/// `0x3109`: the money.
pub fn stopped(money: u32) -> Vec<u8> {
    encode(OP_STOP, &money.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    /// The original's sizes count the opcode but not the length field.
    fn size(packet: &[u8]) -> usize {
        packet.len() - 2
    }

    #[test]
    fn the_answers_have_the_originals_sizes() {
        assert_eq!((size(&sat(true)), size(&bet(1, 100, 900)), size(&spin(true, [0; 9], false, 0, false))), (3, 11, 18));
        assert_eq!(
            (size(&dealer(OP_DOUBLE, 7)), size(&again()), size(&stood(5)), size(&guessed(3, 1, 9)), size(&stopped(5))),
            (3, 2, 6, 8, 6)
        );
    }

    #[test]
    fn the_answers_lay_out_their_fields() {
        assert_eq!(payload(&sat(false)), [0]);
        let b = bet(BET_LINE, 300, 700);
        assert_eq!((opcode(&b), payload(&b)), (OP_BET, &[1, 44, 1, 0, 0, 188, 2, 0, 0][..]));
        let s = spin(true, [1, 2, 3, 4, 5, 6, 7, 8, 9], true, 1000, true);
        assert_eq!(payload(&s), [1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 232, 3, 0, 0, 1]);
        assert_eq!(payload(&spin(false, [0; 9], false, 0, false)), [0; 16]);
        assert_eq!((opcode(&dealer(OP_NEXT, 9)), payload(&dealer(OP_NEXT, 9))), (OP_NEXT, &[9][..]));
        assert_eq!(payload(&guessed(12, GUESS_WON_LAST, 258)), [12, 2, 2, 1, 0, 0]);
        assert_eq!(payload(&stood(70000)), 70000u32.to_le_bytes());
    }

    #[test]
    fn requests_read_their_byte() {
        assert_eq!(parse_sit(&[3]), Some(3));
        assert_eq!(parse_sit(&[]), None);
        assert_eq!(
            (parse_guess(&[0]), parse_guess(&[1]), parse_guess(&[9]), parse_guess(&[])),
            (Some(false), Some(true), Some(true), None)
        );
    }
}
