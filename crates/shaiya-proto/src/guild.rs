//! Guilds (`docs/original/guild.md`): creating with the party's agreement, inviting, join requests, leaving, kicking, ranks, the remark,
//! disbanding, the member, request and guild lists.

use crate::chat::name_field;
use crate::frame::encode;
use crate::world::{ITEMS_PER_PACKET, ItemUnit, bank_unit};

/// Server: the older creation result, which the original sends for failures during the agreement (`u8 code, u32, u8, char name[25]`).
pub const OP_CREATE_FAILED: u16 = 0x0D01;
/// Client: disband. Server: the disband result (`u8 code`).
pub const OP_DISBAND: u16 = 0x0D03;
/// Client: the disband agreement's answer (`u8` 1 yes, 0 no). Server: the agreement dialog (`u32` master id).
pub const OP_DISBAND_AGREE: u16 = 0x0D04;
/// Client: invite (`u32 id`). Server: the invitation (`u32` inviter id).
pub const OP_INVITE: u16 = 0x0D05;
/// Client: the invitation's answer (`u8` 1 yes, 0 no). Server: the answer to the inviter (`u8 ok, u32 id`).
pub const OP_INVITE_ANSWER: u16 = 0x0D06;
/// Client: ask to join (`u32 guild id`). Server: the request's result (`u8`).
pub const OP_JOIN_REQUEST: u16 = 0x0D07;
/// Client: an officer's answer to a request (`u8 ok, u32 id`). Server: joined or refused (`u8 ok, u32 guild id, u8 rank, char name[25]`).
pub const OP_JOINED: u16 = 0x0D08;
/// Client: leave. Server: the result (`u8`).
pub const OP_LEAVE: u16 = 0x0D09;
/// Client: kick (`u32 id`). Server: the result (`u8 ok, u32 id`).
pub const OP_KICK: u16 = 0x0D0A;
/// Client: a rank change (`u8` 0 up / 1 down, `u32 id`). Server: a member's event (`u8 state, u32 id`).
pub const OP_MEMBER_STATE: u16 = 0x0D0C;
pub const OP_LIST_START: u16 = 0x0D0D;
pub const OP_LIST_END: u16 = 0x0D0E;
pub const OP_GUILD_REMOVED: u16 = 0x0D11;
pub const OP_MEMBERS_ONLINE: u16 = 0x0D12;
pub const OP_MEMBERS_OFFLINE: u16 = 0x0D13;
pub const OP_MEMBER_ADD: u16 = 0x0D14;
pub const OP_REQUESTS: u16 = 0x0D16;
pub const OP_REQUEST_ADD: u16 = 0x0D17;
pub const OP_REQUEST_REMOVE: u16 = 0x0D18;
/// Client: a new remark (`char remark[65]`). Server: the remark (`u8 result, char remark[65]`).
pub const OP_REMARK: u16 = 0x0D20;
/// Client: create (`char name[25], char remark[65]`). Server: the result (`u8 code, u32 guild id, u8 rank, name, remark`).
pub const OP_CREATE: u16 = 0x0D21;
/// Client: the creation agreement's answer (`u8` 1 yes, 0 no). Server: the dialog (`u32` requester id, name, remark).
pub const OP_CREATE_AGREE: u16 = 0x0D22;
/// Client: the guild's Etin. Server: `u32 etin`.
pub const OP_ETIN: u16 = 0x0D23;
pub const OP_GUILDS: u16 = 0x0D2F;
pub const OP_GUILD_ADD: u16 = 0x0D30;

/// Guild houses (`docs/original/guild-house.md`). Server: the house NPCs' levels, `u8 count, n × (u16 key, u8 level, u8 slot)`.
pub const OP_NPC_LEVELS: u16 = 0x0D24;
/// Client: `u16 key, u8 level`. Server: `u8 result, u16 key, u8 level, u8 slot`.
pub const OP_NPC_UPGRADE: u16 = 0x0D25;
/// Server: a house NPC refuses, `u8 result, u8 required`.
pub const OP_HOUSE_REFUSED: u16 = 0x0D28;
/// Client: `u32 npc id`. Server: `u8 result, u32 money`.
pub const OP_HOUSE_BUY: u16 = 0x0D34;
/// Client: `u32 npc id`. Server: `u8 count, n × (u8 bag, u8 slot)`.
pub const OP_ETIN_RETURN: u16 = 0x0D35;

/// The guild ranking battle (`docs/original/guild-ranking.md`). Server: a notice, `u16 map, u8 type`.
pub const OP_RANKING_NOTICE: u16 = 0x0D26;
/// Server: the guild's battle points, `u32 points, u32 best points of its country, u32 best guild`.
pub const OP_BATTLE_POINTS: u16 = 0x0D27;
/// Server: the ranking after an update, `u8 count, n × (u32 guild, u32 score, u8 place)`.
pub const OP_RANKING: u16 = 0x0D37;

/// The guild bank (`docs/original/guild-bank.md`). Server: its items, `u8 count, n × bank unit (33 bytes)`.
pub const OP_BANK_LIST: u16 = 0x0D29;
/// Server: a bank slot now holds this item, a bank unit and `u32 character id` of the mover.
pub const OP_BANK_IN: u16 = 0x0D31;
/// Server: a bank slot was emptied, the same layout; the unit is the item that left.
pub const OP_BANK_OUT: u16 = 0x0D32;

pub const GUILD_NAME_LEN: usize = 25;
pub const REMARK_LEN: usize = 65;

/// `0x0D21` and `0x0D01` codes.
pub const CREATE_FAILED: u8 = 0;
pub const CREATED: u8 = 1;
pub const CREATE_NO_GOLD: u8 = 2;
pub const CREATE_MEMBERS: u8 = 3;
pub const CREATE_LEVEL: u8 = 4;
pub const CREATE_MODE: u8 = 5;
pub const CREATE_NAME: u8 = 6;
pub const CREATE_REJECTED: u8 = 7;
pub const CREATE_IN_GUILD: u8 = 8;
pub const CREATE_PENALTY: u8 = 9;

/// `0x0D03` codes.
pub const DISBAND_FAILED: u8 = 0;
pub const DISBAND_MEMBERS: u8 = 2;
pub const DISBAND_REJECTED: u8 = 3;

/// `0x0D07` results.
pub const REQUEST_REFUSED: u8 = 0;
pub const REQUEST_SENT: u8 = 1;
pub const REQUEST_PENALTY: u8 = 2;

/// `0x0D0C` states besides a level (below 100) and a rank (200 + rank).
pub const STATE_DISBANDED: u8 = 101;
pub const STATE_LEFT: u8 = 102;
pub const STATE_KICKED: u8 = 103;
pub const STATE_ONLINE: u8 = 104;
pub const STATE_OFFLINE: u8 = 105;
pub const STATE_RANK: u8 = 200;

/// The `byState` of `0x0D14`.
pub const ADDED_OFFLINE: u8 = 0;
pub const ADDED_ONLINE: u8 = 1;

/// Units per list packet (`SendGuildUserList`, `SendGuildJoinList`, `CGuildMgr::SendGuildList`).
pub const MEMBERS_PER_PACKET: usize = 40;
pub const GUILDS_PER_PACKET: usize = 14;

/// A guild in the guild list (`GUILD_UNIT`, 120 bytes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuildUnit {
    pub id: u32,
    pub name: Vec<u8>,
    pub master: Vec<u8>,
    pub remark: Vec<u8>,
    /// 31 unranked.
    pub ranking: u8,
    pub points: u32,
}

/// A member in the member list (`GUILDUSER_UNIT`, 29 bytes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemberUnit {
    pub id: u32,
    pub rank: u8,
    pub level: u16,
    pub job: u8,
    pub name: Vec<u8>,
}

/// A join request (`GUILDJOINUSER_UNIT`, 28 bytes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestUnit {
    pub id: u32,
    pub level: u16,
    pub job: u8,
    pub name: Vec<u8>,
}

/// `text` in a zero-padded field of `N` bytes, its last byte always 0.
fn field<const N: usize>(text: &[u8]) -> [u8; N] {
    let mut f = [0u8; N];
    let len = text.len().min(N - 1);
    f[..len].copy_from_slice(&text[..len]);
    f
}

impl GuildUnit {
    fn write(&self, p: &mut Vec<u8>) {
        p.extend_from_slice(&self.id.to_le_bytes());
        p.extend_from_slice(&field::<GUILD_NAME_LEN>(&self.name));
        p.extend_from_slice(&name_field(&self.master));
        p.extend_from_slice(&field::<REMARK_LEN>(&self.remark));
        p.push(self.ranking);
        p.extend_from_slice(&self.points.to_le_bytes());
    }
}

impl MemberUnit {
    fn write(&self, p: &mut Vec<u8>) {
        p.extend_from_slice(&self.id.to_le_bytes());
        p.push(self.rank);
        p.extend_from_slice(&self.level.to_le_bytes());
        p.push(self.job);
        p.extend_from_slice(&name_field(&self.name));
    }
}

impl RequestUnit {
    fn write(&self, p: &mut Vec<u8>) {
        p.extend_from_slice(&self.id.to_le_bytes());
        p.extend_from_slice(&self.level.to_le_bytes());
        p.push(self.job);
        p.extend_from_slice(&name_field(&self.name));
    }
}

/// `u8 count` and the units, `per` to a packet; nothing for no units.
fn chunked<T>(opcode: u16, units: &[T], per: usize, write: impl Fn(&T, &mut Vec<u8>)) -> Vec<Vec<u8>> {
    units
        .chunks(per)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for u in chunk {
                write(u, &mut p);
            }
            encode(opcode, &p)
        })
        .collect()
}

fn by_id(opcode: u16, id: u32) -> Vec<u8> {
    encode(opcode, &id.to_le_bytes())
}

fn code_and_id(opcode: u16, code: u8, id: u32) -> Vec<u8> {
    let mut p = vec![code];
    p.extend_from_slice(&id.to_le_bytes());
    encode(opcode, &p)
}

/// `0x0D21`: `u8 code, u32 guild id, u8 rank, char name[25], char remark[65]` (98 bytes).
pub fn create_result(code: u8, guild_id: u32, rank: u8, name: &[u8], remark: &[u8]) -> Vec<u8> {
    let mut p = vec![code];
    p.extend_from_slice(&guild_id.to_le_bytes());
    p.push(rank);
    p.extend_from_slice(&field::<GUILD_NAME_LEN>(name));
    p.extend_from_slice(&field::<REMARK_LEN>(remark));
    encode(OP_CREATE, &p)
}

/// `0x0D01`: `u8 code`, an unused `u32` and `u8` (0 here), `char name[25]` (33 bytes).
pub fn create_failed(code: u8, name: &[u8]) -> Vec<u8> {
    let mut p = vec![code, 0, 0, 0, 0, 0];
    p.extend_from_slice(&field::<GUILD_NAME_LEN>(name));
    encode(OP_CREATE_FAILED, &p)
}

/// `0x0D22`: `u32` requester id, `char name[25], char remark[65]` (96 bytes).
pub fn create_agreement(requester: u32, name: &[u8], remark: &[u8]) -> Vec<u8> {
    let mut p = requester.to_le_bytes().to_vec();
    p.extend_from_slice(&field::<GUILD_NAME_LEN>(name));
    p.extend_from_slice(&field::<REMARK_LEN>(remark));
    encode(OP_CREATE_AGREE, &p)
}

/// `0x0D03`: `u8 code`.
pub fn disband_result(code: u8) -> Vec<u8> {
    encode(OP_DISBAND, &[code])
}

/// `0x0D04`: `u32` master id.
pub fn disband_agreement(master: u32) -> Vec<u8> {
    by_id(OP_DISBAND_AGREE, master)
}

/// `0x0D05`: `u32` inviter id.
pub fn invitation(inviter: u32) -> Vec<u8> {
    by_id(OP_INVITE, inviter)
}

/// `0x0D06`: `u8 ok, u32 id`.
pub fn invite_answer(ok: bool, id: u32) -> Vec<u8> {
    code_and_id(OP_INVITE_ANSWER, u8::from(ok), id)
}

/// `0x0D07`: `u8 result`.
pub fn request_result(result: u8) -> Vec<u8> {
    encode(OP_JOIN_REQUEST, &[result])
}

/// `0x0D08`: `u8 ok, u32 guild id, u8 rank, char name[25]` (33 bytes).
pub fn joined(ok: bool, guild_id: u32, rank: u8, name: &[u8]) -> Vec<u8> {
    let mut p = vec![u8::from(ok)];
    p.extend_from_slice(&guild_id.to_le_bytes());
    p.push(rank);
    p.extend_from_slice(&field::<GUILD_NAME_LEN>(name));
    encode(OP_JOINED, &p)
}

/// `0x0D09`: `u8 ok`.
pub fn leave_result(ok: bool) -> Vec<u8> {
    encode(OP_LEAVE, &[u8::from(ok)])
}

/// `0x0D0A`: `u8 ok, u32 id`.
pub fn kick_result(ok: bool, id: u32) -> Vec<u8> {
    code_and_id(OP_KICK, u8::from(ok), id)
}

/// `0x0D0C`: `u8 state, u32 id`.
pub fn member_state(state: u8, id: u32) -> Vec<u8> {
    code_and_id(OP_MEMBER_STATE, state, id)
}

/// The guild list: `0x0D0D`, `0x0D2F` with 14 guilds a packet, `0x0D0E`.
pub fn guild_list(guilds: &[GuildUnit]) -> Vec<Vec<u8>> {
    let mut packets = vec![encode(OP_LIST_START, &[])];
    packets.extend(chunked(OP_GUILDS, guilds, GUILDS_PER_PACKET, GuildUnit::write));
    packets.push(encode(OP_LIST_END, &[]));
    packets
}

/// `0x0D30`: one guild for the list (122 bytes).
pub fn guild_added(guild: &GuildUnit) -> Vec<u8> {
    let mut p = Vec::new();
    guild.write(&mut p);
    encode(OP_GUILD_ADD, &p)
}

/// `0x0D11`: `u32` guild id.
pub fn guild_removed(guild_id: u32) -> Vec<u8> {
    by_id(OP_GUILD_REMOVED, guild_id)
}

/// The member list: `0x0D12` the online members, `0x0D13` the offline ones, 40 a packet.
pub fn member_list(online: &[MemberUnit], offline: &[MemberUnit]) -> Vec<Vec<u8>> {
    let mut packets = chunked(OP_MEMBERS_ONLINE, online, MEMBERS_PER_PACKET, MemberUnit::write);
    packets.extend(chunked(OP_MEMBERS_OFFLINE, offline, MEMBERS_PER_PACKET, MemberUnit::write));
    packets
}

/// `0x0D14`: `u8 state` (1 online, 0 not), the member (32 bytes).
pub fn member_added(state: u8, member: &MemberUnit) -> Vec<u8> {
    let mut p = vec![state];
    member.write(&mut p);
    encode(OP_MEMBER_ADD, &p)
}

/// `0x0D16`: the join requests, 40 a packet.
pub fn request_list(requests: &[RequestUnit]) -> Vec<Vec<u8>> {
    chunked(OP_REQUESTS, requests, MEMBERS_PER_PACKET, RequestUnit::write)
}

/// `0x0D17`: one join request (30 bytes).
pub fn request_added(request: &RequestUnit) -> Vec<u8> {
    let mut p = Vec::new();
    request.write(&mut p);
    encode(OP_REQUEST_ADD, &p)
}

/// `0x0D18`: `u32 id`.
pub fn request_removed(id: u32) -> Vec<u8> {
    by_id(OP_REQUEST_REMOVE, id)
}

/// `0x0D20`: `u8 result, char remark[65]` (68 bytes).
pub fn remark(ok: bool, remark: &[u8]) -> Vec<u8> {
    let mut p = vec![u8::from(ok)];
    p.extend_from_slice(&field::<REMARK_LEN>(remark));
    encode(OP_REMARK, &p)
}

/// `0x0D23`: `u32 etin`.
pub fn etin(etin: u32) -> Vec<u8> {
    by_id(OP_ETIN, etin)
}

/// `0x0D26`: a ranking battle notice (type 100 started, 101 ends in 10 minutes, 102 in 1 minute, below 24 the start hour) (5 bytes).
pub fn ranking_notice(map: u16, kind: u8) -> Vec<u8> {
    let mut p = map.to_le_bytes().to_vec();
    p.push(kind);
    encode(OP_RANKING_NOTICE, &p)
}

/// `0x0D27`: battle points (14 bytes).
pub fn battle_points(points: u32, best_points: u32, best_guild: u32) -> Vec<u8> {
    let mut p = points.to_le_bytes().to_vec();
    p.extend_from_slice(&best_points.to_le_bytes());
    p.extend_from_slice(&best_guild.to_le_bytes());
    encode(OP_BATTLE_POINTS, &p)
}

/// `0x0D37`: a country's ranked guilds as `(guild, score, place)` in place order (3 + 9n bytes).
pub fn ranking(places: &[(u32, u32, u8)]) -> Vec<u8> {
    let mut p = vec![places.len() as u8];
    for &(guild, score, place) in places {
        p.extend_from_slice(&guild.to_le_bytes());
        p.extend_from_slice(&score.to_le_bytes());
        p.push(place);
    }
    encode(OP_RANKING, &p)
}

/// `0x0D24`: the used NPC slots as `(key, level, slot)`, in slot order. None at all when no slot is used (`CGuild::SendNpcLvList`).
pub fn npc_levels(slots: &[(u16, u8, u8)]) -> Option<Vec<u8>> {
    if slots.is_empty() {
        return None;
    }
    let mut p = vec![slots.len() as u8];
    for &(key, level, slot) in slots {
        p.extend_from_slice(&key.to_le_bytes());
        p.extend_from_slice(&[level, slot]);
    }
    Some(encode(OP_NPC_LEVELS, &p))
}

/// `0x0D29`: the bank's items in the order given (the caller sorts by slot), 50 a packet, none at all for an empty bank
/// (`CGuild::SendBankItemList`).
pub fn bank_list(items: &[ItemUnit]) -> Vec<Vec<u8>> {
    items
        .chunks(ITEMS_PER_PACKET)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for item in chunk {
                bank_unit(&mut p, item);
            }
            encode(OP_BANK_LIST, &p)
        })
        .collect()
}

fn bank_move(op: u16, slot: u8, item: &ItemUnit, by: u32) -> Vec<u8> {
    let mut p = Vec::with_capacity(37);
    bank_unit(&mut p, &ItemUnit { slot, ..item.clone() });
    p.extend_from_slice(&by.to_le_bytes());
    encode(op, &p)
}

/// `0x0D31` (39 bytes): the bank slot `slot` now holds `item`, put there by character `by`.
pub fn bank_item_in(slot: u8, item: &ItemUnit, by: u32) -> Vec<u8> {
    bank_move(OP_BANK_IN, slot, item, by)
}

/// `0x0D32` (39 bytes): the item that left the bank slot `slot`, taken by character `by`.
pub fn bank_item_out(slot: u8, item: &ItemUnit, by: u32) -> Vec<u8> {
    bank_move(OP_BANK_OUT, slot, item, by)
}

/// `0x0D25`: an upgrade's answer.
pub fn npc_upgrade(result: u8, key: u16, level: u8, slot: u8) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&key.to_le_bytes());
    p.extend_from_slice(&[level, slot]);
    encode(OP_NPC_UPGRADE, &p)
}

/// `0x0D28`: a house NPC refuses (1 the ranking is not high enough, `required` the ranking needed; 2 the NPC's level is locked,
/// `required` the level).
pub fn house_refused(result: u8, required: u8) -> Vec<u8> {
    encode(OP_HOUSE_REFUSED, &[result, required])
}

/// `0x0D34`: the purchase's answer with the player's money.
pub fn house_bought(result: u8, money: u32) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_HOUSE_BUY, &p)
}

/// `0x0D35`: the bag places emptied by an Etin return.
pub fn etin_returned(places: &[(u8, u8)]) -> Vec<u8> {
    let mut p = vec![places.len() as u8];
    for &(bag, slot) in places {
        p.extend_from_slice(&[bag, slot]);
    }
    encode(OP_ETIN_RETURN, &p)
}

/// `0x0D25` from the client: `u16 key, u8 level`.
pub fn parse_npc_upgrade(payload: &[u8]) -> Option<(u16, u8)> {
    Some((u16::from_le_bytes(payload.get(..2)?.try_into().ok()?), *payload.get(2)?))
}

/// A text field from the client, cut at its first zero; the original forces the field's last byte to 0.
fn text_field(payload: &[u8], len: usize) -> Option<Vec<u8>> {
    let f = payload.get(..len)?;
    Some(f[..len - 1].iter().copied().take_while(|&b| b != 0).collect())
}

/// `0x0D21` from the client: the name and the remark.
pub fn parse_create(payload: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    Some((text_field(payload, GUILD_NAME_LEN)?, text_field(payload.get(GUILD_NAME_LEN..)?, REMARK_LEN)?))
}

/// `0x0D20` from the client: the remark.
pub fn parse_remark(payload: &[u8]) -> Option<Vec<u8>> {
    text_field(payload, REMARK_LEN)
}

/// `0x0D22`, `0x0D04`, `0x0D06` from the client: `u8` (1 yes).
pub fn parse_yes(payload: &[u8]) -> Option<bool> {
    payload.first().map(|&b| b == 1)
}

/// `0x0D05`, `0x0D07`, `0x0D0A` from the client: `u32 id`.
pub fn parse_id(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.get(..4)?.try_into().ok()?))
}

/// `0x0D08` and `0x0D0C` from the client: `u8, u32 id`.
pub fn parse_flag_and_id(payload: &[u8]) -> Option<(u8, u32)> {
    Some((*payload.first()?, parse_id(payload.get(1..)?)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    fn member(id: u32) -> MemberUnit {
        MemberUnit { id, rank: 9, level: 30, job: 2, name: b"Abc".to_vec() }
    }

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&create_result(CREATED, 1, 1, b"G", b"R")), 98);
        assert_eq!(len(&create_failed(CREATE_REJECTED, b"G")), 33);
        assert_eq!(len(&create_agreement(1, b"G", b"R")), 96);
        assert_eq!(len(&disband_result(DISBAND_MEMBERS)), 3);
        assert_eq!(len(&disband_agreement(1)), 6);
        assert_eq!(len(&invitation(1)), 6);
        assert_eq!(len(&invite_answer(true, 1)), 7);
        assert_eq!(len(&request_result(REQUEST_SENT)), 3);
        assert_eq!(len(&joined(true, 1, 9, b"G")), 33);
        assert_eq!(len(&leave_result(true)), 3);
        assert_eq!(len(&kick_result(true, 1)), 7);
        assert_eq!(len(&member_state(STATE_ONLINE, 1)), 7);
        assert_eq!(len(&guild_added(&GuildUnit::default())), 122);
        assert_eq!(len(&guild_removed(1)), 6);
        assert_eq!(len(&member_added(ADDED_ONLINE, &member(1))), 32);
        assert_eq!(len(&request_added(&RequestUnit::default())), 30);
        assert_eq!(len(&request_removed(1)), 6);
        assert_eq!(len(&remark(true, b"R")), 68);
        assert_eq!(len(&etin(0)), 6);
        assert_eq!(len(&ranking_notice(50, 100)), 5);
        assert_eq!(len(&battle_points(1, 2, 3)), 14);
        assert_eq!(len(&ranking(&[])), 3);
        assert_eq!(len(&ranking(&[(1, 2, 1), (2, 3, 2)])), 21);
        let item = ItemUnit {
            bag: 100,
            slot: 3,
            item_type: 25,
            type_id: 3,
            quality: 0x0102,
            gems: [1, 2, 3, 4, 5, 6],
            count: 7,
            craft_name: b"Made".to_vec(),
        };
        assert_eq!(len(&bank_item_in(9, &item, 5)), 39);
        assert_eq!(len(&bank_item_out(9, &item, 5)), 39);
    }

    #[test]
    fn the_bank_list_is_cut_into_packets_of_fifty() {
        let item = ItemUnit {
            bag: 255,
            slot: 0,
            item_type: 25,
            type_id: 3,
            quality: 0x0102,
            gems: [1, 2, 3, 4, 5, 6],
            count: 7,
            craft_name: b"Made".to_vec(),
        };
        let lens = |p: Vec<Vec<u8>>| p.iter().map(|p| p.len() - 2).collect::<Vec<_>>();
        assert!(bank_list(&[]).is_empty(), "no packet for an empty bank");
        assert_eq!(lens(bank_list(std::slice::from_ref(&item))), [36]);
        assert_eq!(lens(bank_list(&vec![item.clone(); 50])), [1653]);
        assert_eq!(lens(bank_list(&vec![item.clone(); 51])), [1653, 36]);
        let packet = bank_list(std::slice::from_ref(&item)).remove(0);
        assert_eq!(opcode(&packet), OP_BANK_LIST);
        let p = payload(&packet);
        assert_eq!(p[0], 1);
        // the unit: slot, type, type id, quality, gems, count (offset 11), craft name
        assert_eq!(p[1..12], [0, 25, 3, 2, 1, 1, 2, 3, 4, 5, 6]);
        assert_eq!(p[12], 7);
        assert_eq!(&p[13..17], b"Made");
        let moved = bank_item_in(9, &item, 0x0A0B0C0D);
        let p = payload(&moved);
        assert_eq!(p[0], 9, "the bank slot replaces the item's own");
        assert_eq!(p[33..], [0x0D, 0x0C, 0x0B, 0x0A]);
        assert_eq!(opcode(&moved), OP_BANK_IN);
        assert_eq!(opcode(&bank_item_out(9, &item, 1)), OP_BANK_OUT);
    }

    #[test]
    fn lists_are_cut_into_packets() {
        let guilds = vec![GuildUnit::default(); 15];
        let list = guild_list(&guilds);
        assert_eq!(list.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [OP_LIST_START, OP_GUILDS, OP_GUILDS, OP_LIST_END]);
        assert_eq!(payload(&list[1])[0], 14);
        assert_eq!(list[1].len() - 2, 3 + 14 * 120);
        assert_eq!(payload(&list[2])[0], 1);
        assert_eq!(guild_list(&[]).len(), 2, "start and end only");

        let online: Vec<_> = (0..41).map(member).collect();
        let packets = member_list(&online, &[member(99)]);
        assert_eq!(packets.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [OP_MEMBERS_ONLINE, OP_MEMBERS_ONLINE, OP_MEMBERS_OFFLINE]);
        assert_eq!(packets[0].len() - 2, 3 + 40 * 29);
        assert!(member_list(&[], &[]).is_empty());
        assert_eq!(request_list(&[RequestUnit::default()])[0].len() - 2, 3 + 28);
    }

    #[test]
    fn units_are_packed_in_the_original_order() {
        let m = MemberUnit { id: 0x0102_0304, rank: 2, level: 0x0506, job: 3, name: b"Ab".to_vec() };
        let p = payload(&member_added(ADDED_ONLINE, &m)).to_vec();
        assert_eq!(&p[..8], &[1, 4, 3, 2, 1, 2, 6, 5]);
        assert_eq!(p[8], 3);
        assert_eq!(&p[9..12], b"Ab\0");

        let g = GuildUnit { id: 7, name: b"Gld".to_vec(), master: b"Mst".to_vec(), remark: b"Hi".to_vec(), ranking: 31, points: 5 };
        let p = payload(&guild_added(&g)).to_vec();
        assert_eq!(&p[..4], &[7, 0, 0, 0]);
        assert_eq!(&p[4..8], b"Gld\0");
        assert_eq!(&p[29..33], b"Mst\0");
        assert_eq!(&p[50..53], b"Hi\0");
        assert_eq!(p[115], 31);
        assert_eq!(&p[116..], &[5, 0, 0, 0]);

        let p = payload(&create_result(CREATED, 0x0A0B, 1, b"G", b"R")).to_vec();
        assert_eq!(&p[..6], &[1, 0x0B, 0x0A, 0, 0, 1]);
        assert_eq!(p[6], b'G');
        assert_eq!(p[31], b'R');
        let p = payload(&create_failed(CREATE_REJECTED, b"G")).to_vec();
        assert_eq!(p[0], 7);
        assert_eq!(p[6], b'G', "the name at offset 8 of the packet");
    }

    #[test]
    fn requests_parse() {
        let mut create = vec![b'n'; GUILD_NAME_LEN + REMARK_LEN];
        create[3] = 0;
        let (name, remark) = parse_create(&create).unwrap();
        assert_eq!(name, b"nnn");
        assert_eq!(remark.len(), REMARK_LEN - 1, "cut at the field's last byte");
        assert_eq!(parse_create(&create[..GUILD_NAME_LEN + 10]), None);
        assert_eq!(parse_remark(&[b'r'; REMARK_LEN]), Some(vec![b'r'; 64]));
        assert_eq!(parse_yes(&[1]), Some(true));
        assert_eq!(parse_yes(&[0]), Some(false));
        assert_eq!(parse_yes(&[2]), Some(false));
        assert_eq!(parse_id(&[7, 0, 0, 0]), Some(7));
        assert_eq!(parse_flag_and_id(&[1, 7, 0, 0, 0]), Some((1, 7)));
        assert_eq!(parse_flag_and_id(&[1, 7, 0]), None);
    }
}
