//! Using items, the resets, and the moves items make (`docs/original/item-use.md`); saved positions and the body change
//! (`docs/original/item-windows.md`).

use crate::frame::encode;

/// Client: use an item (`u8 bag, u8 slot`). Server: an item used, or the error (`TP_PC_USEITEM`).
pub const OP_USE: u16 = 0x050A;
/// Client: use an item on someone (`u8 bag, u8 slot, u32 target`).
pub const OP_USE_ON: u16 = 0x0557;
/// Client: use an item's skill (`u8 bag, u8 slot, u8 kind, u32 target`).
pub const OP_USE_SKILL: u16 = 0x0559;
/// Client: save a position (`u8 index, u16 map, f32 x, y, z`). Server: the result (`TP_SAVEPOS_WRITE`).
pub const OP_SAVE_POINT: u16 = 0x0220;
/// Server: the saved positions (`TP_SAVEPOS_LIST`).
pub const OP_SAVE_POINTS: u16 = 0x0225;
/// Client: a body change from a scroll (`u8 bag, u8 slot, u8 hair, u8 face, u8 size, u8 sex`). Server: someone's new looks
/// (`TP_BODYCHG`).
pub const OP_BODY_CHANGE: u16 = 0x0226;
/// Server: a sex change refused for the equipment (`TP_BODYCHG_FAIL`).
pub const OP_BODY_CHANGE_FAILED: u16 = 0x0228;
/// Server: a move by item starts (the casting, `u32 id`).
pub const OP_MOVE_CAST: u16 = 0x0221;
/// Server: a move by item ends (`u32 id, u8 result`: 0 failed or cancelled).
pub const OP_MOVE_END: u16 = 0x0222;
/// Server: a party member summons this player (`TP_PARTYCALL_REQUEST`, `u32 summoner id`).
pub const OP_PARTY_CALL: u16 = 0x0223;
/// Client: the answer (`u8`: 0 yes). Server, to the summoner: `u32 member id, u8 answer` (`TP_PARTYCALL_ANSWER`).
pub const OP_PARTY_CALL_ANSWER: u16 = 0x0224;
/// Client: the free stat reset (no payload). Server: a stat reset's result (`TP_USER_RESETSTAT`).
pub const OP_RESET_STATS: u16 = 0x0214;
/// Client: the free skill reset (no payload). Server: a skill reset's result (`TP_USER_RESETSKILL`).
pub const OP_RESET_SKILLS: u16 = 0x0215;
/// Client: a repair (`u8 bag, u8 slot`; 255, 255 for everything). Server: done (`TP_ITEM_REPAIR`: `u8 bag, u8 slot, u32 money`).
pub const OP_REPAIR: u16 = 0x0554;
/// Client: what is my VG (no payload). Server: `i16 VG` (`TP_ITEM_REPAIRVG`, `docs/original/small-items.md` section 8).
pub const OP_VG: u16 = 0x0555;
/// Client: `/town` or `/return` (no payload); never answered (`docs/original/small-items.md` section 6).
pub const OP_TOWN: u16 = 0x0556;

/// `0x0559` kinds: the item's skill on oneself, a player, a monster.
pub const KIND_SELF: u8 = 0;
pub const KIND_PLAYER: u8 = 1;
pub const KIND_MOB: u8 = 2;

/// What one of the three use requests asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    pub bag: u8,
    pub slot: u8,
    /// `None` for `0x050A`: the player itself.
    pub target: Option<u32>,
    pub kind: u8,
}

/// `0x050A`, `0x0557` or `0x0559` from the client; `None` for another opcode or a wrong length.
pub fn parse_use(opcode: u16, payload: &[u8]) -> Option<Use> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(payload.get(at..at + 4)?.try_into().ok()?));
    match (opcode, payload.len()) {
        (OP_USE, 2) => Some(Use { bag: payload[0], slot: payload[1], target: None, kind: KIND_SELF }),
        (OP_USE_ON, 6) => Some(Use { bag: payload[0], slot: payload[1], target: Some(u32_at(2)?), kind: KIND_SELF }),
        (OP_USE_SKILL, 7) => Some(Use { bag: payload[0], slot: payload[1], target: Some(u32_at(3)?), kind: payload[2] }),
        _ => None,
    }
}

/// `0x050A`: `u32 player id, u8 bag, u8 slot, u8 type, u8 type id, u8 count left`.
pub fn used(id: u32, bag: u8, slot: u8, item_type: u8, type_id: u8, count: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&[bag, slot, item_type, type_id, count]);
    encode(OP_USE, &p)
}

/// The error answer: `0x050A` with everything but the id 0.
pub fn use_failed(id: u32) -> Vec<u8> {
    used(id, 0, 0, 0, 0, 0)
}

pub fn move_cast(id: u32) -> Vec<u8> {
    encode(OP_MOVE_CAST, &id.to_le_bytes())
}

pub fn move_end(id: u32, result: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(result);
    encode(OP_MOVE_END, &p)
}

pub fn party_call(summoner: u32) -> Vec<u8> {
    encode(OP_PARTY_CALL, &summoner.to_le_bytes())
}

/// `0x0224` from the client: `u8 answer`.
pub fn parse_party_call_answer(payload: &[u8]) -> Option<u8> {
    match payload {
        [answer] => Some(*answer),
        _ => None,
    }
}

pub fn party_call_answer(member: u32, answer: u8) -> Vec<u8> {
    let mut p = member.to_le_bytes().to_vec();
    p.push(answer);
    encode(OP_PARTY_CALL_ANSWER, &p)
}

/// `0x0214` done: `u8 1, u16 stat points`, then the stats in the order STR, REC, INT, WIS, DEX, LUC. `stats` is in the stored
/// order STR, DEX, REC, INT, WIS, LUC.
pub fn stats_reset(stat_point: u16, stats: [u16; 6]) -> Vec<u8> {
    let [str, dex, rec, int, wis, luc] = stats;
    let mut p = vec![1];
    for v in [stat_point, str, rec, int, wis, dex, luc] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_RESET_STATS, &p)
}

/// `0x0215` done: `u8 1, u16 skill points`, and `u8 1` for the free reset (the client then counts its `0x021B` chance down;
/// `docs/original/item-use.md` section 11, deviation 5).
pub fn skills_reset(skill_point: u16, free: bool) -> Vec<u8> {
    let mut p = vec![1];
    p.extend_from_slice(&skill_point.to_le_bytes());
    if free {
        p.push(1);
    }
    encode(OP_RESET_SKILLS, &p)
}

/// A free reset refused: `0x0214` or `0x0215` with `u8 0`.
pub fn reset_refused(opcode: u16) -> Vec<u8> {
    encode(opcode, &[0])
}

/// `0x0554` from the client: `u8 bag, u8 slot`.
pub fn parse_repair(payload: &[u8]) -> Option<(u8, u8)> {
    let &[bag, slot] = payload else { return None };
    Some((bag, slot))
}

/// `0x0554` after a repair: the place (255, 255 for everything) and the money left.
pub fn repaired(bag: u8, slot: u8, money: u32) -> Vec<u8> {
    let mut p = vec![bag, slot];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_REPAIR, &p)
}

/// `0x0554` for a repair of everything equipped (an NPC's or the Repair Hammer's): bag and slot 255.
pub fn repaired_all(money: u32) -> Vec<u8> {
    repaired(255, 255, money)
}

/// `0x0555`: the character's VG, 4 bytes with the opcode.
pub fn vg(vg: i16) -> Vec<u8> {
    encode(OP_VG, &vg.to_le_bytes())
}

/// A saved position in the packets: index 1 to 4, map, position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavePos {
    pub index: u8,
    pub map: u16,
    pub position: [f32; 3],
}

/// `0x0220` from the client: 15 bytes; a coordinate that is not a finite number is refused (`docs/server/audit.md` R21).
pub fn parse_save_point(payload: &[u8]) -> Option<SavePos> {
    if payload.len() != 15 {
        return None;
    }
    let f = |at: usize| f32::from_le_bytes(payload[at..at + 4].try_into().unwrap());
    let position = [f(3), f(7), f(11)];
    position.iter().all(|v| v.is_finite()).then(|| SavePos {
        index: payload[0],
        map: u16::from_le_bytes([payload[1], payload[2]]),
        position,
    })
}

/// `0x0220` answer: `u8 result` (0 saved, 1 or 2 refused), then the position (zeros on a refusal).
pub fn save_point_result(result: u8, pos: SavePos) -> Vec<u8> {
    let mut p = vec![result, pos.index];
    p.extend_from_slice(&pos.map.to_le_bytes());
    for v in pos.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_SAVE_POINT, &p)
}

/// `0x0225`: `u8 count`, then 15 bytes each; `None` without any.
pub fn save_points(points: &[SavePos]) -> Option<Vec<u8>> {
    if points.is_empty() {
        return None;
    }
    let mut p = vec![points.len() as u8];
    for point in points {
        p.push(point.index);
        p.extend_from_slice(&point.map.to_le_bytes());
        for v in point.position {
            p.extend_from_slice(&v.to_le_bytes());
        }
    }
    Some(encode(OP_SAVE_POINTS, &p))
}

/// `0x0226` from the client: `(bag, slot, [hair, face, size, sex])`.
pub fn parse_body_change(payload: &[u8]) -> Option<(u8, u8, [u8; 4])> {
    let &[bag, slot, hair, face, size, sex] = payload else { return None };
    Some((bag, slot, [hair, face, size, sex]))
}

/// `0x0226` to the view: `u32 id, u8 hair, u8 face, u8 size, u8 sex`.
pub fn body_changed(id: u32, [hair, face, size, sex]: [u8; 4]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&[hair, face, size, sex]);
    encode(OP_BODY_CHANGE, &p)
}

/// `0x0228`: the opcode alone.
pub fn body_change_failed() -> Vec<u8> {
    encode(OP_BODY_CHANGE_FAILED, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests() {
        assert_eq!(parse_use(OP_USE, &[1, 5]), Some(Use { bag: 1, slot: 5, target: None, kind: 0 }));
        assert_eq!(parse_use(OP_USE_ON, &[2, 3, 7, 0, 0, 0]), Some(Use { bag: 2, slot: 3, target: Some(7), kind: 0 }));
        assert_eq!(parse_use(OP_USE_SKILL, &[1, 0, 2, 9, 1, 0, 0]), Some(Use { bag: 1, slot: 0, target: Some(265), kind: 2 }));
        assert_eq!(parse_use(OP_USE, &[1]), None);
        assert_eq!(parse_use(OP_USE_ON, &[1, 2, 3]), None);
        assert_eq!(parse_party_call_answer(&[0]), Some(0));
        assert_eq!(parse_party_call_answer(&[]), None);
    }

    #[test]
    fn answers() {
        assert_eq!(used(0x0102_0304, 1, 2, 25, 19, 4), [13, 0, 0x0A, 0x05, 4, 3, 2, 1, 1, 2, 25, 19, 4]);
        assert_eq!(use_failed(9).len(), 13);
        assert_eq!(move_end(5, 255), [9, 0, 0x22, 0x02, 5, 0, 0, 0, 255]);
        assert_eq!(party_call_answer(6, 1), [9, 0, 0x24, 0x02, 6, 0, 0, 0, 1]);
        // STR 14, DEX 12, REC 9, INT 8, WIS 7, LUC 15 with 45 points: 45, STR, REC, INT, WIS, DEX, LUC.
        let p = stats_reset(45, [14, 12, 9, 8, 7, 15]);
        assert_eq!(p.len(), 19);
        assert_eq!(&p[4..], [1, 45, 0, 14, 0, 9, 0, 8, 0, 7, 0, 12, 0, 15, 0]);
        assert_eq!(skills_reset(32, false), [7, 0, 0x15, 0x02, 1, 32, 0]);
        assert_eq!(skills_reset(32, true), [8, 0, 0x15, 0x02, 1, 32, 0, 1]);
        assert_eq!(reset_refused(OP_RESET_STATS), [5, 0, 0x14, 0x02, 0]);
        assert_eq!(repaired_all(1000), [10, 0, 0x54, 0x05, 255, 255, 0xE8, 0x03, 0, 0]);
        assert_eq!(repaired(0, 5, 7), [10, 0, 0x54, 0x05, 0, 5, 7, 0, 0, 0]);
        assert_eq!(parse_repair(&[0, 5]), Some((0, 5)));
        assert_eq!(parse_repair(&[0]), None);
        let pos = SavePos { index: 2, map: 1, position: [1.0, 2.0, 3.0] };
        let mut wire = vec![2, 1, 0];
        [1.0f32, 2.0, 3.0].iter().for_each(|v| wire.extend_from_slice(&v.to_le_bytes()));
        assert_eq!(parse_save_point(&wire), Some(pos));
        wire[7..11].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(parse_save_point(&wire), None, "a height that is no number");
        assert_eq!(save_point_result(0, pos).len() - 2, 18);
        assert_eq!(save_points(&[pos, pos]).unwrap().len() - 2, 33);
        assert_eq!(save_points(&[]), None);
        assert_eq!(parse_body_change(&[1, 2, 3, 4, 5, 1]), Some((1, 2, [3, 4, 5, 1])));
        assert_eq!(body_changed(7, [3, 4, 5, 1]).len() - 2, 10);
        assert_eq!(body_change_failed().len() - 2, 2);
        assert_eq!(vg(-3), [6, 0, 0x55, 0x05, 0xFD, 0xFF]);
    }
}
