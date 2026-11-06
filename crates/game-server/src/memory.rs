//! In-memory backend for tests. Follows the same rules as the PostgreSQL one, without expiry times
//! (a deletion request is due at once when `delete_due` is set).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

use crate::sync::Relock;
use anyhow::{Result, bail};
use shaiya_crypto::SessionKeys;

use crate::backend::{
    AdminStore, Body, Bought, Changed, CharacterStore, CharacterSummary, Claim, Claimed, CreateOutcome, Faction, FriendStore, FriendWrite,
    GiftNotice, GiftOrder, GiftOutcome, GuildCreated, GuildStore, GuildWrite, Handed, Handover, Heartbeat, ItemStore, ItemUnit, Kept, Mall,
    MarketOp, MarketStore, MarketWrite, MarketWritten, Marks, NewCharacter, NewGuild, NewItem, PlayCharacter, PointLogRow, PointStore,
    Premium, Purchase, QuestEnd, QuestStore, QuickSlot, RenameOutcome, SavePoint, SavedQuest, SessionGate, Skill, SkillStore, Storage,
    Stored, StoredAuction, StoredFriend, StoredGuild, StoredItemRow, StoredMember, StoredMoneyRow, StoredPointItem, StoredQuestion,
};
use crate::buffs::SavedApplied;
use crate::combat::Progress;
use crate::inventory::{BANK, SlotChange};
use shaiya_proto::world::GUILD_BANK;

#[derive(Clone, Debug)]
pub struct StoredSession {
    pub identity: [u8; 16],
    pub keys: SessionKeys,
    pub login_ip: IpAddr,
    pub server_id: Option<u8>,
    pub in_game: bool,
    pub privilege: u8,
    /// When the login made it (`Claimed::created`).
    pub created: i64,
}

#[derive(Clone, Debug)]
struct StoredCharacter {
    user_id: u32,
    server_id: u8,
    summary: CharacterSummary,
    play: PlayCharacter,
    delete_pending: bool,
    deleted: bool,
    /// Renames allowed (`characters.rename_count`).
    rename_count: u8,
    /// The renames made: (old name, new name).
    renames: Vec<(String, String)>,
}

/// A guild as stored: (id, server, name, country, remark, live).
type GuildRow = (u32, u8, String, u8, String, bool);

#[derive(Default)]
pub struct MemoryBackend {
    sessions: Mutex<HashMap<u32, StoredSession>>,
    factions: Mutex<HashMap<(u32, u8), Faction>>,
    characters: Mutex<Vec<StoredCharacter>>,
    /// Last heartbeat: (server, players).
    pub last_heartbeat: Mutex<Option<(u8, u16)>>,
    /// The monsters' object id counter as last saved (`None` until the first save, which reads as 1).
    pub mob_id: Mutex<Option<u32>>,
    /// The factions' kill counts as last saved.
    pub kill_counts: Mutex<[u32; 2]>,
    /// What the next `reload` loads (a test sets it): the monsters and drop tables, or the error to fail with. `None` fails too.
    pub next_live: Mutex<Option<std::result::Result<crate::server::Live, String>>>,
    /// What `reload` reads as the rules' database layers (`rule_layers`); tests set it.
    pub rule_layers: Mutex<crate::rules::RuleSources>,
    /// What `reload` reads as the extensions' scripts (`script_sources`); tests set it with `set_script_sources`.
    pub script_sources: Mutex<crate::script::sources::ScriptSources>,
    /// The scripts' stored values as written: `(extension, character or 0 for the extension's own, key)` → value.
    pub script_values: Mutex<std::collections::BTreeMap<(String, u32, String), serde_json::Value>>,
    /// Batches `write_script_values` received, and a switch to make the next one fail (tests).
    pub script_writes: Mutex<Vec<Vec<crate::script::store::ScriptWrite>>>,
    pub fail_script_writes: std::sync::atomic::AtomicBool,
    /// The obelisks' owners as last saved, by map and number.
    pub obelisk_owners: Mutex<std::collections::BTreeMap<(u16, u16), crate::obelisk::Owner>>,
    /// Treat every pending deletion as due.
    pub delete_due: std::sync::atomic::AtomicBool,
    /// The next `ItemStore::trade` fails with a database error (a test sets it; the flag clears itself).
    pub fail_next_trade: std::sync::atomic::AtomicBool,
    /// When the guild ranking update was left due (unix seconds).
    pub ranking_due: Mutex<Option<i64>>,
    guilds: Mutex<Vec<GuildRow>>,
    /// Each guild's house as stored (`docs/original/guild-house.md`); missing = a new guild's.
    pub guild_houses: Mutex<HashMap<u32, crate::guild_house::House>>,
    /// Each member's and requester's guild and rank, in joining order.
    guild_members: Mutex<Vec<(u32, u32, u8)>>,
    /// Friends (false) and blocked players (true): (block, character, other, memo), in the order they were added.
    friends: Mutex<Vec<(bool, u32, u32, String)>>,
    /// Each account's warehouse on each server: (user, server) → items with bag 100.
    banks: Mutex<HashMap<(u32, u8), Vec<ItemUnit>>>,
    /// Each guild's bank (`docs/original/guild-bank.md`): guild → items with bag 255.
    pub guild_banks: Mutex<HashMap<u32, Vec<ItemUnit>>>,
    /// The accounts the operator commands name (`add_account`): by user id.
    pub accounts: Mutex<HashMap<u32, StoredAccount>>,
    /// The quests a GM closed, by server.
    pub closed_quests: Mutex<std::collections::BTreeSet<(u8, u16)>>,
    /// The event tags: (character, name as typed).
    pub events: Mutex<Vec<(u32, String)>>,
    /// The user questions with their holds and answers, in the order asked.
    pub questions: Mutex<Vec<QuestionRow>>,
    /// The auction house as stored.
    pub market: Mutex<MemoryMarket>,
    /// The item mall's tables (`docs/original/item-mall.md`).
    pub mall: Mutex<MemoryMall>,
    /// The unique id and making time of every item row, which the database rows carry (`ItemUnit` does not).
    ids: Mutex<ItemIds>,
}

/// The rows' ids in the memory store (`docs/original/game-log.md` deviation 4): new items get ids from a counter and the time now,
/// and a row keeps its id while it changes, moves or goes to the warehouse. Keyed by owner, bag and slot.
#[derive(Default)]
struct ItemIds {
    next: i64,
    rows: HashMap<(u64, u8, u8), Stored>,
    /// The item of each auction and each row to collect, by market id.
    market: HashMap<u32, Stored>,
}

/// The owner of a bag's rows: the warehouse's is its account and server, the guild bank's the guild, any other bag's the character.
fn owner_of(bag: u8, character: u32, bank: (u32, u8), guild: u32) -> u64 {
    match bag {
        BANK => (1 << 40) | (u64::from(bank.0) << 8) | u64::from(bank.1),
        GUILD_BANK => (2 << 40) | u64::from(guild),
        _ => u64::from(character),
    }
}

impl ItemIds {
    fn make(&mut self) -> Stored {
        self.next += 1;
        let made_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros() as i64);
        Stored { uid: self.next, made_at }
    }

    /// The row at a place, made when it has none yet (an item the store started with).
    fn at(&mut self, key: (u64, u8, u8)) -> Stored {
        if let Some(&row) = self.rows.get(&key) {
            return row;
        }
        let row = self.make();
        self.rows.insert(key, row);
        row
    }

    /// One change, as the database makes it.
    fn apply(&mut self, owner: &dyn Fn(u8) -> u64, change: &SlotChange) -> Changed {
        let key = |(bag, slot): (u8, u8)| (owner(bag), bag, slot);
        let row = |row| Changed { row: Some(row), swapped: None };
        match change {
            SlotChange::Insert(n) => {
                let made = match n.kept.and_then(|k| k.origin) {
                    Some((uid, made_at)) => Stored { uid, made_at },
                    None => self.make(),
                };
                self.rows.insert(key((n.bag, n.slot)), made);
                row(made)
            }
            SlotChange::SetCount { bag, slot, .. }
            | SlotChange::SetQuality { bag, slot, .. }
            | SlotChange::SetGems { bag, slot, .. }
            | SlotChange::SetCraftName { bag, slot, .. } => row(self.at(key((*bag, *slot)))),
            SlotChange::Remove { bag, slot } => {
                let made = self.at(key((*bag, *slot)));
                self.rows.remove(&key((*bag, *slot)));
                row(made)
            }
            SlotChange::Move { from, to } => {
                let made = self.at(key(*from));
                self.rows.remove(&key(*from));
                self.rows.insert(key(*to), made);
                row(made)
            }
            SlotChange::Swap { a, b } => {
                let (first, second) = (self.at(key(*a)), self.at(key(*b)));
                self.rows.insert(key(*a), second);
                self.rows.insert(key(*b), first);
                Changed { row: Some(second), swapped: Some(first) }
            }
        }
    }
}

/// The item mall as stored: points and premium by account, the log, the storages and the notices.
#[derive(Clone, Debug, Default)]
pub struct MemoryMall {
    pub points: HashMap<u32, u32>,
    pub premium: HashMap<u32, Premium>,
    /// (account, character, kind, row, target name), in the order written.
    pub log: Vec<(u32, u32, u8, PointLogRow, String)>,
    pub stored: HashMap<(u32, u8), Vec<StoredPointItem>>,
    pub billing: HashMap<u32, Vec<StoredPointItem>>,
    pub notices: Vec<(u32, u8, GiftNotice)>,
}

/// The auction house's tables: auctions with their server, the rows to collect, the id counters and the interest lists.
#[derive(Clone, Debug, Default)]
pub struct MemoryMarket {
    pub auctions: std::collections::BTreeMap<u32, (u8, StoredAuction)>,
    pub items: std::collections::BTreeMap<u32, StoredItemRow>,
    pub money: std::collections::BTreeMap<u32, StoredMoneyRow>,
    pub last_auction: u32,
    pub last_money: u32,
    pub interest: HashMap<u32, Vec<u32>>,
}

/// A user question as the memory store keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionRow {
    pub server_id: u8,
    pub question: StoredQuestion,
    pub taken_by: Option<u32>,
    /// `(by, kind, text, to)` once answered.
    pub answered: Option<(u32, u8, Vec<u8>, Option<String>)>,
}

/// An account as the operator commands see it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredAccount {
    pub username: String,
    pub status: i16,
    /// The minutes of the block set by `block_account` (`None` with no end, or not blocked).
    pub blocked_minutes: Option<u16>,
}

impl MemoryBackend {
    /// Sets what the next `reload` reads as the extensions' scripts (`docs/plans/ext-5-lua.md` section 9.2).
    pub fn set_script_sources(&self, sources: crate::script::sources::ScriptSources) {
        *self.script_sources.guard() = sources;
    }

    /// What `CharacterStore::load` puts in `PlayCharacter::mall`.
    fn mall_of(&self, user_id: u32, server_id: u8, character_id: u32) -> Mall {
        let mall = self.mall.guard();
        let mut log: Vec<PointLogRow> = mall.log.iter().filter(|l| l.1 == character_id).map(|l| l.3.clone()).collect();
        log.reverse();
        log.truncate(10);
        let mut stored = mall.stored.get(&(user_id, server_id)).cloned().unwrap_or_default();
        stored.sort_by_key(|r| r.slot);
        let mut billing = mall.billing.get(&user_id).cloned().unwrap_or_default();
        billing.sort_by_key(|r| r.slot);
        Mall {
            points: mall.points.get(&user_id).copied().unwrap_or(0),
            premium: mall.premium.get(&user_id).copied().unwrap_or_default(),
            log,
            stored,
            billing,
        }
    }

    /// What the login server would have written after login and server selection.
    pub fn add_session(&self, user_id: u32, session: StoredSession) {
        self.sessions.guard().insert(user_id, session);
    }

    /// Changes a character's level, as an operator would in the database.
    pub fn set_level(&self, character_id: u32, level: u16) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            c.summary.level = level;
            c.play.level = level;
        }
    }

    /// Changes a character's money, as an operator would in the database.
    pub fn set_money(&self, character_id: u32, money: u32) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            c.play.money = money;
        }
    }

    /// Adds items to a character's bags, as an operator would in the database.
    pub fn give_items(&self, character_id: u32, items: Vec<ItemUnit>) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            c.play.items.extend(items.iter().cloned());
        }
    }

    /// Puts a character on a spot, as an operator would in the database.
    pub fn set_position(&self, character_id: u32, map: u16, position: [f32; 3]) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            c.play.map = map;
            c.play.position = position;
        }
    }

    /// Changes the quality of a character's item, as an operator would in the database.
    pub fn set_quality(&self, character_id: u32, bag: u8, slot: u8, quality: u16) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            for item in c.play.items.iter_mut().filter(|i| (i.bag, i.slot) == (bag, slot)) {
                item.quality = quality;
            }
        }
    }

    /// A character as stored: its money and items.
    pub fn stored(&self, character_id: u32) -> Option<(u32, Vec<ItemUnit>)> {
        let characters = self.characters.guard();
        characters.iter().find(|c| c.summary.id == character_id).map(|c| (c.play.money, c.play.items.clone()))
    }

    /// A character as stored, whole.
    pub fn play(&self, character_id: u32) -> Option<PlayCharacter> {
        self.characters.guard().iter().find(|c| c.summary.id == character_id).map(|c| c.play.clone())
    }

    /// Whether the character is deleted (or fallen).
    pub fn is_deleted(&self, character_id: u32) -> bool {
        self.characters.guard().iter().any(|c| c.summary.id == character_id && c.deleted)
    }

    /// An account's warehouse on a server, as stored.
    pub fn bank(&self, user_id: u32, server_id: u8) -> Vec<ItemUnit> {
        self.banks.guard().get(&(user_id, server_id)).cloned().unwrap_or_default()
    }

    /// Puts items in an account's warehouse, as an operator would in the database.
    pub fn set_bank(&self, user_id: u32, server_id: u8, items: Vec<ItemUnit>) {
        self.banks.guard().insert((user_id, server_id), items);
    }

    /// A character's renames allowed and the renames it made (old name, new name).
    pub fn renames(&self, character_id: u32) -> Option<(u8, Vec<(String, String)>)> {
        let characters = self.characters.guard();
        characters.iter().find(|c| c.summary.id == character_id).map(|c| (c.rename_count, c.renames.clone()))
    }

    /// An account with a name and a status word, as the login server would have stored it.
    pub fn add_account(&self, user_id: u32, username: &str, status: i16) {
        self.accounts.guard().insert(user_id, StoredAccount { username: username.to_owned(), status, blocked_minutes: None });
    }

    pub fn account(&self, user_id: u32) -> Option<StoredAccount> {
        self.accounts.guard().get(&user_id).cloned()
    }

    pub fn session(&self, user_id: u32) -> Option<StoredSession> {
        self.sessions.guard().get(&user_id).cloned()
    }
}

impl SessionGate for MemoryBackend {
    async fn claim(&self, claim: &Claim) -> Result<Option<Claimed>> {
        let mut sessions = self.sessions.guard();
        match sessions.get_mut(&claim.user_id) {
            Some(s)
                if s.identity == claim.identity
                    && s.server_id == Some(claim.server_id)
                    && !s.in_game
                    && claim.ip.is_none_or(|ip| ip == s.login_ip) =>
            {
                s.in_game = true;
                let premium = self.mall.guard().premium.get(&claim.user_id).copied().unwrap_or_default();
                Ok(Some(Claimed { keys: s.keys, privilege: s.privilege, created: s.created, premium }))
            }
            _ => Ok(None),
        }
    }

    async fn claimable(&self, claim: &Claim) -> Result<bool> {
        Ok(self.sessions.guard().get(&claim.user_id).is_some_and(|s| {
            s.identity == claim.identity && s.server_id == Some(claim.server_id) && !s.in_game && claim.ip.is_none_or(|ip| ip == s.login_ip)
        }))
    }

    async fn release(&self, user_id: u32, identity: [u8; 16]) -> Result<()> {
        let mut sessions = self.sessions.guard();
        if sessions.get(&user_id).is_some_and(|s| s.identity == identity) {
            sessions.remove(&user_id);
        }
        Ok(())
    }
}

impl CharacterStore for MemoryBackend {
    async fn faction(&self, user_id: u32, server_id: u8) -> Result<Option<Faction>> {
        Ok(self.factions.guard().get(&(user_id, server_id)).copied())
    }

    async fn choose_faction(&self, user_id: u32, server_id: u8, country: u8) -> Result<bool> {
        if country > 1 {
            bail!("country must be 0 or 1");
        }
        let has_characters = self.characters.guard().iter().any(|c| c.user_id == user_id && c.server_id == server_id && !c.deleted);
        if has_characters {
            return Ok(false);
        }
        self.factions.guard().insert((user_id, server_id), Faction { country, max_grow: 3 });
        Ok(true)
    }

    async fn characters(&self, user_id: u32, server_id: u8) -> Result<Vec<CharacterSummary>> {
        let due = self.delete_due.load(std::sync::atomic::Ordering::Relaxed);
        let mut characters = self.characters.guard();
        let mut removed = false;
        for c in characters.iter_mut().filter(|c| c.user_id == user_id && c.server_id == server_id && !c.deleted) {
            if due && c.delete_pending {
                c.deleted = true;
                removed = true;
            }
        }
        let mut live: Vec<CharacterSummary> = characters
            .iter()
            .filter(|c| c.user_id == user_id && c.server_id == server_id && !c.deleted)
            .map(|c| CharacterSummary { delete_pending: c.delete_pending, rename_allowed: c.rename_count > 0, ..c.summary.clone() })
            .collect();
        if removed && live.is_empty() {
            self.factions.guard().remove(&(user_id, server_id));
        }
        live.sort_by_key(|c| c.slot);
        Ok(live)
    }

    async fn create(&self, n: &NewCharacter) -> Result<CreateOutcome> {
        let mut characters = self.characters.guard();
        if characters.iter().any(|c| !c.deleted && c.summary.name.eq_ignore_ascii_case(n.name.as_bytes())) {
            return Ok(CreateOutcome::NameTaken);
        }
        if characters.iter().any(|c| !c.deleted && c.user_id == n.user_id && c.server_id == n.server_id && c.summary.slot == n.slot) {
            return Ok(CreateOutcome::SlotTaken);
        }
        let id = characters.len() as u32 + 1;
        let mut equipment_type = [0; 8];
        let mut equipment_type_id = [0; 8];
        for item in n.items.iter().filter(|i| i.bag == 0 && i.slot < 8) {
            equipment_type[usize::from(item.slot)] = item.item_type;
            equipment_type_id[usize::from(item.slot)] = item.type_id;
        }
        let items = n
            .items
            .iter()
            .map(|i| ItemUnit {
                bag: i.bag,
                slot: i.slot,
                item_type: i.item_type,
                type_id: i.type_id,
                quality: i.quality,
                count: i.count,
                ..Default::default()
            })
            .collect();
        let play = PlayCharacter {
            id,
            name: n.name.clone(),
            family: n.family,
            grow: n.grow,
            hair: n.hair,
            face: n.face,
            size: n.size,
            job: n.job,
            sex: n.sex,
            level: n.level,
            stat_point: n.stat_point,
            skill_point: n.skill_point,
            stats: n.stats,
            health: n.health,
            map: n.map,
            dir: n.dir,
            position: n.position,
            money: n.money,
            items,
            ..Default::default()
        };
        let summary = CharacterSummary {
            slot: n.slot,
            id,
            level: n.level,
            family: n.family,
            grow: n.grow,
            hair: n.hair,
            face: n.face,
            size: n.size,
            job: n.job,
            sex: n.sex,
            map: n.map,
            stats: n.stats,
            health: n.health,
            equipment_type,
            equipment_type_id,
            name: n.name.clone().into_bytes(),
            ..Default::default()
        };
        characters.push(StoredCharacter {
            user_id: n.user_id,
            server_id: n.server_id,
            summary,
            play,
            delete_pending: false,
            deleted: false,
            rename_count: 0,
            renames: Vec::new(),
        });
        Ok(CreateOutcome::Created(id))
    }

    async fn request_delete(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<bool> {
        let mut characters = self.characters.guard();
        let found = characters
            .iter_mut()
            .find(|c| c.summary.id == character_id && c.user_id == user_id && c.server_id == server_id && !c.deleted && !c.delete_pending);
        Ok(found.map(|c| c.delete_pending = true).is_some())
    }

    async fn restore(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<bool> {
        let mut characters = self.characters.guard();
        let found = characters
            .iter_mut()
            .find(|c| c.summary.id == character_id && c.user_id == user_id && c.server_id == server_id && !c.deleted && c.delete_pending);
        Ok(found.map(|c| c.delete_pending = false).is_some())
    }

    async fn fall(&self, character_id: u32) -> Result<bool> {
        let mut characters = self.characters.guard();
        let found = characters.iter_mut().find(|c| c.summary.id == character_id && !c.deleted);
        Ok(found.map(|c| c.deleted = true).is_some())
    }

    async fn load(&self, user_id: u32, server_id: u8, character_id: u32) -> Result<Option<PlayCharacter>> {
        let characters = self.characters.guard();
        Ok(characters
            .iter()
            .find(|c| c.summary.id == character_id && c.user_id == user_id && c.server_id == server_id && !c.deleted && !c.delete_pending)
            .map(|c| {
                let mut play = c.play.clone();
                play.account_id = user_id;
                play.bank = self.bank(user_id, server_id);
                play.interest = self.market.guard().interest.get(&character_id).cloned().unwrap_or_default();
                play.mall = self.mall_of(user_id, server_id, character_id);
                // The friends and blocked players who are not deleted, with their names and jobs as they are now.
                for (block, _, other, memo) in self.friends.guard().iter().filter(|f| f.1 == character_id) {
                    let Some(o) = characters.iter().find(|o| o.summary.id == *other && !o.deleted && !o.delete_pending) else { continue };
                    let entry = StoredFriend { id: *other, name: o.play.name.clone(), job: o.play.job, memo: memo.clone() };
                    if *block { play.blocks.push(entry) } else { play.friends.push(entry) }
                }
                play
            }))
    }

    async fn save_position(&self, character_id: u32, map: u16, position: [f32; 3], dir: i16, health: [u16; 3]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.health = health;
            c.play.map = map;
            c.play.position = position;
            c.play.dir = dir;
            c.summary.map = map;
        }
        Ok(())
    }

    async fn save_progress(&self, character_id: u32, progress: &Progress, health: [u16; 3]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.level = progress.level;
            c.play.exp = progress.exp;
            c.play.stat_point = progress.stat_point;
            c.play.skill_point = progress.skill_point;
            c.play.stats = progress.stats;
            c.play.health = health;
            c.summary.level = progress.level;
        }
        Ok(())
    }

    async fn save_honor(&self, character_id: u32, honor: u16) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.honor = honor;
        }
        Ok(())
    }

    async fn save_kills(&self, character_id: u32, kills: [u32; 4]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.kills = kills;
        }
        Ok(())
    }

    async fn save_kill_steps(&self, character_id: u32, steps: [u8; 2]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.kill_steps = steps;
        }
        Ok(())
    }

    async fn claim_kill_reward(&self, character_id: u32, stat_point: u16, old_step: u8, new_step: u8) -> Result<bool> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id).filter(|c| c.play.kill_steps[0] == old_step) else {
            return Ok(false);
        };
        c.play.stat_point = stat_point;
        c.play.kill_steps[0] = new_step;
        Ok(true)
    }

    async fn claim_death_reward(&self, character_id: u32, gold: u32, old_step: u8, new_step: u8) -> Result<bool> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id).filter(|c| c.play.kill_steps[1] == old_step) else {
            return Ok(false);
        };
        let Some(money) = c.play.money.checked_add(gold) else { return Ok(false) };
        c.play.money = money;
        c.play.kill_steps[1] = new_step;
        Ok(true)
    }

    async fn save_stat_reset(&self, character_id: u32, progress: &Progress, health: [u16; 3], steps: [u8; 2]) -> Result<()> {
        self.save_progress(character_id, progress, health).await?;
        self.save_kill_steps(character_id, steps).await
    }

    async fn save_point(&self, character_id: u32, point: SavePoint) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.save_points.retain(|p| p.slot != point.slot);
            c.play.save_points.push(point);
            c.play.save_points.sort_by_key(|p| p.slot);
        }
        Ok(())
    }

    async fn change_body(&self, character_id: u32, body: Body, (bag, slot): (u8, u8), had: u8) -> Result<bool> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(false) };
        let Some(i) = c.play.items.iter().position(|i| (i.bag, i.slot) == (bag, slot) && i.count == had) else { return Ok(false) };
        if had > 1 {
            c.play.items[i].count -= 1;
        } else {
            c.play.items.remove(i);
        }
        (c.play.hair, c.play.face, c.play.size, c.play.sex) = (body.hair, body.face, body.size, body.sex);
        (c.summary.hair, c.summary.face, c.summary.size, c.summary.sex) = (body.hair, body.face, body.size, body.sex);
        Ok(true)
    }

    async fn add_rename(&self, character_id: u32, stone: &SlotChange) -> Result<bool> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(false) };
        if c.rename_count == u8::MAX {
            return Ok(false);
        }
        let Some(items) = changed_items(&c.play.items, std::slice::from_ref(stone)) else { return Ok(false) };
        c.play.items = items;
        c.rename_count += 1;
        Ok(true)
    }

    async fn rename(&self, user_id: u32, server_id: u8, character_id: u32, new_name: &str) -> Result<RenameOutcome> {
        let mut characters = self.characters.guard();
        let taken = characters
            .iter()
            .any(|c| !c.deleted && c.summary.id != character_id && c.summary.name.eq_ignore_ascii_case(new_name.as_bytes()));
        let Some(c) =
            characters.iter_mut().find(|c| c.summary.id == character_id && c.user_id == user_id && c.server_id == server_id && !c.deleted)
        else {
            return Ok(RenameOutcome::NotFound);
        };
        if c.rename_count == 0 {
            return Ok(RenameOutcome::NoRename);
        }
        if taken {
            return Ok(RenameOutcome::Taken);
        }
        let old = std::mem::replace(&mut c.play.name, new_name.to_owned());
        c.summary.name = new_name.as_bytes().to_vec();
        c.rename_count -= 1;
        c.renames.push((old.clone(), new_name.to_owned()));
        drop(characters);
        let mut market = self.market.guard();
        for (_, a) in market.auctions.values_mut() {
            if a.seller == character_id {
                a.seller_name = new_name.to_owned();
            }
            if let Some((bidder, name)) = &mut a.bidder
                && *bidder == character_id
            {
                *name = new_name.to_owned();
            }
        }
        Ok(RenameOutcome::Renamed { old })
    }
}

/// The items after `changes`, made on a copy; `None` when a slot is not as a change expects.
fn changed_items(items: &[ItemUnit], changes: &[SlotChange]) -> Option<Vec<ItemUnit>> {
    let mut items = items.to_vec();
    let at = |items: &[ItemUnit], (bag, slot): (u8, u8)| items.iter().position(|i| i.bag == bag && i.slot == slot);
    for change in changes {
        match change {
            SlotChange::Insert(n) => {
                if at(&items, (n.bag, n.slot)).is_some() {
                    return None;
                }
                items.push(n.unit());
            }
            SlotChange::SetCount { bag, slot, count } => {
                let i = at(&items, (*bag, *slot))?;
                items[i].count = *count;
            }
            SlotChange::SetQuality { bag, slot, quality } => {
                let i = at(&items, (*bag, *slot))?;
                items[i].quality = *quality;
            }
            SlotChange::SetGems { bag, slot, gems } => {
                let i = at(&items, (*bag, *slot))?;
                items[i].gems = *gems;
            }
            SlotChange::SetCraftName { bag, slot, craft_name } => {
                let i = at(&items, (*bag, *slot))?;
                items[i].craft_name = craft_name.iter().copied().take_while(|&b| b != 0).collect();
            }
            SlotChange::Remove { bag, slot } => {
                let i = at(&items, (*bag, *slot))?;
                items.remove(i);
            }
            SlotChange::Move { from, to } => {
                let (Some(i), None) = (at(&items, *from), at(&items, *to)) else { return None };
                (items[i].bag, items[i].slot) = *to;
            }
            SlotChange::Swap { a, b } => {
                let (Some(i), Some(j)) = (at(&items, *a), at(&items, *b)) else { return None };
                (items[i].bag, items[i].slot) = *b;
                (items[j].bag, items[j].slot) = *a;
            }
        }
    }
    Some(items)
}

impl ItemStore for MemoryBackend {
    async fn trade(&self, character_id: u32, money: i64, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        if self.fail_next_trade.swap(false, std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!("injected database error");
        }
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Ok(new_money) = u32::try_from(i64::from(c.play.money) + money) else { return Ok(None) };
        // The warehouse's items are bag 100 among the character's for the changes, then go back to the account.
        let mut banks = self.banks.guard();
        let bank = banks.entry((c.user_id, c.server_id)).or_default();
        let both: Vec<ItemUnit> = c.play.items.iter().chain(bank.iter()).cloned().collect();
        let Some(items) = changed_items(&both, changes) else { return Ok(None) };
        let (in_bank, in_bags) = items.into_iter().partition(|i| i.bag == BANK);
        (c.play.items, *bank) = (in_bags, in_bank);
        c.play.money = new_money;
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), 0);
        let mut ids = self.ids.guard();
        Ok(Some(changes.iter().map(|change| ids.apply(&owner, change)).collect()))
    }

    async fn guild_bank(&self, character_id: u32, guild_id: u32, money: i64, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Ok(new_money) = u32::try_from(i64::from(c.play.money) + money) else { return Ok(None) };
        // The guild's items are bag 255 among the character's for the changes, then go back to the guild.
        let mut banks = self.guild_banks.guard();
        let bank = banks.entry(guild_id).or_default();
        let both: Vec<ItemUnit> = c.play.items.iter().chain(bank.iter()).cloned().collect();
        let Some(items) = changed_items(&both, changes) else { return Ok(None) };
        let (in_bank, in_bags) = items.into_iter().partition(|i| i.bag == GUILD_BANK);
        (c.play.items, *bank) = (in_bags, in_bank);
        c.play.money = new_money;
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), guild_id);
        let mut ids = self.ids.guard();
        Ok(Some(changes.iter().map(|change| ids.apply(&owner, change)).collect()))
    }

    async fn guild_return_etin(&self, character_id: u32, guild_id: u32, sum: u32, changes: &[SlotChange]) -> Result<Option<Vec<Changed>>> {
        let member = self.guild_members.guard().iter().any(|m| m.0 == character_id && m.1 == guild_id && m.2 != crate::guild::REQUEST);
        let live = self.guilds.guard().iter().any(|g| g.0 == guild_id && g.5);
        if !member || !live {
            return Ok(None);
        }
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Some(items) = changed_items(&c.play.items, changes) else { return Ok(None) };
        c.play.items = items;
        let mut houses = self.guild_houses.guard();
        let house = houses.entry(guild_id).or_default();
        house.etin = house.etin.saturating_add(sum);
        house.etin_returns += 1;
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), guild_id);
        let mut ids = self.ids.guard();
        Ok(Some(changes.iter().map(|change| ids.apply(&owner, change)).collect()))
    }

    async fn drop_item(&self, character_id: u32, bag: u8, slot: u8, had: u8, count: u8) -> Result<Option<(Kept, Stored)>> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let items = &mut c.play.items;
        let Some(i) = items.iter().position(|i| i.bag == bag && i.slot == slot && i.count == had) else { return Ok(None) };
        let mut ids = self.ids.guard();
        let key = (u64::from(character_id), bag, slot);
        if count < had {
            items[i].count -= count;
            return Ok(Some((Kept::SPLIT, ids.at(key))));
        }
        let item = items.remove(i);
        let row = ids.at(key);
        ids.rows.remove(&key);
        Ok(Some((
            Kept {
                gems: item.gems,
                craft_name: Kept::pad_craft_name(&item.craft_name),
                make_type: 0,
                origin: Some((row.uid, row.made_at)),
            },
            row,
        )))
    }

    async fn take_stakes(&self, character_id: u32, money: u32, stakes: &[(u8, u8, u8, u8)]) -> Result<Option<Vec<(Kept, Stored)>>> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Some(left) = c.play.money.checked_sub(money) else { return Ok(None) };
        // On a copy, so a refused stake leaves everything as it was.
        let mut items = c.play.items.clone();
        let mut ids = self.ids.guard();
        let mut kept = Vec::with_capacity(stakes.len());
        let mut gone = Vec::new();
        for &(bag, slot, had, count) in stakes {
            let Some(i) = items.iter().position(|i| i.bag == bag && i.slot == slot && i.count == had) else { return Ok(None) };
            let item = if count < had {
                items[i].count -= count;
                items[i].clone()
            } else {
                items.remove(i)
            };
            let key = (u64::from(character_id), bag, slot);
            let row = ids.at(key);
            let make_type = if count < had { b'T' } else { 0 };
            let origin = (count >= had).then_some((row.uid, row.made_at));
            if count >= had {
                gone.push(key);
            }
            kept.push((Kept { gems: item.gems, craft_name: Kept::pad_craft_name(&item.craft_name), make_type, origin }, row));
        }
        for key in gone {
            ids.rows.remove(&key);
        }
        (c.play.money, c.play.items) = (left, items);
        Ok(Some(kept))
    }

    async fn make_types(&self, _character_id: u32, places: &[(u8, u8)]) -> Result<Vec<u8>> {
        // The memory store keeps no make types.
        Ok(vec![0; places.len()])
    }

    async fn exchange(&self, money: [(u32, i64); 2], handovers: &[Handover]) -> Result<Option<Vec<Handed>>> {
        let mut characters = self.characters.guard();
        // On copies, so a refused change leaves everything as it was.
        let mut changed: HashMap<u32, (u32, Vec<ItemUnit>)> = HashMap::new();
        let ids = money.iter().map(|&(id, _)| id).chain(handovers.iter().flat_map(|h| [h.giver, h.receiver]));
        for id in ids {
            let Some(c) = characters.iter().find(|c| c.summary.id == id) else { return Ok(None) };
            changed.entry(id).or_insert_with(|| (c.play.money, c.play.items.clone()));
        }
        for (id, amount) in money {
            let entry = changed.get_mut(&id).expect("listed");
            let Ok(new_money) = u32::try_from(i64::from(entry.0) + amount) else { return Ok(None) };
            entry.0 = new_money;
        }
        let mut ids = self.ids.guard();
        let mut rows = Vec::with_capacity(handovers.len());
        for h in handovers {
            let at = |items: &[ItemUnit], (bag, slot): (u8, u8)| items.iter().position(|i| i.bag == bag && i.slot == slot);
            let giver = &mut changed.get_mut(&h.giver).expect("listed").1;
            let Some(i) = at(giver, h.from).filter(|&i| giver[i].count == h.had && h.count <= h.had) else { return Ok(None) };
            let mut item = giver[i].clone();
            let from = (u64::from(h.giver), h.from.0, h.from.1);
            let kept = ids.at(from);
            let handed = if h.count == h.had {
                giver.remove(i);
                ids.rows.remove(&from);
                // The row moves on with its id.
                Handed { receiver: Some(kept), giver: None }
            } else {
                giver[i].count -= h.count;
                Handed { receiver: Some(ids.make()), giver: Some(kept) }
            };
            ids.rows.insert((u64::from(h.receiver), h.to.0, h.to.1), handed.receiver.expect("set"));
            rows.push(handed);
            let receiver = &mut changed.get_mut(&h.receiver).expect("listed").1;
            if at(receiver, h.to).is_some() {
                return Ok(None);
            }
            (item.bag, item.slot, item.count) = (h.to.0, h.to.1, h.count);
            receiver.push(item);
        }
        for c in characters.iter_mut() {
            if let Some((money, items)) = changed.remove(&c.summary.id) {
                (c.play.money, c.play.items) = (money, items);
            }
        }
        Ok(Some(rows))
    }
}

impl SkillStore for MemoryBackend {
    async fn learn(&self, character_id: u32, skill_points: u16, skill: Skill) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.skill_point = skill_points;
            c.play.skills.retain(|s| s.number != skill.number);
            c.play.skills.push(skill);
            c.play.skills.sort_by_key(|s| s.number);
        }
        Ok(())
    }

    async fn reset_skills(&self, character_id: u32, skill_points: u16, free: bool) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.skill_point = skill_points;
            c.play.skills.clear();
            c.play.free_skill_reset_used |= free;
        }
        Ok(())
    }

    async fn save_quick_slots(&self, character_id: u32, slots: &[QuickSlot]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.quick_slots = slots.to_vec();
        }
        Ok(())
    }

    async fn save_cooldowns(&self, character_id: u32, skills: &[Skill]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            for stored in &mut c.play.skills {
                if let Some(s) = skills.iter().find(|s| s.number == stored.number) {
                    stored.delay = s.delay;
                }
            }
        }
        Ok(())
    }

    async fn save_applied(&self, character_id: u32, applied: &[SavedApplied]) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            c.play.applied = applied.to_vec();
        }
        Ok(())
    }
}

impl MemoryBackend {
    /// A character's guild and rank as stored (rank 0: a join request).
    pub fn guild_member(&self, character_id: u32) -> Option<(u32, u8)> {
        self.guild_members.guard().iter().find(|m| m.0 == character_id).map(|m| (m.1, m.2))
    }

    fn joined(&self, character_id: u32) {
        for c in self.characters.guard().iter_mut().filter(|c| c.summary.id == character_id) {
            c.play.guild_joined_secs = Some(0);
        }
    }
}

impl GuildStore for MemoryBackend {
    async fn load_ranking_due(&self) -> Result<Option<i64>> {
        Ok(*self.ranking_due.guard())
    }

    async fn load_guilds(&self, server_id: u8) -> Result<Vec<StoredGuild>> {
        let characters = self.characters.guard();
        let members = self.guild_members.guard();
        let guilds = self.guilds.guard();
        Ok(guilds
            .iter()
            .filter(|g| g.1 == server_id && g.5)
            .map(|g| StoredGuild {
                id: g.0,
                name: g.2.clone(),
                country: g.3,
                remark: g.4.clone(),
                house: self.guild_houses.guard().get(&g.0).cloned().unwrap_or_default(),
                bank: self.guild_banks.guard().get(&g.0).cloned().unwrap_or_default(),
                members: members
                    .iter()
                    .filter(|m| m.1 == g.0)
                    .filter_map(|m| {
                        let c = characters.iter().find(|c| c.summary.id == m.0 && !c.deleted)?;
                        Some(StoredMember { id: m.0, name: c.play.name.clone(), level: c.play.level, job: c.play.job, rank: m.2 })
                    })
                    .collect(),
            })
            .collect())
    }

    async fn create_guild(&self, g: &NewGuild) -> Result<GuildCreated> {
        let id = {
            let mut guilds = self.guilds.guard();
            if guilds.iter().any(|x| x.1 == g.server_id && x.5 && x.2.eq_ignore_ascii_case(&g.name)) {
                return Ok(GuildCreated::NameTaken);
            }
            {
                let mut characters = self.characters.guard();
                let Some(payer) = characters.iter_mut().find(|c| c.summary.id == g.payer && c.play.money >= g.cost) else {
                    return Ok(GuildCreated::NoMoney);
                };
                payer.play.money -= g.cost;
            }
            let id = guilds.len() as u32 + 1;
            guilds.push((id, g.server_id, g.name.clone(), g.country, g.remark.clone(), true));
            id
        };
        let mut members = self.guild_members.guard();
        members.retain(|m| !g.members.iter().any(|&(c, _)| c == m.0));
        members.extend(g.members.iter().map(|&(c, rank)| (c, id, rank)));
        drop(members);
        for &(c, _) in &g.members {
            self.joined(c);
        }
        Ok(GuildCreated::Created(id))
    }

    async fn buy_guild_house(&self, character_id: u32, guild_id: u32, price: u32, keep_etin: u32) -> Result<bool> {
        let master = self.guild_members.guard().iter().any(|m| m.0 == character_id && m.1 == guild_id && m.2 == crate::guild::MASTER);
        let live = self.guilds.guard().iter().any(|g| g.0 == guild_id && g.5);
        let mut houses = self.guild_houses.guard();
        if !master || !live || houses.get(&guild_id).is_some_and(|h| h.bought) {
            return Ok(false);
        }
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(false) };
        let Some(money) = c.play.money.checked_sub(price) else { return Ok(false) };
        c.play.money = money;
        let house = houses.entry(guild_id).or_default();
        (house.bought, house.keep_etin) = (true, keep_etin);
        Ok(true)
    }

    async fn write_guild(&self, write: &GuildWrite) -> Result<()> {
        let mut members = self.guild_members.guard();
        let mut joined = None;
        match write {
            &GuildWrite::Add { guild, character, rank } => {
                members.retain(|m| m.0 != character);
                members.push((character, guild, rank));
                if rank != crate::guild::REQUEST {
                    joined = Some(character);
                }
            }
            &GuildWrite::Accept { character } => {
                for m in members.iter_mut().filter(|m| m.0 == character) {
                    m.2 = crate::guild::NEW_MEMBER;
                }
                joined = Some(character);
            }
            &GuildWrite::Rank { character, rank } => {
                for m in members.iter_mut().filter(|m| m.0 == character) {
                    m.2 = rank;
                }
            }
            &GuildWrite::Remove { character } => members.retain(|m| m.0 != character),
            GuildWrite::Remark { guild, remark } => {
                for g in self.guilds.guard().iter_mut().filter(|g| g.0 == *guild) {
                    g.4 = remark.clone();
                }
            }
            &GuildWrite::Disband { guild } => {
                for g in self.guilds.guard().iter_mut().filter(|g| g.0 == guild) {
                    g.5 = false;
                }
                members.retain(|m| m.1 != guild);
            }
            &GuildWrite::Etin { guild, etin, returns } => {
                let mut houses = self.guild_houses.guard();
                let house = houses.entry(guild).or_default();
                (house.etin, house.etin_returns) = (etin, returns);
            }
            &GuildWrite::Upgrade { guild, slot, key, level, etin, keep_etin } => {
                let mut houses = self.guild_houses.guard();
                let house = houses.entry(guild).or_default();
                house.slots[usize::from(slot)] = (key, u16::from(level));
                (house.etin, house.keep_etin) = (etin, keep_etin);
            }
            &GuildWrite::HouseBought { guild, keep_etin } => {
                let mut houses = self.guild_houses.guard();
                let house = houses.entry(guild).or_default();
                (house.bought, house.keep_etin) = (true, keep_etin);
            }
            GuildWrite::Ranking { rows } => {
                let mut houses = self.guild_houses.guard();
                for r in rows {
                    let house = houses.entry(r.guild).or_default();
                    (house.points, house.etin, house.ranking, house.use_house, house.etin_returns) =
                        (r.points, r.etin, r.ranking, r.use_house, 0);
                }
            }
            &GuildWrite::Points { guild, points } => {
                self.guild_houses.guard().entry(guild).or_default().points = points;
            }
            &GuildWrite::Standing { guild, ranking, use_house } => {
                let mut houses = self.guild_houses.guard();
                let house = houses.entry(guild).or_default();
                (house.ranking, house.use_house) = (ranking, use_house);
            }
            &GuildWrite::BankCount { guild, slot, count } => {
                let mut banks = self.guild_banks.guard();
                for item in banks.entry(guild).or_default().iter_mut().filter(|i| i.slot == slot) {
                    item.count = count;
                }
            }
            &GuildWrite::RankingDue(due) => *self.ranking_due.guard() = due,
        }
        drop(members);
        if let Some(character) = joined {
            self.joined(character);
        }
        Ok(())
    }
}

impl FriendStore for MemoryBackend {
    async fn write_friends(&self, write: &FriendWrite) -> Result<()> {
        let mut rows = self.friends.guard();
        let mut add = |block: bool, character: u32, other: u32| {
            if !rows.iter().any(|r| r.0 == block && r.1 == character && r.2 == other) {
                rows.push((block, character, other, String::new()));
            }
        };
        match write {
            FriendWrite::Friends(pairs) => pairs.iter().for_each(|&(c, f)| add(false, c, f)),
            FriendWrite::Block { character, blocked } => add(true, *character, *blocked),
            FriendWrite::RemoveFriend { character, friend } => rows.retain(|r| (r.0, r.1, r.2) != (false, *character, *friend)),
            FriendWrite::RemoveBlock { character, blocked } => rows.retain(|r| (r.0, r.1, r.2) != (true, *character, *blocked)),
            FriendWrite::FriendMemo { character, friend: other, memo } | FriendWrite::BlockMemo { character, blocked: other, memo } => {
                let block = matches!(write, FriendWrite::BlockMemo { .. });
                if let Some(r) = rows.iter_mut().find(|r| (r.0, r.1, r.2) == (block, *character, *other)) {
                    r.3 = memo.clone();
                }
            }
        }
        Ok(())
    }
}

impl QuestStore for MemoryBackend {
    async fn start_quest(
        &self,
        character_id: u32,
        quest_id: u16,
        seconds_left: Option<u32>,
        changes: &[SlotChange],
    ) -> Result<Option<Vec<Changed>>> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Some(items) = changed_items(&c.play.items, changes) else { return Ok(None) };
        c.play.items = items;
        c.play.quests.retain(|q| q.quest_id != quest_id);
        c.play.quests.push(SavedQuest { quest_id, seconds_left, ..Default::default() });
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), 0);
        let mut ids = self.ids.guard();
        Ok(Some(changes.iter().map(|change| ids.apply(&owner, change)).collect()))
    }

    async fn quest_counts(&self, character_id: u32, quest_id: u16, counts: [u8; 3]) -> Result<()> {
        let mut characters = self.characters.guard();
        let quests = characters.iter_mut().filter(|c| c.summary.id == character_id).flat_map(|c| c.play.quests.iter_mut());
        for q in quests.filter(|q| q.quest_id == quest_id && !q.finished) {
            q.counts = counts;
        }
        Ok(())
    }

    async fn end_quest(&self, character_id: u32, end: &QuestEnd) -> Result<Option<Vec<Changed>>> {
        let mut characters = self.characters.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let Ok(new_money) = u32::try_from(i64::from(c.play.money) + end.money) else { return Ok(None) };
        let Some(items) = changed_items(&c.play.items, &end.changes) else { return Ok(None) };
        c.play.items = items;
        c.play.money = new_money;
        c.play.quests.retain(|q| q.quest_id != end.quest_id);
        if end.keep {
            c.play.quests.push(SavedQuest { quest_id: end.quest_id, finished: true, success: end.success, ..Default::default() });
        }
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), 0);
        let mut ids = self.ids.guard();
        Ok(Some(end.changes.iter().map(|change| ids.apply(&owner, change)).collect()))
    }

    async fn save_quest_times(&self, character_id: u32, times: &[(u16, u32)]) -> Result<()> {
        let mut characters = self.characters.guard();
        let quests = characters.iter_mut().filter(|c| c.summary.id == character_id).flat_map(|c| c.play.quests.iter_mut());
        for q in quests {
            if let Some(&(_, secs)) = times.iter().find(|t| t.0 == q.quest_id && !q.finished) {
                q.seconds_left = Some(secs);
            }
        }
        Ok(())
    }
}

impl Heartbeat for MemoryBackend {
    async fn heartbeat(&self, server_id: u8, players: u16, _open: bool) -> Result<()> {
        *self.last_heartbeat.guard() = Some((server_id, players));
        Ok(())
    }

    async fn load_mob_id(&self, _server_id: u8) -> Result<u32> {
        Ok(self.mob_id.guard().unwrap_or(1))
    }

    async fn save_mob_id(&self, _server_id: u8, id: u32) -> Result<()> {
        *self.mob_id.guard() = Some(id);
        Ok(())
    }

    async fn load_kill_counts(&self, _server_id: u8) -> Result<[u32; 2]> {
        Ok(*self.kill_counts.guard())
    }

    async fn save_kill_counts(&self, _server_id: u8, counts: [u32; 2]) -> Result<()> {
        *self.kill_counts.guard() = counts;
        Ok(())
    }

    async fn load_obelisk_owners(&self, _server_id: u8) -> Result<Vec<crate::obelisk::Owner>> {
        Ok(self.obelisk_owners.guard().values().copied().collect())
    }

    async fn save_obelisk_owner(&self, _server_id: u8, owner: crate::obelisk::Owner) -> Result<()> {
        self.obelisk_owners.guard().insert((owner.map, owner.number), owner);
        Ok(())
    }
}

impl AdminStore for MemoryBackend {
    async fn load_live(&self, _skills: &crate::skills::SkillDefs) -> Result<crate::server::Live> {
        match self.next_live.guard().take() {
            Some(Ok(live)) => Ok(live),
            Some(Err(error)) => bail!(error),
            None => bail!("nothing to load"),
        }
    }

    async fn rule_layers(&self) -> Result<crate::rules::RuleSources> {
        Ok(self.rule_layers.guard().clone())
    }

    async fn script_sources(&self) -> Result<crate::script::sources::ScriptSources> {
        Ok(self.script_sources.guard().clone())
    }

    async fn script_values(&self, character_id: u32) -> Result<Vec<(String, String, serde_json::Value)>> {
        Ok(self
            .script_values
            .guard()
            .iter()
            .filter(|((_, c, _), _)| *c == character_id)
            .map(|((e, _, k), v)| (e.clone(), k.clone(), v.clone()))
            .collect())
    }

    async fn write_script_values(&self, batch: &[crate::script::store::ScriptWrite]) -> Result<()> {
        if self.fail_script_writes.swap(false, std::sync::atomic::Ordering::SeqCst) {
            bail!("the database refused the write");
        }
        let mut values = self.script_values.guard();
        for w in batch {
            let key = (w.extension.clone(), w.character.unwrap_or(0), w.key.clone());
            match &w.value {
                Some(v) => values.insert(key, v.clone()),
                None => values.remove(&key),
            };
        }
        self.script_writes.guard().push(batch.to_vec());
        Ok(())
    }

    async fn set_account_status(&self, user_id: u32, status: i16) -> Result<()> {
        if let Some(a) = self.accounts.guard().get_mut(&user_id) {
            a.status = status;
        }
        Ok(())
    }

    async fn account_id(&self, username: &str) -> Result<Option<(u32, i16)>> {
        let accounts = self.accounts.guard();
        Ok(accounts.iter().find(|(_, a)| a.username.eq_ignore_ascii_case(username)).map(|(&id, a)| (id, a.status)))
    }

    async fn save_marks(&self, character_id: u32, marks: Marks) -> Result<()> {
        let mut characters = self.characters.guard();
        if let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) {
            (c.play.grow, c.play.vg, c.play.cg, c.play.og, c.play.ig) = (marks.grow, marks.vg, marks.cg, marks.og, marks.ig);
        }
        Ok(())
    }

    async fn character_account(&self, name: &str) -> Result<Option<(u32, i16)>> {
        let user = self.characters.guard().iter().find(|c| !c.deleted && c.play.name.eq_ignore_ascii_case(name)).map(|c| c.user_id);
        Ok(user.and_then(|u| self.accounts.guard().get(&u).map(|a| (u, a.status))))
    }

    async fn block_account(&self, user_id: u32, minutes: Option<u16>) -> Result<()> {
        if let Some(a) = self.accounts.guard().get_mut(&user_id) {
            (a.status, a.blocked_minutes) = (2, minutes);
        }
        Ok(())
    }

    async fn lift_block(&self, user_id: u32) -> Result<bool> {
        let mut accounts = self.accounts.guard();
        match accounts.get_mut(&user_id) {
            Some(a) if (2..=6).contains(&a.status) => {
                (a.status, a.blocked_minutes) = (0, None);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn closed_quests(&self, server_id: u8) -> Result<Vec<u16>> {
        Ok(self.closed_quests.guard().iter().filter(|q| q.0 == server_id).map(|q| q.1).collect())
    }

    async fn set_quest_closed(&self, server_id: u8, quest: u16, closed: bool) -> Result<()> {
        let mut quests = self.closed_quests.guard();
        if closed {
            quests.insert((server_id, quest));
        } else {
            quests.remove(&(server_id, quest));
        }
        Ok(())
    }

    async fn character_id(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.characters.guard().iter().find(|c| !c.deleted && c.play.name.eq_ignore_ascii_case(name)).map(|c| c.summary.id))
    }

    async fn add_event(&self, character_id: u32, name: &str) -> Result<bool> {
        let mut events = self.events.guard();
        if events.iter().any(|(c, n)| *c == character_id && n.eq_ignore_ascii_case(name)) {
            return Ok(false);
        }
        events.push((character_id, name.to_owned()));
        Ok(true)
    }

    async fn remove_event(&self, character_id: u32, name: &str) -> Result<bool> {
        let mut events = self.events.guard();
        let before = events.len();
        events.retain(|(c, n)| !(*c == character_id && n.eq_ignore_ascii_case(name)));
        Ok(events.len() < before)
    }

    async fn add_question(&self, server_id: u8, character_id: u32, name: &str, text: &[u8]) -> Result<u32> {
        let mut rows = self.questions.guard();
        let id = rows.last().map_or(1, |r| r.question.id + 1);
        let asked_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let question = StoredQuestion { id, character_id, name: name.to_owned(), text: text.to_vec(), asked_unix };
        rows.push(QuestionRow { server_id, question, taken_by: None, answered: None });
        Ok(id)
    }

    async fn take_question(&self, id: u32, gm: u32) -> Result<()> {
        if let Some(row) = self.questions.guard().iter_mut().find(|r| r.question.id == id && r.answered.is_none()) {
            row.taken_by = Some(gm);
        }
        Ok(())
    }

    async fn release_question(&self, id: u32) -> Result<()> {
        if let Some(row) = self.questions.guard().iter_mut().find(|r| r.question.id == id && r.answered.is_none()) {
            row.taken_by = None;
        }
        Ok(())
    }

    async fn answer_question(&self, id: u32, gm: u32, kind: u8, text: &[u8], to: Option<&str>) -> Result<()> {
        if let Some(row) = self.questions.guard().iter_mut().find(|r| r.question.id == id && r.answered.is_none()) {
            row.answered = Some((gm, kind, text.to_vec(), to.map(str::to_owned)));
        }
        Ok(())
    }

    async fn open_questions(&self, server_id: u8) -> Result<Vec<StoredQuestion>> {
        let mut rows = self.questions.guard();
        let mut open = Vec::new();
        for row in rows.iter_mut().filter(|r| r.server_id == server_id && r.answered.is_none()) {
            row.taken_by = None;
            open.push(row.question.clone());
        }
        Ok(open)
    }
}

impl MarketStore for MemoryBackend {
    async fn load_market(&self, server_id: u8) -> Result<(Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>)> {
        let characters = self.characters.guard();
        let here = |id: u32| characters.iter().any(|c| c.summary.id == id && c.server_id == server_id);
        let market = self.market.guard();
        let auctions = market.auctions.values().filter(|(server, _)| *server == server_id).map(|(_, a)| a.clone()).collect();
        let items = market.items.values().filter(|r| here(r.character)).cloned().collect();
        let money = market.money.values().filter(|r| here(r.character)).cloned().collect();
        Ok((auctions, items, money))
    }

    async fn market(&self, write: &MarketWrite) -> Result<Option<MarketWritten>> {
        let mut characters = self.characters.guard();
        let mut stored = self.market.guard();
        // On copies, so a refused write leaves everything as it was.
        let mut market = stored.clone();
        let mut bags: Option<(usize, u32, Vec<ItemUnit>)> = None;
        if write.money != 0 || write.take.is_some() || write.place.is_some() {
            let Some(i) = characters.iter().position(|c| c.summary.id == write.character) else { return Ok(None) };
            let Ok(money) = u32::try_from(i64::from(characters[i].play.money) + write.money) else { return Ok(None) };
            bags = Some((i, money, characters[i].play.items.clone()));
        }
        let mut written = MarketWritten::default();
        let mut ids = self.ids.guard();
        for op in &write.ops {
            match op {
                MarketOp::NewAuction(a) => {
                    let (Some((bag, slot, had, count)), Some((_, _, items))) = (write.take, bags.as_mut()) else { return Ok(None) };
                    let Some(i) = items.iter().position(|i| i.bag == bag && i.slot == slot && i.count == had) else { return Ok(None) };
                    let key = (u64::from(write.character), bag, slot);
                    let bag_row = ids.at(key);
                    // The whole stack goes with its row; part of it becomes a new row.
                    let auction_row = if count == had {
                        items.remove(i);
                        ids.rows.remove(&key);
                        bag_row
                    } else {
                        items[i].count -= count;
                        ids.make()
                    };
                    written.taken = Some(bag_row);
                    written.ops.push(Some(auction_row));
                    market.last_auction += 1;
                    let id = market.last_auction;
                    let auction = StoredAuction {
                        id,
                        seller: a.seller,
                        seller_name: a.seller_name.clone(),
                        ends_at: a.ends_at,
                        min: a.min,
                        direct: a.direct,
                        period: a.period,
                        deposit: a.deposit,
                        bidder: None,
                        price: a.min,
                        item: a.item.clone(),
                    };
                    market.auctions.insert(id, (a.server_id, auction));
                    ids.market.insert(id, auction_row);
                    written.auction_ids.push(id);
                }
                MarketOp::EndAuction { id, character, result, deadline } => {
                    let Some((_, a)) = market.auctions.remove(id) else { return Ok(None) };
                    written.ops.push(ids.market.get(id).copied());
                    let row = StoredItemRow { market_id: *id, character: *character, result: *result, deadline: *deadline, item: a.item };
                    market.items.insert(*id, row);
                }
                MarketOp::SetBid { id, bidder, name, price } => {
                    let Some((_, a)) = market.auctions.get_mut(id) else { return Ok(None) };
                    written.ops.push(ids.market.get(id).copied());
                    a.bidder = Some((*bidder, name.clone()));
                    a.price = *price;
                }
                MarketOp::NewMoneyRow(m) => {
                    market.last_money += 1;
                    let id = market.last_money;
                    let row = StoredMoneyRow {
                        id,
                        market_id: m.market_id,
                        character: m.character,
                        result: m.result,
                        money: m.money,
                        deposit: m.deposit,
                        return_money: m.return_money,
                        deadline: m.deadline,
                        item: m.item.clone(),
                    };
                    market.money.insert(id, row);
                    written.money_ids.push(id);
                    written.ops.push(None);
                }
                MarketOp::DeleteItemRow(id) => {
                    if market.items.remove(id).is_none() {
                        return Ok(None);
                    }
                    written.ops.push(ids.market.get(id).copied());
                }
                MarketOp::DeleteMoneyRow(id) => {
                    if market.money.remove(id).is_none() {
                        return Ok(None);
                    }
                    written.ops.push(None);
                }
            }
        }
        if let Some((market_id, bag, slot)) = write.place {
            let Some((_, _, items)) = bags.as_mut() else { return Ok(None) };
            if items.iter().any(|i| (i.bag, i.slot) == (bag, slot)) {
                return Ok(None);
            }
            let Some(row) = market.items.remove(&market_id).filter(|r| r.character == write.character) else { return Ok(None) };
            let made = ids.market.remove(&market_id).unwrap_or_else(|| ids.make());
            ids.rows.insert((u64::from(write.character), bag, slot), made);
            written.placed = Some(made);
            let item = row.item;
            items.push(ItemUnit {
                bag,
                slot,
                item_type: item.item_type,
                type_id: item.type_id,
                quality: item.quality,
                gems: item.gems,
                count: item.count,
                craft_name: item.craft_name,
            });
        }
        if let Some((i, money, items)) = bags {
            characters[i].play.money = money;
            characters[i].play.items = items;
        }
        *stored = market;
        Ok(Some(written))
    }

    async fn save_interest(&self, character_id: u32, ids: &[u32]) -> Result<()> {
        self.market.guard().interest.insert(character_id, ids.to_vec());
        Ok(())
    }
}

impl PointStore for MemoryBackend {
    async fn points(&self, user_id: u32) -> Result<u32> {
        Ok(self.mall.guard().points.get(&user_id).copied().unwrap_or(0))
    }

    async fn buy(&self, p: &Purchase) -> Result<Bought> {
        let mut characters = self.characters.guard();
        let mut mall = self.mall.guard();
        let points = mall.points.get(&p.user_id).copied().unwrap_or(0);
        let Some(left) = points.checked_sub(p.price) else { return Ok(Bought::NoPoints) };
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == p.character_id) else { return Ok(Bought::Refused) };
        let changes: Vec<SlotChange> = p.items.iter().map(|i| SlotChange::Insert(*i)).collect();
        let Some(items) = changed_items(&c.play.items, &changes) else { return Ok(Bought::Refused) };
        c.play.items = items;
        let mut ids = self.ids.guard();
        let owner = |bag| owner_of(bag, p.character_id, (c.user_id, c.server_id), 0);
        let rows: Vec<Stored> = changes.iter().filter_map(|change| ids.apply(&owner, change).row).collect();
        mall.points.insert(p.user_id, left);
        let row = PointLogRow { code: p.code.clone(), at: chrono::Utc::now().timestamp(), points: p.price };
        mall.log.push((p.user_id, p.character_id, 1, row, String::new()));
        Ok(Bought::Done(left, rows))
    }

    async fn gift(&self, g: &GiftOrder) -> Result<GiftOutcome> {
        let characters = self.characters.guard();
        let target = characters
            .iter()
            .find(|c| c.server_id == g.server_id && !c.deleted && !c.delete_pending && c.play.name.eq_ignore_ascii_case(&g.target));
        let Some(target_user) = target.map(|c| c.user_id) else { return Ok(GiftOutcome::NoCharacter) };
        if target_user == g.user_id {
            return Ok(GiftOutcome::OwnAccount);
        }
        let mut mall = self.mall.guard();
        let taken: Vec<u8> =
            mall.stored.get(&(target_user, g.server_id)).map(|rows| rows.iter().map(|r| r.slot).collect()).unwrap_or_default();
        let free: Vec<u8> = (0..shaiya_proto::point::STORAGE_SLOTS).filter(|s| !taken.contains(s)).take(g.rows.len()).collect();
        if free.len() < g.rows.len() {
            return Ok(GiftOutcome::StorageFull);
        }
        let points = mall.points.get(&g.user_id).copied().unwrap_or(0);
        let Some(points_left) = points.checked_sub(g.price) else { return Ok(GiftOutcome::NoPoints) };
        mall.points.insert(g.user_id, points_left);
        let rows: Vec<StoredPointItem> =
            free.iter().zip(&g.rows).map(|(&slot, &(item_id, count))| StoredPointItem { slot, item_id, count }).collect();
        mall.stored.entry((target_user, g.server_id)).or_default().extend(rows.iter().copied());
        let row = PointLogRow { code: g.code.clone(), at: chrono::Utc::now().timestamp(), points: g.price };
        mall.log.push((g.user_id, g.character_id, 2, row, g.target.clone()));
        Ok(GiftOutcome::Sent { points_left, target_user, rows })
    }

    async fn add_gift_notice(&self, user_id: u32, server_id: u8, notice: &GiftNotice) -> Result<()> {
        self.mall.guard().notices.push((user_id, server_id, notice.clone()));
        Ok(())
    }

    async fn take_gift_notices(&self, user_id: u32, server_id: u8) -> Result<Vec<GiftNotice>> {
        let mut mall = self.mall.guard();
        let (mine, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut mall.notices).into_iter().partition(|n| n.0 == user_id && n.1 == server_id);
        mall.notices = rest;
        Ok(mine.into_iter().map(|n| n.2).collect())
    }

    async fn take_stored(&self, character_id: u32, storage: Storage, had: StoredPointItem, item: &NewItem) -> Result<Option<Stored>> {
        let mut characters = self.characters.guard();
        let mut mall = self.mall.guard();
        let Some(c) = characters.iter_mut().find(|c| c.summary.id == character_id) else { return Ok(None) };
        let rows = match storage {
            Storage::Mall => mall.stored.entry((c.user_id, c.server_id)).or_default(),
            Storage::Billing => mall.billing.entry(c.user_id).or_default(),
        };
        let Some(at) = rows.iter().position(|r| *r == had) else { return Ok(None) };
        let Some(items) = changed_items(&c.play.items, &[SlotChange::Insert(*item)]) else { return Ok(None) };
        rows.remove(at);
        c.play.items = items;
        let owner = |bag| owner_of(bag, character_id, (c.user_id, c.server_id), 0);
        let row = self.ids.guard().apply(&owner, &SlotChange::Insert(*item)).row;
        Ok(row)
    }
}
