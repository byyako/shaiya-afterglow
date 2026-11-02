//! Parties (`docs/original/party.md`) and unions (`docs/original/union.md`): invitations, the member list, leaving, kicking, the
//! leader, member status and loot notices; a union's settings, sub-leader, groups and failures.

use crate::chat::{NAME_LEN, name_field};
use crate::frame::encode;

/// Server: the members already in the party, for a player joining (`TP_PARTY_LIST`).
pub const OP_LIST: u16 = 0x0B01;
/// Client: invite (`u32 id`). Server: the invitation (`u32 inviter id`).
pub const OP_INVITE: u16 = 0x0B02;
/// Client: the answer (`u8` 0 yes / 1 no, `u32` own id). Server: an invitation failed (`u8 code, u32 id`).
pub const OP_ANSWER: u16 = 0x0B03;
/// Server: a new member (`TP_PARTY_ENTER`).
pub const OP_ENTER: u16 = 0x0B04;
/// Client: leave. Server: someone left (`u32 id`).
pub const OP_LEAVE: u16 = 0x0B05;
/// Client: kick (`u32 id`). Server: someone was kicked (`u32 id`).
pub const OP_KICK: u16 = 0x0B06;
/// Client: make leader (`u32 id`). Server: the new leader (`u32 id`; 0 when the request failed).
pub const OP_LEADER: u16 = 0x0B07;
/// Server: a member's loot (`u32 id, u8 type, u8 type id`).
pub const OP_LOOT: u16 = 0x0B08;
/// Client: invite by name (`char name[21]`). Server: the invitation with the inviter's name.
pub const OP_INVITE_BY_NAME: u16 = 0x0B09;
/// Server: union members for a player joining, 6 to a packet (`TP_RAIDPARTY_LIST`).
pub const OP_UNION_LIST: u16 = 0x0B0B;
/// Server: a new union member (`u16 slot` + unit, `TP_RAIDPARTY_ENTER`).
pub const OP_UNION_ENTER: u16 = 0x0B0C;
/// Client: leave the union (or a party). Server: someone left (`u32 id`).
pub const OP_UNION_LEAVE: u16 = 0x0B0D;
/// Client: make or set up a union (`u8 union, u8 auto-join, u32 loot rule`). Server: the same and the leader's and sub-leader's
/// slots (`TP_PARTY_SETRAID`).
pub const OP_UNION_SET: u16 = 0x0B0E;
/// Both: the loot rule (`u32`).
pub const OP_UNION_RULE: u16 = 0x0B0F;
/// Both: auto-join (`u8`).
pub const OP_UNION_AUTO_JOIN: u16 = 0x0B10;
/// Client: `/join <name>` (`char name[21]`).
pub const OP_UNION_JOIN: u16 = 0x0B11;
/// Client: a group move (`u32 source slot, u32 target slot`). Server: the same and the leader's and sub-leader's slots.
pub const OP_UNION_MOVE: u16 = 0x0B12;
/// Both: the sub-leader (`u32 id`).
pub const OP_UNION_SUB_LEADER: u16 = 0x0B13;
/// Client: invite (`u32 id`). Server: the invitation (`u32 inviter id`).
pub const OP_UNION_INVITE: u16 = 0x0B14;
/// Client: the answer (`u8` 0 yes / 1 no, `u32` own id).
pub const OP_UNION_ANSWER: u16 = 0x0B15;
/// Both: dismantle.
pub const OP_UNION_BREAK: u16 = 0x0B16;
/// Both: the leader (`u32 id`).
pub const OP_UNION_LEADER: u16 = 0x0B17;
/// Client: invite by name (`char name[21]`). Server: the invitation with the inviter's name.
pub const OP_UNION_INVITE_BY_NAME: u16 = 0x0B18;
/// Server: a union request failed (`u32 code, u32 id`).
pub const OP_UNION_FAILED: u16 = 0x0B19;
/// Client: kick (`u32 id`). Server: someone was kicked (`u32 id`).
pub const OP_UNION_KICK: u16 = 0x0B1A;

/// Member status (`TP_PARTY_SET_*`), to the other members.
pub const OP_SET_ONE: u16 = 0x0C01;
pub const OP_SET_ONE_MAX: u16 = 0x0C02;
pub const OP_SET_ALL: u16 = 0x0C03;
pub const OP_SKILL_ADD: u16 = 0x0C04;
pub const OP_SKILL_REMOVE: u16 = 0x0C05;
pub const OP_DEATH: u16 = 0x0C06;
pub const OP_POSITION: u16 = 0x0C07;
pub const OP_SET_ALL_MAX: u16 = 0x0C08;
pub const OP_LEVEL: u16 = 0x0C09;
/// The union's member status and loot notice: the party's layouts (`docs/original/union.md` section 9; there is no `0x0C0F`).
pub const OP_UNION_SET_ALL: u16 = 0x0C0C;
pub const OP_UNION_SET_ALL_MAX: u16 = 0x0C0D;
pub const OP_UNION_SKILL_ADD: u16 = 0x0C0E;
pub const OP_UNION_SKILL_REMOVE: u16 = 0x0C10;
pub const OP_UNION_DEATH: u16 = 0x0C11;
pub const OP_UNION_POSITION: u16 = 0x0C12;
pub const OP_UNION_LEVEL: u16 = 0x0C13;
pub const OP_UNION_LOOT: u16 = 0x0C14;

/// Server: a player's party type to the players in view (`u32 id, u8 type`).
pub const OP_PARTY_TYPE: u16 = 0x0520;

/// Party types (`CUser::GetPartyType`).
pub const TYPE_NONE: u8 = 0;
pub const TYPE_MEMBER: u8 = 1;
pub const TYPE_LEADER: u8 = 2;

/// `0x0B03` failure codes (`CUser::SendPartyError`).
pub const REJECTED: u8 = 0;
pub const CANNOT_INVITE: u8 = 1;
pub const NOT_LEADER: u8 = 2;
pub const IN_ANOTHER_PARTY: u8 = 3;
pub const NO_SUCH_CHARACTER: u8 = 4;
pub const IN_THE_PARTY: u8 = 5;

/// `0x0B19` failure codes (`CUser::SendRaidPartyError`): 0 to 5 as `0x0B03`'s, and `/join`'s.
pub const UNION_NOT_FOUND: u32 = 12;
pub const UNION_NO_AUTO_JOIN: u32 = 13;
pub const UNION_FULL: u32 = 14;

/// The answers of `0x0B03` from the client.
pub const ANSWER_YES: u8 = 0;

/// At most this many applied skills go in a member unit; in a `0x0B0B` list, 35 (`CParty::EnterRaidParty`).
pub const MAX_UNIT_BUFFS: usize = 40;
pub const MAX_LIST_BUFFS: usize = 35;
/// Units in one `0x0B0B`.
pub const LIST_UNITS: usize = 6;

/// A member as the party window shows it (`PARTY_USER_INFO_UNIT`, sent packed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemberUnit {
    pub id: u32,
    pub name: Vec<u8>,
    pub level: u16,
    pub job: u8,
    /// HP, SP, MP (this order) and their maxima.
    pub max: [u16; 3],
    pub health: [u16; 3],
    pub map: u16,
    pub position: [f32; 3],
    /// Applied skills: id, level, seconds left.
    pub buffs: Vec<(u16, u8, u32)>,
}

impl MemberUnit {
    /// `u32 id, char name[21], u16 level, u8 job, u16 max HP, HP, max SP, SP, max MP, MP, u16 map, f32 x, y, z, u8 n`, n × `u16 skill,
    /// u8 level, u32 seconds left`: 55 + 7n bytes.
    fn write(&self, p: &mut Vec<u8>) {
        self.write_with(p, MAX_UNIT_BUFFS);
    }

    fn write_with(&self, p: &mut Vec<u8>, max_buffs: usize) {
        p.extend_from_slice(&self.id.to_le_bytes());
        p.extend_from_slice(&name_field(&self.name));
        p.extend_from_slice(&self.level.to_le_bytes());
        p.push(self.job);
        for i in 0..3 {
            p.extend_from_slice(&self.max[i].to_le_bytes());
            p.extend_from_slice(&self.health[i].to_le_bytes());
        }
        p.extend_from_slice(&self.map.to_le_bytes());
        for v in self.position {
            p.extend_from_slice(&v.to_le_bytes());
        }
        let buffs = &self.buffs[..self.buffs.len().min(max_buffs)];
        p.push(buffs.len() as u8);
        for &(skill, level, left) in buffs {
            p.extend_from_slice(&skill.to_le_bytes());
            p.push(level);
            p.extend_from_slice(&left.to_le_bytes());
        }
    }
}

/// `0x0B01`: `u8 leader index, u8 count`, the units.
pub fn list(leader: u8, members: &[MemberUnit]) -> Vec<u8> {
    let mut p = vec![leader, members.len() as u8];
    for m in members {
        m.write(&mut p);
    }
    encode(OP_LIST, &p)
}

/// `0x0B04`: a new member's unit.
pub fn enter(member: &MemberUnit) -> Vec<u8> {
    let mut p = Vec::new();
    member.write(&mut p);
    encode(OP_ENTER, &p)
}

fn by_id(opcode: u16, id: u32) -> Vec<u8> {
    encode(opcode, &id.to_le_bytes())
}

/// `0x0B02` to the invited player: the inviter's id.
pub fn invitation(inviter: u32) -> Vec<u8> {
    by_id(OP_INVITE, inviter)
}

/// `0x0B09` to the invited player: the inviter's name.
pub fn invitation_by_name(inviter: &[u8]) -> Vec<u8> {
    encode(OP_INVITE_BY_NAME, &name_field(inviter))
}

/// `0x0B03`: `u8 code, u32 id`.
pub fn failed(code: u8, id: u32) -> Vec<u8> {
    let mut p = vec![code];
    p.extend_from_slice(&id.to_le_bytes());
    encode(OP_ANSWER, &p)
}

pub fn left(id: u32) -> Vec<u8> {
    by_id(OP_LEAVE, id)
}

pub fn kicked(id: u32) -> Vec<u8> {
    by_id(OP_KICK, id)
}

pub fn leader(id: u32) -> Vec<u8> {
    by_id(OP_LEADER, id)
}

/// `0x0B08` (`0x0C14` in a union): `u32 id, u8 type, u8 type id`.
pub fn loot(union: bool, id: u32, item_type: u8, type_id: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&[item_type, type_id]);
    encode(if union { OP_UNION_LOOT } else { OP_LOOT }, &p)
}

/// `0x0520`: `u32 id, u8 type`.
pub fn party_type(id: u32, kind: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(kind);
    encode(OP_PARTY_TYPE, &p)
}

/// `0x0C03` (or `0x0C08` with the maxima; `0x0C0C`, `0x0C0D` in a union): `u32 id, u16 HP, SP, MP`.
pub fn status_all(union: bool, maxima: bool, id: u32, values: [u16; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    for v in values {
        p.extend_from_slice(&v.to_le_bytes());
    }
    let op = match (union, maxima) {
        (false, false) => OP_SET_ALL,
        (false, true) => OP_SET_ALL_MAX,
        (true, false) => OP_UNION_SET_ALL,
        (true, true) => OP_UNION_SET_ALL_MAX,
    };
    encode(op, &p)
}

/// `0x0C04` (added) or `0x0C05` (`0x0C0E`, `0x0C10` in a union): `u32 id, u16 skill, u8 level`.
pub fn skill(union: bool, added: bool, id: u32, skill: u16, level: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&skill.to_le_bytes());
    p.push(level);
    let op = match (union, added) {
        (false, true) => OP_SKILL_ADD,
        (false, false) => OP_SKILL_REMOVE,
        (true, true) => OP_UNION_SKILL_ADD,
        (true, false) => OP_UNION_SKILL_REMOVE,
    };
    encode(op, &p)
}

/// `0x0C06` (`0x0C11` in a union).
pub fn death(union: bool, id: u32) -> Vec<u8> {
    by_id(if union { OP_UNION_DEATH } else { OP_DEATH }, id)
}

/// `0x0C07` (`0x0C12` in a union): `u32 id, u16 map, f32 x, y, z`.
pub fn position(union: bool, id: u32, map: u16, at: [f32; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&map.to_le_bytes());
    for v in at {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(if union { OP_UNION_POSITION } else { OP_POSITION }, &p)
}

/// `0x0C09` (`0x0C13` in a union): `u32 id, u16 level`.
pub fn level(union: bool, id: u32, level: u16) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&level.to_le_bytes());
    encode(if union { OP_UNION_LEVEL } else { OP_LEVEL }, &p)
}

/// A union's settings as `0x0B0B` and `0x0B0E` carry them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnionSettings {
    pub leader_slot: u8,
    pub sub_leader_slot: u8,
    pub rule: u32,
    pub auto_join: bool,
}

/// `0x0B0B`, 6 units to a packet: `u8 flag, u8 leader slot, u8 sub-leader slot, u16 loot rule, u8 union (1), u8 auto-join, u8 n`,
/// n × (`u16 slot` + unit with at most 35 applied skills). `flag` is 1 for a packet of 6 when the total is not a multiple of 6.
pub fn union_lists(settings: UnionSettings, members: &[(u8, MemberUnit)]) -> Vec<Vec<u8>> {
    let more = !members.len().is_multiple_of(LIST_UNITS);
    members
        .chunks(LIST_UNITS)
        .map(|chunk| {
            let flag = u8::from(chunk.len() == LIST_UNITS && more);
            let mut p = vec![flag, settings.leader_slot, settings.sub_leader_slot];
            p.extend_from_slice(&(settings.rule as u16).to_le_bytes());
            p.extend_from_slice(&[1, u8::from(settings.auto_join), chunk.len() as u8]);
            for (slot, unit) in chunk {
                p.extend_from_slice(&u16::from(*slot).to_le_bytes());
                unit.write_with(&mut p, MAX_LIST_BUFFS);
            }
            encode(OP_UNION_LIST, &p)
        })
        .collect()
}

/// `0x0B0C`: `u16 slot` + the new member's unit.
pub fn union_enter(slot: u8, member: &MemberUnit) -> Vec<u8> {
    let mut p = u16::from(slot).to_le_bytes().to_vec();
    member.write(&mut p);
    encode(OP_UNION_ENTER, &p)
}

/// `0x0B0E`: `u8 union (1), u8 auto-join, u32 loot rule, u32 leader slot, u32 sub-leader slot`.
pub fn union_set(settings: UnionSettings) -> Vec<u8> {
    let mut p = vec![1, u8::from(settings.auto_join)];
    p.extend_from_slice(&settings.rule.to_le_bytes());
    p.extend_from_slice(&u32::from(settings.leader_slot).to_le_bytes());
    p.extend_from_slice(&u32::from(settings.sub_leader_slot).to_le_bytes());
    encode(OP_UNION_SET, &p)
}

pub fn union_rule(rule: u32) -> Vec<u8> {
    encode(OP_UNION_RULE, &rule.to_le_bytes())
}

pub fn union_auto_join(on: bool) -> Vec<u8> {
    encode(OP_UNION_AUTO_JOIN, &[u8::from(on)])
}

/// `0x0B12`: `u32 source, u32 target, u32 leader slot, u32 sub-leader slot`.
pub fn union_move(source: u8, target: u8, leader_slot: u8, sub_leader_slot: u8) -> Vec<u8> {
    let mut p = Vec::with_capacity(16);
    for v in [source, target, leader_slot, sub_leader_slot] {
        p.extend_from_slice(&u32::from(v).to_le_bytes());
    }
    encode(OP_UNION_MOVE, &p)
}

/// `0x0B19`: `u32 code, u32 id` (the client reads the code).
pub fn union_failed(code: u32, id: u32) -> Vec<u8> {
    let mut p = code.to_le_bytes().to_vec();
    p.extend_from_slice(&id.to_le_bytes());
    encode(OP_UNION_FAILED, &p)
}

/// `0x0B0D`, `0x0B13`, `0x0B14`, `0x0B17`, `0x0B1A`: `u32 id`.
pub fn union_by_id(opcode: u16, id: u32) -> Vec<u8> {
    by_id(opcode, id)
}

/// `0x0B18` to the invited player: the inviter's name.
pub fn union_invitation_by_name(inviter: &[u8]) -> Vec<u8> {
    encode(OP_UNION_INVITE_BY_NAME, &name_field(inviter))
}

pub fn union_break() -> Vec<u8> {
    encode(OP_UNION_BREAK, &[])
}

/// `0x0B0E` from the client: `u8 union, u8 auto-join, u32 loot rule`.
pub fn parse_union_set(payload: &[u8]) -> Option<(u8, bool, u32)> {
    let rule = u32::from_le_bytes(payload.get(2..6)?.try_into().ok()?);
    Some((payload[0], payload[1] != 0, rule))
}

/// `0x0B12` from the client: `u32 source slot, u32 target slot`.
pub fn parse_union_move(payload: &[u8]) -> Option<(u32, u32)> {
    Some((parse_id(payload)?, parse_id(payload.get(4..)?)?))
}

/// `0x0B02`, `0x0B06`, `0x0B07` from the client: `u32 id`.
pub fn parse_id(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.get(..4)?.try_into().ok()?))
}

/// `0x0B03` from the client: `u8 answer` (the id that follows is the player's own).
pub fn parse_answer(payload: &[u8]) -> Option<u8> {
    payload.first().copied()
}

/// `0x0B09` from the client: `char name[21]`, cut at its first zero (the original forces the last byte to 0).
pub fn parse_name(payload: &[u8]) -> Option<Vec<u8>> {
    let name = payload.get(..NAME_LEN)?;
    Some(name[..NAME_LEN - 1].iter().copied().take_while(|&b| b != 0).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    fn unit(buffs: usize) -> MemberUnit {
        MemberUnit { id: 7, name: b"Abc".to_vec(), level: 30, job: 5, buffs: vec![(44, 1, 300); buffs], ..Default::default() }
    }

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&enter(&unit(0))), 2 + 55);
        assert_eq!(len(&enter(&unit(2))), 2 + 55 + 14);
        assert_eq!(len(&enter(&unit(50))), 2 + 55 + 7 * MAX_UNIT_BUFFS, "at most 40 applied skills");
        assert_eq!(len(&list(1, &[unit(0), unit(1)])), 4 + 55 + 62);
        assert_eq!(len(&list(0, &[])), 4);
        assert_eq!(len(&invitation(1)), 6);
        assert_eq!(len(&invitation_by_name(b"Abc")), 23);
        assert_eq!(len(&failed(REJECTED, 1)), 7);
        assert_eq!(len(&left(1)), 6);
        assert_eq!(len(&kicked(1)), 6);
        assert_eq!(len(&leader(0)), 6);
        assert_eq!(len(&loot(false, 1, 5, 2)), 8);
        assert_eq!(len(&party_type(1, TYPE_LEADER)), 7);
        assert_eq!(len(&status_all(false, false, 1, [1, 2, 3])), 12);
        assert_eq!(len(&status_all(false, true, 1, [1, 2, 3])), 12);
        assert_eq!(len(&skill(false, true, 1, 44, 1)), 9);
        assert_eq!(len(&death(false, 1)), 6);
        assert_eq!(len(&position(false, 1, 2, [1.0, 2.0, 3.0])), 20);
        assert_eq!(len(&level(false, 1, 31)), 8);
    }

    #[test]
    fn union_packets_have_the_original_sizes_and_opcodes() {
        let len = |p: &[u8]| p.len() - 2;
        let op = |p: &[u8]| u16::from_le_bytes([p[2], p[3]]);
        let settings = UnionSettings { leader_slot: 0, sub_leader_slot: 2, rule: 4, auto_join: true };
        assert_eq!(len(&union_set(settings)), 16);
        assert_eq!(len(&union_enter(3, &unit(0))), 4 + 55);
        assert_eq!(len(&union_enter(3, &unit(50))), 4 + 55 + 7 * 40);
        assert_eq!(len(&union_move(1, 2, 0, 2)), 18);
        assert_eq!(len(&union_failed(UNION_FULL, 1)), 10);
        assert_eq!(len(&union_rule(2)), 6);
        assert_eq!(len(&union_auto_join(true)), 3);
        assert_eq!(len(&union_break()), 2);
        assert_eq!(len(&union_invitation_by_name(b"Abc")), 23);
        for (union, party) in [
            (status_all(true, false, 1, [0; 3]), OP_UNION_SET_ALL),
            (status_all(true, true, 1, [0; 3]), OP_UNION_SET_ALL_MAX),
            (skill(true, true, 1, 2, 3), OP_UNION_SKILL_ADD),
            (skill(true, false, 1, 2, 3), OP_UNION_SKILL_REMOVE),
            (death(true, 1), OP_UNION_DEATH),
            (position(true, 1, 2, [0.0; 3]), OP_UNION_POSITION),
            (level(true, 1, 2), OP_UNION_LEVEL),
            (loot(true, 1, 2, 3), OP_UNION_LOOT),
        ] {
            assert_eq!(op(&union), party);
        }
    }

    #[test]
    fn union_lists_go_six_to_a_packet() {
        let settings = UnionSettings { leader_slot: 1, sub_leader_slot: 2, rule: 2, auto_join: false };
        let members: Vec<(u8, MemberUnit)> = (0..8).map(|i| (i, unit(if i == 0 { 50 } else { 0 }))).collect();
        let lists = union_lists(settings, &members);
        assert_eq!(lists.len(), 2);
        let first = payload(&lists[0]).to_vec();
        assert_eq!(&first[..8], &[1, 1, 2, 2, 0, 1, 0, 6], "flag 1: 8 is not a multiple of 6");
        assert_eq!(first.len(), 8 + 6 * (2 + 55) + 7 * MAX_LIST_BUFFS, "at most 35 applied skills in a list");
        assert_eq!(&first[8..10], &[0, 0], "the first unit's slot");
        let second = payload(&lists[1]).to_vec();
        assert_eq!(&second[..8], &[0, 1, 2, 2, 0, 1, 0, 2]);
        assert_eq!(&second[8..10], &[6, 0]);
        let six: Vec<(u8, MemberUnit)> = (0..6).map(|i| (i, unit(0))).collect();
        assert_eq!(payload(&union_lists(settings, &six)[0])[0], 0, "a multiple of 6: flag 0");
    }

    #[test]
    fn the_unit_is_packed_in_the_original_order() {
        let u = MemberUnit {
            id: 0x0102_0304,
            name: b"Ab".to_vec(),
            level: 0x0506,
            job: 3,
            max: [100, 200, 300],
            health: [10, 20, 30],
            map: 2,
            position: [1.0, 0.0, 0.0],
            buffs: vec![(0x0708, 2, 9)],
        };
        let p = payload(&enter(&u)).to_vec();
        assert_eq!(&p[..4], &[4, 3, 2, 1]);
        assert_eq!(&p[4..7], b"Ab\0");
        assert_eq!(&p[25..28], &[6, 5, 3]);
        // Max HP, HP, max SP, SP, max MP, MP.
        assert_eq!(&p[28..40], &[100, 0, 10, 0, 200, 0, 20, 0, 44, 1, 30, 0]);
        assert_eq!(&p[40..42], &[2, 0]);
        assert_eq!(&p[42..46], &1.0f32.to_le_bytes());
        assert_eq!(p[54], 1);
        assert_eq!(&p[55..], &[8, 7, 2, 9, 0, 0, 0]);
    }

    #[test]
    fn requests_parse() {
        assert_eq!(parse_id(&[7, 0, 0, 0]), Some(7));
        assert_eq!(parse_id(&[7, 0]), None);
        assert_eq!(parse_answer(&[1, 7, 0, 0, 0]), Some(1));
        let mut name = [b'x'; 21];
        name[3] = 0;
        assert_eq!(parse_name(&name), Some(b"xxx".to_vec()));
        assert_eq!(parse_name(&[b'y'; 21]), Some(vec![b'y'; 20]), "the last byte is forced to 0");
        assert_eq!(parse_name(&[b'y'; 20]), None);
        assert_eq!(parse_union_set(&[1, 0, 4, 0, 0, 0]), Some((1, false, 4)));
        assert_eq!(parse_union_set(&[1, 0, 4]), None);
        assert_eq!(parse_union_move(&[1, 0, 0, 0, 7, 0, 0, 0]), Some((1, 7)));
        assert_eq!(parse_union_move(&[1, 0, 0, 0]), None);
    }
}
