//! Operator (GM) packets (`Fxxx`), as `CUser::PacketAdminChat`, `PacketAdminCmdA/B/E` in `ps_game.exe` read and build them
//! (`docs/original/gm-commands.md` sections 5 to 9, the commands of deviations 14 to 17 from the client's own layouts,
//! `docs/client/gm-and-commands.md`). Parsers take the payload (after the opcode) and need its exact length.

use crate::chat;
use crate::frame::encode;

/// The answer to a command: `u16 value`, 0 for done, else the opcode that failed.
pub const OP_ANSWER: u16 = 0xF501;

pub const OP_CHAT_NORMAL: u16 = 0xF101;
pub const OP_CHAT_WHISPER: u16 = 0xF102;
pub const OP_CHAT_TRADE: u16 = 0xF103;
pub const OP_CHAT_GUILD: u16 = 0xF104;
pub const OP_CHAT_PARTY: u16 = 0xF105;
pub const OP_CHAT_UNION: u16 = chat::OP_UNION_NOTICE;

pub const OP_SET_LEVEL: u16 = 0xF601;
pub const OP_CLEAR_LEVEL: u16 = 0xF602;

pub const OP_SET: u16 = 0xF701;
pub const OP_MAKE_ITEM: u16 = 0xF702;
pub const OP_RECALL_ITEM: u16 = 0xF703;
pub const OP_MOB_MAKE: u16 = 0xF704;
pub const OP_MOB_REMOVE: u16 = 0xF705;
pub const OP_SERVER_ON: u16 = 0xF706;
pub const OP_SERVER_OFF: u16 = 0xF707;
pub const OP_SHUTDOWN: u16 = 0xF708;
pub const OP_ACCOUNT_KICK: u16 = 0xF709;
pub const OP_ACCOUNT_BAN: u16 = 0xF70A;
pub const OP_ACCOUNT_UNBAN: u16 = 0xF70B;
pub const OP_NPC_MAKE: u16 = 0xF70C;
pub const OP_NPC_REMOVE: u16 = 0xF70D;
pub const OP_REMOVE_TARGETED: u16 = 0xF70E;

pub const OP_BAN: u16 = 0xF805;
pub const OP_UNBAN: u16 = 0xF806;
pub const OP_QUEST_ON: u16 = 0xF809;
pub const OP_QUEST_OFF: u16 = 0xF80A;
pub const OP_EVENT_ADD: u16 = 0xF80F;
pub const OP_EVENT_REMOVE: u16 = 0xF810;

/// `/request GM <text>`, the one player opcode of the user questions.
pub const OP_USER_QUESTION: u16 = 0x1201;

pub const OP_UQ_GET: u16 = 0xF201;
pub const OP_UQ_ANSWER: u16 = 0xF202;
pub const OP_UQ_ANSWER_TO: u16 = 0xF203;
pub const OP_UQ_COUNT: u16 = 0xF204;

pub const OP_CURE: u16 = 0xF801;
pub const OP_CLEAR_BAGS: u16 = 0xF802;
pub const OP_CLEAR_EQUIPMENT: u16 = 0xF803;
pub const OP_KICK: u16 = 0xF804;
pub const OP_QUEST_ADD: u16 = 0xF80B;
pub const OP_QUEST_REMOVE: u16 = 0xF80C;
pub const OP_HEAL: u16 = 0xF80D;
pub const OP_CURE_ALL: u16 = 0xF80E;

pub const OP_QUIRY: u16 = 0xF901;
pub const OP_SUMMON: u16 = 0xF904;
pub const OP_SUMMON_TO: u16 = 0xF905;
pub const OP_NOTICE_STAFF: u16 = 0xF906;
pub const OP_NOTICE_VIEW: u16 = 0xF907;
pub const OP_NOTICE_TO: u16 = 0xF908;
pub const OP_NOTICE_ZONE: u16 = 0xF909;
pub const OP_NOTICE_COUNTRY: u16 = 0xF90A;
pub const OP_NOTICE_ALL: u16 = 0xF90B;
pub const OP_SUMMON_PARTY: u16 = 0xF90C;

/// `/char on`: the GM becomes invisible.
pub const OP_INVISIBLE: u16 = 0xFA01;
/// `/char off`: visible.
pub const OP_VISIBLE: u16 = 0xFA02;
/// `/attack on`: "won't be attacked".
pub const OP_UNTOUCHABLE: u16 = 0xFA05;
pub const OP_TOUCHABLE: u16 = 0xFA06;
pub const OP_WARNING: u16 = 0xFA07;
pub const OP_WATCH: u16 = 0xFA08;
pub const OP_MOVE_TO_PLAYER: u16 = 0xFA09;
pub const OP_MOVE_TO_SPOT: u16 = 0xFA0A;
pub const OP_LISTEN: u16 = 0xFA0B;
pub const OP_LISTEN_OFF: u16 = 0xFA0C;
/// The copy of a listened-to player's line (`F502`, server to the GM).
pub const OP_LISTEN_CHAT: u16 = 0xF502;
pub const OP_SILENCE: u16 = 0xFA0D;
pub const OP_SILENCE_OFF: u16 = 0xFA0E;
pub const OP_STOP: u16 = 0xFA0F;
pub const OP_STOP_OFF: u16 = 0xFA10;
pub const OP_MOVE_BIND: u16 = 0xFA11;
pub const OP_MOVE_GUILD: u16 = 0xFA12;
pub const OP_GUILD_GRADE: u16 = 0xFA13;
pub const OP_GUILD_RANK: u16 = 0xFA14;
pub const OP_GUILD_POINTS: u16 = 0xFA15;
pub const OP_PVP_ON: u16 = 0xFA16;
pub const OP_PVP_OFF: u16 = 0xFA17;
pub const OP_CALL: u16 = 0xFA18;
pub const OP_AUCTION_RECALL: u16 = 0xFA19;
pub const OP_AUCTION_SEARCH: u16 = 0xFA1A;

/// The longest user question or answer, the zero counted.
pub const QUESTION_MAX: usize = 1000;
/// The name a command's result whisper comes from.
pub const GM_CMD_NAME: &[u8] = b"<GM Cmd>";

const NAME_LEN: usize = 21;
const GUILD_LEN: usize = 25;

/// The staff level an opcode's family needs (`level <=` it): `F1xx` and `F2xx` 10, `F6xx` 1, `F7xx` 2, `F8xx` 3, `F9xx` 4, `FAxx` 5.
/// `None` for any other `Fxxx` family (the original closes the connection).
pub fn family(opcode: u16) -> Option<u8> {
    match opcode >> 8 {
        0xF1 | 0xF2 => Some(10),
        0xF6 => Some(1),
        0xF7 => Some(2),
        0xF8 => Some(3),
        0xF9 => Some(4),
        0xFA => Some(5),
        _ => None,
    }
}

/// `F501 u16 value`.
pub fn answer(value: u16) -> Vec<u8> {
    encode(OP_ANSWER, &value.to_le_bytes())
}

/// A name from a zero-padded field of 21 or 25 bytes: up to the first zero, the field's last byte forced to zero as the original does.
pub fn name_field(bytes: &[u8]) -> Vec<u8> {
    let usable = bytes.len().saturating_sub(1);
    bytes[..usable].iter().copied().take_while(|&b| b != 0).collect()
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(bytes, at))
}

/// `F601`, in the client's layout (`name[21], u8 level`; the original reads `u8 level, name[21]`, deviation 2).
pub fn parse_set_level(payload: &[u8]) -> Option<(Vec<u8>, u8)> {
    (payload.len() == NAME_LEN + 1).then(|| (name_field(&payload[..NAME_LEN]), payload[NAME_LEN]))
}

/// One name of 21 bytes: `F602`, `F709`, `F70B`, `F806`, `FA08`, `FA09`, `FA0B`, `FA0E`, `FA10`.
pub fn parse_name(payload: &[u8]) -> Option<Vec<u8>> {
    (payload.len() == NAME_LEN).then(|| name_field(payload))
}

/// `F701`: `u8 stat, u32 value, name[21]`.
pub struct SetRequest {
    pub stat: u8,
    pub value: u32,
    pub name: Vec<u8>,
}

pub fn parse_set(payload: &[u8]) -> Option<SetRequest> {
    (payload.len() == 5 + NAME_LEN).then(|| SetRequest { stat: payload[0], value: u32_at(payload, 1), name: name_field(&payload[5..]) })
}

/// `F702`: `u8 type, u8 type id, u8 count`.
pub fn parse_make_item(payload: &[u8]) -> Option<(u8, u8, u8)> {
    (payload.len() == 3).then(|| (payload[0], payload[1], payload[2]))
}

/// `F703`: `name[21], u8 type, u8 type id`.
pub fn parse_recall_item(payload: &[u8]) -> Option<(Vec<u8>, u8, u8)> {
    (payload.len() == NAME_LEN + 2).then(|| (name_field(&payload[..NAME_LEN]), payload[NAME_LEN], payload[NAME_LEN + 1]))
}

/// `FA19` `/auctionrecall`: `name[21], u32 market id` (0: every auction of the seller).
pub fn parse_auction_recall(payload: &[u8]) -> Option<(Vec<u8>, u32)> {
    (payload.len() == NAME_LEN + 4).then(|| (name_field(&payload[..NAME_LEN]), u32_at(payload, NAME_LEN)))
}

/// `F704`, `F705`: `u16 mob, u8 count`.
pub fn parse_mob(payload: &[u8]) -> Option<(u16, u8)> {
    (payload.len() == 3).then(|| (u16_at(payload, 0), payload[2]))
}

/// `F70E`: `u32 object id`.
pub fn parse_mob_id(payload: &[u8]) -> Option<u32> {
    (payload.len() == 4).then(|| u32_at(payload, 0))
}

/// `F70C`, `F70D`: `u8 type, u16 type id, u8 count`.
pub fn parse_npc(payload: &[u8]) -> Option<(u8, u16, u8)> {
    (payload.len() == 4).then(|| (payload[0], u16_at(payload, 1), payload[3]))
}

/// `FA07`: `name[21], u8 len, text`, `len` 2 to 128 and the payload exactly `22 + len`. The text is returned with its zero.
pub fn parse_named_text(payload: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    let len = usize::from(*payload.get(NAME_LEN)?);
    ((2..=chat::MAX_TEXT).contains(&len) && payload.len() >= NAME_LEN + 1 + len)
        .then(|| (name_field(&payload[..NAME_LEN]), &payload[NAME_LEN + 1..NAME_LEN + 1 + len]))
}

/// `F70A`, `F805`, `FA0D`, `FA0F`: `u16 minutes, name[21]`.
pub fn parse_ban(payload: &[u8]) -> Option<(u16, Vec<u8>)> {
    (payload.len() == 2 + NAME_LEN).then(|| (u16_at(payload, 0), name_field(&payload[2..])))
}

/// `F809`, `F80A`: `u16 quest`.
pub fn parse_quest(payload: &[u8]) -> Option<u16> {
    (payload.len() == 2).then(|| u16_at(payload, 0))
}

/// `F80F`, `F810`: `name[21], event[21]`; the event has 1 to 20 bytes.
pub fn parse_event(payload: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    if payload.len() < 2 * NAME_LEN {
        return None;
    }
    let event = name_field(&payload[NAME_LEN..2 * NAME_LEN]);
    (!event.is_empty()).then(|| (name_field(&payload[..NAME_LEN]), event))
}

/// `FA0A`: `f32 x, f32 z, u16 map`.
pub fn parse_go(payload: &[u8]) -> Option<(f32, f32, u16)> {
    (payload.len() == 10)
        .then(|| (f32_at(payload, 0), f32_at(payload, 4), u16_at(payload, 8)))
        .filter(|(x, z, _)| x.is_finite() && z.is_finite())
}

/// `FA11`: `u16 map` (`0xFFFF` = the current one).
pub fn parse_map(payload: &[u8]) -> Option<u16> {
    (payload.len() == 2).then(|| u16_at(payload, 0))
}

/// `FA12`: `f32 x, f32 z, u16 map, name[25]`.
pub struct GuildGo {
    pub x: f32,
    pub z: f32,
    pub map: u16,
    pub name: Vec<u8>,
}

pub fn parse_guild_go(payload: &[u8]) -> Option<GuildGo> {
    (payload.len() == 10 + GUILD_LEN)
        .then(|| GuildGo { x: f32_at(payload, 0), z: f32_at(payload, 4), map: u16_at(payload, 8), name: name_field(&payload[10..]) })
        .filter(|go| go.x.is_finite() && go.z.is_finite())
}

/// `FA13`: `guild[25], name[21], u8 grade`.
pub fn parse_guild_grade(payload: &[u8]) -> Option<(Vec<u8>, Vec<u8>, u8)> {
    (payload.len() == GUILD_LEN + NAME_LEN + 1)
        .then(|| (name_field(&payload[..GUILD_LEN]), name_field(&payload[GUILD_LEN..GUILD_LEN + NAME_LEN]), payload[GUILD_LEN + NAME_LEN]))
}

/// `FA14`: `guild[25], u8 ranking`.
pub fn parse_guild_rank(payload: &[u8]) -> Option<(Vec<u8>, u8)> {
    (payload.len() == GUILD_LEN + 1).then(|| (name_field(&payload[..GUILD_LEN]), payload[GUILD_LEN]))
}

/// `FA15`: `guild[25], u32 points`.
pub fn parse_guild_points(payload: &[u8]) -> Option<(Vec<u8>, u32)> {
    (payload.len() == GUILD_LEN + 4).then(|| (name_field(&payload[..GUILD_LEN]), u32_at(payload, GUILD_LEN)))
}

/// `FA18`: `u8 country`.
pub fn parse_call(payload: &[u8]) -> Option<u8> {
    (payload.len() == 1).then(|| payload[0])
}

/// `text[n]` as the original keeps it: the last byte forced to zero.
fn zero_ended(text: &[u8]) -> Vec<u8> {
    let mut text = text.to_vec();
    if let Some(last) = text.last_mut() {
        *last = 0;
    }
    text
}

/// `0x1201`: `u16 n, text[n]`, 2 to 1000 and the payload exactly `2 + n`. The text has its zero.
pub fn parse_user_question(payload: &[u8]) -> Option<Vec<u8>> {
    let n = usize::from(u16::from_le_bytes([*payload.first()?, *payload.get(1)?]));
    ((2..=QUESTION_MAX).contains(&n) && payload.len() == 2 + n).then(|| zero_ended(&payload[2..]))
}

/// `F202`: `u8 kind, u32 question id, u16 n, text[n]`, `n` 1 to 1000 and the payload exactly `7 + n`.
pub fn parse_uq_answer(payload: &[u8]) -> Option<(u8, u32, Vec<u8>)> {
    let n = usize::from(u16_at(payload.get(..7)?, 5));
    ((1..=QUESTION_MAX).contains(&n) && payload.len() == 7 + n).then(|| (payload[0], u32_at(payload, 1), zero_ended(&payload[7..])))
}

/// `F203`: `u32 question id, name[21], u16 n, text[n]`, `n` 1 to 1000 and the payload exactly `27 + n`.
pub fn parse_uq_answer_to(payload: &[u8]) -> Option<(u32, Vec<u8>, Vec<u8>)> {
    let n = usize::from(u16_at(payload.get(..27)?, 25));
    ((1..=QUESTION_MAX).contains(&n) && payload.len() == 27 + n)
        .then(|| (u32_at(payload, 0), name_field(&payload[4..4 + NAME_LEN]), zero_ended(&payload[27..])))
}

/// `F80B`, `F80C`: `name[21], u16 quest`.
pub fn parse_name_quest(payload: &[u8]) -> Option<(Vec<u8>, u16)> {
    (payload.len() == NAME_LEN + 2).then(|| (name_field(&payload[..NAME_LEN]), u16_at(payload, NAME_LEN)))
}

/// `F905`, `F90C`: `name[21], f32 x, f32 z, u16 map`.
pub fn parse_summon_to(payload: &[u8]) -> Option<(Vec<u8>, f32, f32, u16)> {
    (payload.len() == NAME_LEN + 10)
        .then(|| {
            (name_field(&payload[..NAME_LEN]), f32_at(payload, NAME_LEN), f32_at(payload, NAME_LEN + 4), u16_at(payload, NAME_LEN + 8))
        })
        .filter(|(_, x, z, _)| x.is_finite() && z.is_finite())
}

/// `F906`: `u8 len, text`, `len` 2 to 128 and the payload exactly `1 + len`. The text is returned with its zero.
pub fn parse_notice(payload: &[u8]) -> Option<&[u8]> {
    let len = usize::from(*payload.first()?);
    ((2..=chat::MAX_TEXT).contains(&len) && payload.len() > len).then(|| &payload[1..1 + len])
}

/// A notice that may repeat (`F907` to `F90B`): the seconds between repeats, the length as sent (under 2 is the "off" form, over 128
/// is refused by the caller) and the text.
#[derive(Debug, PartialEq, Eq)]
pub struct RepeatNotice<'a> {
    pub repeat: u16,
    pub len: u8,
    pub text: &'a [u8],
}

fn repeat_notice(payload: &[u8]) -> Option<RepeatNotice<'_>> {
    let len = *payload.get(2)?;
    (payload.len() >= 3 + usize::from(len)).then(|| RepeatNotice {
        repeat: u16_at(payload, 0),
        len,
        text: &payload[3..3 + usize::from(len)],
    })
}

/// `F907`, `F909`, `F90B`: `u16 repeat, u8 len, text`.
pub fn parse_repeat_notice(payload: &[u8]) -> Option<RepeatNotice<'_>> {
    repeat_notice(payload)
}

/// `F908`: `name[21], u16 repeat, u8 len, text`.
pub fn parse_notice_to(payload: &[u8]) -> Option<(Vec<u8>, RepeatNotice<'_>)> {
    Some((name_field(payload.get(..NAME_LEN)?), repeat_notice(&payload[NAME_LEN..])?))
}

/// `F90A`: `u8 country, u16 repeat, u8 len, text`.
pub fn parse_notice_country(payload: &[u8]) -> Option<(u8, RepeatNotice<'_>)> {
    Some((*payload.first()?, repeat_notice(&payload[1..])?))
}

/// `F201` to the GM: `u16 n` (the waiting count plus one), `u32 question id`, `u32 time`, `name[21]`, `u16 len`, the text with its zero.
pub fn uq_question(waiting_plus_one: u16, id: u32, time: u32, name: &[u8], text: &[u8]) -> Vec<u8> {
    let mut p = waiting_plus_one.to_le_bytes().to_vec();
    p.extend_from_slice(&id.to_le_bytes());
    p.extend_from_slice(&time.to_le_bytes());
    p.extend_from_slice(&chat::name_field(name));
    p.extend_from_slice(&(text.len() as u16).to_le_bytes());
    p.extend_from_slice(text);
    encode(OP_UQ_GET, &p)
}

/// `F201 u16 0`: no question waits.
pub fn uq_none() -> Vec<u8> {
    encode(OP_UQ_GET, &0u16.to_le_bytes())
}

/// `F204 u16 waiting`.
pub fn uq_count(waiting: u16) -> Vec<u8> {
    encode(OP_UQ_COUNT, &waiting.to_le_bytes())
}

/// `F502`, a watched player's line copied to the listening GM (`CUser::SendListenChat` 0x4702f0): `char name[21]` (the speaker),
/// `u8 len, text[len]`, 24 + len bytes with the opcode. The text is `[kind]` or, for a whisper, `[kind,target]`, then the line as the
/// speaker sent it. The line ends in a counted zero: the client's line has one already, else one is added (the DLL that shows it,
/// `docs/client/operator-listen.md`, prefers one).
pub fn listen_chat(speaker: &[u8], kind: &[u8], target: Option<&[u8]>, line: &[u8]) -> Vec<u8> {
    let mut p = chat::name_field(speaker).to_vec();
    let mut text = vec![b'['];
    text.extend_from_slice(kind);
    if let Some(target) = target {
        text.push(b',');
        text.extend(target.iter().take(NAME_LEN - 1).take_while(|&&b| b != 0));
    }
    text.push(b']');
    text.extend_from_slice(line);
    if text.last() != Some(&0) {
        text.push(0);
    }
    text.truncate(usize::from(u8::MAX));
    if let Some(last) = text.last_mut() {
        *last = 0;
    }
    p.push(text.len() as u8);
    p.extend_from_slice(&text);
    encode(OP_LISTEN_CHAT, &p)
}

/// One equipment line of `F901`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuiryItem {
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
    pub quality: u16,
    pub gems: [u8; 6],
    pub craft_name: [u8; 21],
}

/// `F901`'s character sheet, the fields in wire order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuirySheet {
    pub mode: u8,
    pub level: u16,
    pub exp: u32,
    pub money: u32,
    pub stat_point: u16,
    pub skill_point: u16,
    /// STR, DEX, REC, INT, LUC, WIS (totals).
    pub stats: [u16; 6],
    pub hg: u16,
    pub vg: u16,
    pub cg: u8,
    pub og: u8,
    pub ig: u8,
    pub items: Vec<QuiryItem>,
}

/// `F901` to the GM (37 + 32 k bytes with the opcode).
pub fn sheet(s: &QuirySheet) -> Vec<u8> {
    let mut p = vec![s.mode];
    p.extend_from_slice(&s.level.to_le_bytes());
    p.extend_from_slice(&s.exp.to_le_bytes());
    p.extend_from_slice(&s.money.to_le_bytes());
    p.extend_from_slice(&s.stat_point.to_le_bytes());
    p.extend_from_slice(&s.skill_point.to_le_bytes());
    for stat in s.stats {
        p.extend_from_slice(&stat.to_le_bytes());
    }
    p.extend_from_slice(&s.hg.to_le_bytes());
    p.extend_from_slice(&s.vg.to_le_bytes());
    p.extend_from_slice(&[s.cg, s.og, s.ig, s.items.len() as u8]);
    for item in &s.items {
        p.extend_from_slice(&[item.slot, item.item_type, item.type_id]);
        p.extend_from_slice(&item.quality.to_le_bytes());
        p.extend_from_slice(&item.gems);
        p.extend_from_slice(&item.craft_name);
    }
    encode(OP_QUIRY, &p)
}

/// `F904`, `F905`, `F90C` to the summoned player: `u32 id, u16 map, f32 x, y, z` (the layout of `world::position`).
pub fn summoned(opcode: u16, id: u32, map: u16, position: [f32; 3]) -> Vec<u8> {
    let mut packet = crate::world::position(id, map, position);
    packet[2..4].copy_from_slice(&opcode.to_le_bytes());
    packet
}

/// `F106` to everyone: `u16 len, text` (`CWorld::SendNotice`; the client shows "[Notice] <text>").
pub const OP_NOTICE_WORLD: u16 = 0xF106;

/// The longest text `CWorld::SendNotice` sends (`len` 2 to 2000, the zero counted).
pub const MAX_WORLD_NOTICE: usize = 1999;

/// `F106` (`docs/original/operator-console.md` section 1, `/nt`): the text and its zero; `None` for an empty text or one above 1999 bytes.
pub fn world_notice(text: &str) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() > MAX_WORLD_NOTICE {
        return None;
    }
    let mut p = (text.len() as u16 + 1).to_le_bytes().to_vec();
    p.extend_from_slice(text.as_bytes());
    p.push(0);
    Some(encode(OP_NOTICE_WORLD, &p))
}

/// `F906` to `F90B` to the players: `u8 len, text` (the text with its zero).
pub fn notice(opcode: u16, text: &[u8]) -> Vec<u8> {
    let mut p = vec![text.len() as u8];
    p.extend_from_slice(text);
    encode(opcode, &p)
}

/// The result whisper of `/event` (`F102 u8 0, "<GM Cmd>"[21], u8 len, text`).
pub fn gm_cmd_whisper(text: &[u8]) -> Vec<u8> {
    chat::whisper_op(OP_CHAT_WHISPER, false, GM_CMD_NAME, text)
}

/// `F701` to the target: `u8 stat, u32 value`.
pub fn set_answer(stat: u8, value: u32) -> Vec<u8> {
    let mut p = vec![stat];
    p.extend_from_slice(&value.to_le_bytes());
    encode(OP_SET, &p)
}

/// `F601` / `F602` to the target: `u8 level`.
pub fn level_answer(opcode: u16, level: u8) -> Vec<u8> {
    encode(opcode, &[level])
}

/// A bare opcode (`FA0D` to `FA10` to a target whose ban started or ended).
pub fn bare(opcode: u16) -> Vec<u8> {
    encode(opcode, &[])
}

/// `F70A` / `F805` to a banned player: `u16 minutes` (0 = no end); the client shows 340 or 341.
pub fn ban_notice(opcode: u16, minutes: u16) -> Vec<u8> {
    encode(opcode, &minutes.to_le_bytes())
}

/// `FA07` to the target: `u8 len, text` (the text with its zero).
pub fn warning(text: &[u8]) -> Vec<u8> {
    let mut p = vec![text.len() as u8];
    p.extend_from_slice(text);
    encode(OP_WARNING, &p)
}

/// `FA08` to the GM: `u16 map, f32 x, y, z`.
pub fn watch(map: u16, position: [f32; 3]) -> Vec<u8> {
    let mut p = map.to_le_bytes().to_vec();
    for v in position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_WATCH, &p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &[u8], len: usize) -> Vec<u8> {
        let mut field = text.to_vec();
        field.resize(len, 0);
        field
    }

    /// Each parser takes its exact length and nothing else.
    fn exact<T>(parse: impl Fn(&[u8]) -> Option<T>, len: usize) {
        let ok = vec![b'a'; len];
        assert!(parse(&ok).is_some(), "length {len}");
        assert!(parse(&ok[..len - 1]).is_none(), "one short of {len}");
        let mut long = ok;
        long.push(0);
        assert!(parse(&long).is_none(), "one over {len}");
    }

    #[test]
    fn a_world_notice_counts_its_zero() {
        let packet = world_notice("Hi").unwrap();
        assert_eq!(&packet[2..], &[0x06u8, 0xF1, 3, 0, b'H', b'i', 0][..], "u16 length 3: the text and its zero");
        assert_eq!(packet[0] as usize | (packet[1] as usize) << 8, packet.len());
        assert!(world_notice("").is_none() && world_notice(&"x".repeat(2000)).is_none() && world_notice(&"x".repeat(1999)).is_some());
    }

    #[test]
    fn parsers_need_the_exact_length() {
        exact(parse_set_level, 22);
        exact(parse_name, 21);
        exact(parse_set, 26);
        exact(parse_make_item, 3);
        exact(parse_recall_item, 23);
        exact(parse_mob, 3);
        exact(parse_mob_id, 4);
        exact(parse_npc, 4);
        exact(parse_ban, 23);
        exact(parse_quest, 2);
        exact(parse_go, 10);
        exact(parse_map, 2);
        exact(parse_guild_go, 35);
        exact(parse_guild_grade, 47);
        exact(parse_guild_rank, 26);
        exact(parse_guild_points, 29);
        exact(parse_call, 1);
        exact(parse_name_quest, 23);
        exact(parse_summon_to, 31);
    }

    fn text_packet(prefix: &[u8], n: usize) -> Vec<u8> {
        let mut p = prefix.to_vec();
        p.extend_from_slice(&vec![b'a'; n]);
        p
    }

    #[test]
    fn a_user_question_has_two_to_a_thousand_bytes_and_ends_in_a_zero() {
        let with = |n: u16, len: usize| text_packet(&n.to_le_bytes(), len);
        assert!(parse_user_question(&with(1, 1)).is_none(), "n 1");
        assert!(parse_user_question(&with(1001, 1001)).is_none(), "n 1001");
        assert!(parse_user_question(&with(0, 0)).is_none());
        assert!(parse_user_question(&with(1000, 1000)).is_some());
        assert!(parse_user_question(&with(5, 4)).is_none(), "one short");
        assert!(parse_user_question(&with(5, 6)).is_none(), "one over");
        assert!(parse_user_question(&[2]).is_none());
        assert_eq!(parse_user_question(&with(3, 3)), Some(b"aa\0".to_vec()));
    }

    #[test]
    fn an_answer_has_one_to_a_thousand_bytes() {
        let mut prefix = vec![2];
        prefix.extend_from_slice(&77u32.to_le_bytes());
        let answer = |n: u16, len: usize| {
            let mut p = prefix.clone();
            p.extend_from_slice(&n.to_le_bytes());
            text_packet(&p, len)
        };
        assert_eq!(parse_uq_answer(&answer(2, 2)), Some((2, 77, b"a\0".to_vec())));
        assert!(parse_uq_answer(&answer(0, 0)).is_none(), "n 0");
        assert!(parse_uq_answer(&answer(1, 1)).is_some());
        assert!(parse_uq_answer(&answer(1001, 1001)).is_none());
        assert!(parse_uq_answer(&answer(3, 2)).is_none() && parse_uq_answer(&answer(3, 4)).is_none());
        assert!(parse_uq_answer(&prefix).is_none());

        let mut to = 9u32.to_le_bytes().to_vec();
        to.extend_from_slice(&name(b"Bob", 21));
        let answer_to = |n: u16, len: usize| {
            let mut p = to.clone();
            p.extend_from_slice(&n.to_le_bytes());
            text_packet(&p, len)
        };
        assert_eq!(parse_uq_answer_to(&answer_to(2, 2)), Some((9, b"Bob".to_vec(), b"a\0".to_vec())));
        assert!(parse_uq_answer_to(&answer_to(0, 0)).is_none());
        assert!(parse_uq_answer_to(&answer_to(2, 1)).is_none() && parse_uq_answer_to(&answer_to(2, 3)).is_none());
        assert!(parse_uq_answer_to(&answer_to(1001, 1001)).is_none());
    }

    #[test]
    fn notices_tell_the_off_form_from_a_text() {
        assert_eq!(parse_notice(b"\x03hi\0"), Some(&b"hi\0"[..]));
        assert!(parse_notice(b"\x01\0").is_none() && parse_notice(b"\x03hi").is_none() && parse_notice(&[]).is_none());
        let mut big = vec![129];
        big.extend_from_slice(&[b'a'; 129]);
        assert!(parse_notice(&big).is_none());

        let repeat = parse_repeat_notice(&[5, 0, 3, b'h', b'i', 0]).unwrap();
        assert_eq!((repeat.repeat, repeat.len, repeat.text), (5, 3, &b"hi\0"[..]));
        let off = parse_repeat_notice(&[0, 0, 0]).unwrap();
        assert_eq!((off.len, off.text.len()), (0, 0));
        assert_eq!(parse_repeat_notice(&[0, 0, 1, 0]).unwrap().len, 1);
        assert!(parse_repeat_notice(&[0, 0, 3, b'h']).is_none(), "shorter than announced");
        assert!(parse_repeat_notice(&[0, 0, 0, 0]).is_some(), "padding after the text is ignored");
        let mut over = vec![0, 0, 200];
        over.extend_from_slice(&[b'a'; 200]);
        assert_eq!(parse_repeat_notice(&over).unwrap().len, 200, "the caller refuses it");

        let mut to = name(b"Bob", 21);
        to.extend_from_slice(&[9, 0, 2, b'h', 0]);
        let (target, notice) = parse_notice_to(&to).unwrap();
        assert_eq!((target, notice.repeat, notice.len), (b"Bob".to_vec(), 9, 2));
        assert!(parse_notice_to(&to[..to.len() - 1]).is_none());
        let (country, notice) = parse_notice_country(&[1, 7, 0, 2, b'h', 0]).unwrap();
        assert_eq!((country, notice.repeat, notice.text), (1, 7, &b"h\0"[..]));
        assert!(parse_notice_country(&[1, 7, 0, 2, b'h']).is_none());
    }

    #[test]
    fn summon_fields_are_read_in_order() {
        let mut p = name(b"Bob", 21);
        p.extend_from_slice(&1300.5f32.to_le_bytes());
        p.extend_from_slice(&1650.0f32.to_le_bytes());
        p.extend_from_slice(&1u16.to_le_bytes());
        assert_eq!(parse_summon_to(&p), Some((b"Bob".to_vec(), 1300.5, 1650.0, 1)));
        p[21..25].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(parse_summon_to(&p), None, "a position that is no number");
        let mut q = name(b"Bob", 21);
        q.extend_from_slice(&5u16.to_le_bytes());
        assert_eq!(parse_name_quest(&q), Some((b"Bob".to_vec(), 5)));
    }

    #[test]
    fn question_and_sheet_packets_have_the_original_sizes() {
        assert_eq!(uq_question(3, 9, 77, b"Bob", b"hello\0").len() - 2, 35 + 6);
        assert_eq!(uq_none().len() - 2, 4);
        assert_eq!(uq_count(2)[2..], [0x04, 0xF2, 2, 0]);
        let item = QuiryItem { slot: 1, item_type: 2, type_id: 3, quality: 4, gems: [0; 6], craft_name: [0; 21] };
        let mut sheet_data = QuirySheet {
            mode: 1,
            level: 30,
            exp: 5,
            money: 6,
            stat_point: 7,
            skill_point: 8,
            stats: [1, 2, 3, 4, 5, 6],
            hg: 1,
            vg: 2,
            cg: 3,
            og: 4,
            ig: 5,
            items: vec![],
        };
        assert_eq!(sheet(&sheet_data).len() - 2, 37);
        sheet_data.items = vec![item.clone(), item];
        assert_eq!(sheet(&sheet_data).len() - 2, 37 + 64);
        assert_eq!(sheet(&sheet_data)[2..4], [0x01, 0xF9]);
    }

    #[test]
    fn a_summon_is_a_position_under_the_commands_opcode() {
        let packet = summoned(OP_SUMMON, 7, 1, [1.0, 2.0, 3.0]);
        assert_eq!(packet.len() - 2, 20);
        let mut position = crate::world::position(7, 1, [1.0, 2.0, 3.0]);
        position[2..4].copy_from_slice(&OP_SUMMON.to_le_bytes());
        assert_eq!(packet, position);
        assert_eq!(notice(OP_NOTICE_ALL, b"hi\0")[2..], [0x0B, 0xF9, 3, b'h', b'i', 0]);
        let whisper = gm_cmd_whisper(b"ok\0");
        assert_eq!(whisper.len() - 2, 25 + 3);
        assert_eq!(whisper[2..4], [0x02, 0xF1]);
        assert_eq!(whisper[4], 0);
    }

    #[test]
    fn set_level_is_in_the_clients_layout() {
        let mut payload = name(b"TestVail", 21);
        payload.push(3);
        assert_eq!(parse_set_level(&payload), Some((b"TestVail".to_vec(), 3)));
    }

    #[test]
    fn names_end_at_the_first_zero_or_the_last_byte() {
        assert_eq!(name_field(&name(b"abc", 21)), b"abc");
        assert_eq!(name_field(&[b'x'; 21]), vec![b'x'; 20], "the last byte is the terminator");
        assert_eq!(name_field(&[b'x'; 25]), vec![b'x'; 24]);
        assert_eq!(name_field(&[]), b"");
    }

    #[test]
    fn named_text_has_two_to_128_bytes_and_the_exact_length() {
        let mut payload = name(b"Bob", 21);
        payload.push(4);
        payload.extend_from_slice(b"hi!\0");
        assert_eq!(parse_named_text(&payload), Some((b"Bob".to_vec(), &b"hi!\0"[..])));
        payload.push(0);
        assert_eq!(parse_named_text(&payload), Some((b"Bob".to_vec(), &b"hi!\0"[..])), "padding after the text is ignored");
        payload.truncate(payload.len() - 2);
        assert!(parse_named_text(&payload).is_none(), "shorter than announced");
        let mut one = name(b"Bob", 21);
        one.extend_from_slice(&[1, 0]);
        assert!(parse_named_text(&one).is_none(), "length 1");
        let mut big = name(b"Bob", 21);
        big.push(129);
        big.extend_from_slice(&[b'a'; 129]);
        assert!(parse_named_text(&big).is_none(), "length 129");
        let mut top = name(b"Bob", 21);
        top.push(128);
        top.extend_from_slice(&[b'a'; 128]);
        assert!(parse_named_text(&top).is_some());
    }

    #[test]
    fn the_event_has_one_to_twenty_bytes() {
        let mut payload = name(b"Bob", 21);
        payload.extend_from_slice(&name(b"winners", 21));
        assert_eq!(parse_event(&payload), Some((b"Bob".to_vec(), b"winners".to_vec())));
        payload[21..].fill(0);
        assert!(parse_event(&payload).is_none());
        payload[21..].fill(b'e');
        assert_eq!(parse_event(&payload).unwrap().1.len(), 20);
    }

    #[test]
    fn fields_are_read_in_order() {
        let mut go = 1.5f32.to_le_bytes().to_vec();
        go.extend_from_slice(&(-2.0f32).to_le_bytes());
        go.extend_from_slice(&7u16.to_le_bytes());
        assert_eq!(parse_go(&go), Some((1.5, -2.0, 7)));
        let mut nan = go.clone();
        nan[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(parse_go(&nan), None, "a position that is no number");
        let mut guild_go = go.clone();
        guild_go.extend_from_slice(&name(b"Guild", GUILD_LEN));
        assert!(parse_guild_go(&guild_go).is_some());
        guild_go[4..8].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert!(parse_guild_go(&guild_go).is_none());
        let mut ban = 90u16.to_le_bytes().to_vec();
        ban.extend_from_slice(&name(b"Bob", 21));
        assert_eq!(parse_ban(&ban), Some((90, b"Bob".to_vec())));
        let mut set = vec![16];
        set.extend_from_slice(&123456u32.to_le_bytes());
        set.extend_from_slice(&name(b"Bob", 21));
        let set = parse_set(&set).unwrap();
        assert_eq!((set.stat, set.value, set.name), (16, 123456, b"Bob".to_vec()));
        let mut points = name(b"Builders", 25);
        points.extend_from_slice(&100u32.to_le_bytes());
        assert_eq!(parse_guild_points(&points), Some((b"Builders".to_vec(), 100)));
        let mut recall = name(b"Bob", 21);
        recall.extend_from_slice(&7u32.to_le_bytes());
        assert_eq!(parse_auction_recall(&recall), Some((b"Bob".to_vec(), 7)));
        assert_eq!(parse_auction_recall(&recall[..24]), None);
    }

    #[test]
    fn the_level_limit_is_by_family() {
        let limits: Vec<_> = [0xF101u16, 0xF10A, 0xF201, 0xF601, 0xF701, 0xF805, 0xF901, 0xFA16, 0xF301, 0xF501, 0xFB00]
            .iter()
            .map(|&op| family(op))
            .collect();
        assert_eq!(limits, [Some(10), Some(10), Some(10), Some(1), Some(2), Some(3), Some(4), Some(5), None, None, None]);
    }

    #[test]
    fn packets_have_the_original_sizes() {
        assert_eq!(answer(0xF701)[2..], [0x01, 0xF5, 0x01, 0xF7]);
        assert_eq!(answer(0).len(), 6, "4 bytes after the length field");
        assert_eq!(set_answer(16, 5).len() - 2, 7);
        assert_eq!(level_answer(OP_SET_LEVEL, 3).len() - 2, 3);
        assert_eq!(bare(OP_SILENCE).len() - 2, 2);
        assert_eq!(ban_notice(OP_ACCOUNT_BAN, 5)[2..], [0x0A, 0xF7, 5, 0]);
        assert_eq!(warning(b"hi\0").len() - 2, 2 + 1 + 3);
        assert_eq!(watch(1, [1.0, 2.0, 3.0]).len() - 2, 16);
    }

    #[test]
    fn listen_copies_are_24_bytes_and_the_text() {
        let p = listen_chat(b"Alice", b"NORMAL", None, b"hi\0");
        assert_eq!(&p[2..4], &[0x02, 0xF5]);
        assert_eq!(&p[4..25], &name(b"Alice", 21)[..]);
        assert_eq!(p[25], 11);
        assert_eq!(&p[26..], b"[NORMAL]hi\0");
        assert_eq!(p.len() - 2, 24 + usize::from(p[25]), "24 + len with the opcode");

        let p = listen_chat(b"Alice", b"TO", Some(b"Bob"), b"psst\0");
        assert_eq!(&p[26..], b"[TO,Bob]psst\0");
        // A line without its zero gets one; a name of 21 bytes keeps 20 and its zero.
        assert_eq!(&listen_chat(b"A", b"ZONE", None, b"x")[26..], b"[ZONE]x\0");
        let long = [b'n'; 30];
        let p = listen_chat(&long, b"TO", Some(&long), b"x\0");
        assert_eq!((p[4 + 20], &p[26..]), (0, &b"[TO,nnnnnnnnnnnnnnnnnnnn]x\0"[..]));
        // Never above 255, still ending in its zero.
        let p = listen_chat(b"A", b"TO", Some(&long), &[b'a'; 300]);
        assert_eq!((p[25], p.len() - 2, *p.last().unwrap()), (255, 24 + 255, 0));
    }
}
