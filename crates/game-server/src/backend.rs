//! What the game server needs from its storage, as small traits. `PgBackend` implements them on
//! PostgreSQL, `MemoryBackend` in memory for tests.

use crate::combat::Progress;
use std::net::IpAddr;

use anyhow::Result;
use shaiya_crypto::SessionKeys;
pub use shaiya_proto::game::CharacterSummary;
pub use shaiya_proto::world::ItemUnit;

pub use crate::buffs::SavedApplied;
use crate::inventory::SlotChange;
pub use crate::skills::{QuickSlot, Skill};

/// The handshake's view of a login session (the `sessions` row the login server wrote).
pub struct Claim {
    pub user_id: u32,
    pub identity: [u8; 16],
    pub server_id: u8,
    /// The connecting address, when it must equal the login address.
    pub ip: Option<IpAddr>,
}

/// What a successful claim hands the game connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claimed {
    /// The AES key and IV negotiated on the login connection.
    pub keys: SessionKeys,
    /// The account's privilege byte (11 for players, 1 to 5 for staff).
    pub privilege: u8,
    /// When the login made the session, in microseconds since the epoch: a newer login's notification ends this connection
    /// (`docs/original/client-extras.md` section 3.2).
    pub created: i64,
    /// The account's premium (`docs/original/item-mall.md` section 7, deviation 4) for `0xB103`.
    pub premium: Premium,
}

/// A premium account: the billing type 1 to 3 (0 none) and its end in unix seconds (`None`: no end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Premium {
    pub kind: u8,
    pub until: Option<i64>,
}

impl Premium {
    /// The billing type in effect at `now` (unix seconds): 0 once it has ended.
    pub fn kind_at(&self, now: i64) -> u8 {
        if self.until.is_some_and(|until| until <= now) { 0 } else { self.kind }
    }
}

pub trait SessionGate: Send + Sync + 'static {
    /// Takes the session for a game connection. Succeeds once: the identity, this server and (if given) the address
    /// must match, the session must not have expired and must not have been used for a game connection already.
    fn claim(&self, claim: &Claim) -> impl Future<Output = Result<Option<Claimed>>> + Send;
    /// Whether `claim` would succeed now, asked without taking the session: the same conditions, no change.
    fn claimable(&self, claim: &Claim) -> impl Future<Output = Result<bool>> + Send;
    /// Ends the session when the game connection closes, unless a newer login has replaced it.
    fn release(&self, user_id: u32, identity: [u8; 16]) -> impl Future<Output = Result<()>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faction {
    pub country: u8,
    pub max_grow: u8,
}

/// Values of a new character, decided by the server (`crate::creation`).
#[derive(Clone, Debug, PartialEq)]
pub struct NewCharacter {
    pub user_id: u32,
    pub server_id: u8,
    pub slot: u8,
    pub name: String,
    pub family: u8,
    pub grow: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    pub level: u16,
    pub stat_point: u16,
    pub skill_point: u16,
    /// str, dex, rec, int, wis, luc
    pub stats: [u16; 6],
    /// hp, mp, sp
    pub health: [u16; 3],
    pub map: u16,
    pub dir: i16,
    pub position: [f32; 3],
    pub money: u32,
    /// Starting items, stored with the character.
    pub items: Vec<NewItem>,
}

/// An item changing hands in a trade: `count` of the `had` at `from` of `giver` goes to `to` of `receiver`; the row itself when it is
/// all of it, otherwise a copy with that count (`docs/original/trade.md` section 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handover {
    pub giver: u32,
    pub from: (u8, u8),
    pub had: u8,
    pub count: u8,
    pub receiver: u32,
    pub to: (u8, u8),
}

/// An item to store: where it goes and what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewItem {
    pub bag: u8,
    pub slot: u8,
    /// `defs.items` id (type * 1000 + type id).
    pub item_id: u32,
    pub item_type: u8,
    pub type_id: u8,
    pub quality: u16,
    pub count: u8,
    /// What an item from the ground brings back with it (`docs/original/item-drop.md` section 5); `None` for a new item.
    pub kept: Option<Kept>,
}

/// What an item keeps of itself while on the ground, beyond type, type id, count and quality: what a new row would not have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub gems: [u8; 6],
    /// The craft name, padded with zeros.
    pub craft_name: [u8; CRAFT_NAME_MAX],
    /// The original's Maketype; 0 for none.
    pub make_type: u8,
    /// The row's unique id and making time (microseconds since 1970) of an item that left whole; `None` makes new ones.
    pub origin: Option<(i64, i64)>,
}

impl NewItem {
    /// The item as it sits in its slot once stored.
    pub fn unit(&self) -> ItemUnit {
        let kept = self.kept.unwrap_or_default();
        ItemUnit {
            bag: self.bag,
            slot: self.slot,
            item_type: self.item_type,
            type_id: self.type_id,
            quality: self.quality,
            gems: kept.gems,
            count: self.count,
            craft_name: kept.craft_name().to_vec(),
        }
    }
}

/// Longest craft name kept (the `craft_name` column).
pub const CRAFT_NAME_MAX: usize = 20;

impl Kept {
    /// A part of a stack dropped: a new item of make type `'U'` (`CUser::ItemDrop`).
    pub const SPLIT: Kept = Kept { gems: [0; 6], craft_name: [0; CRAFT_NAME_MAX], make_type: b'U', origin: None };

    /// The craft name without its padding.
    pub fn craft_name(&self) -> &[u8] {
        let len = self.craft_name.iter().position(|&b| b == 0).unwrap_or(CRAFT_NAME_MAX);
        &self.craft_name[..len]
    }

    /// The craft name padded (cut at `CRAFT_NAME_MAX`).
    pub fn pad_craft_name(name: &[u8]) -> [u8; CRAFT_NAME_MAX] {
        let mut padded = [0; CRAFT_NAME_MAX];
        let len = name.len().min(CRAFT_NAME_MAX);
        padded[..len].copy_from_slice(&name[..len]);
        padded
    }
}

/// The row an item write left for one of its changes (`docs/original/game-log.md` deviation 4): the item's unique id and its making
/// time in microseconds since 1970 (as `Kept::origin`). The game log names every item by them, so a duplicated item shows as a
/// `uid` that appears without the record that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stored {
    pub uid: i64,
    pub made_at: i64,
}

/// What an item write left for one `SlotChange`: the row it inserted, changed, moved or deleted. `swapped` is a swap's other row
/// (`row` is the one now at `a`, `swapped` the one now at `b`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changed {
    pub row: Option<Stored>,
    pub swapped: Option<Stored>,
}

/// What an item write left for one `Handover`: the row the receiver ends up with (the moved row, or the new copy for part of a stack)
/// and the giver's row when it remains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Handed {
    pub receiver: Option<Stored>,
    pub giver: Option<Stored>,
}

/// A saved position: slot 0 to 3 (index 1 to 4 in the packets), map and position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SavePoint {
    pub slot: u8,
    pub map: u16,
    pub position: [f32; 3],
}

/// What a body change sets (`0x0226`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Body {
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub sex: u8,
}

/// A character as entering the world needs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayCharacter {
    pub id: u32,
    pub name: String,
    /// The account's id and name, for the game log's header (`docs/original/game-log.md` section 2); the name is empty in the memory
    /// backend.
    pub account_id: u32,
    pub account_name: String,
    pub family: u8,
    pub grow: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    pub level: u16,
    pub stat_point: u16,
    pub skill_point: u16,
    /// str, dex, rec, int, wis, luc as stored (the base stats; equipment adds to them, `crate::stats`).
    pub stats: [u16; 6],
    /// hp, mp, sp as stored (the maxima are computed, `crate::stats`).
    pub health: [u16; 3],
    pub map: u16,
    pub dir: i16,
    pub position: [f32; 3],
    pub exp: u32,
    pub money: u32,
    /// kills, deaths, victories, defeats (the original's K1 to K4)
    pub kills: [u32; 4],
    /// The original's Hg, sent as honor.
    pub honor: u16,
    /// The original's CG, OG and IG (`/set`, `docs/original/gm-commands.md` section 7.1).
    pub cg: u8,
    pub og: u8,
    pub ig: u8,
    /// The original's Vg: lowers lapis prices by a tenth of it (`docs/original/lapis.md` section 3).
    pub vg: i16,
    /// The free skill reset (`0x0215`) has been used (`docs/original/item-use.md` section 8).
    pub free_skill_reset_used: bool,
    /// The kill and death reward steps taken, 0 to 16 (the original's KillLevel and DeadLevel, `docs/original/pvp.md` section 8).
    pub kill_steps: [u8; 2],
    pub items: Vec<ItemUnit>,
    /// The account's warehouse on this server, bag 100 (`docs/original/warehouse.md`).
    pub bank: Vec<ItemUnit>,
    /// Learnt skills, by number.
    pub skills: Vec<Skill>,
    pub quick_slots: Vec<QuickSlot>,
    /// The skills applied when it was last saved, and how many seconds ago that was (`crate::buffs`).
    pub applied: Vec<SavedApplied>,
    pub offline_secs: u64,
    /// How many seconds ago it last joined a guild (the join penalty, `docs/original/guild.md` section 10); `None` never.
    pub guild_joined_secs: Option<u64>,
    /// Its quests, in progress and finished (`crate::quests`).
    pub quests: Vec<SavedQuest>,
    /// Its friends and blocked players who are not deleted (`docs/original/friends.md` section 11), in the order they were added.
    pub friends: Vec<StoredFriend>,
    pub blocks: Vec<StoredFriend>,
    /// The saved positions, slots 0 to 3 (`docs/original/item-windows.md` section 1).
    pub save_points: Vec<SavePoint>,
    /// The auction house interest list, in order (`docs/original/auction.md` section 2.6).
    pub interest: Vec<u32>,
    /// The account's points, this character's purchase log and the two storages (`docs/original/item-mall.md`).
    pub mall: Mall,
}

/// What the item mall loads with a character: the account's points and premium, this character's last purchases and gifts (newest
/// first, at most 10), the account's item-mall storage on this server and its billing storage.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mall {
    pub points: u32,
    pub premium: Premium,
    pub log: Vec<PointLogRow>,
    pub stored: Vec<StoredPointItem>,
    pub billing: Vec<StoredPointItem>,
}

/// A purchase or gift as logged: the product code, when (unix seconds) and the points used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointLogRow {
    pub code: String,
    pub at: i64,
    pub points: u32,
}

/// A storage row: its slot (0 to 239), the item (`type * 1000 + type id`) and the count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredPointItem {
    pub slot: u8,
    pub item_id: u32,
    pub count: u8,
}

/// A purchase, all together (deviation 1): `price` off the account's points if it has them, the items into the character's empty
/// places, the log row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purchase {
    pub user_id: u32,
    pub character_id: u32,
    pub code: String,
    pub price: u32,
    pub items: Vec<NewItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bought {
    /// The points left, and the rows the items got (`Purchase::items` in order).
    Done(u32, Vec<Stored>),
    NoPoints,
    /// A place was not empty; nothing changed.
    Refused,
}

/// A gift, all together (section 4, deviations 1 and 3): the recipient a live character of this server, not of the buyer's account;
/// `price` off the buyer's points; `rows` (item, count) into the recipient account's lowest free storage slots on this server; the log
/// row with the recipient's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GiftOrder {
    pub user_id: u32,
    pub server_id: u8,
    pub character_id: u32,
    pub target: String,
    pub code: String,
    pub price: u32,
    pub rows: Vec<(u32, u8)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GiftOutcome {
    Sent {
        points_left: u32,
        target_user: u32,
        rows: Vec<StoredPointItem>,
    },
    NoCharacter,
    OwnAccount,
    /// Not enough free storage slots.
    StorageFull,
    NoPoints,
}

/// A gift that came while its account was away (`PointGiftNotify`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GiftNotice {
    pub sender: String,
    pub code: String,
}

/// The item-mall storage (`0x2607`) or the billing storage (`0xB102`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Mall,
    Billing,
}

pub trait PointStore: Send + Sync + 'static {
    /// The account's points.
    fn points(&self, user_id: u32) -> impl Future<Output = Result<u32>> + Send;
    fn buy(&self, purchase: &Purchase) -> impl Future<Output = Result<Bought>> + Send;
    fn gift(&self, gift: &GiftOrder) -> impl Future<Output = Result<GiftOutcome>> + Send;
    fn add_gift_notice(&self, user_id: u32, server_id: u8, notice: &GiftNotice) -> impl Future<Output = Result<()>> + Send;
    /// The account's notices on this server, oldest first, deleted as they are read.
    fn take_gift_notices(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Vec<GiftNotice>>> + Send;
    /// A storage row out and `item` into the character's bags, together. `None`, with nothing changed, when the row is not `had` (its
    /// item and count) or the place is taken; otherwise the row the item got.
    fn take_stored(
        &self,
        character_id: u32,
        storage: Storage,
        had: StoredPointItem,
        item: &NewItem,
    ) -> impl Future<Output = Result<Option<Stored>>> + Send;
}

/// A friend or blocked player as stored, with its current name and job.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoredFriend {
    pub id: u32,
    pub name: String,
    pub job: u8,
    pub memo: String,
}

/// A character's quest as stored (`docs/original/quest.md` section 12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedQuest {
    pub quest_id: u16,
    pub finished: bool,
    /// Finished: done (true) or failed.
    pub success: bool,
    pub counts: [u8; 3],
    /// In progress with a time limit: the seconds left when it was last saved.
    pub seconds_left: Option<u32>,
}

/// A quest ending, all together: the money by `money`, the bags by `changes` (as `ItemStore::trade`), and the quest finished with
/// `success`, or dropped when `keep` is false (a repeatable quest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestEnd {
    pub quest_id: u16,
    pub success: bool,
    pub keep: bool,
    pub money: i64,
    pub changes: Vec<SlotChange>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateOutcome {
    Created(u32),
    /// A live character on any server has the name, or it is still reserved after a recent deletion.
    NameTaken,
    SlotTaken,
}

/// What a rename came to (`docs/original/small-items.md` section 9.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameOutcome {
    Renamed {
        old: String,
    },
    /// The character has no rename left.
    NoRename,
    /// A live character on any server has the name (any case), or it is still reserved after a recent deletion.
    Taken,
    /// No live character of that id on this account and server.
    NotFound,
}

pub trait CharacterStore: Send + Sync + 'static {
    /// The account's faction on this server; `None` until one is chosen.
    fn faction(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Option<Faction>>> + Send;
    /// Records the faction. Refused (false) while the account has characters on this server.
    fn choose_faction(&self, user_id: u32, server_id: u8, country: u8) -> impl Future<Output = Result<bool>> + Send;
    /// Live characters, after removing those whose deletion is due. When that leaves none, the faction is cleared,
    /// as the original did, so the player may choose again.
    fn characters(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Vec<CharacterSummary>>> + Send;
    fn create(&self, character: &NewCharacter) -> impl Future<Output = Result<CreateOutcome>> + Send;
    /// Marks a live character of this account for deletion. False if there is no such character.
    fn request_delete(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<bool>> + Send;
    /// Cancels a pending deletion. False if there is nothing to restore.
    fn restore(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<bool>> + Send;
    /// An ultimate-mode character's death without a rune (`CUser::KillSoul`'s `0x0406`, `docs/original/item-flags.md` deviation 2):
    /// out of the list as a deletion, and marked fallen so that it can be restored. False if it was already gone.
    fn fall(&self, character_id: u32) -> impl Future<Output = Result<bool>> + Send;
    /// A live character of this account on this server with its items, for entering the world. `None` if there is no such
    /// character or its deletion is pending.
    fn load(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<Option<PlayCharacter>>> + Send;
    /// Stores where a character is and its HP, MP and SP, when it leaves the world.
    fn save_position(
        &self,
        character_id: u32,
        map: u16,
        position: [f32; 3],
        dir: i16,
        health: [u16; 3],
    ) -> impl Future<Output = Result<()>> + Send;
    /// Stores experience, level, points, base stats and HP/MP/SP after they changed in the world.
    fn save_progress(&self, character_id: u32, progress: &Progress, health: [u16; 3]) -> impl Future<Output = Result<()>> + Send;
    /// Stores the honor (Hg).
    fn save_honor(&self, character_id: u32, honor: u16) -> impl Future<Output = Result<()>> + Send;
    /// Stores the kills, deaths, victories and defeats (the original's `0x060C` indexes 0 to 3).
    fn save_kills(&self, character_id: u32, kills: [u32; 4]) -> impl Future<Output = Result<()>> + Send;
    /// Stores the kill and death reward steps taken (`0x060C` indexes 4 and 5).
    fn save_kill_steps(&self, character_id: u32, steps: [u8; 2]) -> impl Future<Output = Result<()>> + Send;
    /// A kill reward claimed (audit R12): the stat points (the new total) and the kill step, in one write that holds only while the
    /// step is still `old_step`. False, with nothing changed, when it is not.
    fn claim_kill_reward(
        &self,
        character_id: u32,
        stat_point: u16,
        old_step: u8,
        new_step: u8,
    ) -> impl Future<Output = Result<bool>> + Send;
    /// A death reward claimed (audit R12): `gold` added to the money and the death step, in one write that holds only while the step
    /// is still `old_step` and the money stays within 32 bits. False, with nothing changed, when not.
    fn claim_death_reward(&self, character_id: u32, gold: u32, old_step: u8, new_step: u8) -> impl Future<Output = Result<bool>> + Send;
    /// A stat reset saved (audit R12): the points and base stats of `progress` and the reward steps, in one write.
    fn save_stat_reset(
        &self,
        character_id: u32,
        progress: &Progress,
        health: [u16; 3],
        steps: [u8; 2],
    ) -> impl Future<Output = Result<()>> + Send;
    /// Stores a saved position, replacing the slot's.
    fn save_point(&self, character_id: u32, point: SavePoint) -> impl Future<Output = Result<()>> + Send;
    /// Stores the hair, face, size and sex, and takes one of the scroll in a bag slot that holds `had` (gone at 1), together. False,
    /// with nothing changed, when the slot does not hold `had`.
    fn change_body(&self, character_id: u32, body: Body, scroll: (u8, u8), had: u8) -> impl Future<Output = Result<bool>> + Send;
    /// One more rename allowed, and the Name Change Stone's slot change, together. False, with nothing changed, at 255 renames already
    /// or when the slot is not as the change expects.
    fn add_rename(&self, character_id: u32, stone: &SlotChange) -> impl Future<Output = Result<bool>> + Send;
    /// Renames a live character of this account and server that has a rename left, all together: the name, the old name kept, one
    /// rename less, the log row, and the auction house's seller and bidder names of that character.
    fn rename(&self, user_id: u32, server_id: u8, character_id: u32, new_name: &str) -> impl Future<Output = Result<RenameOutcome>> + Send;
}

pub trait ItemStore: Send + Sync + 'static {
    /// Changes a character's money by `money` and its bags by `changes`, in order, all together. False, with nothing changed,
    /// when the money would leave 0 to `u32::MAX` or a slot is not as its change expects (an insert's or a move's target empty,
    /// every other place taken). Bag 100 is the warehouse of the character's account on its server; an item moves between a bag
    /// and the warehouse only by `SlotChange::Move`.
    ///
    /// The rows come back (`docs/original/game-log.md` deviation 4): `None` when refused, otherwise one `Changed` per change, in order.
    fn trade(&self, character_id: u32, money: i64, changes: &[SlotChange]) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send;
    /// As `trade`, with bag 255 the bank of the guild `guild_id` (`docs/original/guild-bank.md`): an item crosses between a bag and
    /// the bank by `Move` or `Swap`, and the bank's slots are inside the same transaction as the money and the bags.
    fn guild_bank(
        &self,
        character_id: u32,
        guild_id: u32,
        money: i64,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send;
    /// Etin returned (`docs/server/audit.md` R16): `changes` take the Etin items out of the bags and the guild's Etin grows by `sum`
    /// (to 32 bits at most) with one more return, together. `None`, with nothing changed, when the character is not a member of the
    /// guild any more (a join request is not membership) or a slot is not as a change expects; otherwise the rows as `trade` gives them.
    fn guild_return_etin(
        &self,
        character_id: u32,
        guild_id: u32,
        sum: u32,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send;
    /// A trade between two characters, all together: each one's money by its amount, then the items in order. False, with nothing
    /// changed, when a money would leave 0 to `u32::MAX`, a giver's slot does not hold `had`, or a receiver's slot is taken. The rows
    /// come back, one `Handed` per handover.
    fn exchange(&self, money: [(u32, i64); 2], handovers: &[Handover]) -> impl Future<Output = Result<Option<Vec<Handed>>>> + Send;
    /// Drops `count` of the item in a bag slot that holds `had` (`docs/original/item-drop.md` section 5): all of it takes the item off
    /// the character and returns what it keeps, with its origin; part of it lowers the count and returns `Kept::SPLIT`. `None`, with
    /// nothing changed, when the slot does not hold `had`. With it comes the giver's row: the deleted one for all of it, the one that
    /// keeps the rest for part of it.
    fn drop_item(
        &self,
        character_id: u32,
        bag: u8,
        slot: u8,
        had: u8,
        count: u8,
    ) -> impl Future<Output = Result<Option<(Kept, Stored)>>> + Send;
    /// A duel's stakes out of the bags, all together (`docs/original/duel.md` section 4, deviation 5): `money` off, then each
    /// `(bag, slot, had, count)` as `drop_item` takes it, except that part of a stack returns the item's gems and craft name with make
    /// type 'T' (trade deviation 3). `None`, with nothing changed, when the money is short or a slot does not hold `had`. Each comes
    /// with the giver's row as `drop_item` gives it.
    fn take_stakes(
        &self,
        character_id: u32,
        money: u32,
        items: &[(u8, u8, u8, u8)],
    ) -> impl Future<Output = Result<Option<Vec<(Kept, Stored)>>>> + Send;
    /// The make types (the original's Maketype, 0 for none or an empty place) of bag places, in the order asked
    /// (`docs/original/lapis.md` sections 6 and 7).
    fn make_types(&self, character_id: u32, places: &[(u8, u8)]) -> impl Future<Output = Result<Vec<u8>>> + Send;
}

pub trait SkillStore: Send + Sync + 'static {
    /// Stores a learnt skill (new, or a higher level under its number) and the skill points left, together.
    fn learn(&self, character_id: u32, skill_points: u16, skill: Skill) -> impl Future<Output = Result<()>> + Send;
    /// A skill reset, all together: every learnt skill deleted and the skill points set; `free` marks the free reset used.
    fn reset_skills(&self, character_id: u32, skill_points: u16, free: bool) -> impl Future<Output = Result<()>> + Send;
    /// Replaces the character's quick slots.
    fn save_quick_slots(&self, character_id: u32, slots: &[QuickSlot]) -> impl Future<Output = Result<()>> + Send;
    /// Stores the cooldown left (`delay`) of each of the character's skills.
    fn save_cooldowns(&self, character_id: u32, skills: &[Skill]) -> impl Future<Output = Result<()>> + Send;
    /// Replaces the character's applied skills, saved now.
    fn save_applied(&self, character_id: u32, applied: &[SavedApplied]) -> impl Future<Output = Result<()>> + Send;
}

/// A live guild as stored, with its members and join requests (`docs/original/guild.md` section 15).
#[derive(Clone, Debug, PartialEq)]
pub struct StoredGuild {
    pub id: u32,
    pub name: String,
    pub country: u8,
    pub remark: String,
    /// Members and requests (rank 0), longest in the guild first.
    pub members: Vec<StoredMember>,
    /// Its house, ranking and Etin (`docs/original/guild-house.md` section 1).
    pub house: crate::guild_house::House,
    /// Its bank's items, bag 255 (`docs/original/guild-bank.md`).
    pub bank: Vec<ItemUnit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredMember {
    pub id: u32,
    pub name: String,
    pub level: u16,
    pub job: u8,
    pub rank: u8,
}

/// A guild to create, all together: the guild, its members with their ranks (their join requests elsewhere withdrawn, their join time
/// now), and `cost` off the payer's money.
#[derive(Clone, Debug, PartialEq)]
pub struct NewGuild {
    pub server_id: u8,
    pub name: String,
    pub country: u8,
    pub remark: String,
    pub members: Vec<(u32, u8)>,
    pub payer: u32,
    pub cost: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuildCreated {
    Created(u32),
    /// A live guild of this server has the name (any case).
    NameTaken,
    /// The payer has less than the cost.
    NoMoney,
}

/// One change to a guild, written in the order the server made them.
#[derive(Clone, Debug, PartialEq)]
pub enum GuildWrite {
    /// A new member (its join time now) or, with rank 0, a join request.
    Add {
        guild: u32,
        character: u32,
        rank: u8,
    },
    /// A join request accepted: rank 9, its join time now.
    Accept {
        character: u32,
    },
    Rank {
        character: u32,
        rank: u8,
    },
    /// Out of its guild, or its request withdrawn or refused.
    Remove {
        character: u32,
    },
    Remark {
        guild: u32,
        remark: String,
    },
    /// The guild closed and every row of it removed.
    Disband {
        guild: u32,
    },
    /// Etin returned (`docs/original/guild-house.md` section 3): the guild's Etin and return count now.
    Etin {
        guild: u32,
        etin: u32,
        returns: u32,
    },
    /// An NPC upgrade (section 4): the slot's key and level, the Etin and keep Etin now.
    Upgrade {
        guild: u32,
        slot: u8,
        key: u16,
        level: u8,
        etin: u32,
        keep_etin: u32,
    },
    /// The house bought (section 5), with its keep Etin.
    HouseBought {
        guild: u32,
        keep_etin: u32,
    },
    /// The ranking update (`docs/original/guild-ranking.md` section 8), all guilds together: `(guild, points, Etin, ranking,
    /// byUseHouse)`; every guild's Etin return count goes back to 0.
    Ranking {
        rows: Vec<RankingRow>,
    },
    /// When the ranking update is due (unix seconds), or none left (deviation 4).
    RankingDue(Option<i64>),
    /// `FA15` (`docs/original/gm-commands.md` deviation 10): the guild's points.
    Points {
        guild: u32,
        points: u32,
    },
    /// `FA14`: the guild's ranking, and `byUseHouse` with it.
    Standing {
        guild: u32,
        ranking: u8,
        use_house: u8,
    },
    /// A bank item's count lowered to its stack limit at loading (`docs/original/guild-bank.md` section 2).
    BankCount {
        guild: u32,
        slot: u8,
        count: u8,
    },
}

/// One guild's row of the ranking update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RankingRow {
    pub guild: u32,
    pub points: u32,
    pub etin: u32,
    pub ranking: u8,
    pub use_house: u8,
}

pub trait GuildStore: Send + Sync + 'static {
    /// The guild house bought (`docs/server/audit.md` R16): `price` taken from the master's money and the guild's house flag and keep
    /// Etin set, together. False, with nothing changed, when the character is not the guild's master, the guild is gone, the house is
    /// bought already or the money is short.
    fn buy_guild_house(&self, character_id: u32, guild_id: u32, price: u32, keep_etin: u32) -> impl Future<Output = Result<bool>> + Send;
    /// The live guilds of this server.
    fn load_guilds(&self, server_id: u8) -> impl Future<Output = Result<Vec<StoredGuild>>> + Send;
    fn create_guild(&self, guild: &NewGuild) -> impl Future<Output = Result<GuildCreated>> + Send;
    fn write_guild(&self, write: &GuildWrite) -> impl Future<Output = Result<()>> + Send;
    /// When a ranking update was left due (unix seconds), if one was.
    fn load_ranking_due(&self) -> impl Future<Output = Result<Option<i64>>> + Send;
}

pub trait QuestStore: Send + Sync + 'static {
    /// A quest taken, with its time limit in seconds, and the bags by `changes` (the start items), all together. `None`, with nothing
    /// changed, when a slot is not as its change expects; otherwise the rows the changes touched, as `ItemStore::trade` gives them.
    fn start_quest(
        &self,
        character_id: u32,
        quest_id: u16,
        seconds_left: Option<u32>,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send;
    /// A quest in progress's counters.
    fn quest_counts(&self, character_id: u32, quest_id: u16, counts: [u8; 3]) -> impl Future<Output = Result<()>> + Send;
    /// A quest ended (section 9). `None`, with nothing changed, as `ItemStore::trade`; otherwise the rows of `end.changes`.
    fn end_quest(&self, character_id: u32, end: &QuestEnd) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send;
    /// The seconds left of the quests in progress with a time limit, when the character leaves the world.
    fn save_quest_times(&self, character_id: u32, times: &[(u16, u32)]) -> impl Future<Output = Result<()>> + Send;
}

/// One change to the friend or block lists, written in the order the server made them (`docs/original/friends.md` section 11).
#[derive(Clone, Debug, PartialEq)]
pub enum FriendWrite {
    /// Each `(character, friend)` pair stored, all together; a pair already there stays as it is.
    Friends(Vec<(u32, u32)>),
    RemoveFriend {
        character: u32,
        friend: u32,
    },
    FriendMemo {
        character: u32,
        friend: u32,
        memo: String,
    },
    Block {
        character: u32,
        blocked: u32,
    },
    RemoveBlock {
        character: u32,
        blocked: u32,
    },
    BlockMemo {
        character: u32,
        blocked: u32,
        memo: String,
    },
}

pub trait FriendStore: Send + Sync + 'static {
    fn write_friends(&self, write: &FriendWrite) -> impl Future<Output = Result<()>> + Send;
}

pub trait Heartbeat: Send + Sync + 'static {
    /// Reports this server alive with its player count, and keeps its players' sessions from expiring. `open` false (an operator's
    /// `/server off`) lists the server as closed to the login server until a heartbeat says open again.
    fn heartbeat(&self, server_id: u8, players: u16, open: bool) -> impl Future<Output = Result<()>> + Send;
    /// The factions' kill counts of this server (`docs/original/pvp-2.md` section 9), 0 when never saved.
    /// The monsters' object id counter of this server (`docs/original/quest-drops.md` section 4.3), 1 when never saved.
    fn load_mob_id(&self, server_id: u8) -> impl Future<Output = Result<u32>> + Send;
    fn save_mob_id(&self, server_id: u8, id: u32) -> impl Future<Output = Result<()>> + Send;
    fn load_kill_counts(&self, server_id: u8) -> impl Future<Output = Result<[u32; 2]>> + Send;
    fn save_kill_counts(&self, server_id: u8, counts: [u32; 2]) -> impl Future<Output = Result<()>> + Send;
    /// Who holds the obelisks of this server (`docs/original/obelisks.md` section 7); none when never saved.
    fn load_obelisk_owners(&self, server_id: u8) -> impl Future<Output = Result<Vec<crate::obelisk::Owner>>> + Send;
    fn save_obelisk_owner(&self, server_id: u8, owner: crate::obelisk::Owner) -> impl Future<Output = Result<()>> + Send;
}

/// What `/set` changes that has no other store call: a character's mode and the VG, CG, OG and IG marks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub grow: u8,
    pub vg: i16,
    pub cg: u8,
    pub og: u8,
    pub ig: u8,
}

/// An unanswered user question as stored (`docs/original/gm-commands-2.md` deviation 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredQuestion {
    pub id: u32,
    pub character_id: u32,
    pub name: String,
    /// The text as sent, with its zero.
    pub text: Vec<u8>,
    /// When it was asked, in unix seconds.
    pub asked_unix: i64,
}

/// What the operator commands (`docs/original/gm-commands.md`) read and write besides the character's own saves.
pub trait AdminStore: Send + Sync + 'static {
    /// The console's `reload` (`docs/plans/done/data-editing.md` section 7): the monsters and the drop tables read again from the
    /// database, with `skills` attached. An error means nothing was loaded.
    fn load_live(&self, skills: &crate::skills::SkillDefs) -> impl Future<Output = Result<crate::server::Live>> + Send;
    /// The database layers of the rules, read again by `reload` (`docs/plans/ext-2-rules.md` section 4.6): the enabled extensions'
    /// and the data editor's. An error means nothing was read.
    fn rule_layers(&self) -> impl Future<Output = Result<crate::rules::RuleSources>> + Send;
    /// `F601`, `F602`: the account's status word (the staff level in bits 4 to 6).
    fn set_account_status(&self, user_id: u32, status: i16) -> impl Future<Output = Result<()>> + Send;
    /// An account by name, ignoring case, with its status; `None` when there is none (or it is deleted).
    fn account_id(&self, username: &str) -> impl Future<Output = Result<Option<(u32, i16)>>> + Send;
    /// `/set` grow, VG, CG, OG, IG.
    fn save_marks(&self, character_id: u32, marks: Marks) -> impl Future<Output = Result<()>> + Send;
    /// The account of a live character by name, ignoring case, with its status.
    fn character_account(&self, name: &str) -> impl Future<Output = Result<Option<(u32, i16)>>> + Send;
    /// `/aban`, `/ban` (deviation 14): the account is blocked (`status` 2) for `minutes`, or with no end for `None`.
    fn block_account(&self, user_id: u32, minutes: Option<u16>) -> impl Future<Output = Result<()>> + Send;
    /// `/abancan`, `/bancan`: a blocked account (`status` 2 to 6) is let in again. False when it was not blocked.
    fn lift_block(&self, user_id: u32) -> impl Future<Output = Result<bool>> + Send;
    /// The quests a GM closed on this server (deviation 16).
    fn closed_quests(&self, server_id: u8) -> impl Future<Output = Result<Vec<u16>>> + Send;
    fn set_quest_closed(&self, server_id: u8, quest: u16, closed: bool) -> impl Future<Output = Result<()>> + Send;
    /// A live character by name, ignoring case (deviation 17).
    fn character_id(&self, name: &str) -> impl Future<Output = Result<Option<u32>>> + Send;
    /// An event tag on a character; false when it has it already (also in another case).
    fn add_event(&self, character_id: u32, name: &str) -> impl Future<Output = Result<bool>> + Send;
    /// False when it had none of that name.
    fn remove_event(&self, character_id: u32, name: &str) -> impl Future<Output = Result<bool>> + Send;
    /// A user question written when it is asked; its id (the table's) is the question's.
    fn add_question(&self, server_id: u8, character_id: u32, name: &str, text: &[u8]) -> impl Future<Output = Result<u32>> + Send;
    /// `/UQGET`: the GM (a character id) took the question.
    fn take_question(&self, id: u32, gm: u32) -> impl Future<Output = Result<()>> + Send;
    /// A GM left with the question unanswered.
    fn release_question(&self, id: u32) -> impl Future<Output = Result<()>> + Send;
    /// `F202` (kind 1 to 3) or `F203` (kind 4 with the name `to`): the question is answered, for good.
    fn answer_question(&self, id: u32, gm: u32, kind: u8, text: &[u8], to: Option<&str>) -> impl Future<Output = Result<()>> + Send;
    /// The server's unanswered questions, oldest first; the GMs' holds are cleared (they were lost with the last run).
    fn open_questions(&self, server_id: u8) -> impl Future<Output = Result<Vec<StoredQuestion>>> + Send;
}

/// The item of an auction or of a row to collect, as the rows show it.
pub use shaiya_proto::market::ItemRow as MarketItem;

/// An auction as stored (`market_auctions`, `docs/original/auction.md` section 2.1). Times are microseconds since 1970.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAuction {
    pub id: u32,
    pub seller: u32,
    pub seller_name: String,
    pub ends_at: i64,
    pub min: u32,
    pub direct: u32,
    pub period: u8,
    pub deposit: u32,
    /// The top bidder and its name; `None` before the first bid.
    pub bidder: Option<(u32, String)>,
    /// The minimum until the first bid, then the top bid.
    pub price: u32,
    pub item: MarketItem,
}

/// An item to collect (`market_result_items`, section 2.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredItemRow {
    pub market_id: u32,
    pub character: u32,
    pub result: u8,
    pub deadline: i64,
    pub item: MarketItem,
}

/// Gold to collect (`market_result_money`, section 2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMoneyRow {
    pub id: u32,
    pub market_id: u32,
    pub character: u32,
    pub result: u8,
    pub money: u32,
    pub deposit: u32,
    pub return_money: u32,
    pub deadline: i64,
    pub item: MarketItem,
}

/// A new auction of the item `MarketWrite::take` takes. `item` is the item as the auction holds it: for part of a stack the new
/// row's quality, gems, count and craft name (its item id, type and make type are the bag row's; a new uid, made now).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewAuction {
    pub server_id: u8,
    pub seller: u32,
    pub seller_name: String,
    pub ends_at: i64,
    pub min: u32,
    pub direct: u32,
    pub period: u8,
    pub deposit: u32,
    pub item: MarketItem,
}

/// A new row of gold to collect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewMoneyRow {
    pub market_id: u32,
    pub character: u32,
    pub result: u8,
    pub money: u32,
    pub deposit: u32,
    pub return_money: u32,
    pub deadline: i64,
    pub item: MarketItem,
}

/// One change of a market write, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketOp {
    /// Its id goes to `MarketWritten::auction_ids`.
    NewAuction(NewAuction),
    /// The auction ends: its item, every column kept, becomes the item row of `character` with `result`, and the auction goes.
    EndAuction {
        id: u32,
        character: u32,
        result: u8,
        deadline: i64,
    },
    SetBid {
        id: u32,
        bidder: u32,
        name: String,
        price: u32,
    },
    /// Its id goes to `MarketWritten::money_ids`.
    NewMoneyRow(NewMoneyRow),
    DeleteItemRow(u32),
    DeleteMoneyRow(u32),
}

/// A market change, all together (`docs/original/auction.md` section 15): `character`'s gold by `money` (0 leaves it alone), the
/// bag place `take` (`bag, slot, had, count`) for a `NewAuction` as `ItemStore::drop_item` takes it (the whole row keeping its uid,
/// or part of the stack), the item row `place` (`market id, bag, slot`) into one of `character`'s empty bag places (its uid kept),
/// and `ops` in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketWrite {
    pub character: u32,
    pub money: i64,
    pub take: Option<(u8, u8, u8, u8)>,
    pub place: Option<(u32, u8, u8)>,
    pub ops: Vec<MarketOp>,
}

/// The ids the inserts of a write got, in op order, and the item rows it touched (`docs/original/game-log.md` deviation 4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketWritten {
    pub auction_ids: Vec<u32>,
    pub money_ids: Vec<u32>,
    /// The row each op touched, in op order: the auction a `NewAuction` made, the item an `EndAuction` moved, the auction a `SetBid`
    /// changed, the item row a `DeleteItemRow` deleted; `None` for the money rows' ops.
    pub ops: Vec<Option<Stored>>,
    /// The bag row `take` took (the whole row, or the row that keeps the rest of the stack).
    pub taken: Option<Stored>,
    /// The bag row `place` made (its uid kept).
    pub placed: Option<Stored>,
}

pub trait MarketStore: Send + Sync + 'static {
    /// The auctions of this server and the rows to collect of its characters.
    #[allow(clippy::type_complexity)]
    fn load_market(
        &self,
        server_id: u8,
    ) -> impl Future<Output = Result<(Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>)>> + Send;
    /// `None`, with nothing changed, when the gold would leave 0 to `u32::MAX`, the taken place does not hold `had`, the placed row is
    /// not `character`'s or its place is taken, or an op's auction or row is not there.
    fn market(&self, write: &MarketWrite) -> impl Future<Output = Result<Option<MarketWritten>>> + Send;
    /// Replaces the character's interest list (positions in order).
    fn save_interest(&self, character_id: u32, ids: &[u32]) -> impl Future<Output = Result<()>> + Send;
}

pub trait Backend:
    SessionGate
    + CharacterStore
    + ItemStore
    + SkillStore
    + GuildStore
    + QuestStore
    + FriendStore
    + Heartbeat
    + AdminStore
    + MarketStore
    + PointStore
{
}
impl<
    T: SessionGate
        + CharacterStore
        + ItemStore
        + SkillStore
        + GuildStore
        + QuestStore
        + FriendStore
        + Heartbeat
        + AdminStore
        + MarketStore
        + PointStore,
> Backend for T
{
}

impl<T: PointStore> PointStore for std::sync::Arc<T> {
    fn points(&self, user_id: u32) -> impl Future<Output = Result<u32>> + Send {
        (**self).points(user_id)
    }
    fn buy(&self, purchase: &Purchase) -> impl Future<Output = Result<Bought>> + Send {
        (**self).buy(purchase)
    }
    fn gift(&self, gift: &GiftOrder) -> impl Future<Output = Result<GiftOutcome>> + Send {
        (**self).gift(gift)
    }
    fn add_gift_notice(&self, user_id: u32, server_id: u8, notice: &GiftNotice) -> impl Future<Output = Result<()>> + Send {
        (**self).add_gift_notice(user_id, server_id, notice)
    }
    fn take_gift_notices(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Vec<GiftNotice>>> + Send {
        (**self).take_gift_notices(user_id, server_id)
    }
    fn take_stored(
        &self,
        character_id: u32,
        storage: Storage,
        had: StoredPointItem,
        item: &NewItem,
    ) -> impl Future<Output = Result<Option<Stored>>> + Send {
        (**self).take_stored(character_id, storage, had, item)
    }
}

impl<T: MarketStore> MarketStore for std::sync::Arc<T> {
    fn load_market(
        &self,
        server_id: u8,
    ) -> impl Future<Output = Result<(Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>)>> + Send {
        (**self).load_market(server_id)
    }
    fn market(&self, write: &MarketWrite) -> impl Future<Output = Result<Option<MarketWritten>>> + Send {
        (**self).market(write)
    }
    fn save_interest(&self, character_id: u32, ids: &[u32]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_interest(character_id, ids)
    }
}

// A shared backend works wherever an owned one does, so tests can keep a handle to it.
impl<T: SessionGate> SessionGate for std::sync::Arc<T> {
    fn claim(&self, claim: &Claim) -> impl Future<Output = Result<Option<Claimed>>> + Send {
        (**self).claim(claim)
    }
    fn claimable(&self, claim: &Claim) -> impl Future<Output = Result<bool>> + Send {
        (**self).claimable(claim)
    }
    fn release(&self, user_id: u32, identity: [u8; 16]) -> impl Future<Output = Result<()>> + Send {
        (**self).release(user_id, identity)
    }
}

impl<T: CharacterStore> CharacterStore for std::sync::Arc<T> {
    fn faction(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Option<Faction>>> + Send {
        (**self).faction(user_id, server_id)
    }
    fn choose_faction(&self, user_id: u32, server_id: u8, country: u8) -> impl Future<Output = Result<bool>> + Send {
        (**self).choose_faction(user_id, server_id, country)
    }
    fn characters(&self, user_id: u32, server_id: u8) -> impl Future<Output = Result<Vec<CharacterSummary>>> + Send {
        (**self).characters(user_id, server_id)
    }
    fn create(&self, character: &NewCharacter) -> impl Future<Output = Result<CreateOutcome>> + Send {
        (**self).create(character)
    }
    fn request_delete(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<bool>> + Send {
        (**self).request_delete(user_id, server_id, character_id)
    }
    fn restore(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<bool>> + Send {
        (**self).restore(user_id, server_id, character_id)
    }
    fn fall(&self, character_id: u32) -> impl Future<Output = Result<bool>> + Send {
        (**self).fall(character_id)
    }
    fn load(&self, user_id: u32, server_id: u8, character_id: u32) -> impl Future<Output = Result<Option<PlayCharacter>>> + Send {
        (**self).load(user_id, server_id, character_id)
    }
    fn save_position(
        &self,
        character_id: u32,
        map: u16,
        position: [f32; 3],
        dir: i16,
        health: [u16; 3],
    ) -> impl Future<Output = Result<()>> + Send {
        (**self).save_position(character_id, map, position, dir, health)
    }
    fn save_progress(&self, character_id: u32, progress: &Progress, health: [u16; 3]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_progress(character_id, progress, health)
    }
    fn save_honor(&self, character_id: u32, honor: u16) -> impl Future<Output = Result<()>> + Send {
        (**self).save_honor(character_id, honor)
    }
    fn save_kills(&self, character_id: u32, kills: [u32; 4]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_kills(character_id, kills)
    }
    fn save_kill_steps(&self, character_id: u32, steps: [u8; 2]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_kill_steps(character_id, steps)
    }
    fn claim_kill_reward(
        &self,
        character_id: u32,
        stat_point: u16,
        old_step: u8,
        new_step: u8,
    ) -> impl Future<Output = Result<bool>> + Send {
        (**self).claim_kill_reward(character_id, stat_point, old_step, new_step)
    }
    fn claim_death_reward(&self, character_id: u32, gold: u32, old_step: u8, new_step: u8) -> impl Future<Output = Result<bool>> + Send {
        (**self).claim_death_reward(character_id, gold, old_step, new_step)
    }
    fn save_stat_reset(
        &self,
        character_id: u32,
        progress: &Progress,
        health: [u16; 3],
        steps: [u8; 2],
    ) -> impl Future<Output = Result<()>> + Send {
        (**self).save_stat_reset(character_id, progress, health, steps)
    }
    fn save_point(&self, character_id: u32, point: SavePoint) -> impl Future<Output = Result<()>> + Send {
        (**self).save_point(character_id, point)
    }
    fn change_body(&self, character_id: u32, body: Body, scroll: (u8, u8), had: u8) -> impl Future<Output = Result<bool>> + Send {
        (**self).change_body(character_id, body, scroll, had)
    }
    fn add_rename(&self, character_id: u32, stone: &SlotChange) -> impl Future<Output = Result<bool>> + Send {
        (**self).add_rename(character_id, stone)
    }
    fn rename(&self, user_id: u32, server_id: u8, character_id: u32, new_name: &str) -> impl Future<Output = Result<RenameOutcome>> + Send {
        (**self).rename(user_id, server_id, character_id, new_name)
    }
}

impl<T: ItemStore> ItemStore for std::sync::Arc<T> {
    fn trade(&self, character_id: u32, money: i64, changes: &[SlotChange]) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send {
        (**self).trade(character_id, money, changes)
    }
    fn guild_return_etin(
        &self,
        character_id: u32,
        guild_id: u32,
        sum: u32,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send {
        (**self).guild_return_etin(character_id, guild_id, sum, changes)
    }
    fn guild_bank(
        &self,
        character_id: u32,
        guild_id: u32,
        money: i64,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send {
        (**self).guild_bank(character_id, guild_id, money, changes)
    }
    fn exchange(&self, money: [(u32, i64); 2], handovers: &[Handover]) -> impl Future<Output = Result<Option<Vec<Handed>>>> + Send {
        (**self).exchange(money, handovers)
    }
    fn drop_item(
        &self,
        character_id: u32,
        bag: u8,
        slot: u8,
        had: u8,
        count: u8,
    ) -> impl Future<Output = Result<Option<(Kept, Stored)>>> + Send {
        (**self).drop_item(character_id, bag, slot, had, count)
    }
    fn take_stakes(
        &self,
        character_id: u32,
        money: u32,
        items: &[(u8, u8, u8, u8)],
    ) -> impl Future<Output = Result<Option<Vec<(Kept, Stored)>>>> + Send {
        (**self).take_stakes(character_id, money, items)
    }
    fn make_types(&self, character_id: u32, places: &[(u8, u8)]) -> impl Future<Output = Result<Vec<u8>>> + Send {
        (**self).make_types(character_id, places)
    }
}

impl<T: SkillStore> SkillStore for std::sync::Arc<T> {
    fn learn(&self, character_id: u32, skill_points: u16, skill: Skill) -> impl Future<Output = Result<()>> + Send {
        (**self).learn(character_id, skill_points, skill)
    }
    fn reset_skills(&self, character_id: u32, skill_points: u16, free: bool) -> impl Future<Output = Result<()>> + Send {
        (**self).reset_skills(character_id, skill_points, free)
    }
    fn save_quick_slots(&self, character_id: u32, slots: &[QuickSlot]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_quick_slots(character_id, slots)
    }
    fn save_cooldowns(&self, character_id: u32, skills: &[Skill]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_cooldowns(character_id, skills)
    }
    fn save_applied(&self, character_id: u32, applied: &[SavedApplied]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_applied(character_id, applied)
    }
}

impl<T: GuildStore> GuildStore for std::sync::Arc<T> {
    fn buy_guild_house(&self, character_id: u32, guild_id: u32, price: u32, keep_etin: u32) -> impl Future<Output = Result<bool>> + Send {
        (**self).buy_guild_house(character_id, guild_id, price, keep_etin)
    }
    fn load_guilds(&self, server_id: u8) -> impl Future<Output = Result<Vec<StoredGuild>>> + Send {
        (**self).load_guilds(server_id)
    }
    fn create_guild(&self, guild: &NewGuild) -> impl Future<Output = Result<GuildCreated>> + Send {
        (**self).create_guild(guild)
    }
    fn write_guild(&self, write: &GuildWrite) -> impl Future<Output = Result<()>> + Send {
        (**self).write_guild(write)
    }
    fn load_ranking_due(&self) -> impl Future<Output = Result<Option<i64>>> + Send {
        (**self).load_ranking_due()
    }
}

impl<T: QuestStore> QuestStore for std::sync::Arc<T> {
    fn start_quest(
        &self,
        character_id: u32,
        quest_id: u16,
        seconds_left: Option<u32>,
        changes: &[SlotChange],
    ) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send {
        (**self).start_quest(character_id, quest_id, seconds_left, changes)
    }
    fn quest_counts(&self, character_id: u32, quest_id: u16, counts: [u8; 3]) -> impl Future<Output = Result<()>> + Send {
        (**self).quest_counts(character_id, quest_id, counts)
    }
    fn end_quest(&self, character_id: u32, end: &QuestEnd) -> impl Future<Output = Result<Option<Vec<Changed>>>> + Send {
        (**self).end_quest(character_id, end)
    }
    fn save_quest_times(&self, character_id: u32, times: &[(u16, u32)]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_quest_times(character_id, times)
    }
}

impl<T: FriendStore> FriendStore for std::sync::Arc<T> {
    fn write_friends(&self, write: &FriendWrite) -> impl Future<Output = Result<()>> + Send {
        (**self).write_friends(write)
    }
}

impl<T: Heartbeat> Heartbeat for std::sync::Arc<T> {
    fn heartbeat(&self, server_id: u8, players: u16, open: bool) -> impl Future<Output = Result<()>> + Send {
        (**self).heartbeat(server_id, players, open)
    }
    fn load_mob_id(&self, server_id: u8) -> impl Future<Output = Result<u32>> + Send {
        (**self).load_mob_id(server_id)
    }
    fn save_mob_id(&self, server_id: u8, id: u32) -> impl Future<Output = Result<()>> + Send {
        (**self).save_mob_id(server_id, id)
    }
    fn load_kill_counts(&self, server_id: u8) -> impl Future<Output = Result<[u32; 2]>> + Send {
        (**self).load_kill_counts(server_id)
    }
    fn save_kill_counts(&self, server_id: u8, counts: [u32; 2]) -> impl Future<Output = Result<()>> + Send {
        (**self).save_kill_counts(server_id, counts)
    }
    fn load_obelisk_owners(&self, server_id: u8) -> impl Future<Output = Result<Vec<crate::obelisk::Owner>>> + Send {
        (**self).load_obelisk_owners(server_id)
    }
    fn save_obelisk_owner(&self, server_id: u8, owner: crate::obelisk::Owner) -> impl Future<Output = Result<()>> + Send {
        (**self).save_obelisk_owner(server_id, owner)
    }
}

impl<T: AdminStore> AdminStore for std::sync::Arc<T> {
    fn load_live(&self, skills: &crate::skills::SkillDefs) -> impl Future<Output = Result<crate::server::Live>> + Send {
        (**self).load_live(skills)
    }
    fn rule_layers(&self) -> impl Future<Output = Result<crate::rules::RuleSources>> + Send {
        (**self).rule_layers()
    }
    fn set_account_status(&self, user_id: u32, status: i16) -> impl Future<Output = Result<()>> + Send {
        (**self).set_account_status(user_id, status)
    }
    fn account_id(&self, username: &str) -> impl Future<Output = Result<Option<(u32, i16)>>> + Send {
        (**self).account_id(username)
    }
    fn save_marks(&self, character_id: u32, marks: Marks) -> impl Future<Output = Result<()>> + Send {
        (**self).save_marks(character_id, marks)
    }
    fn character_account(&self, name: &str) -> impl Future<Output = Result<Option<(u32, i16)>>> + Send {
        (**self).character_account(name)
    }
    fn block_account(&self, user_id: u32, minutes: Option<u16>) -> impl Future<Output = Result<()>> + Send {
        (**self).block_account(user_id, minutes)
    }
    fn lift_block(&self, user_id: u32) -> impl Future<Output = Result<bool>> + Send {
        (**self).lift_block(user_id)
    }
    fn closed_quests(&self, server_id: u8) -> impl Future<Output = Result<Vec<u16>>> + Send {
        (**self).closed_quests(server_id)
    }
    fn set_quest_closed(&self, server_id: u8, quest: u16, closed: bool) -> impl Future<Output = Result<()>> + Send {
        (**self).set_quest_closed(server_id, quest, closed)
    }
    fn character_id(&self, name: &str) -> impl Future<Output = Result<Option<u32>>> + Send {
        (**self).character_id(name)
    }
    fn add_event(&self, character_id: u32, name: &str) -> impl Future<Output = Result<bool>> + Send {
        (**self).add_event(character_id, name)
    }
    fn remove_event(&self, character_id: u32, name: &str) -> impl Future<Output = Result<bool>> + Send {
        (**self).remove_event(character_id, name)
    }
    fn add_question(&self, server_id: u8, character_id: u32, name: &str, text: &[u8]) -> impl Future<Output = Result<u32>> + Send {
        (**self).add_question(server_id, character_id, name, text)
    }
    fn take_question(&self, id: u32, gm: u32) -> impl Future<Output = Result<()>> + Send {
        (**self).take_question(id, gm)
    }
    fn release_question(&self, id: u32) -> impl Future<Output = Result<()>> + Send {
        (**self).release_question(id)
    }
    fn answer_question(&self, id: u32, gm: u32, kind: u8, text: &[u8], to: Option<&str>) -> impl Future<Output = Result<()>> + Send {
        (**self).answer_question(id, gm, kind, text, to)
    }
    fn open_questions(&self, server_id: u8) -> impl Future<Output = Result<Vec<StoredQuestion>>> + Send {
        (**self).open_questions(server_id)
    }
}
