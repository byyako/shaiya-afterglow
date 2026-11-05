//! The game log (`docs/original/game-log.md`): what the original's `ps_game` sent to its log server, as rows for `game_log`
//! (`PS_GameLog.dbo.ActionLog`) and `chat_log` (`PS_ChatLog.dbo.ChatLog`). Every record of section 3 has a constructor here with
//! its action number (decimal, `ActionType`) and the placement of values and texts as the original's writers make them; the game
//! calls `GameLog::action` and `GameLog::chat` where the original's senders sit, and a background task writes the rows in batches
//! (deviation 1). The game never waits for the log.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::backend::{PlayCharacter, Stored};
use crate::config::Config;
use crate::sync::Relock;

/// The queue's size (deviation 1): when it is full, later records are dropped.
const QUEUE: usize = 10_000;
/// Entries written in one go.
const BATCH: usize = 500;
/// The columns' limit on a text (`Text1` to `Text4` are `varchar(100)`).
const TEXT_MAX: usize = 100;
/// The chat log's column (`text varchar(200)`; the packet holds 128 bytes).
const CHAT_MAX: usize = 200;
/// The least time between two warnings about dropped records.
const WARN_EVERY: Duration = Duration::from_secs(60);
/// How often the retention pass runs (deviation 5).
const PURGE_EVERY: Duration = Duration::from_secs(24 * 3600);

/// The action numbers of section 3 (`ActionType`).
pub mod action {
    pub const DEATH_BY_MOB: u8 = 102;
    pub const KILL_PLAYER: u8 = 103;
    pub const REBIRTH: u8 = 105;
    pub const REVIVED: u8 = 106;
    pub const ENTER_WORLD: u8 = 107;
    pub const LEAVE_WORLD: u8 = 108;
    pub const ITEM_GET: u8 = 111;
    pub const ITEM_LOSE: u8 = 112;
    pub const ITEM_BUY: u8 = 113;
    pub const ITEM_SELL: u8 = 114;
    pub const ITEM_TRADE_IN: u8 = 115;
    pub const ITEM_TRADE_OUT: u8 = 116;
    pub const ITEM_DEATH_DROP: u8 = 117;
    pub const ITEM_DEATH_GET: u8 = 118;
    pub const GEM_ADD: u8 = 119;
    pub const GEM_REMOVE: u8 = 120;
    pub const BAG_TO_BANK: u8 = 121;
    pub const BANK_TO_BAG: u8 = 122;
    pub const DUEL_WIN: u8 = 123;
    pub const DUEL_LOSE: u8 = 124;
    pub const DUEL_CANCEL: u8 = 125;
    pub const QUEST_START: u8 = 131;
    pub const QUEST_END: u8 = 133;
    pub const SKILL_LEARN: u8 = 141;
    pub const LEVEL_UP: u8 = 146;
    /// 151 str, 152 dex, 153 int, 154 wis, 155 rec, 156 luc (157 for any other).
    pub const STAT_BASE: u8 = 151;
    pub const MONEY_GET: u8 = 161;
    pub const MONEY_IN: u8 = 163;
    pub const MONEY_OUT: u8 = 164;
    pub const KEEP_TOP_GRADE: u8 = 170;
    pub const OBELISK_RESPAWN: u8 = 171;
    pub const OBELISK_DESTROY: u8 = 172;
    pub const BOTH_MOB: u8 = 173;
    pub const RENAME: u8 = 174;
    pub const GUILD_CREATE: u8 = 175;
    pub const GUILD_DISBAND: u8 = 176;
    pub const GUILD_REQUEST: u8 = 177;
    pub const GUILD_JOIN: u8 = 178;
    pub const GUILD_LEAVE: u8 = 179;
    pub const STAFF_COMMAND: u8 = 180;
    pub const GAMBLE_SIT: u8 = 190;
    pub const GAMBLE_STAND: u8 = 191;
    pub const GAMBLE_MONEY: u8 = 192;
    pub const GAMBLE_SLOT: u8 = 193;
    pub const GAMBLE_CARD: u8 = 194;
    pub const KILL_STEP: u8 = 195;
    pub const DEATH_STEP: u8 = 196;
    pub const SEQUENCE_DROP: u8 = 198;
    pub const GUILD_BANK_IN: u8 = 200;
    pub const GUILD_BANK_OUT: u8 = 201;
    pub const GUILD_NPC_UPGRADE: u8 = 203;
    pub const GUILD_BATTLE_POINTS: u8 = 204;
    pub const GUILD_ETIN: u8 = 205;
    pub const PARTY_ENTER: u8 = 210;
    pub const PARTY_LEAVE: u8 = 211;
    pub const LAPISIAN_ADD: u8 = 212;
    pub const ITEM_COMPOSE: u8 = 213;
    pub const ITEM_CONSIDER: u8 = 214;
    pub const ITEM_REMAKE: u8 = 215;
    pub const MARKET_REGISTER: u8 = 216;
    pub const MARKET_CANCEL: u8 = 217;
    pub const MARKET_BUY: u8 = 218;
    pub const MARKET_BID: u8 = 219;
    pub const MARKET_ITEM_GET: u8 = 220;
    pub const MARKET_MONEY_GET: u8 = 221;
    pub const MARKET_SOLD: u8 = 222;
    pub const MARKET_UNSOLD: u8 = 223;
    pub const MARKET_BID_SELLER: u8 = 224;
    pub const MARKET_WON: u8 = 225;
    pub const MARKET_REFUND: u8 = 226;
    pub const MAP_MOVE: u8 = 232;
}

/// The columns of the header of section 2 that a record fills: the account, the character, its level and experience, the map and
/// the position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub user_id: u32,
    pub user_name: String,
    pub character_id: u32,
    pub character_name: String,
    pub level: u16,
    pub exp: u32,
    pub map: u16,
    pub position: [f32; 3],
}

impl Header {
    /// `CUser::SetGameLogMain`: the character as it is now, on `map` at `position`.
    pub fn of(character: &PlayCharacter, map: u16, position: [f32; 3]) -> Self {
        Self {
            user_id: character.account_id,
            user_name: character.account_name.clone(),
            character_id: character.id,
            character_name: character.name.clone(),
            level: character.level,
            exp: character.exp,
            map,
            position,
        }
    }

    /// The records without user columns (`0x0A01` to `0x0A04`).
    pub fn none() -> Self {
        Self::default()
    }

    /// The records that fill only the map and the position (`0x0A02` to `0x0A04`).
    pub fn map_only(map: u16, position: [f32; 3]) -> Self {
        Self { map, position, ..Self::default() }
    }

    /// The records that fill only a character's id and name (`0x0A06`, `0x0A08`).
    pub fn character_only(character_id: u32, character_name: &str) -> Self {
        Self { character_id, character_name: character_name.to_owned(), ..Self::default() }
    }
}

/// An item as the item records name it (`TP_GAMELOG_ITEM`, section 2): the unique id and making time are those of the database row as
/// the write that made the record left it (deviation 4); `None` where no row exists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemPart {
    pub uid: Option<i64>,
    pub item_id: u32,
    pub name: String,
    pub gems: [u8; 6],
    pub craft_name: Vec<u8>,
    /// Microseconds since 1970.
    pub made_at: Option<i64>,
}

impl ItemPart {
    /// `G`: `"%d,%d,%d,%d,%d,%d (Option:%s)"`, the six gem bytes and the craft name cut at 20 characters.
    pub fn gems_text(&self) -> String {
        let [a, b, c, d, e, f] = self.gems;
        let craft = &self.craft_name[..self.craft_name.len().min(crate::backend::CRAFT_NAME_MAX)];
        format!("{a},{b},{c},{d},{e},{f} (Option:{})", String::from_utf8_lossy(craft))
    }

    /// `C`: the making time as `YYYY-MM-DD hh:mm:ss` in the server's local time (`ConvTimeToString`).
    pub fn made_text(&self) -> Option<String> {
        time_text(self.made_at?)
    }

    /// `"G-C"`, the text of the records that name the item by both.
    fn gems_made(&self) -> String {
        format!("{}-{}", self.gems_text(), self.made_text().unwrap_or_default())
    }
}

/// A time in microseconds since 1970 as `YYYY-MM-DD hh:mm:ss` in the server's local time (`ConvTimeToString`).
pub fn time_text(micros: i64) -> Option<String> {
    use chrono::TimeZone;
    let when = chrono::Local.timestamp_micros(micros).single()?;
    Some(when.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// One `game_log` row: the header, the moment, the action, `Value1` to `Value10` and `Text1` to `Text4`.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub header: Header,
    pub at: SystemTime,
    pub action: u8,
    pub values: [Option<i64>; 10],
    pub texts: [Option<String>; 4],
}

impl Row {
    fn new(header: Header, action: u8) -> Self {
        Self { header, at: SystemTime::now(), action, values: [None; 10], texts: [None, None, None, None] }
    }

    /// `Value`*n* (1 to 10).
    fn v(mut self, n: usize, value: impl Into<i64>) -> Self {
        self.values[n - 1] = Some(value.into());
        self
    }

    /// `Value`*n* from an option (NULL when there is none).
    fn vo(mut self, n: usize, value: Option<i64>) -> Self {
        self.values[n - 1] = value;
        self
    }

    /// `Text`*n* (1 to 4), cut at the column's 100 bytes.
    fn t(mut self, n: usize, text: impl AsRef<str>) -> Self {
        self.texts[n - 1] = Some(cut(text.as_ref()));
        self
    }

    /// The item block: `Value1` the unique id, `Value2` the item id, `Text1` the name.
    fn item(self, item: &ItemPart) -> Self {
        self.vo(1, item.uid).v(2, item.item_id).t(1, &item.name)
    }

    /// `Text`*n* from an option.
    fn to(mut self, n: usize, text: Option<String>) -> Self {
        self.texts[n - 1] = text.map(|t| cut(&t));
        self
    }
}

/// A text cut at 100 bytes, on a character boundary.
fn cut(text: &str) -> String {
    cut_at(text, TEXT_MAX)
}

/// A text cut at `max` bytes, on a character boundary.
fn cut_at(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// One `chat_log` row (section 3.5).
#[derive(Clone, Debug, PartialEq)]
pub struct ChatRow {
    pub user_id: u32,
    pub character_id: u32,
    pub chat_type: u8,
    pub target: Option<String>,
    pub text: String,
    pub map: u16,
    pub at: SystemTime,
}

/// What the writer takes from the queue.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Entry {
    Action(Row),
    Chat(ChatRow),
}

/// The names the texts carry (items, monsters, skills), by their definitions' ids; a missing one is written empty.
#[derive(Clone, Debug, Default)]
pub struct Names {
    /// Item type and type id.
    pub items: HashMap<(u8, u8), String>,
    pub mobs: HashMap<u16, String>,
    pub skills: HashMap<u16, String>,
}

#[derive(Clone)]
enum Sink {
    Off,
    Queue(mpsc::Sender<Entry>),
    Memory(Arc<Mutex<Vec<Entry>>>),
}

/// The log, held by the game's `Context` and cloned freely.
#[derive(Clone)]
pub struct GameLog {
    sink: Sink,
    chat: bool,
    names: Arc<Names>,
    /// Records dropped because the queue was full, and when that last made a warning.
    dropped: Arc<AtomicU64>,
    warned: Arc<Mutex<Option<Instant>>>,
    /// Entries queued and not yet written (`flush`).
    pending: Arc<AtomicU64>,
}

impl Default for GameLog {
    fn default() -> Self {
        Self::off()
    }
}

impl GameLog {
    fn with(sink: Sink, chat: bool) -> Self {
        Self { sink, chat, names: Arc::default(), dropped: Arc::default(), warned: Arc::default(), pending: Arc::default() }
    }

    /// No log: every call does nothing (`log` off, and the tests that do not look).
    pub fn off() -> Self {
        Self::with(Sink::Off, false)
    }

    /// A sink for tests: the entries land in the vector, in order. `chat` as the setting.
    pub fn memory(chat: bool) -> (Self, Arc<Mutex<Vec<Entry>>>) {
        let entries = Arc::new(Mutex::new(Vec::new()));
        (Self::with(Sink::Memory(entries.clone()), chat), entries)
    }

    /// A queue of `capacity` entries and its receiving end, for `spawn` and for tests of a full queue.
    pub fn queue(capacity: usize, chat: bool) -> (Self, mpsc::Receiver<Entry>) {
        let (tx, rx) = mpsc::channel(capacity);
        (Self::with(Sink::Queue(tx), chat), rx)
    }

    /// The names the records carry.
    pub fn with_names(mut self, names: Names) -> Self {
        self.names = Arc::new(names);
        self
    }

    /// Starts the writer task (section 7 step 2): `log` is on.
    pub fn spawn(pool: PgPool, config: &Config, names: Names) -> Self {
        let (log, rx) = Self::queue(QUEUE, config.game_chat_log);
        let log = log.with_names(names);
        tokio::spawn(write_loop(pool, config.server_id, config.game_log_days, rx, log.pending.clone()));
        log
    }

    /// Whether anything is recorded.
    pub fn is_on(&self) -> bool {
        !matches!(self.sink, Sink::Off)
    }

    /// The entries a memory sink holds (empty for the other sinks), in order.
    pub fn recorded(&self) -> Vec<Entry> {
        match &self.sink {
            Sink::Memory(entries) => entries.guard().clone(),
            _ => Vec::new(),
        }
    }

    /// The name of an item (empty when unknown).
    pub fn item_name(&self, item_type: u8, type_id: u8) -> &str {
        self.names.items.get(&(item_type, type_id)).map_or("", String::as_str)
    }

    pub fn mob_name(&self, mob_id: u16) -> &str {
        self.names.mobs.get(&mob_id).map_or("", String::as_str)
    }

    pub fn skill_name(&self, skill_id: u16) -> &str {
        self.names.skills.get(&skill_id).map_or("", String::as_str)
    }

    /// An item as the records name it: `unit` as it sits in its slot, with the row the write left (deviation 4).
    pub fn item(&self, unit: &crate::backend::ItemUnit, stored: Option<Stored>) -> ItemPart {
        ItemPart {
            uid: stored.map(|s| s.uid),
            item_id: u32::from(unit.item_type) * 1000 + u32::from(unit.type_id),
            name: self.item_name(unit.item_type, unit.type_id).to_owned(),
            gems: unit.gems,
            craft_name: unit.craft_name.clone(),
            made_at: stored.map(|s| s.made_at),
        }
    }

    /// Records an action.
    pub fn action(&self, row: Row) {
        self.send(Entry::Action(row));
    }

    /// Records a chat line, unless the chat log is off (`chat_log`).
    pub fn chat(&self, row: ChatRow) {
        if self.chat {
            self.send(Entry::Chat(row));
        }
    }

    fn send(&self, entry: Entry) {
        match &self.sink {
            Sink::Off => {}
            Sink::Memory(entries) => entries.guard().push(entry),
            Sink::Queue(tx) => match tx.try_send(entry) {
                Ok(()) => {
                    self.pending.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => self.drop_one(),
            },
        }
    }

    /// A full queue: the record is lost (the original also loses a record whose `EXEC` fails), counted and warned about at most once
    /// a minute.
    fn drop_one(&self) {
        let total = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
        let mut warned = self.warned.guard();
        if warned.is_none_or(|at| at.elapsed() >= WARN_EVERY) {
            *warned = Some(Instant::now());
            tracing::warn!(dropped = total, "the game log queue is full; records are dropped");
        }
    }

    /// How many records were dropped because the queue was full.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Waits, for at most `limit`, until everything queued has been written (shutdown).
    pub async fn flush(&self, limit: Duration) {
        let start = Instant::now();
        while self.pending.load(Ordering::Relaxed) > 0 && start.elapsed() < limit {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// The writer: takes up to 500 entries at a time and writes them; an error loses the batch and warns with its size. With
/// `log_days` set, it deletes the older rows at start and every 24 hours (deviation 5).
async fn write_loop(pool: PgPool, server_id: u8, days: u32, mut rx: mpsc::Receiver<Entry>, pending: Arc<AtomicU64>) {
    let mut purge = tokio::time::interval(PURGE_EVERY);
    let mut batch = Vec::with_capacity(BATCH);
    loop {
        tokio::select! {
            count = rx.recv_many(&mut batch, BATCH) => {
                if count == 0 {
                    break;
                }
                if let Err(error) = crate::pg::write_game_log(&pool, server_id, &batch).await {
                    tracing::warn!(%error, lost = batch.len(), "the game log could not be written; the records are lost");
                }
                pending.fetch_sub(batch.len() as u64, Ordering::Relaxed);
                batch.clear();
            }
            _ = purge.tick(), if days > 0 => match crate::pg::purge_game_log(&pool, days).await {
                Ok(0) => {}
                Ok(rows) => tracing::info!(rows, days, "old game log rows deleted"),
                Err(error) => tracing::warn!(%error, "the game log's old rows could not be deleted"),
            },
        }
    }
}

// ---- 3.1 Character (0x03xx) ----

/// `0x0301`, action 102: a monster killed the player.
pub fn death_by_mob(h: Header, mob_id: u32, skill: Option<u32>, mob_name: &str) -> Row {
    Row::new(h, action::DEATH_BY_MOB).v(1, mob_id).vo(2, skill.map(i64::from)).t(1, mob_name)
}

/// `0x0302`, action 103: the killer's record of a player kill; `counters` are its kills, deaths, victories and defeats after.
pub fn kill_player(h: Header, victim_id: u32, skill: Option<u32>, victim_name: &str, counters: [u32; 4]) -> Row {
    let [kill, death, win, lose] = counters;
    Row::new(h, action::KILL_PLAYER)
        .v(1, victim_id)
        .vo(2, skill.map(i64::from))
        .t(1, victim_name)
        .t(2, format!("kill={kill}, death={death}, win={win}, lose={lose}"))
}

/// `0x0304`, action 105: the player is reborn.
pub fn rebirth(h: Header) -> Row {
    Row::new(h, action::REBIRTH)
}

/// The ways back to life that `0x0305` names (section 3.1): `V2` is the number, `T1` the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revive {
    SelBase = 1,
    SelParty,
    SelItemSmall,
    SelItemBig,
    TimeOver,
    Logout,
}

impl Revive {
    fn name(self) -> &'static str {
        match self {
            Self::SelBase => "sel_base",
            Self::SelParty => "sel_party",
            Self::SelItemSmall => "sel_itemsmall",
            Self::SelItemBig => "sel_itembig",
            Self::TimeOver => "timeover",
            Self::Logout => "logout",
        }
    }
}

/// `0x0305`, action 106: back to life in one of the six ways (`V1` 0).
pub fn revived(h: Header, way: Revive) -> Row {
    Row::new(h, action::REVIVED).v(1, 0).v(2, way as u8).t(1, way.name())
}

/// `0x0305` otherwise: revived by another player's skill.
pub fn revived_by_skill(h: Header, caster_id: u32, skill: u32, caster_name: &str) -> Row {
    Row::new(h, action::REVIVED).v(1, caster_id).v(2, skill).t(1, caster_name)
}

/// What `0x0306` and `0x0307` say of the character: money, the warehouse's money, the account's points (cash), level, skill and stat
/// points and the base stats in the server's order (str, dex, rec, int, wis, luc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub money: u32,
    pub bank: u32,
    pub cash: u32,
    pub level: u16,
    pub skill_points: u16,
    pub stat_points: u16,
    pub stats: [u16; 6],
}

impl Summary {
    fn level_text(&self) -> String {
        format!("level={},skp={},stp={}", self.level, self.skill_points, self.stat_points)
    }

    /// `"s=%d,d=%d,i=%d,w=%d,r=%d,l=%d"`: the original's order (str, dex, int, wis, rec, luc), so by meaning from the server's.
    fn stats_text(&self) -> String {
        let [s, d, r, i, w, l] = self.stats;
        format!("s={s},d={d},i={i},w={w},r={r},l={l}")
    }
}

/// `0x0306`, action 107: the character enters the world. `billing_type` is the premium kind; the billing time has no source here
/// (NULL).
pub fn enter_world(h: Header, billing_type: u8, ip: &str, s: &Summary) -> Row {
    Row::new(h, action::ENTER_WORLD)
        .v(1, billing_type)
        .t(1, ip)
        .t(2, format!("money={}, bank={}, cash={}", s.money, s.bank, s.cash as i32))
        .t(3, s.level_text())
        .t(4, s.stats_text())
}

/// `0x0307`, action 108: the character leaves; `first_money` is the money at entry.
pub fn leave_world(h: Header, s: &Summary, first_money: u32) -> Row {
    Row::new(h, action::LEAVE_WORLD)
        .t(2, format!("money={}, bank={}, cash={}, firstmoney={}", s.money, s.bank, s.cash as i32, first_money))
        .t(3, s.level_text())
        .t(4, s.stats_text())
}

/// `0x0308`, action 146: a level gained (the header carries the new level).
pub fn level_up(h: Header) -> Row {
    Row::new(h, action::LEVEL_UP)
}

/// `0x0309`, action 141: a skill learnt.
pub fn skill_learned(h: Header, skill_id: u32, level: u8, points_left: u16, name: &str) -> Row {
    Row::new(h, action::SKILL_LEARN).v(1, skill_id).v(2, level).v(3, points_left).t(1, name)
}

/// The stats of `0x030A`, in the original's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Str,
    Dex,
    Int,
    Wis,
    Rec,
    Luc,
}

/// `0x030A`, actions 151 to 156: a stat raised to `value` by `put` points.
pub fn stat_raised(h: Header, stat: Stat, value: u16, put: u16) -> Row {
    Row::new(h, action::STAT_BASE + stat as u8).v(1, value).v(2, put)
}

/// `0x030B`, action 195: a kill-count reward step.
pub fn kill_step(h: Header, step: u8, result: u32) -> Row {
    Row::new(h, action::KILL_STEP).v(1, step).v(2, result)
}

/// `0x030C`, action 196: a death-count reward step.
pub fn death_step(h: Header, step: u8, result: u32) -> Row {
    Row::new(h, action::DEATH_STEP).v(1, step).v(2, result)
}

/// `0x030F`, action 232: entering a map (`'Enter'`).
pub fn map_enter(h: Header) -> Row {
    Row::new(h, action::MAP_MOVE).t(1, "Enter")
}

/// `0x0310`, action 232: leaving a map (`'Leave'`).
pub fn map_leave(h: Header) -> Row {
    Row::new(h, action::MAP_MOVE).t(1, "Leave")
}

// ---- 3.2 Items (0x04xx) ----

/// `0x0401`, action 111: an item picked up from the ground, or made (`T2` NULL).
pub fn item_get(h: Header, item: &ItemPart, count: u8) -> Row {
    Row::new(h, action::ITEM_GET).item(item).v(3, count).t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x0401` with `T2` `'ItemCreate'`: a new item made by the server (a box, a quest reward, a staff command). The original's creation
/// record (`0x0205`) is never stored; here a new item always has the record that made it.
pub fn item_created(h: Header, item: &ItemPart, count: u8) -> Row {
    item_get(h, item, count).t(2, "ItemCreate")
}

/// `0x0401` of `ItemRemake`: the new item (`T2` `'Remake'`).
pub fn item_remade_get(h: Header, item: &ItemPart, count: u8) -> Row {
    item_get(h, item, count).t(2, "Remake")
}

/// The bag place text of the records that lose an item.
fn bag_slot(bag: u8, slot: u8) -> String {
    format!("bag={bag},slot={slot}")
}

/// `0x0402`, action 112: an item dropped or removed.
pub fn item_drop(h: Header, item: &ItemPart, bag: u8, slot: u8, count: u8) -> Row {
    Row::new(h, action::ITEM_LOSE)
        .item(item)
        .t(2, item.gems_text())
        .to(3, item.made_text())
        .t(4, format!("bag={bag},slot={slot},Count={count}"))
}

/// `0x0403`, action 113: bought from an NPC shop.
pub fn shop_buy(h: Header, item: &ItemPart, npc_id: u32, count: u8, price: u32, money_after: u32) -> Row {
    Row::new(h, action::ITEM_BUY)
        .item(item)
        .v(3, npc_id)
        .v(4, count)
        .v(5, price)
        .v(6, money_after)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x0404`, action 114: sold to an NPC shop.
#[allow(clippy::too_many_arguments)]
pub fn shop_sell(h: Header, item: &ItemPart, npc_id: Option<u32>, count: u8, price: u32, money_after: u32, bag: u8, slot: u8) -> Row {
    Row::new(h, action::ITEM_SELL)
        .item(item)
        .vo(3, npc_id.map(i64::from))
        .v(4, count)
        .v(5, price)
        .v(6, money_after)
        .t(2, item.gems_text())
        .to(3, item.made_text())
        .t(4, bag_slot(bag, slot))
}

/// `0x0405`, action 115: the receiving side of a trade, or a personal shop's.
pub fn trade_receive(h: Header, item: &ItemPart, other_id: u32, count: u8, money: u32, other_name: &str) -> Row {
    Row::new(h, action::ITEM_TRADE_IN)
        .item(item)
        .v(3, other_id)
        .v(4, count)
        .v(5, money)
        .t(2, other_name)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x0406`, action 116: the giving side of a trade.
#[allow(clippy::too_many_arguments)]
pub fn trade_give(h: Header, item: &ItemPart, other_id: u32, count: u8, money: u32, other_name: &str, bag: u8, slot: u8) -> Row {
    Row::new(h, action::ITEM_TRADE_OUT)
        .item(item)
        .v(3, other_id)
        .v(4, count)
        .v(5, money)
        .t(2, other_name)
        .t(3, item.gems_made())
        .t(4, bag_slot(bag, slot))
}

/// `0x0407`, action 117: dropped at a death (`killer_id` the one who killed).
pub fn death_drop(h: Header, item: &ItemPart, killer_id: u32, killer_name: &str, bag: u8, slot: u8) -> Row {
    Row::new(h, action::ITEM_DEATH_DROP).item(item).v(3, killer_id).t(2, killer_name).t(3, item.gems_made()).t(4, bag_slot(bag, slot))
}

/// `0x0408`, action 118: an item dropped at a death picked up (`dead_id` the player who died).
pub fn death_pickup(h: Header, item: &ItemPart, dead_id: u32, dead_name: &str) -> Row {
    Row::new(h, action::ITEM_DEATH_GET).item(item).v(3, dead_id).t(2, dead_name).t(3, item.gems_text()).to(4, item.made_text())
}

/// The lapis a gem record names: its item id, unique id, name.
fn lapis_columns(row: Row, lapis: Option<&ItemPart>, npc_id: Option<u32>, money: u32, success: bool) -> Row {
    let row = row.vo(5, npc_id.map(i64::from)).v(6, money).v(7, success as u8);
    match lapis {
        Some(lapis) => row.v(3, lapis.item_id).vo(4, lapis.uid).t(2, &lapis.name),
        None => row,
    }
}

/// `0x0409`, action 119: a lapis set into `item` (`lapis` and `npc_id` NULL where unknown).
pub fn gem_add(h: Header, item: &ItemPart, lapis: Option<&ItemPart>, npc_id: Option<u32>, money: u32, success: bool) -> Row {
    lapis_columns(Row::new(h, action::GEM_ADD).item(item), lapis, npc_id, money, success).t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x040A`, action 120: a lapis taken out of `item`.
pub fn gem_remove(h: Header, item: &ItemPart, lapis: Option<&ItemPart>, npc_id: Option<u32>, money: u32, success: bool) -> Row {
    lapis_columns(Row::new(h, action::GEM_REMOVE).item(item), lapis, npc_id, money, success).t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x0426`, action 212: a lapisian added.
pub fn lapisian_add(h: Header, item: &ItemPart, lapis: Option<&ItemPart>, npc_id: Option<u32>, money: u32, success: bool) -> Row {
    lapis_columns(Row::new(h, action::LAPISIAN_ADD).item(item), lapis, npc_id, money, success)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x040B`, action 121: an item into the warehouse.
pub fn bag_to_bank(h: Header, item: &ItemPart, count: u8, bag: u8, slot: u8, bank_slot: u8) -> Row {
    Row::new(h, action::BAG_TO_BANK)
        .item(item)
        .v(3, count)
        .v(4, bag)
        .v(5, slot)
        .v(7, bank_slot)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x040C`, action 122: an item out of the warehouse, with the fee.
pub fn bank_to_bag(h: Header, item: &ItemPart, count: u8, bank_slot: u8, bag: u8, slot: u8, fee: u32) -> Row {
    Row::new(h, action::BANK_TO_BAG)
        .item(item)
        .v(3, count)
        .v(4, bank_slot)
        .v(6, bag)
        .v(7, slot)
        .v(8, fee)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x040D`, action 113: the billing storage's item into the bag.
pub fn billing_take(h: Header, item: &ItemPart, npc_id: Option<u32>, count: u8) -> Row {
    Row::new(h, action::ITEM_BUY)
        .item(item)
        .vo(3, npc_id.map(i64::from))
        .v(4, count)
        .t(2, "Billing")
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x040E`, action 112: an item used up.
pub fn item_used(h: Header, item: &ItemPart, bag: u8, slot: u8) -> Row {
    Row::new(h, action::ITEM_LOSE).item(item).t(2, "use_item").t(3, item.gems_made()).t(4, bag_slot(bag, slot))
}

/// `0x040F`, action 116: a duel's stake handed over.
#[allow(clippy::too_many_arguments)]
pub fn stake_give(h: Header, item: &ItemPart, other_id: u32, count: u8, money: u32, other_name: &str, bag: u8, slot: u8) -> Row {
    trade_give(h, item, other_id, count, money, other_name, bag, slot).t(3, format!("pvp-{}", item.gems_made()))
}

/// `0x0410`, action 113: an item mall purchase.
pub fn mall_buy(h: Header, item: &ItemPart, count: u8) -> Row {
    Row::new(h, action::ITEM_BUY).item(item).v(4, count).t(2, "PointItem").t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x0411`, action 113: an item taken from the item mall storage.
pub fn mall_stored_take(h: Header, item: &ItemPart, count: u8) -> Row {
    Row::new(h, action::ITEM_BUY).item(item).v(4, count).t(2, "StoredPointItem").t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x0412`, action 113: a cloak badge added.
pub fn cloak_add(h: Header, item: &ItemPart, count: u8, money: u32, money_after: u32) -> Row {
    Row::new(h, action::ITEM_BUY)
        .item(item)
        .v(4, count)
        .v(5, money)
        .v(6, money_after)
        .t(2, "Cloak")
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x0413`, action 200: an item into the guild bank.
#[allow(clippy::too_many_arguments)]
pub fn guild_bank_in(h: Header, item: &ItemPart, count: u8, bank_slot: u8, bag: u8, slot: u8, guild_id: u32, guild_name: &str) -> Row {
    Row::new(h, action::GUILD_BANK_IN)
        .item(item)
        .v(3, count)
        .v(4, bank_slot)
        .v(5, bag)
        .v(6, slot)
        .v(7, guild_id)
        .t(2, guild_name)
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x0414`, action 201: an item out of the guild bank.
#[allow(clippy::too_many_arguments)]
pub fn guild_bank_out(h: Header, item: &ItemPart, count: u8, bank_slot: u8, bag: u8, slot: u8, guild_id: u32, guild_name: &str) -> Row {
    let mut row = guild_bank_in(h, item, count, bank_slot, bag, slot, guild_id, guild_name);
    row.action = action::GUILD_BANK_OUT;
    row
}

/// `0x0415`, action 112: an Etin item handed in.
pub fn etin_item(h: Header, item: &ItemPart, bag: u8, slot: u8) -> Row {
    Row::new(h, action::ITEM_LOSE).item(item).t(2, "etin_return").t(3, item.gems_made()).t(4, bag_slot(bag, slot))
}

/// `0x0427`, action 213: items composed; `old_gems` is the item's `G` before, with the old craft name.
pub fn item_compose(h: Header, item: &ItemPart, compose: Option<&ItemPart>, old_gems: &str) -> Row {
    let row = Row::new(h, action::ITEM_COMPOSE).item(item);
    let row = match compose {
        Some(compose) => row.v(3, compose.item_id).vo(4, compose.uid),
        None => row,
    };
    row.t(2, old_gems).t(3, item.gems_text()).to(4, item.made_text())
}

/// `0x0428`, action 214: an item considered (gems judged); `T2` the gems with `(Option:)`.
pub fn item_consider(h: Header, item: &ItemPart) -> Row {
    let [a, b, c, d, e, f] = item.gems;
    Row::new(h, action::ITEM_CONSIDER)
        .item(item)
        .t(2, format!("{a},{b},{c},{d},{e},{f} (Option:)"))
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// `0x0429`, action 215: an item remade; `used` are the three items used up, by unique id.
pub fn item_remake(h: Header, item: &ItemPart, used: [Option<i64>; 3]) -> Row {
    let id = |i: Option<i64>| i.unwrap_or(0);
    Row::new(h, action::ITEM_REMAKE)
        .item(item)
        .t(2, format!("{},{},{}", id(used[0]), id(used[1]), id(used[2])))
        .t(3, item.gems_text())
        .to(4, item.made_text())
}

/// What a market registration records (`0x042A`, section 3.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registered {
    pub market_id: u32,
    pub bag: u8,
    pub slot: u8,
    pub count: u8,
    pub min_bid: u32,
    pub direct: u32,
    pub deposit: u32,
    pub money_after: u32,
    pub hours: u8,
}

/// `0x042A`, action 216: an item put up for auction. `end_date` is the auction's end as `YYYY-MM-DD hh:mm:ss`.
pub fn market_register(h: Header, item: &ItemPart, r: &Registered, end_date: &str) -> Row {
    Row::new(h, action::MARKET_REGISTER)
        .item(item)
        .v(3, r.market_id)
        .v(4, r.bag)
        .v(5, r.slot)
        .v(6, r.count)
        .v(7, r.min_bid)
        .v(8, r.direct)
        .v(9, r.deposit)
        .v(10, r.money_after)
        .t(2, item.gems_text())
        .to(3, item.made_text())
        .t(4, format!("Term={}H, EndDate={end_date}", r.hours))
}

/// `T2` the gems, `T3` the making time: the tail the market records share.
fn market_item(row: Row, item: &ItemPart) -> Row {
    row.t(2, item.gems_text()).to(3, item.made_text())
}

/// `0x042B`, action 217: an auction cancelled.
pub fn market_cancel(h: Header, item: &ItemPart, market_id: u32, bidder_id: u32, bidder_name: &str) -> Row {
    market_item(Row::new(h, action::MARKET_CANCEL).item(item).v(3, market_id).v(4, bidder_id), item).t(4, bidder_name)
}

/// `0x042C`, action 218: bought at the buy-now price.
pub fn market_buy(h: Header, item: &ItemPart, market_id: u32, price: u32, money_after: u32, seller_id: u32, seller_name: &str) -> Row {
    market_item(Row::new(h, action::MARKET_BUY).item(item).v(3, market_id).v(4, price).v(5, money_after).v(6, seller_id), item)
        .t(4, seller_name)
}

/// `0x042D`, action 219: a bid; `outbid_id` is the char it outbid (0 for none).
#[allow(clippy::too_many_arguments)]
pub fn market_bid(
    h: Header,
    item: &ItemPart,
    market_id: u32,
    bid: u32,
    money_after: u32,
    seller_id: u32,
    outbid_id: u32,
    seller_name: &str,
) -> Row {
    market_item(
        Row::new(h, action::MARKET_BID).item(item).v(3, market_id).v(4, bid).v(5, money_after).v(6, seller_id).v(7, outbid_id),
        item,
    )
    .t(4, seller_name)
}

/// `0x042E`, action 220: an item collected from the auction house.
pub fn market_item_get(h: Header, item: &ItemPart, market_id: u32, bag: u8, slot: u8, count: u8) -> Row {
    market_item(Row::new(h, action::MARKET_ITEM_GET).item(item).v(3, market_id).v(4, bag).v(5, slot).v(6, count), item)
}

/// `0x042F`, action 221: gold collected (header only; `V1` and `V2` 0).
pub fn market_money_get(h: Header, market_id: u32, money_id: u32, money: u32, deposit: u32, returned: u32, money_after: u32) -> Row {
    Row::new(h, action::MARKET_MONEY_GET)
        .v(1, 0)
        .v(2, 0)
        .v(3, market_id)
        .v(4, money_id)
        .v(5, money)
        .v(6, deposit)
        .v(7, returned)
        .v(8, money_after)
}

// ---- 3.3 Quests (0x05xx) ----

/// `0x0501`, action 131: a quest started. The NPC's name has no source here (NULL).
pub fn quest_start(h: Header, quest_id: u16, npc_id: u32, quest_name: &str) -> Row {
    Row::new(h, action::QUEST_START).v(1, quest_id).v(2, npc_id).t(1, quest_name)
}

/// `0x0502`, action 133: a quest ended; `reward` the reward item's id and name and its count.
pub fn quest_end(h: Header, quest_id: u16, success: bool, money: u32, reward: Option<(u32, &str, u8)>, quest_name: &str) -> Row {
    let row = Row::new(h, action::QUEST_END).v(1, quest_id).v(2, success as u8).v(3, money).t(1, quest_name);
    match reward {
        Some((item_id, name, count)) => row.v(4, item_id).v(5, count).t(2, name),
        None => row.v(4, 0).v(5, 0),
    }
}

// ---- 3.4 Money (0x08xx) ----

/// `0x0801`, action 161: gold picked up.
pub fn money_get(h: Header, amount: u32, money_after: u32) -> Row {
    Row::new(h, action::MONEY_GET).v(1, amount).v(2, money_after)
}

/// `0x0803`, action 163: gold received in a trade; `other_ip` the other's address when known.
pub fn money_receive(h: Header, amount: u32, money_after: u32, other_id: u32, other_name: &str, other_ip: Option<&str>) -> Row {
    Row::new(h, action::MONEY_IN).v(1, amount).v(2, money_after).v(3, other_id).t(1, other_name).to(2, other_ip.map(str::to_owned))
}

/// `0x0804`, action 164: gold given in a trade.
pub fn money_give(h: Header, amount: u32, money_after: u32, other_id: u32, other_name: &str) -> Row {
    Row::new(h, action::MONEY_OUT).v(1, amount).v(2, money_after).v(3, other_id).t(1, other_name)
}

/// `0x0805`, action 164: duel stakes handed over (`T3` `'pvp'`).
pub fn stake_money(h: Header, amount: u32, money_after: u32, other_id: u32, other_name: &str) -> Row {
    money_give(h, amount, money_after, other_id, other_name).t(3, "pvp")
}

// ---- 3.5 Chat (0x09xx) ----

/// A chat line (`0x0901`, `0x0902`). `target` is the whisper's. The text is kept as typed.
pub fn chat_line(h: &Header, chat_type: u8, target: Option<&str>, text: &str) -> ChatRow {
    ChatRow {
        user_id: h.user_id,
        character_id: h.character_id,
        chat_type,
        target: target.map(str::to_owned),
        text: cut_at(text, CHAT_MAX),
        map: h.map,
        at: SystemTime::now(),
    }
}

// ---- 3.6 Keep, monsters and the market (0x0Axx) ----

/// `0x0A01`, action 170: a country's kill count reached its top grade (no header).
pub fn keep_top_grade(country: u8) -> Row {
    Row::new(Header::none(), action::KEEP_TOP_GRADE).v(1, country)
}

/// `0x0A02`, action 171: an obelisk rose again for `country`.
pub fn obelisk_respawn(h: Header, country: u8, mob_name: &str) -> Row {
    Row::new(h, action::OBELISK_RESPAWN).v(1, country).t(1, mob_name)
}

/// `0x0A03`, action 172: an obelisk fell; `next` is the country that holds it next.
pub fn obelisk_destroy(h: Header, country: u8, next: u8, mob_name: &str) -> Row {
    Row::new(h, action::OBELISK_DESTROY).v(1, country).v(2, next).t(1, mob_name)
}

/// The actions of `0x0A04` (`T2`, section 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobAction {
    Attack = 1,
    Damage,
    Heal,
    Death,
    DebuffAdd,
    DebuffRemove,
    ItemDrop,
    MoneyDrop,
}

impl MobAction {
    fn name(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Damage => "damage",
            Self::Heal => "heal",
            Self::Death => "death",
            Self::DebuffAdd => "debuffadd",
            Self::DebuffRemove => "debuffremove",
            Self::ItemDrop => "itemdrop",
            Self::MoneyDrop => "moneydrop",
        }
    }
}

/// `0x0A04`, action 173: a monster's action (a Lua-AI monster or a boss group's). `player` and `skill` are NULL where there are none.
#[allow(clippy::too_many_arguments)]
pub fn both_mob(
    h: Header,
    hp: u32,
    damage: u32,
    mob_id: u32,
    mob_name: &str,
    what: MobAction,
    player: Option<&str>,
    skill: Option<&str>,
) -> Row {
    Row::new(h, action::BOTH_MOB)
        .v(1, hp)
        .v(2, damage)
        .v(3, mob_id)
        .t(1, mob_name)
        .t(2, what.name())
        .to(3, player.map(str::to_owned))
        .to(4, skill.map(str::to_owned))
}

/// `0x0A05`, action 174: a rename (the header carries the old name).
pub fn rename(h: Header, old_name: &str, new_name: &str) -> Row {
    let character_id = h.character_id;
    Row::new(h, action::RENAME).v(1, character_id).t(1, old_name).t(2, new_name)
}

/// `0x0A06`, action 198: a sequence item dropped for the character in the header (`drop_id` the ground object's id, NULL while the
/// item is not on the ground yet).
pub fn sequence_drop(h: Header, item: &ItemPart, mob_id: u32, mob_name: &str, drop_id: Option<u32>) -> Row {
    Row::new(h, action::SEQUENCE_DROP)
        .vo(1, item.uid)
        .v(2, item.item_id)
        .v(3, mob_id)
        .vo(4, drop_id.map(i64::from))
        .t(1, &item.name)
        .t(2, mob_name)
        .to(3, item.made_text())
}

/// `0x0A08`, action 222: the seller's record of a sale (header: char id and name only).
pub fn market_sold(seller_id: u32, seller_name: &str, item: &ItemPart, m: &Sale) -> Row {
    Row::new(Header::character_only(seller_id, seller_name), action::MARKET_SOLD)
        .item_values(item)
        .v(3, m.market_id)
        .v(4, m.money_id)
        .v(5, m.money)
        .v(6, m.deposit)
        .v(7, m.returned)
        .v(8, m.buyer_id)
        .t(1, &item.name)
        .t(2, item.gems_text())
        .to(3, item.made_text())
        .t(4, &m.buyer_name)
}

/// What `0x0A08` adds to the item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sale {
    pub market_id: u32,
    pub money_id: u32,
    pub money: u32,
    pub deposit: u32,
    pub returned: u32,
    pub buyer_id: u32,
    pub buyer_name: String,
}

impl Row {
    /// `Value1` and `Value2` of the item without its name.
    fn item_values(self, item: &ItemPart) -> Self {
        self.vo(1, item.uid).v(2, item.item_id)
    }
}

/// The tail of the market's keep records: `T1` the name, `T2` the gems, `T3` the making time.
fn keep_item(row: Row, item: &ItemPart) -> Row {
    row.item_values(item).t(1, &item.name).t(2, item.gems_text()).to(3, item.made_text())
}

/// `0x0A09`, action 223: unsold, or recalled by a GM (`how` `'TimeOut'` or `"GM(<name>)"`).
pub fn market_unsold(item: &ItemPart, market_id: u32, bidder_id: u32, how: &str) -> Row {
    keep_item(Row::new(Header::none(), action::MARKET_UNSOLD).v(3, market_id).v(4, bidder_id), item).t(4, how)
}

/// `0x0A0A`, action 224: the seller's record of a bid.
pub fn market_bid_seller(item: &ItemPart, market_id: u32, bidder_id: u32, bidder_name: &str) -> Row {
    keep_item(Row::new(Header::none(), action::MARKET_BID_SELLER).v(3, market_id).v(4, bidder_id), item).t(4, bidder_name)
}

/// `0x0A0B`, action 225: won by the highest bid.
pub fn market_won(item: &ItemPart, market_id: u32, seller_id: u32, seller_name: &str) -> Row {
    keep_item(Row::new(Header::none(), action::MARKET_WON).v(3, market_id).v(4, seller_id), item).t(4, seller_name)
}

/// `0x0A0C`, action 226: a bidder outbid or refunded; `reason` is the original's reason number.
pub fn market_refund(item: &ItemPart, market_id: u32, money_id: u32, reason: u8, seller_id: u32, seller_name: &str) -> Row {
    keep_item(Row::new(Header::none(), action::MARKET_REFUND).v(3, market_id).v(4, money_id).v(5, reason).v(6, seller_id), item)
        .t(4, seller_name)
}

// ---- 3.7 Guilds (0x0Bxx) ----

/// A guild record with `V1` the guild's id and `T1` its name.
fn guild_row(h: Header, action: u8, guild_id: u32, guild_name: &str) -> Row {
    Row::new(h, action).v(1, guild_id).t(1, guild_name)
}

/// `0x0B01`, action 175: a guild created.
pub fn guild_create(h: Header, guild_id: u32, name: &str) -> Row {
    guild_row(h, action::GUILD_CREATE, guild_id, name)
}

/// `0x0B02`, action 176: a guild disbanded.
pub fn guild_disband(h: Header, guild_id: u32, name: &str) -> Row {
    guild_row(h, action::GUILD_DISBAND, guild_id, name)
}

/// `0x0B03`, action 177: a join request.
pub fn guild_request(h: Header, guild_id: u32, name: &str) -> Row {
    guild_row(h, action::GUILD_REQUEST, guild_id, name)
}

/// `0x0B04`, action 178: a member joined.
pub fn guild_join(h: Header, guild_id: u32, name: &str) -> Row {
    guild_row(h, action::GUILD_JOIN, guild_id, name)
}

/// How a member left (`V2` of action 179, section 3.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// `0x0B05`.
    Left = 0,
    /// `0x0B06`: expelled (the original never stores it, deviation 3).
    Expelled = 1,
    /// `0x0B07`: the member's character was deleted (the same).
    Deleted = 2,
}

/// `0x0B05` to `0x0B07`, action 179.
pub fn guild_leave(h: Header, guild_id: u32, name: &str, how: Leave) -> Row {
    guild_row(h, action::GUILD_LEAVE, guild_id, name).v(2, how as u8)
}

/// The names of `0x0B09`'s `T2`: the NPC kind (the low byte) and the shop type (the high byte).
fn npc_kind(word: u16) -> String {
    const KINDS: [&str; 10] = ["NpcInit", "Shop", "GateKeeper", "Smith", "Priest", "Gamble", "Warehouse", "Story", "Guard", "Animal"];
    const SHOPS: [&str; 10] = ["", "Weapon", "Defence", "General", "Potion", "Job Product", "Vehicle", "Cloak", "Emblem", "Bank"];
    let kind = KINDS.get(usize::from(word & 0xff)).copied().unwrap_or("");
    let shop = SHOPS.get(usize::from(word >> 8)).copied().unwrap_or("");
    format!("{kind} {shop}")
}

/// `0x0B09`, action 203: a house NPC upgraded; `npc` is the NPC type word, `level` its new level.
pub fn guild_npc_upgrade(h: Header, guild_id: u32, name: &str, npc: u16, level: u8, etin: u32, keep_etin: u32) -> Row {
    guild_row(h, action::GUILD_NPC_UPGRADE, guild_id, name).v(2, npc).v(3, level).v(4, etin).v(5, keep_etin).t(2, npc_kind(npc))
}

/// `0x0B0A`, action 205: Etin handed in; `etin` the guild's Etin, `returned` what was returned.
pub fn guild_etin(h: Header, guild_id: u32, name: &str, etin: u32, returned: u32) -> Row {
    guild_row(h, action::GUILD_ETIN, guild_id, name).v(2, etin).v(3, returned)
}

/// `0x0B0B`, action 204: battle points; `got` the points won, then the battle total and the total.
pub fn guild_battle_points(h: Header, guild_id: u32, name: &str, got: u32, battle_total: u32, total: u32) -> Row {
    guild_row(h, action::GUILD_BATTLE_POINTS, guild_id, name).v(2, got).v(3, battle_total).v(4, total)
}

// ---- 3.8 Staff (0x0Cxx) ----

/// `0x0C01` to `0x0C03`, action 180: a staff command with `level` the staff level, `command` its name, `target` and `detail` where it
/// has them.
pub fn staff_command(h: Header, level: u8, command: &str, target: Option<&str>, detail: Option<&str>) -> Row {
    Row::new(h, action::STAFF_COMMAND).v(1, level).t(1, command).to(2, target.map(str::to_owned)).to(3, detail.map(str::to_owned))
}

// ---- 3.9 Mini-games (0x0Dxx; `docs/original/mini-game.md` section 7) ----

/// `0x0D01`, action 190 (`CMiniGame::GameEnter`): V1 the money, V2 the table + 1, V3 the table's bet. With the map and position,
/// level and experience 0 (the caller's header).
pub fn gamble_sit(h: Header, money: u32, table: u8, bet: u32) -> Row {
    Row::new(h, action::GAMBLE_SIT).v(1, money).v(2, i64::from(table) + 1).v(3, bet)
}

/// `0x0D02`, action 191 (`GameExit`): V1 the money.
pub fn gamble_stand(h: Header, money: u32) -> Row {
    Row::new(h, action::GAMBLE_STAND).v(1, money)
}

/// `0x0D03`, action 192 (`SendGameLogChgMoney`): V1 the money, V2 the change (signed).
pub fn gamble_money(h: Header, money: u32, change: i64) -> Row {
    Row::new(h, action::GAMBLE_MONEY).v(1, money).v(2, change)
}

/// `0x0D04`, action 193 (`SendGameLogSlotRun`): V1 the lines bet, V2 the bet, V3 the lines won, V4 the winnings.
pub fn gamble_slot(h: Header, lines: u8, bet: u32, won: u8, winnings: u32) -> Row {
    Row::new(h, action::GAMBLE_SLOT).v(1, lines).v(2, bet).v(3, won).v(4, winnings)
}

/// `0x0D05`, action 194 (`SendGameLogCardRun`): V1 the round, V2 1 won or 0 lost, V3 the stake (or what came back).
pub fn gamble_card(h: Header, round: u8, won: bool, amount: u32) -> Row {
    Row::new(h, action::GAMBLE_CARD).v(1, round).v(2, u8::from(won)).v(3, amount)
}

// ---- 3.10 Duels (0x0Exx) ----

fn duel_row(h: Header, action: u8, opponent_id: u32, opponent_name: &str) -> Row {
    Row::new(h, action).v(1, opponent_id).t(1, opponent_name)
}

/// `0x0E01`, action 123: the winner's record.
pub fn duel_win(h: Header, opponent_id: u32, opponent_name: &str) -> Row {
    duel_row(h, action::DUEL_WIN, opponent_id, opponent_name)
}

/// `0x0E02`, action 124: the loser's record.
pub fn duel_lose(h: Header, opponent_id: u32, opponent_name: &str) -> Row {
    duel_row(h, action::DUEL_LOSE, opponent_id, opponent_name)
}

/// `0x0E03`, action 125: a duel cancelled, with the reason.
pub fn duel_cancel(h: Header, opponent_id: u32, opponent_name: &str, reason: u8) -> Row {
    duel_row(h, action::DUEL_CANCEL, opponent_id, opponent_name).v(2, reason)
}

// ---- 3.11 Parties (0x0Fxx) ----

/// `0x0F01`, action 210: joined a party (`V1` the party's index).
pub fn party_enter(h: Header, party: u32) -> Row {
    Row::new(h, action::PARTY_ENTER).v(1, party)
}

/// `0x0F02`, action 211: left, was expelled from, or saw the end of a party.
pub fn party_leave(h: Header, party: u32) -> Row {
    Row::new(h, action::PARTY_LEAVE).v(1, party)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h() -> Header {
        Header {
            user_id: 7,
            user_name: "acct".into(),
            character_id: 42,
            character_name: "Hero".into(),
            level: 30,
            exp: 1234,
            map: 5,
            position: [1.0, 2.0, 3.0],
        }
    }

    fn sword() -> ItemPart {
        // 2020-01-02 03:04:05 UTC is not local-time safe, so the making time is built from the local clock.
        use chrono::TimeZone;
        let made = chrono::Local.with_ymd_and_hms(2020, 1, 2, 3, 4, 5).unwrap().timestamp_micros();
        ItemPart {
            uid: Some(900),
            item_id: 1001,
            name: "Sword".into(),
            gems: [1, 2, 3, 4, 5, 6],
            craft_name: b"abcdefghijklmnopqrstuvwxyz".to_vec(),
            made_at: Some(made),
        }
    }

    fn values(row: &Row) -> Vec<(usize, i64)> {
        row.values.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i + 1, v))).collect()
    }

    fn texts(row: &Row) -> Vec<(usize, &str)> {
        row.texts.iter().enumerate().filter_map(|(i, t)| t.as_deref().map(|t| (i + 1, t))).collect()
    }

    #[test]
    fn the_item_texts() {
        let item = sword();
        assert_eq!(item.gems_text(), "1,2,3,4,5,6 (Option:abcdefghijklmnopqrst)", "the craft name is cut at 20");
        assert_eq!(item.made_text().as_deref(), Some("2020-01-02 03:04:05"));
        assert_eq!(ItemPart::default().made_text(), None);
    }

    #[test]
    fn character_records_follow_section_3_1() {
        let row = death_by_mob(h(), 5, Some(0), "Wolf");
        assert_eq!((row.action, values(&row), texts(&row)), (102, vec![(1, 5), (2, 0)], vec![(1, "Wolf")]));
        let row = kill_player(h(), 9, Some(3), "Foe", [4, 3, 2, 1]);
        assert_eq!(row.action, 103);
        assert_eq!(texts(&row), vec![(1, "Foe"), (2, "kill=4, death=3, win=2, lose=1")]);
        // The ways back to life: V1 0, V2 the way, T1 its name.
        let row = revived(h(), Revive::SelItemBig);
        assert_eq!((row.action, values(&row), texts(&row)), (106, vec![(1, 0), (2, 4)], vec![(1, "sel_itembig")]));
        let row = revived(h(), Revive::Logout);
        assert_eq!((values(&row), texts(&row)), (vec![(1, 0), (2, 6)], vec![(1, "logout")]));
        let row = revived_by_skill(h(), 11, 77, "Priest");
        assert_eq!((values(&row), texts(&row)), (vec![(1, 11), (2, 77)], vec![(1, "Priest")]));
        // The stats are written by meaning: the server holds str, dex, rec, int, wis, luc.
        let s = Summary { money: 10, bank: 0, cash: 5, level: 30, skill_points: 2, stat_points: 3, stats: [11, 12, 13, 14, 15, 16] };
        let row = enter_world(h(), 1, "10.0.0.1", &s);
        assert_eq!(row.action, 107);
        assert_eq!(values(&row), vec![(1, 1)]);
        assert_eq!(
            texts(&row),
            vec![(1, "10.0.0.1"), (2, "money=10, bank=0, cash=5"), (3, "level=30,skp=2,stp=3"), (4, "s=11,d=12,i=14,w=15,r=13,l=16"),]
        );
        let row = leave_world(h(), &s, 8);
        assert_eq!((row.action, row.texts[0].clone()), (108, None));
        assert_eq!(row.texts[1].as_deref(), Some("money=10, bank=0, cash=5, firstmoney=8"));
        assert_eq!(map_enter(h()).texts[0].as_deref(), Some("Enter"));
        assert_eq!((map_leave(h()).action, map_leave(h()).texts[0].clone()), (232, Some("Leave".to_owned())));
        assert_eq!(level_up(h()).action, 146);
        let row = skill_learned(h(), 21, 2, 4, "Slash");
        assert_eq!((row.action, values(&row), texts(&row)), (141, vec![(1, 21), (2, 2), (3, 4)], vec![(1, "Slash")]));
    }

    #[test]
    fn stats_raised_use_the_actions_151_to_156() {
        let actions: Vec<u8> = [Stat::Str, Stat::Dex, Stat::Int, Stat::Wis, Stat::Rec, Stat::Luc]
            .into_iter()
            .map(|stat| stat_raised(h(), stat, 50, 2).action)
            .collect();
        assert_eq!(actions, vec![151, 152, 153, 154, 155, 156]);
        assert_eq!(values(&stat_raised(h(), Stat::Rec, 50, 2)), vec![(1, 50), (2, 2)]);
    }

    #[test]
    fn item_records_follow_section_3_2() {
        let item = sword();
        let made = item.made_text().unwrap();
        let gems = item.gems_text();
        // A pick-up: V3 the count, T3 G, T4 C.
        let row = item_get(h(), &item, 3);
        assert_eq!(row.action, 111);
        assert_eq!(values(&row), vec![(1, 900), (2, 1001), (3, 3)]);
        assert_eq!(texts(&row), vec![(1, "Sword"), (3, gems.as_str()), (4, made.as_str())]);
        // A shop buy: NPC, count, price, money after.
        let row = shop_buy(h(), &item, 8, 1, 500, 9500);
        assert_eq!((row.action, values(&row)), (113, vec![(1, 900), (2, 1001), (3, 8), (4, 1), (5, 500), (6, 9500)]));
        // The giving side names G-C and the place; the receiving side G and C.
        let row = trade_give(h(), &item, 3, 1, 0, "Friend", 1, 4);
        assert_eq!(row.action, 116);
        assert_eq!(texts(&row), vec![(1, "Sword"), (2, "Friend"), (3, format!("{gems}-{made}").as_str()), (4, "bag=1,slot=4")]);
        let row = trade_receive(h(), &item, 3, 1, 0, "Friend");
        assert_eq!((row.action, row.texts[3].clone()), (115, Some(made.clone())));
        // A death drop picked up names the dead player.
        let row = death_pickup(h(), &item, 77, "Dead");
        assert_eq!((row.action, values(&row)), (118, vec![(1, 900), (2, 1001), (3, 77)]));
        let row = bag_to_bank(h(), &item, 2, 1, 3, 130);
        assert_eq!((row.action, values(&row)), (121, vec![(1, 900), (2, 1001), (3, 2), (4, 1), (5, 3), (7, 130)]));
        let row = bank_to_bag(h(), &item, 2, 130, 1, 3, 50);
        assert_eq!(values(&row), vec![(1, 900), (2, 1001), (3, 2), (4, 130), (6, 1), (7, 3), (8, 50)]);
        // The lapis records name the lapis in V3 and V4 and its name in T2.
        let lapis = ItemPart { uid: Some(55), item_id: 25001, name: "Lapis".into(), ..Default::default() };
        let row = gem_add(h(), &item, Some(&lapis), Some(9), 1000, true);
        assert_eq!((row.action, values(&row)), (119, vec![(1, 900), (2, 1001), (3, 25001), (4, 55), (5, 9), (6, 1000), (7, 1)]));
        assert_eq!(row.texts[1].as_deref(), Some("Lapis"));
        assert_eq!(gem_remove(h(), &item, Some(&lapis), Some(9), 1000, false).action, 120);
        // A failed extraction names no lapis: V3, V4 and T2 stay NULL.
        let row = gem_remove(h(), &item, None, None, 1000, false);
        assert_eq!((values(&row), row.texts[1].clone()), (vec![(1, 900), (2, 1001), (6, 1000), (7, 0)], None));
        assert_eq!(lapisian_add(h(), &item, Some(&lapis), Some(9), 1000, true).action, 212);
        // The mall and the billing storage use the same action with their own T2.
        assert_eq!(mall_buy(h(), &item, 2).texts[1].as_deref(), Some("PointItem"));
        assert_eq!(mall_stored_take(h(), &item, 2).texts[1].as_deref(), Some("StoredPointItem"));
        let row = billing_take(h(), &item, Some(4), 1);
        assert_eq!((row.action, row.texts[1].clone()), (113, Some("Billing".to_owned())));
        let row = stake_give(h(), &item, 3, 1, 0, "Foe", 1, 4);
        assert_eq!((row.action, row.texts[2].clone()), (116, Some(format!("pvp-{gems}-{made}"))));
        assert_eq!(item_used(h(), &item, 1, 2).texts[1].as_deref(), Some("use_item"));
        // No row, no ids.
        let none = ItemPart { uid: None, made_at: None, ..item };
        let row = item_get(h(), &none, 1);
        assert_eq!((row.values[0], row.texts[3].clone()), (None, None));
    }

    #[test]
    fn money_and_quest_records() {
        let row = money_get(h(), 100, 600);
        assert_eq!((row.action, values(&row)), (161, vec![(1, 100), (2, 600)]));
        let row = money_receive(h(), 50, 650, 3, "Friend", Some("1.2.3.4"));
        assert_eq!((row.action, values(&row), texts(&row)), (163, vec![(1, 50), (2, 650), (3, 3)], vec![(1, "Friend"), (2, "1.2.3.4")]));
        assert_eq!(money_give(h(), 50, 650, 3, "Friend").action, 164);
        assert_eq!(stake_money(h(), 50, 650, 3, "Foe").texts[2].as_deref(), Some("pvp"));
        let row = quest_start(h(), 12, 4, "Wolves");
        assert_eq!((row.action, values(&row)), (131, vec![(1, 12), (2, 4)]));
        let row = quest_end(h(), 12, true, 300, Some((1001, "Sword", 1)), "Wolves");
        assert_eq!(
            (row.action, values(&row), texts(&row)),
            (133, vec![(1, 12), (2, 1), (3, 300), (4, 1001), (5, 1)], vec![(1, "Wolves"), (2, "Sword")])
        );
    }

    #[test]
    fn chat_and_keep_records() {
        let line = chat_line(&h(), 2, Some("O'Neil"), "it's me");
        assert_eq!((line.user_id, line.character_id, line.chat_type, line.map), (7, 42, 2, 5));
        assert_eq!((line.target.as_deref(), line.text.as_str()), (Some("O'Neil"), "it's me"));
        let row = keep_top_grade(1);
        assert_eq!((row.action, row.header.clone(), values(&row)), (170, Header::none(), vec![(1, 1)]));
        let row = obelisk_destroy(Header::map_only(5, [1.0, 2.0, 3.0]), 0, 1, "Obelisk");
        assert_eq!((row.action, values(&row), row.header.character_id, row.header.map), (172, vec![(1, 0), (2, 1)], 0, 5));
        // 0x0A04: the action's name goes in T2, the player and the skill in T3 and T4.
        let row = both_mob(Header::none(), 900, 40, 321, "Boss", MobAction::DebuffRemove, Some("Hero"), None);
        assert_eq!(
            (row.action, values(&row), texts(&row)),
            (173, vec![(1, 900), (2, 40), (3, 321)], vec![(1, "Boss"), (2, "debuffremove"), (3, "Hero")])
        );
        assert_eq!(texts(&both_mob(Header::none(), 0, 0, 1, "M", MobAction::MoneyDrop, None, Some("Skill")))[1], (2, "moneydrop"));
        let row = rename(h(), "Old", "New");
        assert_eq!((row.action, values(&row), texts(&row)), (174, vec![(1, 42)], vec![(1, "Old"), (2, "New")]));
        let item = sword();
        let row = market_sold(
            5,
            "Seller",
            &item,
            &Sale { market_id: 3, money_id: 4, money: 100, deposit: 10, returned: 5, buyer_id: 6, buyer_name: "Buyer".into() },
        );
        assert_eq!((row.action, row.header.character_id, row.header.user_id), (222, 5, 0));
        assert_eq!(values(&row), vec![(1, 900), (2, 1001), (3, 3), (4, 4), (5, 100), (6, 10), (7, 5), (8, 6)]);
        assert_eq!(row.texts[3].as_deref(), Some("Buyer"));
    }

    #[test]
    fn guild_records_follow_section_3_7() {
        let row = guild_leave(h(), 8, "Guild", Leave::Expelled);
        assert_eq!((row.action, values(&row), texts(&row)), (179, vec![(1, 8), (2, 1)], vec![(1, "Guild")]));
        assert_eq!(values(&guild_leave(h(), 8, "Guild", Leave::Deleted)), vec![(1, 8), (2, 2)]);
        assert_eq!(values(&guild_leave(h(), 8, "Guild", Leave::Left)), vec![(1, 8), (2, 0)]);
        // The NPC word: the low byte the kind, the high byte the shop type.
        let row = guild_npc_upgrade(h(), 8, "Guild", 0x0201, 3, 100, 50);
        assert_eq!((row.action, values(&row)), (203, vec![(1, 8), (2, 0x0201), (3, 3), (4, 100), (5, 50)]));
        assert_eq!(row.texts[1].as_deref(), Some("Shop Defence"));
        assert_eq!(guild_npc_upgrade(h(), 8, "G", 0x0008, 1, 0, 0).texts[1].as_deref(), Some("Guard "));
        assert_eq!(guild_etin(h(), 8, "G", 10, 5).action, 205);
        let row = guild_battle_points(h(), 8, "G", 3, 30, 300);
        assert_eq!((row.action, values(&row)), (204, vec![(1, 8), (2, 3), (3, 30), (4, 300)]));
        assert_eq!(
            [
                guild_create(h(), 1, "G").action,
                guild_disband(h(), 1, "G").action,
                guild_request(h(), 1, "G").action,
                guild_join(h(), 1, "G").action
            ],
            [175, 176, 177, 178]
        );
    }

    #[test]
    fn gambling_house_records() {
        let row = gamble_sit(h(), 5000, 2, 500);
        assert_eq!((row.action, values(&row)), (190, vec![(1, 5000), (2, 3), (3, 500)]));
        assert_eq!((gamble_stand(h(), 70).action, values(&gamble_stand(h(), 70))), (191, vec![(1, 70)]));
        assert_eq!(values(&gamble_money(h(), 4500, -500)), vec![(1, 4500), (2, -500)]);
        assert_eq!(gamble_money(h(), 1, 1).action, 192);
        let row = gamble_slot(h(), 3, 300, 1, 440);
        assert_eq!((row.action, values(&row)), (193, vec![(1, 3), (2, 300), (3, 1), (4, 440)]));
        let row = gamble_card(h(), 2, false, 80);
        assert_eq!((row.action, values(&row)), (194, vec![(1, 2), (2, 0), (3, 80)]));
        assert_eq!(values(&gamble_card(h(), 1, true, 200)), vec![(1, 1), (2, 1), (3, 200)]);
    }

    #[test]
    fn staff_duel_and_party_records() {
        let row = staff_command(h(), 3, "FA0A", Some("Target"), Some("detail"));
        assert_eq!((row.action, values(&row), texts(&row)), (180, vec![(1, 3)], vec![(1, "FA0A"), (2, "Target"), (3, "detail")]));
        let row = staff_command(h(), 3, "FA0A", None, None);
        assert_eq!(texts(&row), vec![(1, "FA0A")]);
        let row = duel_win(h(), 9, "Foe");
        assert_eq!((row.action, values(&row), texts(&row)), (123, vec![(1, 9)], vec![(1, "Foe")]));
        assert_eq!(duel_lose(h(), 9, "Foe").action, 124);
        let row = duel_cancel(h(), 9, "Foe", 2);
        assert_eq!((row.action, values(&row)), (125, vec![(1, 9), (2, 2)]));
        assert_eq!((party_enter(h(), 5).action, party_leave(h(), 5).action, values(&party_leave(h(), 5))), (210, 211, vec![(1, 5)]));
    }

    #[test]
    fn texts_are_cut_at_100_bytes() {
        let long = "é".repeat(80);
        let row = death_by_mob(h(), 1, Some(0), &long);
        let text = row.texts[0].as_ref().unwrap();
        assert!(text.len() <= 100 && text.chars().all(|c| c == 'é'), "cut on a character boundary");
    }

    #[test]
    fn a_full_queue_drops_and_counts() {
        let (log, mut rx) = GameLog::queue(2, true);
        for i in 0..5 {
            log.action(level_up(Header { character_id: i, ..h() }));
        }
        assert_eq!(log.dropped(), 3);
        assert!(matches!(rx.try_recv(), Ok(Entry::Action(_))) && matches!(rx.try_recv(), Ok(Entry::Action(_))) && rx.try_recv().is_err());
    }

    #[test]
    fn chat_off_sends_no_chat_rows() {
        let (log, entries) = GameLog::memory(false);
        log.chat(chat_line(&h(), 1, None, "hello"));
        log.action(level_up(h()));
        assert!(matches!(entries.guard()[..], [Entry::Action(_)]));
        let (log, entries) = GameLog::memory(true);
        log.chat(chat_line(&h(), 1, None, "hello"));
        assert!(matches!(entries.guard()[..], [Entry::Chat(_)]));
        GameLog::off().action(level_up(h()));
    }
}
