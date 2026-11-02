//! Skill packets: learning (`0x0209`), the skill list (`0x0108`) and the quick slots (`0x010B`) (`docs/original/skills.md`); using
//! skills (`docs/original/skills-use.md` section 13); applied skills (`docs/original/skills-buffs.md` section 14); shapes, running,
//! stopping and scouting (`docs/original/skills-special.md` section 11); monster skills (`docs/original/mob-skills.md` section 10).

use crate::frame::encode;

/// Client: learn a skill (`TPC_USER_GETSKILL`). Server: the result (`TP_USER_GETSKILL`).
pub const OP_LEARN: u16 = 0x0209;
/// Server: the learnt skills, in the enter-world burst (`TP_CHAR_SKILL`).
pub const OP_CHAR_SKILL: u16 = 0x0108;
/// Client: save the quick slots. Server: the quick slots (`TP_CHAR_QUICK`), the last packet of the enter-world burst and the last
/// sent with AES: the server switches its side to XOR after it and the client's handler switches too.
pub const OP_CHAR_QUICK: u16 = 0x010B;

/// Server: a cast starts, at a player (or oneself) and at a monster (`TP_PC_SKILL_CASTING`).
pub const OP_CAST_USER: u16 = 0x0510;
pub const OP_CAST_MOB: u16 = 0x0516;
/// Client: use a skill on a player or oneself, on a monster (`u8 number, u32 target`). Server: the outcome (`TP_PC_SKILL_USE`).
pub const OP_USE_USER: u16 = 0x0511;
pub const OP_USE_MOB: u16 = 0x0517;
/// Server: a drain on a monster (`TP_PC_SKILL_USE_DRAIN`), one monster hit in an area (`TP_PC_SKILL_RANGE`), additional damage
/// (`TP_PC_SKILL_ADD`).
pub const OP_DRAIN_MOB: u16 = 0x0518;
pub const OP_AREA_MOB: u16 = 0x0519;
pub const OP_ADD_MOB: u16 = 0x051A;
/// Server: SP and MP a skill took (`TP_PC_USESPMP`).
pub const OP_COSTS: u16 = 0x050C;

/// Server: a skill applied to the player or refreshed (`TP_PC_SKILL_SET`), one taken off (`TP_PC_SKILL_REMOVE`), a heal over time
/// ending (`TP_SKILL_KEEP`).
pub const OP_APPLIED: u16 = 0x050D;
pub const OP_APPLIED_END: u16 = 0x050E;
pub const OP_KEEP_USER: u16 = 0x050F;
/// Server: a heal of a party member, or of oneself through a party skill without a party, and an area hit on a player
/// (`0x0511`'s layout; `docs/original/pvp-2.md` section 3).
pub const OP_USE_PARTY: u16 = 0x0513;
/// A drain on a player (`docs/original/pvp-2.md` section 3).
pub const OP_DRAIN_USER: u16 = 0x0512;
/// Additional damage on a player.
pub const OP_ADD_USER: u16 = 0x0514;
/// Server: a monster's attack reflected onto itself.
pub const OP_REFLECT: u16 = 0x051B;
/// Server: a damage-over-time or death-sentence debuff on a monster ending (`TP_SKILL_KEEP`).
pub const OP_KEEP_MOB: u16 = 0x0607;
/// Server: a monster's skill on one player, its cast announcement (result 4) and a monster healing itself (`TP_MOB_SKILL_USE`); a
/// hit of an area or multi-hit skill and a monster healed by another; the additional damage (both laid out by the original's code,
/// `docs/original/mob-skills.md` section 10).
pub const OP_MOB_SKILL: u16 = 0x060B;
pub const OP_MOB_AREA: u16 = 0x060D;
pub const OP_MOB_ADD: u16 = 0x060E;
/// `0x060B`'s result for a cast announcement.
pub const RESULT_ANNOUNCE: u8 = 4;
/// Server: the applied skills, in the enter-world burst (`TP_CHAR_SETSKILL`).
pub const OP_CHAR_APPLIED: u16 = 0x010A;
/// The `0x0511` number that cancels one of one's own buffs, the target being its key.
pub const CANCEL_NUMBER: u8 = 255;

/// Server: a player's shape (`TP_PC_SHAPE`: hidden 13, transformed, 0 normal) and an illusion's shape with its monster
/// (`TP_PC_SHAPE2`) (`docs/original/skills-special.md` section 2).
pub const OP_SHAPE: u16 = 0x051D;
pub const OP_SHAPE_MOB: u16 = 0x0524;
/// Client: switch between running and walking. Server: the mode now (`u8`, 1 running); also to make the client walk while hidden.
pub const OP_RUN: u16 = 0x0210;
/// Client: stop attacking (no payload; the server's own `0x0212` is "attack start").
pub const OP_STOP_ATTACK: u16 = 0x0212;
/// Server: what scouting found out about an enemy player (`TP_PC_SCOUTING_INFO`).
pub const OP_SCOUTING: u16 = 0x0558;
/// The shapes the server uses: hidden, disguised (another player's looks), an illusion (a monster's looks).
pub const SHAPE_HIDDEN: u8 = 13;
pub const SHAPE_DISGUISE: u8 = 100;
pub const SHAPE_ILLUSION: u8 = 102;

/// Most quick slots a character keeps (`m_pQuick[128]`); the client's save must hold fewer.
pub const MAX_QUICK_SLOTS: usize = 128;

/// `0x0209` from the client: `u16 skill id, u8 level`.
pub fn parse_learn(payload: &[u8]) -> Option<(u16, u8)> {
    match payload {
        [a, b, level] => Some((u16::from_le_bytes([*a, *b]), *level)),
        _ => None,
    }
}

pub fn learn_request(skill_id: u16, level: u8) -> Vec<u8> {
    let mut p = skill_id.to_le_bytes().to_vec();
    p.push(level);
    encode(OP_LEARN, &p)
}

/// `0x0209` to the client: `u8 result, u8 number, u16 skill id, u8 level` (the number is 0 on failure).
pub fn learn_result(result: u8, number: u8, skill_id: u16, level: u8) -> Vec<u8> {
    let mut p = vec![result, number];
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    encode(OP_LEARN, &p)
}

/// `0x0511` or `0x0517` from the client: `u8 number, u32 target`.
pub fn parse_use(payload: &[u8]) -> Option<(u8, u32)> {
    match payload {
        [number, a, b, c, d] => Some((*number, u32::from_le_bytes([*a, *b, *c, *d]))),
        _ => None,
    }
}

pub fn use_request(opcode: u16, number: u8, target: u32) -> Vec<u8> {
    let mut p = vec![number];
    p.extend_from_slice(&target.to_le_bytes());
    encode(opcode, &p)
}

/// `0x0510` or `0x0516`: `u32 caster, u32 target, u16 skill id, u8 level`.
pub fn casting(opcode: u16, caster: u32, target: u32, skill_id: u16, level: u8) -> Vec<u8> {
    let mut p = caster.to_le_bytes().to_vec();
    p.extend_from_slice(&target.to_le_bytes());
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    encode(opcode, &p)
}

fn use_payload(result: u8, caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&caster.to_le_bytes());
    p.extend_from_slice(&target.to_le_bytes());
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p
}

/// `0x0511`, `0x0517` or `0x0519`: `u8 result, u32 caster, u32 target, u16 skill id, u8 level, u16 HP, SP, MP`.
pub fn skill_use(opcode: u16, result: u8, caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    encode(opcode, &use_payload(result, caster, target, skill_id, level, values))
}

/// `0x051D` (7 bytes): `u32 player, u8 shape`.
pub fn shape(player: u32, shape: u8) -> Vec<u8> {
    let mut p = player.to_le_bytes().to_vec();
    p.push(shape);
    encode(OP_SHAPE, &p)
}

/// `0x0524` (9 bytes): `u32 player, u8 102, u16 monster id`.
pub fn shape_mob(player: u32, mob_id: u16) -> Vec<u8> {
    let mut p = player.to_le_bytes().to_vec();
    p.push(SHAPE_ILLUSION);
    p.extend_from_slice(&mob_id.to_le_bytes());
    encode(OP_SHAPE_MOB, &p)
}

/// `0x0210` (3 bytes): `u8 run`.
pub fn run_mode(run: bool) -> Vec<u8> {
    encode(OP_RUN, &[u8::from(run)])
}

/// `0x0558` (9 bytes): `i32 element defence, u16 level, u8 mode`.
pub fn scouting(element_defence: i32, level: u16, grow: u8) -> Vec<u8> {
    let mut p = element_defence.to_le_bytes().to_vec();
    p.extend_from_slice(&level.to_le_bytes());
    p.push(grow);
    encode(OP_SCOUTING, &p)
}

/// `0x0518`: as `0x0517`, then `u16 HP, SP, MP` the caster regained.
pub fn drain(result: u8, caster: u32, mob: u32, skill_id: u16, level: u8, values: [u16; 3], heals: [u16; 3]) -> Vec<u8> {
    drain_on(OP_DRAIN_MOB, result, caster, mob, skill_id, level, values, heals)
}

/// `0x0512` (`TP_PC_SKILL_USE_DRAIN` on a player, `docs/original/pvp-2.md` section 3): as `0x0518`.
pub fn drain_user(result: u8, caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3], heals: [u16; 3]) -> Vec<u8> {
    drain_on(OP_DRAIN_USER, result, caster, target, skill_id, level, values, heals)
}

#[allow(clippy::too_many_arguments)]
fn drain_on(opcode: u16, result: u8, caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3], heals: [u16; 3]) -> Vec<u8> {
    let mut p = use_payload(result, caster, target, skill_id, level, values);
    for v in heals {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(opcode, &p)
}

/// `0x051A`: `u32 caster, u32 monster, u16 skill id, u8 level, u16 HP, SP, MP`.
pub fn add_damage(caster: u32, mob: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    add_on(OP_ADD_MOB, caster, mob, skill_id, level, values)
}

/// `0x0514` (`TP_PC_SKILL_ADD` on a player): as `0x051A`.
pub fn add_damage_user(caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    add_on(OP_ADD_USER, caster, target, skill_id, level, values)
}

fn add_on(opcode: u16, caster: u32, target: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = caster.to_le_bytes().to_vec();
    p.extend_from_slice(&target.to_le_bytes());
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(opcode, &p)
}

/// `0x050C`: `u16 SP spent, u16 MP spent`.
pub fn costs(sp: u16, mp: u16) -> Vec<u8> {
    let mut p = sp.to_le_bytes().to_vec();
    p.extend_from_slice(&mp.to_le_bytes());
    encode(OP_COSTS, &p)
}

/// `0x050D`: `u32 key, u16 skill id, u8 level`.
pub fn applied(key: u32, skill_id: u16, level: u8) -> Vec<u8> {
    let mut p = key.to_le_bytes().to_vec();
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    encode(OP_APPLIED, &p)
}

/// `0x050E`: `u32 key`.
pub fn applied_end(key: u32) -> Vec<u8> {
    encode(OP_APPLIED_END, &key.to_le_bytes())
}

/// `0x050F` (a player) and `0x0607` (a monster): `u32 id, u16 skill id, u8 level, u16 HP, SP, MP`.
pub fn keep(opcode: u16, id: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(opcode, &p)
}

/// `0x051B`: `u32 player, u32 monster, u16 HP, SP, MP` the monster took back.
pub fn reflect(player: u32, mob: u32, values: [u16; 3]) -> Vec<u8> {
    let mut p = player.to_le_bytes().to_vec();
    p.extend_from_slice(&mob.to_le_bytes());
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_REFLECT, &p)
}

fn push_values(p: &mut Vec<u8>, values: [u16; 3]) {
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
}

/// `0x060B`: `u8 result, u32 monster, u32 player, u8 attack entry, u16 skill id, u8 level, u16 HP, SP, MP`.
pub fn mob_skill(result: u8, mob: u32, player: u32, entry: u8, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&mob.to_le_bytes());
    p.extend_from_slice(&player.to_le_bytes());
    p.push(entry);
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    push_values(&mut p, values);
    encode(OP_MOB_SKILL, &p)
}

/// `0x060D`: `u8 result, u32 monster, u32 player, u16 skill id, u8 level, u16 HP, SP, MP`.
pub fn mob_area(result: u8, mob: u32, player: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&mob.to_le_bytes());
    p.extend_from_slice(&player.to_le_bytes());
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    push_values(&mut p, values);
    encode(OP_MOB_AREA, &p)
}

/// `0x060E`: `u32 monster, u32 player, u16 skill id, u8 level, u16 HP, SP, MP` of additional damage.
pub fn mob_add(mob: u32, player: u32, skill_id: u16, level: u8, values: [u16; 3]) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    p.extend_from_slice(&player.to_le_bytes());
    p.extend_from_slice(&skill_id.to_le_bytes());
    p.push(level);
    push_values(&mut p, values);
    encode(OP_MOB_ADD, &p)
}

/// One applied skill of `0x010A`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppliedEntry {
    pub key: u32,
    pub skill_id: u16,
    pub level: u8,
    /// Seconds left.
    pub seconds: u32,
}

/// `0x010A`: `u8 count`, then `u32 key, u16 skill id, u8 level, u32 seconds left` each, 128 per packet; nothing when empty.
pub fn char_applied(entries: &[AppliedEntry]) -> Vec<Vec<u8>> {
    entries
        .chunks(128)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for e in chunk {
                p.extend_from_slice(&e.key.to_le_bytes());
                p.extend_from_slice(&e.skill_id.to_le_bytes());
                p.push(e.level);
                p.extend_from_slice(&e.seconds.to_le_bytes());
            }
            encode(OP_CHAR_APPLIED, &p)
        })
        .collect()
}

/// One `TP_SKILL_UNIT` of `0x0108`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillEntry {
    pub skill_id: u16,
    pub level: u8,
    pub number: u8,
    /// Seconds until it can be used again.
    pub delay: u32,
}

/// `0x0108`: `u8 count`, then `u16 skill id, u8 level, u8 number, u32 delay` per skill (at most 255 here).
pub fn char_skills(skills: &[SkillEntry]) -> Vec<u8> {
    let skills = &skills[..skills.len().min(255)];
    let mut p = vec![skills.len() as u8];
    for s in skills {
        p.extend_from_slice(&s.skill_id.to_le_bytes());
        p.extend_from_slice(&[s.level, s.number]);
        p.extend_from_slice(&s.delay.to_le_bytes());
    }
    encode(OP_CHAR_SKILL, &p)
}

/// A quick slot (`stQuick`), as the client sends it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuickSlot {
    pub bar: u8,
    pub slot: u8,
    pub bag: u8,
    pub number: u16,
}

/// `0x010B` from the client: `u8 count` (below 128), then `u8 bar, u8 slot, u8 bag, u16 number` each.
pub fn parse_quick(payload: &[u8]) -> Option<Vec<QuickSlot>> {
    let (&count, rest) = payload.split_first()?;
    if usize::from(count) >= MAX_QUICK_SLOTS || rest.len() != usize::from(count) * 5 {
        return None;
    }
    let (slots, _) = rest.as_chunks::<5>();
    Some(slots.iter().map(|&[bar, slot, bag, a, b]| QuickSlot { bar, slot, bag, number: u16::from_le_bytes([a, b]) }).collect())
}

/// `0x010B` in either direction: the count, then 5 bytes per slot.
pub fn char_quick(slots: &[QuickSlot]) -> Vec<u8> {
    let slots = &slots[..slots.len().min(MAX_QUICK_SLOTS - 1)];
    let mut p = vec![slots.len() as u8];
    for q in slots {
        p.extend_from_slice(&[q.bar, q.slot, q.bag]);
        p.extend_from_slice(&q.number.to_le_bytes());
    }
    encode(OP_CHAR_QUICK, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    #[test]
    fn monster_skill_packets_have_the_pdb_sizes() {
        let hit = mob_skill(1, 9, 2, 1, 46, 100, [300, 0, 0]);
        assert_eq!(hit.len(), 2 + 21, "TP_MOB_SKILL_USE");
        assert_eq!(opcode(&hit), OP_MOB_SKILL);
        assert_eq!(payload(&hit), [1, 9, 0, 0, 0, 2, 0, 0, 0, 1, 46, 0, 100, 44, 1, 0, 0, 0, 0]);
        assert_eq!(mob_area(3, 9, 2, 46, 100, [0; 3]).len(), 2 + 20);
        assert_eq!(mob_add(9, 2, 46, 100, [5, 0, 0]).len(), 2 + 19);
    }

    #[test]
    fn learn_packets_have_the_pdb_sizes() {
        let request = learn_request(258, 2);
        assert_eq!(request.len(), 2 + 5);
        assert_eq!(parse_learn(payload(&request)), Some((258, 2)));
        assert_eq!(parse_learn(&[1, 2]), None);
        let reply = learn_result(0, 4, 258, 2);
        assert_eq!(reply.len(), 2 + 7);
        assert_eq!(payload(&reply), [0, 4, 2, 1, 2]);
    }

    #[test]
    fn shape_packets_have_the_pdb_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(payload(&shape(7, SHAPE_HIDDEN)), [7, 0, 0, 0, 13]);
        assert_eq!(len(&shape(7, 0)), 7);
        assert_eq!(payload(&shape_mob(7, 413)), [7, 0, 0, 0, 102, 0x9d, 1]);
        assert_eq!(len(&shape_mob(7, 413)), 9);
        assert_eq!((opcode(&run_mode(false)), payload(&run_mode(false))), (OP_RUN, &[0][..]));
        assert_eq!(payload(&scouting(-1, 30, 2)), [0xff, 0xff, 0xff, 0xff, 30, 0, 2]);
        assert_eq!(len(&scouting(1, 30, 2)), 9);
    }

    #[test]
    fn use_packets_have_the_original_sizes() {
        // Sizes as the original counts them: opcode and payload.
        let len = |p: &[u8]| p.len() - 2;
        let request = use_request(OP_USE_MOB, 3, 9);
        assert_eq!(len(&request), 7);
        assert_eq!(parse_use(payload(&request)), Some((3, 9)));
        assert_eq!(parse_use(&[3, 9, 0, 0]), None);
        assert_eq!(len(&casting(OP_CAST_MOB, 7, 9, 258, 2)), 13);
        let hit = skill_use(OP_USE_MOB, 1, 7, 9, 258, 2, [300, 4, 5]);
        assert_eq!(len(&hit), 20);
        assert_eq!(payload(&hit), [1, 7, 0, 0, 0, 9, 0, 0, 0, 2, 1, 2, 0x2c, 1, 4, 0, 5, 0]);
        assert_eq!(len(&drain(0, 7, 9, 258, 2, [1, 2, 3], [4, 5, 6])), 26);
        assert_eq!(drain_user(0, 7, 9, 258, 2, [1, 2, 3], [4, 5, 6])[2..4], [0x12, 0x05]);
        assert_eq!(len(&drain_user(0, 7, 9, 258, 2, [1, 2, 3], [4, 5, 6])), 26);
        assert_eq!(len(&add_damage_user(7, 9, 258, 2, [1, 2, 3])), 19);
        assert_eq!(add_damage_user(7, 9, 258, 2, [1, 2, 3])[2..4], [0x14, 0x05]);
        assert_eq!(payload(&drain(0, 7, 9, 258, 2, [1, 2, 3], [4, 5, 6]))[18..], [4, 0, 5, 0, 6, 0]);
        assert_eq!(len(&add_damage(7, 9, 258, 2, [1, 2, 3])), 19);
        assert_eq!(payload(&costs(30, 2)), [30, 0, 2, 0]);
    }

    #[test]
    fn applied_skill_packets_have_the_pdb_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&applied(5, 30, 1)), 9);
        assert_eq!(payload(&applied(5, 30, 1)), [5, 0, 0, 0, 30, 0, 1]);
        assert_eq!(len(&applied_end(5)), 6);
        assert_eq!(len(&keep(OP_KEEP_USER, 7, 124, 1, [10, 0, 0])), 15);
        assert_eq!(payload(&keep(OP_KEEP_MOB, 7, 82, 2, [954, 0, 0]))[..9], [7, 0, 0, 0, 82, 0, 2, 0xba, 3]);
        assert_eq!(len(&reflect(7, 9, [40, 0, 0])), 16);
        assert!(char_applied(&[]).is_empty(), "nothing without applied skills");
        let one = AppliedEntry { key: 5, skill_id: 30, level: 1, seconds: 290 };
        let list = char_applied(&[one]);
        assert_eq!((opcode(&list[0]), len(&list[0])), (OP_CHAR_APPLIED, 3 + 11));
        assert_eq!(payload(&list[0]), [1, 5, 0, 0, 0, 30, 0, 1, 0x22, 1, 0, 0]);
        let many = char_applied(&[one; 130]);
        assert_eq!((many.len(), payload(&many[0])[0], payload(&many[1])[0]), (2, 128, 2), "128 a packet");
    }

    #[test]
    fn the_skill_list_is_eight_bytes_a_skill() {
        let list = char_skills(&[
            SkillEntry { skill_id: 3, level: 1, number: 0, delay: 0 },
            SkillEntry { skill_id: 300, level: 3, number: 1, delay: 7 },
        ]);
        assert_eq!(opcode(&list), OP_CHAR_SKILL);
        assert_eq!(payload(&list), [2, 3, 0, 1, 0, 0, 0, 0, 0, 44, 1, 3, 1, 7, 0, 0, 0]);
        assert_eq!(payload(&char_skills(&[])), [0]);
    }

    #[test]
    fn quick_slots_round_trip_and_are_checked() {
        let slots = [QuickSlot { bar: 0, slot: 1, bag: 100, number: 2 }, QuickSlot { bar: 1, slot: 0, bag: 1, number: 300 }];
        let packet = char_quick(&slots);
        assert_eq!(payload(&packet), [2, 0, 1, 100, 2, 0, 1, 0, 1, 44, 1]);
        assert_eq!(parse_quick(payload(&packet)), Some(slots.to_vec()));
        assert_eq!(parse_quick(&[0]), Some(Vec::new()));
        assert_eq!(parse_quick(&[1, 0, 0, 0, 0]), None, "short");
        assert_eq!(parse_quick(&[]), None);
        let mut full = vec![128u8];
        full.extend(std::iter::repeat_n(0, 128 * 5));
        assert_eq!(parse_quick(&full), None, "128 or more");
    }
}
