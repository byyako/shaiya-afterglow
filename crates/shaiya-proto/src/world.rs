//! Packets of entering the world: the burst the server sends after a character is selected, and the zone entry that
//! follows the client's `0x0201`. Layouts as read from `ps_game.exe` and its PDB (`docs/original/enter-world.md`).

use crate::frame::encode;

pub use crate::skill::{OP_CHAR_QUICK, OP_CHAR_SKILL};
pub const OP_CHAR_DATA: u16 = 0x0105;
pub const OP_CHAR_ITEM: u16 = 0x0106;
/// The warehouse's money and items (`TP_CHAR_BANK`), sent in the burst after the items.
pub const OP_CHAR_BANK: u16 = 0x0711;
pub const OP_SERVER_TIME: u16 = 0x0114;
/// Client: map loaded, place me in the zone. Server: a player entering the view (`TP_USER_ENTER`).
pub const OP_USER_ENTER: u16 = 0x0201;
pub const OP_KC_STATUS: u16 = 0x0211;
/// The faction's kill count (`docs/original/pvp-2.md` section 9).
pub const OP_FACTION_KILL_COUNT: u16 = 0x020F;
/// The free resets left, two display numbers (`docs/original/small-items.md` section 2).
pub const OP_RESET_FLAGS: u16 = 0x021B;
pub const OP_HONOR: u16 = 0x0230;
/// The player's money (`TP_USER_SETMONEY`): `u32`.
pub const OP_SET_MONEY: u16 = 0x0213;
pub const OP_WORLD_DAY: u16 = 0x0404;
pub const OP_SPEED: u16 = 0x051C;
/// Client: a step. Server: another player's step.
pub const OP_MOVE: u16 = 0x0501;
/// Sent with `0x020B` when a move is refused.
pub const OP_MOVE_REFUSED: u16 = 0x0502;
/// Sit (1) or stand (0); the server answers with the same opcode.
pub const OP_MOTION: u16 = 0x0506;
/// Puts an object at a position on a map (used to undo a refused move, and for teleports).
pub const OP_POSITION: u16 = 0x020B;
/// Client: use a gatekeeper's destination. Server: the result.
pub const OP_GATEKEEPER: u16 = 0x020C;
/// Client: move an item between bag slots (bag 0 is the equipment). Server: both slots as they are now.
pub const OP_ITEM_MOVE: u16 = 0x0204;
/// Server: a player's visible equipment slot changed (`TP_PC_EQUIPMENT`), to everyone in view.
pub const OP_EQUIPMENT: u16 = 0x0507;
/// Server: a player's maximum HP, SP or MP changed (`TP_SET_MAX`).
pub const OP_SET_MAX: u16 = 0x050B;
/// `TP_SET_MAX` types (`CUser::SendMaxHP`, `SendMaxSP`, `SendMaxMP`).
pub const MAX_HP: u8 = 0;
pub const MAX_SP: u8 = 1;
pub const MAX_MP: u8 = 2;
/// Client: spend stat points (`TP_USER_STATUSUP`). Server: the request echoed, or all zeros when refused.
pub const OP_STATUS_UP: u16 = 0x0208;
/// Client: use a portal of the map. Server: why it was refused (a successful use answers with `0x020B` only).
pub const OP_PORTAL: u16 = 0x020A;
/// Client: the yes to the ranking battle question (reason 5 of `0x020A`), the same `u8 portal` as `0x020A`.
pub const OP_PORTAL_CONFIRM: u16 = 0x0227;
/// Client: leave the world to the character screen in 10 s (only while sitting). Server: done, back to the character screen.
pub const OP_LOGOUT_TO_SELECT: u16 = 0x0107;
/// Client: quit the game in 10 s (only while sitting). Server: done, the client closes.
pub const OP_LOGOUT_QUIT: u16 = 0x010D;
/// Client: what is this player's status (`u32 id`). Server: `TP_USER_INFO`-like reply (12 bytes).
pub const OP_USER_INFO: u16 = 0x0301;
/// Client: what does this player look like (`u32 id`). Server: the player's looks (`CZone::SendUserShape`).
pub const OP_USER_SHAPE: u16 = 0x0303;
/// Client: show me this player's equipment (`u32 id`). Server: its equipped items (`TP_USER_ITEMS`, `docs/original/small-items.md`
/// section 7).
pub const OP_USER_ITEMS: u16 = 0x0307;
/// Motion byte of the position sync that precedes the `0x0301` and `0x0303` replies.
pub const MOTION_SYNC: u8 = 0xC4;
/// A player leaving the view (`u32 object id`).
pub const OP_USER_LEAVE: u16 = 0x0202;
/// The zone's weather.
pub const OP_WEATHER: u16 = 0x0451;
/// Client: a player's HP (`u32 id`). Server: `u32 id, u16 max HP, u16 HP`.
pub const OP_USER_HP: u16 = 0x0302;
/// Client: a monster's state (`u32 id`). Server: `u32 id, u32 HP, u8 attack speed, u8 move speed`, after its `0x0603`.
pub const OP_MOB_INFO: u16 = 0x0304;
/// Client: a monster's HP (`u32 id`). Server: `u32 id, u32 HP` (`TP_MOB_GETHP`).
pub const OP_MOB_HP: u16 = 0x0305;
/// A monster in the view (`TP_MOB_ENTER`).
pub const OP_MOB_ENTER: u16 = 0x0601;
/// A monster leaving the view (`u32 object id`).
pub const OP_MOB_LEAVE: u16 = 0x0602;
/// A monster's max and current HP, which this client sets as they are (`docs/original/client-extras.md`; the original never sends it).
pub const OP_MOB_MAX_HP: u16 = 0x0608;
/// A monster moving (`TP_MOB_MOVE`); also its position before an `0x0304` answer.
pub const OP_MOB_MOVE: u16 = 0x0603;
/// An NPC in the view (`TP_NPC_ENTER`).
pub const OP_NPC_ENTER: u16 = 0x0E01;
/// An NPC leaving the view (`u32 object id`).
pub const OP_NPC_LEAVE: u16 = 0x0E02;
/// An NPC walking (`TP_NPC_MOVE`).
pub const OP_NPC_MOVE: u16 = 0x0E03;
/// Client: the player clicked an NPC (`u32 object id`, `TP_NPC_USE`).
pub const OP_NPC_USE: u16 = 0x0E04;
/// A guard's blow on a player and on a monster, as the original sends them and the client reads them (`docs/original/guards.md`
/// section 6; the PDB's names `TP_NPC_ATTACKMOB` / `TP_NPC_ATTACKPC` are the other way round).
pub const OP_NPC_ATTACK_USER: u16 = 0x0E05;
pub const OP_NPC_ATTACK_MOB: u16 = 0x0E06;
/// Motions the server accepts (`CUser::ChangeMotion`).
pub const MOTION_STAND: u8 = 0;
pub const MOTION_SIT: u8 = 1;

/// Items per `0x0106` packet; more items go into further packets.
/// The guild's bank in `0x0204` (`CUser::IsGuildBank`: bag 255, slot below 240); the warehouse is 100.
pub const GUILD_BANK: u8 = 255;
pub const ITEMS_PER_PACKET: usize = 50;
/// Bytes of `TP_ITEM_UNIT`.
pub const ITEM_UNIT_LEN: usize = 34;
/// Bytes of the guild name field the client may receive in `0x0105` (`szGuildName[25]`).
pub const GUILD_NAME_LEN: usize = 25;
/// Bytes of a craft name (`szCraftName[21]`).
pub const CRAFT_NAME_LEN: usize = 21;

/// `TP_CHAR_DATA` contents (`CUser::SendCharacterData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharData {
    pub stat_point: u16,
    pub skill_point: u16,
    /// max hp, mp, sp
    pub max_health: [u16; 3],
    pub dir: u16,
    /// Experience total needed for the current level and for the next one.
    pub prev_exp: u32,
    pub next_exp: u32,
    pub exp: u32,
    pub money: u32,
    pub position: [f32; 3],
    /// The four kill counters (the original's K1 to K4).
    pub kills: [u32; 4],
    pub guild_name: Vec<u8>,
}

/// `0x0105`: fixed fields, then the guild name and its zero; the packet ends there (59 bytes without a guild).
pub fn char_data(d: &CharData) -> Vec<u8> {
    let mut p = Vec::with_capacity(64);
    for v in [d.stat_point, d.skill_point, d.max_health[0], d.max_health[1], d.max_health[2], d.dir] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    for v in [d.prev_exp, d.next_exp, d.exp, d.money] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    for v in d.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    for v in d.kills {
        p.extend_from_slice(&v.to_le_bytes());
    }
    let len = d.guild_name.len().min(GUILD_NAME_LEN - 1);
    p.extend_from_slice(&d.guild_name[..len]);
    p.push(0);
    encode(OP_CHAR_DATA, &p)
}

/// An item as `TP_ITEM_UNIT` carries it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemUnit {
    pub bag: u8,
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
    pub quality: u16,
    pub gems: [u8; 6],
    pub count: u8,
    pub craft_name: Vec<u8>,
}

/// `0x0106`: `u8 count`, then 34 bytes per item, at most 50 per packet. No packet at all for no items, as the original.
pub fn char_items(items: &[ItemUnit]) -> Vec<Vec<u8>> {
    items
        .chunks(ITEMS_PER_PACKET)
        .map(|chunk| {
            let mut p = Vec::with_capacity(1 + chunk.len() * ITEM_UNIT_LEN);
            p.push(chunk.len() as u8);
            for item in chunk {
                p.extend_from_slice(&[item.bag, item.slot, item.item_type, item.type_id]);
                p.extend_from_slice(&item.quality.to_le_bytes());
                p.extend_from_slice(&item.gems);
                p.push(item.count);
                let mut name = [0u8; CRAFT_NAME_LEN];
                let len = item.craft_name.len().min(CRAFT_NAME_LEN - 1);
                name[..len].copy_from_slice(&item.craft_name[..len]);
                p.extend_from_slice(&name);
            }
            encode(OP_CHAR_ITEM, &p)
        })
        .collect()
}

/// A warehouse unit (`TP_BANK_UNIT`, 33 bytes), the layout of the guild bank's as well: `u8 slot, u8 type, u8 type id, u16 quality,
/// u8 gems[6], u8 count, char craft name[21]`.
pub fn bank_unit(p: &mut Vec<u8>, item: &ItemUnit) {
    p.extend_from_slice(&[item.slot, item.item_type, item.type_id]);
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    p.push(item.count);
    let mut name = [0u8; CRAFT_NAME_LEN];
    let len = item.craft_name.len().min(CRAFT_NAME_LEN - 1);
    name[..len].copy_from_slice(&item.craft_name[..len]);
    p.extend_from_slice(&name);
}

/// `0x0711` (`TP_CHAR_BANK`): `u32 bank money, u8 count`, then per item 33 bytes (`TP_BANK_UNIT`): `u8 slot, u8 type, u8 type id,
/// u16 quality, u8 gems[6], u8 count, char craft name[21]`; at most 50 items per packet, none at all for an empty warehouse
/// (`CUser::SendCharacterBank`).
pub fn char_bank(money: u32, items: &[ItemUnit]) -> Vec<Vec<u8>> {
    items
        .chunks(ITEMS_PER_PACKET)
        .map(|chunk| {
            let mut p = Vec::with_capacity(5 + chunk.len() * (ITEM_UNIT_LEN - 1));
            p.extend_from_slice(&money.to_le_bytes());
            p.push(chunk.len() as u8);
            for item in chunk {
                bank_unit(&mut p, item);
            }
            encode(OP_CHAR_BANK, &p)
        })
        .collect()
}

/// `0x0404`: the world clock, in seconds.
pub fn world_day(seconds: u32) -> Vec<u8> {
    encode(OP_WORLD_DAY, &seconds.to_le_bytes())
}

/// `0x051C`: `u32 object id, u8 attack speed (0 to 9), u8 move speed (0 to 4)`; 0xFF means "cannot act".
pub fn speed(object_id: u32, attack_speed: u8, move_speed: u8) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&[attack_speed, move_speed]);
    encode(OP_SPEED, &p)
}

/// `0x020F` (7 bytes): `u8 faction, u32 kill count` to that faction's players (`CWorldThread::SendAllKillCount`,
/// `docs/original/pvp-2.md` section 9).
pub fn faction_kill_count(country: u8, count: u32) -> Vec<u8> {
    let mut p = vec![country];
    p.extend_from_slice(&count.to_le_bytes());
    encode(OP_FACTION_KILL_COUNT, &p)
}

/// `0x0211`: `u8 country, u32 kill count, u32 ms left of the top grade` (`CUser::SendKCStatus`, `docs/original/pvp-2.md`
/// section 9).
pub fn kc_status(country: u8, value: u32, remaining_ms: u32) -> Vec<u8> {
    let mut p = vec![country];
    p.extend_from_slice(&value.to_le_bytes());
    p.extend_from_slice(&remaining_ms.to_le_bytes());
    encode(OP_KC_STATUS, &p)
}

/// `0x0213`: `u32` the player's money.
pub fn set_money(money: u32) -> Vec<u8> {
    encode(OP_SET_MONEY, &money.to_le_bytes())
}

/// `0x0230`: `u16` honor.
pub fn honor(value: u16) -> Vec<u8> {
    encode(OP_HONOR, &value.to_le_bytes())
}

/// `0x0114` (`TP_CHARACTER_SERVERTIME`): the server's local time, packed as the original's `GetSystemTime` does.
pub fn server_time(packed: u32) -> Vec<u8> {
    encode(OP_SERVER_TIME, &packed.to_le_bytes())
}

/// The original's packed local time: `(year - 2000) << 26 | month << 22 | day << 17 | hour << 12 | minute << 6 | second`
/// (computed there as `(year - 16)` with overflow, which leaves the same low 6 bits for years 2000 to 2063).
pub fn pack_time(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> u32 {
    let year = u32::from(year).wrapping_sub(16) & 0x3f;
    (year << 26)
        | (u32::from(month) << 22)
        | (u32::from(day) << 17)
        | (u32::from(hour) << 12)
        | (u32::from(minute) << 6)
        | u32::from(second)
}

/// `0x021B`: the free stat and skill resets left, as the client shows them ("<v> chances left"; `docs/original/small-items.md` section 2).
pub fn reset_counts(stats_left: u8, skills_left: u8) -> Vec<u8> {
    encode(OP_RESET_FLAGS, &[stats_left, skills_left])
}

/// Equipment slots `0x0307` lists (bag 0 slots 0 to 12).
pub const USER_ITEMS_MAX: usize = 13;

/// `0x0307` (`TP_USER_ITEMS`): `u8 n`, then 32 bytes per item (`TP_GETINFO_ITEMUNIT`): `u8 slot, u8 type, u8 type id, u16 quality,
/// u8 gems[6], char craft name[21]`; at most 13 (`docs/original/small-items.md` section 7).
pub fn user_items(items: &[ItemUnit]) -> Vec<u8> {
    let items = &items[..items.len().min(USER_ITEMS_MAX)];
    let mut p = Vec::with_capacity(1 + items.len() * 32);
    p.push(items.len() as u8);
    for item in items {
        p.extend_from_slice(&[item.slot, item.item_type, item.type_id]);
        p.extend_from_slice(&item.quality.to_le_bytes());
        p.extend_from_slice(&item.gems);
        let mut name = [0u8; CRAFT_NAME_LEN];
        let len = item.craft_name.len().min(CRAFT_NAME_LEN - 1);
        name[..len].copy_from_slice(&item.craft_name[..len]);
        p.extend_from_slice(&name);
    }
    encode(OP_USER_ITEMS, &p)
}

/// `TP_USER_ENTER` contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserEnter {
    pub char_id: u32,
    /// 1 for players, 2 for staff (permission 10 or lower).
    pub kind: u8,
    pub dir: u16,
    pub position: [f32; 3],
    pub guild_id: u32,
    /// The character id of a passenger when riding, else 0.
    pub vehicle: u32,
}

/// `0x0201` from the server, 29 bytes with the opcode.
pub fn user_enter(u: &UserEnter) -> Vec<u8> {
    let mut p = u.char_id.to_le_bytes().to_vec();
    p.push(u.kind);
    p.extend_from_slice(&u.dir.to_le_bytes());
    for v in u.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p.extend_from_slice(&u.guild_id.to_le_bytes());
    p.extend_from_slice(&u.vehicle.to_le_bytes());
    encode(OP_USER_ENTER, &p)
}

/// `0x0501` from the client: `u8 motion, u16 dir, f32 x, y, z` (15 payload bytes, required exactly; a coordinate that is not a finite
/// number is refused, `docs/server/audit.md` R21).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub motion: u8,
    pub dir: u16,
    pub position: [f32; 3],
}

pub fn parse_move(payload: &[u8]) -> Option<Move> {
    if payload.len() != 15 {
        return None;
    }
    let f = |at: usize| f32::from_le_bytes(payload[at..at + 4].try_into().unwrap());
    let position = [f(3), f(7), f(11)];
    position.iter().all(|v| v.is_finite()).then(|| Move { motion: payload[0], dir: u16::from_le_bytes([payload[1], payload[2]]), position })
}

pub fn move_request(m: &Move) -> Vec<u8> {
    let mut p = vec![m.motion];
    p.extend_from_slice(&m.dir.to_le_bytes());
    for v in m.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_MOVE, &p)
}

/// `0x0501` from the server: a player's step, `u32 object id, u8 motion, u16 dir, f32 x, y, z` (21 bytes with the opcode).
pub fn user_move(object_id: u32, m: &Move) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(m.motion);
    p.extend_from_slice(&m.dir.to_le_bytes());
    for v in m.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_MOVE, &p)
}

/// `0x0301` reply: `u32 id, u16 max HP, u16 HP, u8 attack speed, u8 move speed`.
pub fn user_info(object_id: u32, max_hp: u16, hp: u16, attack_speed: u8, move_speed: u8) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&max_hp.to_le_bytes());
    p.extend_from_slice(&hp.to_le_bytes());
    p.extend_from_slice(&[attack_speed, move_speed]);
    encode(OP_USER_INFO, &p)
}

/// What `0x0303` tells about a player (`CZone::SendUserShape`, ordinary players).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserShape {
    pub id: u32,
    /// 1 in one state of the player (`m_nStatus == 1`, not identified); 0 otherwise.
    pub status_flag: u8,
    pub motion: u8,
    pub country: u8,
    pub family: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    pub party_type: u8,
    pub grow: u8,
    pub kills: u32,
    /// Bag 0 slots 0 to 7: type, type id, enchant level.
    pub equipment: [[u8; 3]; 8],
    pub name: Vec<u8>,
    /// The cloak's gems; sent only when slot 7 holds an item.
    pub cloak: Option<[u8; 6]>,
    pub guild_name: Vec<u8>,
}

/// `0x0303` reply: `u32 id`, 11 bytes of state and looks, `u32 kills`, 8 x 3 equipment bytes, `char name[21]`, the cloak's
/// 6 bytes if one is worn, then the guild name and its zero.
pub fn user_shape(s: &UserShape) -> Vec<u8> {
    let mut p = s.id.to_le_bytes().to_vec();
    p.extend_from_slice(&[s.status_flag, s.motion, s.country, s.family, s.hair, s.face, s.size, s.job, s.sex, s.party_type, s.grow]);
    p.extend_from_slice(&s.kills.to_le_bytes());
    for slot in &s.equipment {
        p.extend_from_slice(slot);
    }
    let mut name = [0u8; 21];
    let len = s.name.len().min(20);
    name[..len].copy_from_slice(&s.name[..len]);
    p.extend_from_slice(&name);
    if let Some(cloak) = s.cloak {
        p.extend_from_slice(&cloak);
    }
    let len = s.guild_name.len().min(GUILD_NAME_LEN - 1);
    p.extend_from_slice(&s.guild_name[..len]);
    p.push(0);
    encode(OP_USER_SHAPE, &p)
}

/// The original's enchant level of an item: the last two characters of a 20-character craft name, when they are digits.
pub fn enchant_level(craft_name: &[u8]) -> u8 {
    match craft_name {
        [.., a, b] if craft_name.len() == 20 && a.is_ascii_digit() && b.is_ascii_digit() => (a - b'0') * 10 + (b - b'0'),
        _ => 0,
    }
}

/// `0x0202`: the player left the view.
pub fn user_leave(object_id: u32) -> Vec<u8> {
    encode(OP_USER_LEAVE, &object_id.to_le_bytes())
}

/// `0x0451`: `u8 0, u8 weather type, u8 power`; all zero for no weather.
pub fn weather(kind: u8, power: u8) -> Vec<u8> {
    encode(OP_WEATHER, &[0, kind, power])
}

/// An NPC as `0x0E01` describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NpcEnter {
    pub object_id: u32,
    /// 1 to 13 (merchant, gatekeeper, ...; `docs/original/maps-npcs.md` section 3).
    pub kind: u8,
    pub type_id: u16,
    pub position: [f32; 3],
    /// Degrees.
    pub dir: u16,
}

/// `0x0E01` (`CZone::EnterNpc`): `u32 object id, u8 type, u16 type id, f32 x, y, z, u16 direction`.
pub fn npc_enter(n: &NpcEnter) -> Vec<u8> {
    let mut p = n.object_id.to_le_bytes().to_vec();
    p.push(n.kind);
    p.extend_from_slice(&n.type_id.to_le_bytes());
    for v in n.position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p.extend_from_slice(&n.dir.to_le_bytes());
    encode(OP_NPC_ENTER, &p)
}

/// `0x0601`: `u32 object id, u8 byNew` (1 when it has just appeared), `u16 monster id, f32 x, f32 z`.
pub fn mob_enter(object_id: u32, new: bool, mob_id: u16, x: f32, z: f32) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(u8::from(new));
    p.extend_from_slice(&mob_id.to_le_bytes());
    p.extend_from_slice(&x.to_le_bytes());
    p.extend_from_slice(&z.to_le_bytes());
    encode(OP_MOB_ENTER, &p)
}

/// `0x0608`: `u32 object id, u32 max HP, u32 HP`; after the monster's `0x0601`.
pub fn mob_max_hp(object_id: u32, max_hp: u32, hp: u32) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&max_hp.to_le_bytes());
    p.extend_from_slice(&hp.to_le_bytes());
    encode(OP_MOB_MAX_HP, &p)
}

/// `0x0602`: the monster left the view.
pub fn mob_leave(object_id: u32) -> Vec<u8> {
    encode(OP_MOB_LEAVE, &object_id.to_le_bytes())
}

/// `0x0603`: `u32 object id, u8 motion, f32 x, f32 z`.
pub fn mob_move(object_id: u32, motion: u8, x: f32, z: f32) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(motion);
    p.extend_from_slice(&x.to_le_bytes());
    p.extend_from_slice(&z.to_le_bytes());
    encode(OP_MOB_MOVE, &p)
}

/// `0x0304`: `u32 object id, u32 HP, u8 attack speed, u8 move speed`.
pub fn mob_info(object_id: u32, hp: u32, attack_speed: u8, move_speed: u8) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&hp.to_le_bytes());
    p.extend_from_slice(&[attack_speed, move_speed]);
    encode(OP_MOB_INFO, &p)
}

/// `0x0305`: `u32 object id, u32 HP`.
pub fn mob_hp(object_id: u32, hp: u32) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&hp.to_le_bytes());
    encode(OP_MOB_HP, &p)
}

/// `0x0302`: `u32 object id, u16 max HP, u16 HP`.
pub fn user_hp(object_id: u32, max_hp: u16, hp: u16) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&max_hp.to_le_bytes());
    p.extend_from_slice(&hp.to_le_bytes());
    encode(OP_USER_HP, &p)
}

/// `0x0E02`: the NPC left the view.
pub fn npc_leave(object_id: u32) -> Vec<u8> {
    encode(OP_NPC_LEAVE, &object_id.to_le_bytes())
}

/// `0x0E03`: `u32 object id, u8 motion` (0 walk, 1 run), `f32 x, y, z`.
pub fn npc_move(object_id: u32, motion: u8, position: [f32; 3]) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(motion);
    for v in position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_NPC_MOVE, &p)
}

/// `0x0E05` (a player) or `0x0E06` (a monster): `u8 1, u32 guard id, u32 target id, u16 damage`.
pub fn npc_attack(on_player: bool, npc: u32, target: u32, damage: u16) -> Vec<u8> {
    let mut p = vec![1];
    p.extend_from_slice(&npc.to_le_bytes());
    p.extend_from_slice(&target.to_le_bytes());
    p.extend_from_slice(&damage.to_le_bytes());
    encode(if on_player { OP_NPC_ATTACK_USER } else { OP_NPC_ATTACK_MOB }, &p)
}

/// `0x0E04` from the client: `u32 object id`.
pub fn parse_npc_use(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

/// `0x0506` from the client: `u8 motion`.
pub fn parse_motion(payload: &[u8]) -> Option<u8> {
    match payload {
        [motion] => Some(*motion),
        _ => None,
    }
}

/// `0x0506` from the server: `u32 object id, u8 motion` (the motion the client asked for).
pub fn motion(object_id: u32, motion: u8) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(motion);
    encode(OP_MOTION, &p)
}

/// `0x020B`: `u32 object id, u16 map, f32 x, y, z`.
pub fn position(object_id: u32, map: u16, position: [f32; 3]) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.extend_from_slice(&map.to_le_bytes());
    for v in position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_POSITION, &p)
}

/// `0x0204` from the client: `u8 from bag, u8 from slot, u8 to bag, u8 to slot`.
pub fn parse_item_move(payload: &[u8]) -> Option<((u8, u8), (u8, u8))> {
    match payload {
        [from_bag, from_slot, to_bag, to_slot] => Some(((*from_bag, *from_slot), (*to_bag, *to_slot))),
        _ => None,
    }
}

pub fn item_move_request(from: (u8, u8), to: (u8, u8)) -> Vec<u8> {
    encode(OP_ITEM_MOVE, &[from.0, from.1, to.0, to.1])
}

/// One place of `TP_ITEM_BAGTOBAG`: `u8 bag, u8 slot, u8 type, u8 type id, u8 count, u16 quality, u8 gems[6], char craft name[21]`.
fn item_place(p: &mut Vec<u8>, (bag, slot): (u8, u8), item: Option<&ItemUnit>) {
    p.extend_from_slice(&[bag, slot]);
    let Some(item) = item else {
        p.extend_from_slice(&[0; 32]);
        return;
    };
    p.extend_from_slice(&[item.item_type, item.type_id, item.count]);
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    let mut name = [0u8; CRAFT_NAME_LEN];
    let len = item.craft_name.len().min(CRAFT_NAME_LEN - 1);
    name[..len].copy_from_slice(&item.craft_name[..len]);
    p.extend_from_slice(&name);
}

/// `0x0204` from the server (`TP_ITEM_BAGTOBAG`, 70 bytes): the source place, then the target place, as they are now.
pub fn item_move_result(from: (u8, u8), from_item: Option<&ItemUnit>, to: (u8, u8), to_item: Option<&ItemUnit>) -> Vec<u8> {
    let mut p = Vec::with_capacity(68);
    item_place(&mut p, from, from_item);
    item_place(&mut p, to, to_item);
    encode(OP_ITEM_MOVE, &p)
}

/// `0x0204` for an item taken out of the warehouse (`TP_ITEM_BANKTOBAG`, 74 bytes): the warehouse place, the bag place, then the
/// player's money after the fee.
pub fn bank_to_bag_result(from: (u8, u8), from_item: Option<&ItemUnit>, to: (u8, u8), to_item: Option<&ItemUnit>, money: u32) -> Vec<u8> {
    let mut p = Vec::with_capacity(72);
    item_place(&mut p, from, from_item);
    item_place(&mut p, to, to_item);
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_ITEM_MOVE, &p)
}

/// `0x0507`: `u32 char id, u8 slot, u8 type, u8 type id, u8 enchant level`, and the cloak's 6 badge bytes (its gems) when slot 7
/// holds one (`CUser::SendEquipment`).
pub fn equipment(char_id: u32, slot: u8, item: Option<&ItemUnit>) -> Vec<u8> {
    let mut p = char_id.to_le_bytes().to_vec();
    p.push(slot);
    match item {
        None => p.extend_from_slice(&[0, 0, 0]),
        Some(item) => {
            p.extend_from_slice(&[item.item_type, item.type_id, enchant_level(&item.craft_name)]);
            if slot == 7 {
                p.extend_from_slice(&item.gems);
            }
        }
    }
    encode(OP_EQUIPMENT, &p)
}

/// `0x050B`: `u32 object id, u8 type` (0 HP, 1 SP, 2 MP), `u16 max`.
pub fn set_max(object_id: u32, kind: u8, max: u16) -> Vec<u8> {
    let mut p = object_id.to_le_bytes().to_vec();
    p.push(kind);
    p.extend_from_slice(&max.to_le_bytes());
    encode(OP_SET_MAX, &p)
}

/// `0x020A` from the client: `u8 portal`, the portal's position in the map file's list. game.exe sends 3 bytes; the PDB
/// struct (`TPC_USER_USEPORTAL`) has a second byte for the reply, so 4 are accepted too.
pub fn parse_portal(payload: &[u8]) -> Option<u8> {
    match payload {
        [portal] | [portal, _] => Some(*portal),
        _ => None,
    }
}

pub fn portal_request(portal: u8) -> Vec<u8> {
    encode(OP_PORTAL, &[portal])
}

/// `0x020A` from the server: `u8 portal, u8 reason` (0 needs a guild, 4 needs a party, ...).
pub fn portal_refused(portal: u8, reason: u8) -> Vec<u8> {
    encode(OP_PORTAL, &[portal, reason])
}

/// `0x0208` from the client: `u16` points for STR, DEX, REC, INT, WIS, LUC (12 bytes).
pub fn parse_status_up(payload: &[u8]) -> Option<[u16; 6]> {
    let bytes: &[u8; 12] = payload.try_into().ok()?;
    Some(std::array::from_fn(|i| u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]])))
}

/// `0x0208` from the server: the points added, in the request's order (all 0: refused).
pub fn status_up(points: [u16; 6]) -> Vec<u8> {
    encode(OP_STATUS_UP, &points.iter().flat_map(|p| p.to_le_bytes()).collect::<Vec<_>>())
}

/// `0x020C` from the client: `u32 npc object id, u8 destination` (0 to 2).
pub fn parse_gatekeeper(payload: &[u8]) -> Option<(u32, u8)> {
    match payload {
        [a, b, c, d, destination] => Some((u32::from_le_bytes([*a, *b, *c, *d]), *destination)),
        _ => None,
    }
}

pub fn gatekeeper_request(npc_id: u32, destination: u8) -> Vec<u8> {
    let mut p = npc_id.to_le_bytes().to_vec();
    p.push(destination);
    encode(OP_GATEKEEPER, &p)
}

/// `0x020C` from the server: `u8 result` (1 moved, 0 failed), `u32 money`.
pub fn gatekeeper_result(moved: bool, money: u32) -> Vec<u8> {
    let mut p = vec![u8::from(moved)];
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_GATEKEEPER, &p)
}

/// `0x0502` as the original sends it after refusing a move: `u8 5, u32 object id`, then 10 zero bytes.
pub fn move_refused(object_id: u32) -> Vec<u8> {
    let mut p = vec![5];
    p.extend_from_slice(&object_id.to_le_bytes());
    p.extend_from_slice(&[0; 10]);
    encode(OP_MOVE_REFUSED, &p)
}

/// The end of a logout: `0x0107` (to the character screen) or `0x010D` (quit), no payload.
pub fn logout_done(opcode: u16) -> Vec<u8> {
    encode(opcode, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn char_data_has_the_original_offsets() {
        let d = CharData {
            stat_point: 1,
            skill_point: 2,
            max_health: [3, 4, 5],
            dir: 6,
            prev_exp: 7,
            next_exp: 8,
            exp: 9,
            money: 10,
            position: [1.5, 2.5, 3.5],
            kills: [11, 12, 13, 14],
            guild_name: Vec::new(),
        };
        let packet = char_data(&d);
        assert_eq!(packet.len(), 2 + 59, "59 bytes without a guild name");
        // Offsets from the start of the struct (the opcode), docs/original/enter-world.md section 3.1.
        let s = &packet[2..];
        assert_eq!(s[2..4], [1, 0]);
        assert_eq!(s[10..12], [5, 0], "max SP");
        assert_eq!(s[12..14], [6, 0], "dir");
        assert_eq!(s[14..18], 7u32.to_le_bytes(), "prev exp");
        assert_eq!(s[26..30], 10u32.to_le_bytes(), "money");
        assert_eq!(s[30..34], 1.5f32.to_le_bytes(), "x");
        assert_eq!(s[42..46], 11u32.to_le_bytes(), "first kill counter");
        assert_eq!(s[54..58], 14u32.to_le_bytes());
        assert_eq!(s[58], 0, "empty guild name");

        let guild = char_data(&CharData { guild_name: b"Knights".to_vec(), ..d });
        assert_eq!(guild.len(), 2 + 59 + 7);
        assert_eq!(&guild[2 + 58..], b"Knights\0");
    }

    #[test]
    fn mob_max_hp_is_14_bytes() {
        let packet = mob_max_hp(7, 1_115_438, 900);
        assert_eq!(packet.len(), 2 + 14, "the length, then the client's 14 bytes");
        assert_eq!(packet[2..4], 0x0608u16.to_le_bytes());
        let p = payload(&packet);
        assert_eq!(p[..4], 7u32.to_le_bytes());
        assert_eq!(p[4..8], 1_115_438u32.to_le_bytes());
        assert_eq!(p[8..], 900u32.to_le_bytes());
    }

    #[test]
    fn items_are_34_bytes_and_split_at_50() {
        let item = ItemUnit {
            bag: 0,
            slot: 5,
            item_type: 1,
            type_id: 1,
            quality: 0x0102,
            gems: [1, 2, 3, 4, 5, 6],
            count: 1,
            craft_name: b"x".to_vec(),
        };
        let packets = char_items(std::slice::from_ref(&item));
        assert_eq!(packets.len(), 1);
        let p = payload(&packets[0]);
        assert_eq!(p.len(), 1 + 34);
        assert_eq!(p[..7], [1, 0, 5, 1, 1, 2, 1], "count, bag, slot, type, type id, quality");
        assert_eq!(p[7..13], [1, 2, 3, 4, 5, 6]);
        assert_eq!(p[13], 1, "item count");
        assert_eq!(p[14..16], [b'x', 0]);

        let many = char_items(&vec![item; 120]);
        assert_eq!(many.iter().map(|p| payload(p)[0]).collect::<Vec<_>>(), [50, 50, 20]);
        assert!(char_items(&[]).is_empty());
    }

    #[test]
    fn small_packets_have_the_original_sizes() {
        assert_eq!(world_day(5).len(), 2 + 6);
        assert_eq!(speed(1, 0, 2).len(), 2 + 8);
        assert_eq!(kc_status(0, 0, 0).len(), 2 + 11);
        assert_eq!(faction_kill_count(1, 300), [9, 0, 0x0F, 0x02, 1, 44, 1, 0, 0]);
        assert_eq!(honor(0).len(), 2 + 4);
        assert_eq!(server_time(0).len(), 2 + 6);
        assert_eq!(payload(&reset_counts(0, 1)), [0x00, 0x01]);
        assert_eq!(payload(&reset_counts(0, 0)), [0x00, 0x00]);
        let enter = user_enter(&UserEnter { char_id: 9, kind: 1, dir: 3, position: [1.0, 2.0, 3.0], guild_id: 0, vehicle: 0 });
        assert_eq!(enter.len(), 2 + 29);
        assert_eq!(payload(&enter)[..7], [9, 0, 0, 0, 1, 3, 0]);
    }

    #[test]
    fn user_items_are_32_bytes_each() {
        assert_eq!(user_items(&[]).len() - 2, 3, "an empty list");
        let unit = |slot| ItemUnit {
            bag: 0,
            slot,
            item_type: 16,
            type_id: 7,
            quality: 300,
            gems: [1, 2, 3, 4, 5, 6],
            count: 1,
            craft_name: b"0102030405".to_vec(),
        };
        let one = user_items(&[unit(5)]);
        assert_eq!(one.len(), 2 + 3 + 32);
        assert_eq!(payload(&one)[..12], [1, 5, 16, 7, 0x2C, 0x01, 1, 2, 3, 4, 5, 6]);
        assert_eq!(&payload(&one)[12..22], b"0102030405");
        assert!(payload(&one)[22..].iter().all(|&b| b == 0), "craft name zero-padded to 21");
        let all: Vec<ItemUnit> = (0..15).map(unit).collect();
        assert_eq!(user_items(&all[..13]).len(), 2 + 419);
        assert_eq!(user_items(&all), user_items(&all[..13]), "at most 13");
    }

    #[test]
    fn world_packets_have_the_original_sizes() {
        let m = Move { motion: 0x81, dir: 300, position: [1.0, 2.0, 3.0] };
        let request = move_request(&m);
        assert_eq!(request.len(), 2 + 17);
        assert_eq!(parse_move(payload(&request)), Some(m));
        assert_eq!(parse_move(&payload(&request)[..14]), None);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for axis in 0..3 {
                let mut position = [1.0, 2.0, 3.0];
                position[axis] = bad;
                assert_eq!(parse_move(payload(&move_request(&Move { position, ..m }))), None, "{bad} in axis {axis}");
            }
        }
        assert_eq!(parse_motion(&[1]), Some(1));
        assert_eq!(parse_motion(&[1, 2]), None);
        assert_eq!(motion(7, 1).len(), 2 + 7);
        assert_eq!(position(7, 1, [1.0, 2.0, 3.0]).len(), 2 + 20);
        let refused = move_refused(7);
        assert_eq!(refused.len(), 2 + 17);
        assert_eq!(payload(&refused)[..5], [5, 7, 0, 0, 0]);
        assert_eq!(logout_done(OP_LOGOUT_QUIT).len(), 2 + 2);
        let moved = user_move(7, &m);
        assert_eq!(moved.len(), 2 + 21);
        assert_eq!(payload(&moved)[..7], [7, 0, 0, 0, 0x81, 44, 1]);
        assert_eq!(user_leave(7).len(), 2 + 6);
        assert_eq!(weather(0, 0).len(), 2 + 5);
        assert_eq!(user_info(7, 300, 250, 0, 2).len(), 2 + 12);
    }

    #[test]
    fn npc_packets_have_the_original_sizes() {
        let npc = npc_enter(&NpcEnter { object_id: 7, kind: 1, type_id: 0x0102, position: [1.0, 2.0, 3.0], dir: 270 });
        // 23 bytes with the opcode: TP_NPC_ENTER's size in the PDB, and what game.exe reads.
        assert_eq!(npc.len(), 2 + 23);
        let p = payload(&npc);
        assert_eq!(p[..7], [7, 0, 0, 0, 1, 2, 1]);
        assert_eq!(p[7..11], 1.0f32.to_le_bytes());
        assert_eq!(p[19..], [14, 1]);
        assert_eq!(npc_leave(7).len(), 2 + 6);
        // TP_NPC_MOVE 19, TP_NPC_ATTACKMOB / TP_NPC_ATTACKPC 13.
        let moved = npc_move(7, 1, [1.0, 2.0, 3.0]);
        assert_eq!(moved.len(), 2 + 19);
        assert_eq!(payload(&moved)[..5], [7, 0, 0, 0, 1]);
        assert_eq!(payload(&moved)[13..], 3.0f32.to_le_bytes());
        let blow = npc_attack(true, 7, 9, 300);
        assert_eq!(blow.len(), 2 + 13);
        assert_eq!(blow[2..4], OP_NPC_ATTACK_USER.to_le_bytes());
        assert_eq!(payload(&blow), [1, 7, 0, 0, 0, 9, 0, 0, 0, 44, 1]);
        assert_eq!(npc_attack(false, 7, 9, 300)[2..4], OP_NPC_ATTACK_MOB.to_le_bytes());
        assert_eq!(parse_npc_use(&[7, 0, 0, 0]), Some(7));
        assert_eq!(parse_npc_use(&[7, 0, 0]), None);
    }

    #[test]
    fn mob_packets_have_the_original_sizes() {
        // Sizes with the opcode as game.exe reads them (docs/client/client-packets.generated.md).
        let enter = mob_enter(7, true, 0x0102, 1.0, 2.0);
        assert_eq!(enter.len(), 2 + 17);
        assert_eq!(payload(&enter)[..7], [7, 0, 0, 0, 1, 2, 1]);
        assert_eq!(payload(&enter)[7..11], 1.0f32.to_le_bytes());
        assert_eq!(mob_leave(7).len(), 2 + 6);
        assert_eq!(mob_move(7, 1, 1.0, 2.0).len(), 2 + 15);
        assert_eq!(payload(&mob_info(7, 300, 3, 2)), [7, 0, 0, 0, 44, 1, 0, 0, 3, 2]);
        assert_eq!(mob_hp(7, 300).len(), 2 + 10);
        assert_eq!(payload(&user_hp(7, 300, 250)), [7, 0, 0, 0, 44, 1, 250, 0]);
    }

    #[test]
    fn item_packets_have_the_original_sizes() {
        // As captured from the US client: bag 1 slot 0 to bag 0 slot 5 (equip a weapon).
        assert_eq!(parse_item_move(&[1, 0, 0, 5]), Some(((1, 0), (0, 5))));
        assert_eq!(item_move_request((1, 0), (0, 5)).len(), 2 + 6);
        let sword = ItemUnit {
            bag: 0,
            slot: 5,
            item_type: 5,
            type_id: 1,
            quality: 1200,
            gems: [1, 2, 3, 4, 5, 6],
            count: 1,
            craft_name: b"11111111111111111107".to_vec(),
        };
        let result = item_move_result((1, 0), None, (0, 5), Some(&sword));
        // 70 bytes with the opcode: TP_ITEM_BAGTOBAG in the PDB and what game.exe reads.
        assert_eq!(result.len(), 2 + 70);
        let p = payload(&result);
        assert_eq!(p[..34], [[1, 0].as_slice(), &[0; 32]].concat()[..], "an empty source");
        assert_eq!(p[34..41], [0, 5, 5, 1, 1, 0xB0, 0x04]);
        assert_eq!(p[41..47], [1, 2, 3, 4, 5, 6]);
        assert_eq!(&p[47..67], b"11111111111111111107");
        assert_eq!(p[67], 0);
        let equip = equipment(9, 5, Some(&sword));
        assert_eq!(equip.len(), 2 + 10);
        assert_eq!(payload(&equip), [9, 0, 0, 0, 5, 5, 1, 7]);
        assert_eq!(equipment(9, 7, Some(&ItemUnit { slot: 7, item_type: 24, ..sword.clone() })).len(), 2 + 16, "a cloak");
        assert_eq!(equipment(9, 7, None).len(), 2 + 10);
        let max = set_max(9, MAX_SP, 300);
        assert_eq!(max.len(), 2 + 9, "TP_SET_MAX");
        assert_eq!(payload(&max), [9, 0, 0, 0, 1, 44, 1]);
    }

    #[test]
    fn warehouse_packets_have_the_original_sizes() {
        let potion =
            ItemUnit { bag: 100, slot: 130, item_type: 25, type_id: 1, quality: 7, gems: [0; 6], count: 9, craft_name: Vec::new() };
        // TP_CHAR_BANK: 7 bytes and 33 per item, the count after the gems.
        let bank = char_bank(0, std::slice::from_ref(&potion));
        assert_eq!(bank.len(), 1);
        assert_eq!(bank[0].len(), 2 + 7 + 33);
        assert_eq!(payload(&bank[0])[..16], [0, 0, 0, 0, 1, 130, 25, 1, 7, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(payload(&bank[0])[16], 9);
        let many = char_bank(0, &vec![potion.clone(); 51]);
        assert_eq!(many.iter().map(Vec::len).collect::<Vec<_>>(), [2 + 1657, 2 + 7 + 33], "a full TP_CHAR_BANK holds 50");
        assert!(char_bank(0, &[]).is_empty(), "nothing for an empty warehouse");
        // TP_ITEM_BANKTOBAG: 74 bytes, the money last.
        let out = bank_to_bag_result((100, 130), None, (1, 3), Some(&ItemUnit { bag: 1, slot: 3, ..potion }), 1234);
        assert_eq!(out.len(), 2 + 74);
        assert_eq!(payload(&out)[..2], [100, 130]);
        assert_eq!(payload(&out)[34..39], [1, 3, 25, 1, 9]);
        assert_eq!(payload(&out)[68..], 1234u32.to_le_bytes());
    }

    #[test]
    fn portal_packets_have_the_original_sizes() {
        // As captured from the US client: portal 5.
        assert_eq!(parse_portal(&[5]), Some(5));
        assert_eq!(parse_portal(&[5, 0]), Some(5));
        assert_eq!(parse_portal(&[]), None);
        assert_eq!(portal_request(5).len(), 2 + 3);
        assert_eq!(portal_refused(5, 4).len(), 2 + 4);
    }

    #[test]
    fn status_up_is_six_words_in_the_request_order() {
        let points = [1, 2, 3, 4, 5, 6];
        let packet = status_up(points);
        assert_eq!(packet.len(), 2 + 14);
        assert_eq!(payload(&packet), [1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0]);
        assert_eq!(parse_status_up(payload(&packet)), Some(points));
        assert_eq!(parse_status_up(&[0; 11]), None);
    }

    #[test]
    fn gatekeeper_packets_have_the_original_sizes() {
        // As captured from the US client: NPC 389, destination 1.
        assert_eq!(parse_gatekeeper(&[0x85, 0x01, 0, 0, 1]), Some((389, 1)));
        assert_eq!(parse_gatekeeper(&[0x85, 0x01, 0, 0]), None);
        assert_eq!(gatekeeper_request(389, 1).len(), 2 + 7);
        let result = gatekeeper_result(true, 0x01020304);
        assert_eq!(result.len(), 2 + 7);
        assert_eq!(payload(&result), [1, 4, 3, 2, 1]);
    }

    #[test]
    fn user_shape_has_the_original_offsets() {
        let mut s = UserShape {
            id: 9,
            motion: 1,
            country: 0,
            family: 1,
            hair: 2,
            face: 3,
            size: 4,
            job: 3,
            sex: 1,
            grow: 2,
            kills: 0x01020304,
            name: b"Elf".to_vec(),
            ..Default::default()
        };
        s.equipment[5] = [13, 1, 0];
        let packet = user_shape(&s);
        // Offsets from the opcode, as SendUserShape fills its buffer: 6 status, 7 motion, 8 country ... 16 grow, 17 kills,
        // 21 equipment, 45 name, 66 guild name (no cloak).
        let b = &packet[2..];
        assert_eq!(b[2..6], 9u32.to_le_bytes());
        assert_eq!(b[6..17], [0, 1, 0, 1, 2, 3, 4, 3, 1, 0, 2]);
        assert_eq!(b[17..21], 0x01020304u32.to_le_bytes());
        assert_eq!(b[21 + 15..21 + 18], [13, 1, 0], "slot 5");
        assert_eq!(&b[45..49], b"Elf\0");
        assert_eq!(b.len(), 67, "66 + empty guild name");
        s.cloak = Some([1, 2, 3, 4, 5, 6]);
        s.guild_name = b"G".to_vec();
        let with_cloak = user_shape(&s);
        assert_eq!(with_cloak[2..][66..72], [1, 2, 3, 4, 5, 6]);
        assert_eq!(with_cloak.len() - 2, 74, "72 + 'G' + zero");
        assert_eq!(enchant_level(b"00000000000000000012"), 12);
        assert_eq!(enchant_level(b"short"), 0);
    }

    #[test]
    fn time_is_packed_like_the_original() {
        // 2026-10-13 15:42:07
        let t = pack_time(2026, 10, 13, 15, 42, 7);
        assert_eq!(t >> 26, 26);
        assert_eq!((t >> 22) & 0xf, 10);
        assert_eq!((t >> 17) & 0x1f, 13);
        assert_eq!((t >> 12) & 0x1f, 15);
        assert_eq!((t >> 6) & 0x3f, 42);
        assert_eq!(t & 0x3f, 7);
    }
}
