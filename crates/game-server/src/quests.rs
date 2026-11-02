//! Quests (`docs/original/quest.md`): the definitions from `world.quests`, a player's quests in progress and finished, and the rules for
//! taking, counting and ending them. The functions only decide; the connection saves the changes and sends the packets.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::backend::SavedQuest;
use crate::inventory::{Inventory, ItemDefs, Kept, NewItem, SlotChange, real_type};

/// The make type of quest items.
pub const QUEST_MAKE_TYPE: u8 = b'Q';

/// Real type of quest items (item types 27 to 29): taken whole when a quest ends with them (section 7.2).
const QUEST_ITEM: u8 = 27;
/// Counters stop here (`u8`).
const MAX_COUNT: u8 = 255;

/// `byStartType` (section 5).
pub mod start {
    pub const ANY_NPC: u8 = 0;
    pub const CLICK_NPC: u8 = 1;
    pub const GIVE_ITEM: u8 = 2;
    pub const HAVE_ITEM: u8 = 3;
    pub const USE_ITEM: u8 = 4;
}

/// `byEndType` (section 7).
pub mod end {
    pub const TALK: u8 = 0;
    pub const ITEMS: u8 = 1;
    pub const KILL_MOBS: u8 = 2;
    pub const KILL_PLAYERS: u8 = 3;
}

/// One of a quest's three results (`stQuestResult`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuestResult {
    /// 1-based index into the monster kills (1 or 2) and the count it needs; 0 none.
    pub need_mob: (u8, u8),
    /// 1-based index into the end items (1 to 3) and the count it needs; 0 none.
    pub need_item: (u8, u8),
    pub exp: u32,
    pub money: u32,
    /// The reward item, count 1; type 0 none.
    pub item: (u8, u8),
}

/// A quest definition (`stQuestInfo`, section 1). Unused fields are not kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuestDef {
    pub id: u16,
    pub name: String,
    pub level_min: u16,
    pub level_max: u16,
    /// Who by race (section 4).
    pub country: u8,
    pub grow: u8,
    pub jobs: [bool; 6],
    /// `wPrevQuest`, `wHG`, `shVG`: one of those set must be finished with success.
    pub prev: [u16; 3],
    /// Time limit in minutes; 0 none.
    pub time_limit: u32,
    /// `dwTickRecvCount` ≠ 0: not kept as finished, taken again at will.
    pub repeatable: bool,
    pub start_type: u8,
    pub start_npc: (u8, u16),
    pub start_item: (u8, u8),
    /// Given at the start: type, type id, count (type 0: none).
    pub start_items: [(u8, u8, u8); 3],
    pub end_type: u8,
    pub end_npc: (u8, u16),
    /// To bring: type, type id, count.
    pub end_items: [(u8, u8, u8); 3],
    pub pc_kills: u8,
    /// Monster id and count, twice.
    pub mob_kills: [(u16, u8); 2],
    /// 1: always result 0.
    pub result_type: u8,
    /// Not 0: the player chooses one of this many results.
    pub user_select: u8,
    pub results: [QuestResult; 3],
}

impl QuestDef {
    /// Whether the counters have reached the kills needed (end types 2 and 3).
    fn kills_done(&self, counts: [u8; 3]) -> bool {
        match self.end_type {
            end::KILL_MOBS => counts[0] >= self.mob_kills[0].1 && counts[1] >= self.mob_kills[1].1,
            end::KILL_PLAYERS => counts[0] >= self.pc_kills,
            _ => false,
        }
    }

    /// The experience a result gives: doubled unless the quest is for ultimate mode only (section 7.4).
    pub fn exp(&self, result: usize) -> u32 {
        let exp = self.results[result].exp;
        if self.grow <= 2 { exp.saturating_mul(2) } else { exp }
    }
}

/// The definitions by id; quest `n` is the `n`-th.
#[derive(Default)]
pub struct QuestDefs(Vec<QuestDef>);

impl QuestDefs {
    /// The definitions in id order. Ids must run from 1 without gaps (the original finds a quest by position); the caller checks.
    /// A `user_select` above the three results a quest has is cut to 3, with a warning (audit R24): the choice indexes the results.
    pub fn new(mut defs: Vec<QuestDef>) -> Self {
        let results = QuestDef::default().results.len() as u8;
        for (n, def) in defs.iter_mut().enumerate().filter(|(_, def)| def.user_select > results) {
            tracing::warn!(
                quest = n + 1,
                user_select = def.user_select,
                "a quest offers more rewards to choose from than it has results; cut to {results}"
            );
            def.user_select = results;
        }
        Self(defs)
    }

    pub fn get(&self, id: u16) -> Option<&QuestDef> {
        self.0.get(usize::from(id).checked_sub(1)?)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A quest in progress (`CQuest` in `m_Quest`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Active {
    pub id: u16,
    pub counts: [u8; 3],
    /// When a time limit runs out.
    pub deadline: Option<Instant>,
    /// Waiting for the player's choice of reward (`bySuccess` 1, section 7.1); not kept.
    pub choosing: bool,
}

/// What the player is, as section 4 needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Who {
    pub level: u16,
    pub family: u8,
    pub job: u8,
    pub grow: u8,
}

/// A player's quests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quests {
    /// In the order they were taken.
    active: Vec<Active>,
    /// Quest id → success.
    finished: BTreeMap<u16, bool>,
}

impl Quests {
    /// From what was stored: quests in progress with their time left running from `now`, and finished ones. Unknown quests are
    /// dropped (and returned, for the log), as the original's login skips them.
    pub fn load(saved: &[SavedQuest], defs: &QuestDefs, now: Instant) -> (Self, Vec<u16>) {
        let mut quests = Self::default();
        let mut unknown = Vec::new();
        for s in saved {
            let Some(def) = defs.get(s.quest_id) else {
                unknown.push(s.quest_id);
                continue;
            };
            if s.finished {
                quests.finished.insert(s.quest_id, s.success);
            } else {
                let left = s.seconds_left.map_or(u64::from(def.time_limit) * 60, u64::from);
                let deadline = (def.time_limit > 0).then(|| now + Duration::from_secs(left));
                quests.active.push(Active { id: s.quest_id, counts: s.counts, deadline, choosing: false });
            }
        }
        (quests, unknown)
    }

    pub fn active(&self) -> &[Active] {
        &self.active
    }

    pub fn find(&self, id: u16) -> Option<&Active> {
        self.active.iter().find(|a| a.id == id)
    }

    fn find_mut(&mut self, id: u16) -> Option<&mut Active> {
        self.active.iter_mut().find(|a| a.id == id)
    }

    pub fn finished(&self) -> impl Iterator<Item = (u16, bool)> + '_ {
        self.finished.iter().map(|(&id, &success)| (id, success))
    }

    /// Finished with success (`QuestFindFinishSuccess`).
    fn done(&self, id: u16) -> bool {
        self.finished.get(&id) == Some(&true)
    }

    /// Section 4, and neither in progress nor finished.
    pub fn may_take(&self, def: &QuestDef, who: Who) -> bool {
        if self.find(def.id).is_some() || self.finished.contains_key(&def.id) {
            return false;
        }
        if who.level < def.level_min || who.level > def.level_max {
            return false;
        }
        let race = match def.country {
            0 => who.family == 0,
            1 => who.family == 1,
            2 => who.family <= 1,
            3 => who.family == 3,
            4 => who.family == 2,
            5 => who.family == 2 || who.family == 3,
            _ => true,
        };
        if !race || !def.jobs.get(usize::from(who.job)).copied().unwrap_or(false) || def.grow > who.grow {
            return false;
        }
        let set: Vec<u16> = def.prev.iter().copied().filter(|&p| p != 0).collect();
        set.is_empty() || set.iter().any(|&p| self.done(p))
    }

    /// Takes a quest that `may_take` allowed.
    pub fn take(&mut self, def: &QuestDef, now: Instant) {
        let deadline = (def.time_limit > 0).then(|| now + Duration::from_secs(u64::from(def.time_limit) * 60));
        self.active.push(Active { id: def.id, counts: [0; 3], deadline, choosing: false });
    }

    /// A quest leaves the list (`CUser::QuestRemove`): a repeatable one is dropped, any other is finished with `success`.
    pub fn remove(&mut self, def: &QuestDef, success: bool) {
        self.active.retain(|a| a.id != def.id);
        if !def.repeatable {
            self.finished.insert(def.id, success);
        }
    }

    /// `F80C` (`docs/original/gm-commands-2.md` section 4.4, the original's `0x0A02` with 2): the quest leaves the list with no finished
    /// mark, so it can be taken again. False when it was not in progress.
    pub fn drop(&mut self, id: u16) -> bool {
        let before = self.active.len();
        self.active.retain(|a| a.id != id);
        self.active.len() < before
    }

    /// `F80B` giving a finished quest again (deviation 9): its finished mark is replaced by the new start.
    pub fn forget_finished(&mut self, id: u16) {
        self.finished.remove(&id);
    }

    pub fn set_choosing(&mut self, id: u16) {
        if let Some(a) = self.find_mut(id) {
            a.choosing = true;
        }
    }

    /// A monster of `mob_id` killed (`CUser::QuestUpdateKillMOB`): each counter that names it moves. Returns the moves (quest,
    /// counter, count) and the quests without an end NPC whose kills are now complete.
    pub fn mob_killed(&mut self, defs: &QuestDefs, mob_id: u16) -> (Vec<(u16, u8, u8)>, Vec<u16>) {
        let (mut moves, mut complete) = (Vec::new(), Vec::new());
        for a in &mut self.active {
            let Some(def) = defs.get(a.id).filter(|d| d.end_type == end::KILL_MOBS) else { continue };
            for i in 0..2 {
                if def.mob_kills[i].0 == mob_id && a.counts[i] < MAX_COUNT {
                    a.counts[i] += 1;
                    moves.push((a.id, i as u8, a.counts[i]));
                }
            }
            if def.end_npc.0 == 0 && moves.iter().any(|m| m.0 == a.id) && def.kills_done(a.counts) && !complete.contains(&a.id) {
                complete.push(a.id);
            }
        }
        (moves, complete)
    }

    /// A player killed (`CUser::QuestUpdateKillPC`, `docs/original/quest.md` section 6): only the first quest in progress with end
    /// type 3 counts it, on counter 0. Returns the move (quest, counter, count) and whether that quest, without an end NPC, is now
    /// complete.
    pub fn player_killed(&mut self, defs: &QuestDefs) -> Option<((u16, u8, u8), bool)> {
        let (a, def) = self.active.iter_mut().find_map(|a| defs.get(a.id).filter(|d| d.end_type == end::KILL_PLAYERS).map(|d| (a, d)))?;
        if a.counts[0] == MAX_COUNT {
            return None;
        }
        a.counts[0] += 1;
        Some(((a.id, 0, a.counts[0]), def.end_npc.0 == 0 && def.kills_done(a.counts)))
    }

    /// The quests whose time ran out by `now`.
    pub fn expired(&self, now: Instant) -> Vec<u16> {
        self.active.iter().filter(|a| a.deadline.is_some_and(|d| d <= now)).map(|a| a.id).collect()
    }

    /// The next time limit to run out.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.active.iter().filter_map(|a| a.deadline).min()
    }

    /// What to store when the character leaves: each limited quest's seconds left (at least 1, as the original keeps a minute).
    pub fn times_left(&self, now: Instant) -> Vec<(u16, u32)> {
        self.active
            .iter()
            .filter_map(|a| a.deadline.map(|d| (a.id, d.saturating_duration_since(now).as_secs().clamp(1, u64::from(u32::MAX)) as u32)))
            .collect()
    }

    /// `0x0901`'s entries: the whole minutes left of a limit, 0 without one or when less than a minute is left.
    pub fn list_entries(&self, now: Instant) -> Vec<shaiya_proto::quest::ActiveEntry> {
        self.active
            .iter()
            .map(|a| shaiya_proto::quest::ActiveEntry {
                quest_id: a.id,
                minutes_left: a.deadline.map_or(0, |d| (d.saturating_duration_since(now).as_secs() / 60).min(u64::from(u16::MAX)) as u16),
                counts: a.counts,
            })
            .collect()
    }
}

/// The items a start or an end moves: changes to save in order, and where a new item went (`0x0205`) or what a slot holds now
/// (`0x0206`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemPlan {
    pub changes: Vec<SlotChange>,
}

impl ItemPlan {
    /// The slots emptied by the plan's removals.
    fn emptied(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.changes.iter().filter_map(|c| match c {
            SlotChange::Remove { bag, slot } => Some((*bag, *slot)),
            _ => None,
        })
    }

    /// The first empty usable slots once the plan's removals are made, in the original's search order.
    fn free_slots(&self, inventory: &Inventory) -> Vec<(u8, u8)> {
        let mut free = inventory.free_slots();
        let taken: Vec<(u8, u8)> = self
            .changes
            .iter()
            .filter_map(|c| match c {
                SlotChange::Insert(n) => Some((n.bag, n.slot)),
                _ => None,
            })
            .collect();
        free.extend(self.emptied());
        free.retain(|s| !taken.contains(s));
        free.sort_unstable();
        free.dedup();
        free
    }

    /// A new item into the first free slot, of make type `'Q'` (`CUser::QuestAddItem`; such items cannot get new craft stats,
    /// `docs/original/lapis.md` section 6); false when there is none or no such item.
    fn insert(&mut self, inventory: &Inventory, items: &ItemDefs, (item_type, type_id): (u8, u8), count: u8) -> Option<bool> {
        let def = items.get(item_type, type_id)?;
        let Some(&(bag, slot)) = self.free_slots(inventory).first() else { return Some(false) };
        let count = count.max(1);
        let kept = Some(Kept { make_type: QUEST_MAKE_TYPE, ..Kept::default() });
        self.changes.push(SlotChange::Insert(NewItem {
            bag,
            slot,
            item_id: def.item_id,
            item_type,
            type_id,
            quality: def.quality,
            count,
            kept,
        }));
        Some(true)
    }
}

/// Why a start does not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Silently, as the original.
    Ignored,
    /// The items would not fit: "Insufficient inventory" (deviation 3).
    NoRoom,
}

/// A start that may happen: the item taken (type 2), the start items given, and the NPC id `0x0902` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartPlan {
    pub items: ItemPlan,
    pub npc_id: u32,
}

/// The NPC a request names, as the player sees it: object id, type, type id.
pub type Npc = (u32, u8, u16);

/// Section 5 for a quest the player may take. `npc` is the NPC with the packet's id in the player's view; `npc_field` the packet's NPC
/// id itself (types 3 and 4 put the start item's type id there).
/// `used_item` is the type id of the quest item used in the last 5 minutes, if any (`m_byQuestLastUseItemID`, start type 4).
pub fn plan_start(
    def: &QuestDef,
    npc: Option<Npc>,
    npc_field: u32,
    used_item: Option<u8>,
    inventory: &Inventory,
    items: &ItemDefs,
) -> Result<StartPlan, Refused> {
    let npc_matches = |(want_type, want_id): (u8, u16)| npc.filter(|&(_, kind, id)| kind == want_type && id == want_id);
    let mut plan = ItemPlan::default();
    let npc_id = match def.start_type {
        start::ANY_NPC => {
            npc.ok_or(Refused::Ignored)?;
            if def.start_npc.0 != 0 && npc_matches(def.start_npc).is_none() {
                return Err(Refused::Ignored);
            }
            0
        }
        start::CLICK_NPC => npc_matches(def.start_npc).ok_or(Refused::Ignored)?.0,
        start::GIVE_ITEM => {
            let npc = npc_matches(def.start_npc).ok_or(Refused::Ignored)?;
            let item = find_item(inventory, def.start_item).ok_or(Refused::Ignored)?;
            plan.changes.push(if item.count > 1 {
                SlotChange::SetCount { bag: item.bag, slot: item.slot, count: item.count - 1 }
            } else {
                SlotChange::Remove { bag: item.bag, slot: item.slot }
            });
            npc.0
        }
        start::HAVE_ITEM => {
            if npc_field != u32::from(def.start_item.1) || find_item(inventory, def.start_item).is_none() {
                return Err(Refused::Ignored);
            }
            0
        }
        // Only the type id is compared, as in the original.
        start::USE_ITEM => {
            if npc_field != u32::from(def.start_item.1) || used_item != Some(def.start_item.1) {
                return Err(Refused::Ignored);
            }
            0
        }
        _ => return Err(Refused::Ignored),
    };
    for &(item_type, type_id, count) in def.start_items.iter().filter(|i| i.0 != 0) {
        // An item the definitions lack is skipped, as the original's QuestAddItem fails on it.
        if plan.insert(inventory, items, (item_type, type_id), count) == Some(false) {
            return Err(Refused::NoRoom);
        }
    }
    Ok(StartPlan { items: plan, npc_id })
}

/// `F80B` (`docs/original/gm-commands-2.md` deviation 9): only the start items, which must all fit. No other check.
pub fn plan_admin_start(def: &QuestDef, inventory: &Inventory, items: &ItemDefs) -> Result<ItemPlan, Refused> {
    let mut plan = ItemPlan::default();
    for &(item_type, type_id, count) in def.start_items.iter().filter(|i| i.0 != 0) {
        if plan.insert(inventory, items, (item_type, type_id), count) == Some(false) {
            return Err(Refused::NoRoom);
        }
    }
    Ok(plan)
}

/// The first item of this type and type id in the usable bags.
fn find_item(inventory: &Inventory, (item_type, type_id): (u8, u8)) -> Option<&crate::inventory::ItemUnit> {
    inventory.usable_items().find(|i| i.item_type == item_type && i.type_id == type_id)
}

/// How many of each end item the bags hold; an item counts for the first end item it matches.
fn end_item_counts(def: &QuestDef, inventory: &Inventory) -> [u32; 3] {
    let mut counts = [0u32; 3];
    for item in inventory.usable_items() {
        if let Some(i) = def.end_items.iter().position(|e| e.0 == item.item_type && e.1 == item.type_id) {
            counts[i] += u32::from(item.count);
        }
    }
    counts
}

/// Whether the bags hold every end item's count (section 7.2).
fn has_end_items(def: &QuestDef, inventory: &Inventory) -> bool {
    let counts = end_item_counts(def, inventory);
    (0..3).all(|i| counts[i] >= u32::from(def.end_items[i].2))
}

/// The result by what was brought or killed (section 7.1): the highest whose need is met, else 0.
fn auto_result(def: &QuestDef, counts: [u8; 3], inventory: &Inventory) -> usize {
    let items = end_item_counts(def, inventory);
    (0..3)
        .rev()
        .find(|&r| {
            let result = &def.results[r];
            match def.end_type {
                end::ITEMS => {
                    let (index, need) = result.need_item;
                    need >= 1 && (1..=3).contains(&index) && items[usize::from(index) - 1] >= u32::from(need)
                }
                end::KILL_MOBS => {
                    let (index, need) = result.need_mob;
                    need >= 1 && (1..=2).contains(&index) && counts[usize::from(index) - 1] >= need
                }
                _ => false,
            }
        })
        .unwrap_or(0)
}

/// What an end request comes to (section 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndOutcome {
    /// Nothing happens.
    Ignored,
    /// The player chooses the reward (`0x0907`).
    Choose,
    /// It ends with this result.
    Finish(usize),
}

/// Section 7 for a quest in progress. `npc` is the NPC with the packet's id in the player's view. A quest that names an end NPC is only
/// handed in there (deviation 2); one that does not ignores the NPC.
pub fn end_request(def: &QuestDef, active: &Active, npc: Option<Npc>, inventory: &Inventory) -> EndOutcome {
    if def.end_npc.0 != 0 && !npc.is_some_and(|(_, kind, id)| (kind, id) == def.end_npc) {
        return EndOutcome::Ignored;
    }
    let met = match def.end_type {
        end::TALK => def.end_npc.0 != 0,
        end::ITEMS => has_end_items(def, inventory),
        end::KILL_MOBS | end::KILL_PLAYERS => def.kills_done(active.counts),
        _ => false,
    };
    if !met {
        return EndOutcome::Ignored;
    }
    outcome(def, active.counts, inventory)
}

/// Which result a quest whose end is met gets (section 7.1, `CUser::QuestEnd`): also for a kill that completes it.
pub fn outcome(def: &QuestDef, counts: [u8; 3], inventory: &Inventory) -> EndOutcome {
    if def.result_type == 1 {
        EndOutcome::Finish(0)
    } else if def.user_select != 0 {
        EndOutcome::Choose
    } else {
        EndOutcome::Finish(auto_result(def, counts, inventory))
    }
}

/// The NPC id `0x0903` carries for an end by request: the packet's for talking and bringing items, 0 for kills (section 7).
pub fn end_npc_id(def: &QuestDef, npc_field: u32) -> u32 {
    if matches!(def.end_type, end::TALK | end::ITEMS) { npc_field } else { 0 }
}

/// A finished quest's items and reward: the end items taken, then the reward item into the first free slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reward {
    pub items: ItemPlan,
    /// The reward item's slot and what it is; `None` when the result has none or the definitions lack it.
    pub item: Option<((u8, u8), (u8, u8))>,
}

/// Why a successful end cannot happen now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoReward {
    /// The end items are no longer all there (a reward chosen after the end request).
    Items,
    /// The reward item does not fit (deviation 3).
    Room,
}

/// Section 7.2 and 7.4: the items a successful end takes and gives for `result`.
pub fn plan_reward(def: &QuestDef, result: usize, inventory: &Inventory, items: &ItemDefs) -> Result<Reward, NoReward> {
    let mut plan = ItemPlan::default();
    if def.end_type == end::ITEMS {
        if !has_end_items(def, inventory) {
            return Err(NoReward::Items);
        }
        // What to take of each end item: its count, or the chosen result's need for the item it names.
        let mut left: [u32; 3] = def.end_items.map(|e| u32::from(e.2));
        let (index, need) = def.results[result].need_item;
        if (1..=3).contains(&index) {
            left[usize::from(index) - 1] = u32::from(need);
        }
        for item in inventory.usable_items() {
            let Some(i) = def.end_items.iter().position(|e| e.0 == item.item_type && e.1 == item.type_id) else { continue };
            let (bag, slot) = (item.bag, item.slot);
            if real_type(item.item_type) == QUEST_ITEM {
                plan.changes.push(SlotChange::Remove { bag, slot });
            } else if left[i] > 0 {
                let take = left[i].min(u32::from(item.count));
                left[i] -= take;
                plan.changes.push(if take == u32::from(item.count) {
                    SlotChange::Remove { bag, slot }
                } else {
                    SlotChange::SetCount { bag, slot, count: item.count - take as u8 }
                });
            }
        }
    }
    let reward = def.results[result].item;
    let mut item = None;
    if reward.0 != 0 {
        match plan.insert(inventory, items, reward, 1) {
            Some(false) => return Err(NoReward::Room),
            Some(true) => {
                let Some(SlotChange::Insert(n)) = plan.changes.last() else { unreachable!("insert pushed an insert") };
                item = Some(((n.bag, n.slot), reward));
            }
            // An item the definitions lack is not given, as the original's QuestAddItem fails on it.
            None => {}
        }
    }
    Ok(Reward { items: plan, item })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemDef, ItemUnit};

    fn defs() -> ItemDefs {
        let def = |item_type: u8, type_id: u8, max_count: u8| ItemDef {
            item_id: u32::from(item_type) * 1000 + u32::from(type_id),
            item_type,
            type_id,
            max_count,
            quality: 100,
            ..Default::default()
        };
        ItemDefs::new([def(27, 30, 255), def(28, 1, 1), def(25, 1, 50), def(22, 11, 1)])
    }

    fn quest(id: u16) -> QuestDef {
        QuestDef {
            id,
            level_min: 1,
            level_max: 60,
            country: 6,
            jobs: [true; 6],
            start_type: start::CLICK_NPC,
            start_npc: (7, 62),
            ..Default::default()
        }
    }

    fn item(bag: u8, slot: u8, item_type: u8, type_id: u8, count: u8) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id, count, ..Default::default() }
    }

    const WHO: Who = Who { level: 10, family: 0, job: 2, grow: 1 };

    #[test]
    fn who_may_take_a_quest() {
        let mut quests = Quests::default();
        let q = quest(5);
        assert!(quests.may_take(&q, WHO));
        assert!(!quests.may_take(&QuestDef { level_min: 11, ..q.clone() }, WHO));
        assert!(!quests.may_take(&QuestDef { level_max: 9, ..q.clone() }, WHO));
        // Race: 2 is human or elf, 5 vail or deatheater, 4 vail alone.
        assert!(quests.may_take(&QuestDef { country: 2, ..q.clone() }, WHO));
        assert!(!quests.may_take(&QuestDef { country: 5, ..q.clone() }, WHO));
        assert!(quests.may_take(&QuestDef { country: 4, ..q.clone() }, Who { family: 2, ..WHO }));
        assert!(!quests.may_take(&QuestDef { country: 3, ..q.clone() }, Who { family: 2, ..WHO }));
        let mut jobs = [true; 6];
        jobs[2] = false;
        assert!(!quests.may_take(&QuestDef { jobs, ..q.clone() }, WHO));
        assert!(!quests.may_take(&QuestDef { grow: 2, ..q.clone() }, WHO));
        // Previous quests: any one of the three set, finished with success.
        let after = QuestDef { prev: [3, 0, 4], ..q.clone() };
        assert!(!quests.may_take(&after, WHO));
        quests.remove(&quest(3), false);
        assert!(!quests.may_take(&after, WHO));
        quests.remove(&quest(4), true);
        assert!(quests.may_take(&after, WHO));
        // Never twice: in progress, or finished either way.
        let now = Instant::now();
        quests.take(&q, now);
        assert!(!quests.may_take(&q, WHO));
        quests.remove(&q, false);
        assert!(!quests.may_take(&q, WHO));
        // A repeatable quest is not kept as finished.
        let again = QuestDef { repeatable: true, ..quest(9) };
        quests.take(&again, now);
        quests.remove(&again, true);
        assert!(quests.may_take(&again, WHO));
    }

    #[test]
    fn starts_need_their_npc_and_items() {
        let items = defs();
        let empty = Inventory::new(&[]);
        let q = quest(1);
        let right = Some((40, 7, 62));
        assert_eq!(plan_start(&q, None, 40, None, &empty, &items), Err(Refused::Ignored));
        assert_eq!(plan_start(&q, Some((40, 7, 63)), 40, None, &empty, &items), Err(Refused::Ignored));
        assert_eq!(plan_start(&q, right, 40, None, &empty, &items).unwrap().npc_id, 40);
        // Type 0 takes any NPC unless one is named, and 0x0902 names none.
        let any = QuestDef { start_type: start::ANY_NPC, start_npc: (0, 0), ..q.clone() };
        assert_eq!(plan_start(&any, Some((41, 1, 3)), 41, None, &empty, &items).unwrap().npc_id, 0);
        assert_eq!(plan_start(&any, None, 41, None, &empty, &items), Err(Refused::Ignored));
        // Type 2 takes one of the start item.
        let give = QuestDef { start_type: start::GIVE_ITEM, start_item: (25, 1), ..q.clone() };
        assert_eq!(plan_start(&give, right, 40, None, &empty, &items), Err(Refused::Ignored));
        let bags = Inventory::new(&[item(1, 3, 25, 1, 4)]);
        assert_eq!(
            plan_start(&give, right, 40, None, &bags, &items).unwrap().items.changes,
            [SlotChange::SetCount { bag: 1, slot: 3, count: 3 }]
        );
        // Type 3: the packet's NPC field is the start item's type id, and the item is kept.
        let have = QuestDef { start_type: start::HAVE_ITEM, start_item: (25, 1), ..q.clone() };
        assert!(plan_start(&have, None, 1, None, &bags, &items).unwrap().items.changes.is_empty());
        assert_eq!(plan_start(&have, None, 2, None, &bags, &items), Err(Refused::Ignored));
        // Type 4: the same, and that item used in the last 5 minutes (the type id alone counts).
        let used = QuestDef { start_type: start::USE_ITEM, ..have };
        assert_eq!(plan_start(&used, None, 1, None, &bags, &items), Err(Refused::Ignored));
        assert_eq!(plan_start(&used, None, 1, Some(2), &bags, &items), Err(Refused::Ignored));
        assert_eq!(plan_start(&used, None, 2, Some(1), &bags, &items), Err(Refused::Ignored));
        assert_eq!(plan_start(&used, None, 1, Some(1), &empty, &items).unwrap().npc_id, 0);
    }

    #[test]
    fn an_admin_start_checks_nothing_but_the_room_for_the_start_items() {
        let items = defs();
        let q = QuestDef { level_min: 60, start_type: start::GIVE_ITEM, start_items: [(27, 30, 5), (0, 0, 0), (0, 0, 0)], ..quest(1) };
        let plan = plan_admin_start(&q, &Inventory::new(&[]), &items).unwrap();
        assert_eq!(plan.changes.len(), 1, "no NPC, level or item asked for");
        let full: Vec<ItemUnit> = (1..=2).flat_map(|bag| (0..24).map(move |slot| item(bag, slot, 25, 1, 1))).collect();
        assert_eq!(plan_admin_start(&q, &Inventory::new(&full), &items), Err(Refused::NoRoom));
        let none = QuestDef { start_items: [(0, 0, 0); 3], ..q };
        assert_eq!(plan_admin_start(&none, &Inventory::new(&full), &items).unwrap().changes, []);
    }

    #[test]
    fn a_dropped_quest_leaves_no_mark_and_a_finished_one_can_be_given_again() {
        let mut quests = Quests::default();
        let q = quest(5);
        quests.take(&q, Instant::now());
        assert!(quests.drop(5) && !quests.drop(5));
        assert!(quests.may_take(&q, WHO), "no finished mark");
        quests.take(&q, Instant::now());
        quests.remove(&q, true);
        assert!(!quests.may_take(&q, WHO));
        quests.forget_finished(5);
        assert!(quests.may_take(&q, WHO));
        assert_eq!(quests.finished().count(), 0);
    }

    #[test]
    fn start_items_go_to_empty_slots_or_the_start_does_not_happen() {
        let items = defs();
        let q = QuestDef { start_items: [(28, 1, 1), (27, 30, 5), (0, 0, 0)], ..quest(1) };
        let bags = Inventory::new(&[item(1, 0, 25, 1, 1)]);
        let plan = plan_start(&q, Some((40, 7, 62)), 40, None, &bags, &items).unwrap();
        let slots: Vec<_> = plan
            .items
            .changes
            .iter()
            .map(|c| match c {
                SlotChange::Insert(n) => (n.bag, n.slot, n.count),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(slots, [(1, 1, 1), (1, 2, 5)]);
        // Full bags (two usable bags of 24): nothing starts.
        let full: Vec<ItemUnit> = (1..=2).flat_map(|bag| (0..24).map(move |slot| item(bag, slot, 25, 1, 1))).collect();
        assert_eq!(plan_start(&q, Some((40, 7, 62)), 40, None, &Inventory::new(&full), &items), Err(Refused::NoRoom));
        // Giving the last of the start item frees its slot for the start items.
        let mut last = full.clone();
        last[0] = item(1, 0, 22, 11, 1);
        let give = QuestDef { start_type: start::GIVE_ITEM, start_item: (22, 11), start_items: [(28, 1, 1), (0, 0, 0), (0, 0, 0)], ..q };
        let plan = plan_start(&give, Some((40, 7, 62)), 40, None, &Inventory::new(&last), &items).unwrap();
        assert!(matches!(
            plan.items.changes[..],
            [SlotChange::Remove { bag: 1, slot: 0 }, SlotChange::Insert(NewItem { bag: 1, slot: 0, .. })]
        ));
    }

    #[test]
    fn kills_count_and_end_by_themselves_without_an_end_npc() {
        let mut list = vec![quest(1), quest(2)];
        list[0].end_type = end::KILL_MOBS;
        list[0].mob_kills = [(2008, 2), (0, 0)];
        list[1].end_type = end::KILL_MOBS;
        list[1].mob_kills = [(2008, 1), (2009, 1)];
        list[1].end_npc = (7, 62);
        let defs = QuestDefs::new(list);
        let mut quests = Quests::default();
        let now = Instant::now();
        quests.take(defs.get(1).unwrap(), now);
        quests.take(defs.get(2).unwrap(), now);
        assert_eq!(quests.mob_killed(&defs, 2008), (vec![(1, 0, 1), (2, 0, 1)], vec![]));
        assert_eq!(quests.mob_killed(&defs, 7), (vec![], vec![]));
        // Quest 1 has no end NPC: done at its second kill. Quest 2 waits for its NPC.
        assert_eq!(quests.mob_killed(&defs, 2008), (vec![(1, 0, 2), (2, 0, 2)], vec![1]));
        let (moves, done) = quests.mob_killed(&defs, 2009);
        assert_eq!((moves, done), (vec![(2, 1, 1)], vec![]));
        let two = *quests.find(2).unwrap();
        let empty = Inventory::new(&[]);
        assert_eq!(end_request(defs.get(2).unwrap(), &two, None, &empty), EndOutcome::Ignored);
        assert_eq!(end_request(defs.get(2).unwrap(), &two, Some((40, 7, 62)), &empty), EndOutcome::Finish(0));
    }

    #[test]
    fn player_kills_count_on_the_first_such_quest_only() {
        let mut list = vec![quest(1), quest(2), quest(3)];
        list[1].end_type = end::KILL_PLAYERS;
        list[1].pc_kills = 2;
        list[2].end_type = end::KILL_PLAYERS;
        list[2].pc_kills = 1;
        let defs = QuestDefs::new(list);
        let mut quests = Quests::default();
        let now = Instant::now();
        assert_eq!(quests.player_killed(&defs), None);
        for id in 1..=3 {
            quests.take(defs.get(id).unwrap(), now);
        }
        assert_eq!(quests.player_killed(&defs), Some(((2, 0, 1), false)));
        assert_eq!(quests.player_killed(&defs), Some(((2, 0, 2), true)), "no end NPC: done");
        assert_eq!(quests.find(3).unwrap().counts[0], 0, "only the first counts");
    }

    #[test]
    fn bring_more_for_more_takes_the_quest_items_whole() {
        let items = defs();
        let mut q = quest(41);
        q.end_type = end::ITEMS;
        q.end_npc = (7, 62);
        q.end_items = [(27, 30, 5), (0, 0, 0), (0, 0, 0)];
        q.result_type = 0;
        for (r, need) in [5, 10, 15].into_iter().enumerate() {
            q.results[r] = QuestResult { need_item: (1, need), exp: 100 * (r as u32 + 1), ..Default::default() };
        }
        q.results[2].item = (22, 11);
        let active = Active { id: 41, counts: [0; 3], deadline: None, choosing: false };
        let npc = Some((40, 7, 62));
        let bags = |count: u8| Inventory::new(&[item(1, 0, 27, 30, count.min(10)), item(1, 1, 27, 30, count.saturating_sub(10))]);
        assert_eq!(end_request(&q, &active, npc, &bags(4)), EndOutcome::Ignored);
        assert_eq!(end_request(&q, &active, npc, &bags(9)), EndOutcome::Finish(0));
        assert_eq!(end_request(&q, &active, npc, &bags(14)), EndOutcome::Finish(1));
        assert_eq!(end_request(&q, &active, npc, &bags(17)), EndOutcome::Finish(2));
        // All of the quest items go; the reward takes the first slot freed.
        let reward = plan_reward(&q, 2, &bags(17), &items).unwrap();
        assert_eq!(reward.item, Some(((1, 0), (22, 11))));
        assert_eq!(reward.items.changes.len(), 3);
        assert_eq!(&reward.items.changes[..2], &[SlotChange::Remove { bag: 1, slot: 0 }, SlotChange::Remove { bag: 1, slot: 1 }]);
        // Other items are taken only as far as needed.
        let mut potions = q.clone();
        potions.end_items[0] = (25, 1, 5);
        potions.results = [QuestResult { need_item: (1, 7), ..Default::default() }, QuestResult::default(), QuestResult::default()];
        let stock = Inventory::new(&[item(1, 0, 25, 1, 4), item(1, 5, 25, 1, 6)]);
        assert_eq!(
            plan_reward(&potions, 0, &stock, &items).unwrap().items.changes,
            [SlotChange::Remove { bag: 1, slot: 0 }, SlotChange::SetCount { bag: 1, slot: 5, count: 3 }]
        );
        // The experience doubles unless the quest is for ultimate mode only.
        assert_eq!(q.exp(1), 400);
        assert_eq!(QuestDef { grow: 3, ..q.clone() }.exp(1), 200);
    }

    #[test]
    fn a_reward_that_does_not_fit_stops_the_end() {
        let items = defs();
        let q = QuestDef {
            end_type: end::TALK,
            end_npc: (7, 62),
            result_type: 1,
            results: [QuestResult { item: (22, 11), ..Default::default() }, QuestResult::default(), QuestResult::default()],
            ..quest(1)
        };
        let full: Vec<ItemUnit> = (1..=2).flat_map(|bag| (0..24).map(move |slot| item(bag, slot, 25, 1, 1))).collect();
        assert_eq!(plan_reward(&q, 0, &Inventory::new(&full), &items), Err(NoReward::Room));
        let reward = plan_reward(&q, 0, &Inventory::new(&[]), &items).unwrap();
        assert_eq!(reward.item, Some(((1, 0), (22, 11))));
        // No reward item, or one the definitions lack: nothing to fit.
        assert_eq!(
            plan_reward(&QuestDef { results: Default::default(), ..q.clone() }, 0, &Inventory::new(&full), &items).unwrap().item,
            None
        );
    }

    #[test]
    fn choosing_and_time_limits() {
        let q = QuestDef { end_type: end::TALK, end_npc: (7, 62), user_select: 3, time_limit: 60, ..quest(1) };
        let defs = QuestDefs::new(vec![q.clone()]);
        let now = Instant::now();
        let mut quests = Quests::default();
        quests.take(&q, now);
        let active = *quests.find(1).unwrap();
        assert_eq!(end_request(&q, &active, Some((40, 7, 62)), &Inventory::new(&[])), EndOutcome::Choose);
        assert_eq!(quests.next_deadline(), Some(now + Duration::from_secs(3600)));
        assert!(quests.expired(now + Duration::from_secs(3599)).is_empty());
        assert_eq!(quests.expired(now + Duration::from_secs(3600)), vec![1]);
        assert_eq!(quests.times_left(now + Duration::from_secs(600)), vec![(1, 3000)]);
        assert_eq!(quests.list_entries(now + Duration::from_secs(630))[0].minutes_left, 49);
        // Stored and loaded: the time left runs from the load.
        let saved = [
            SavedQuest { quest_id: 1, finished: false, success: false, counts: [1, 2, 3], seconds_left: Some(120) },
            SavedQuest { quest_id: 9, finished: true, success: true, counts: [0; 3], seconds_left: None },
        ];
        let later = now + Duration::from_secs(5000);
        let (loaded, unknown) = Quests::load(&saved, &defs, later);
        assert_eq!(unknown, vec![9]);
        assert_eq!(loaded.find(1).unwrap().deadline, Some(later + Duration::from_secs(120)));
        assert_eq!(loaded.find(1).unwrap().counts, [1, 2, 3]);
    }

    #[test]
    fn a_reward_choice_beyond_the_three_results_is_cut_at_load() {
        let defs = QuestDefs::new(vec![QuestDef { user_select: 200, ..quest(1) }, QuestDef { user_select: 2, ..quest(2) }]);
        assert_eq!(defs.get(1).unwrap().user_select, 3);
        assert_eq!(defs.get(2).unwrap().user_select, 2);
        let def = defs.get(1).unwrap();
        assert!((0..def.user_select).all(|index| def.results.get(usize::from(index)).is_some()), "every offered choice has a result");
    }
}
