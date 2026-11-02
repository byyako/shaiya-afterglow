//! What a dead monster leaves on the ground (`CMob::ItemDrop`, `docs/original/combat.md` section 7): items by drop grade and gold.

use std::collections::HashMap;

use crate::inventory::{ItemDefs, Kept, real_type};
use crate::mobs::MobDef;

/// Item type of gold on the ground.
pub const GOLD_TYPE: u8 = 26;
/// Drop entries 1 to 6 roll in percent, 7 to 9 in millionths (`SRandom::Random(1000000)`); entry 0 is never used.
const PERCENT_ENTRIES: std::ops::RangeInclusive<usize> = 1..=6;
const DROP_ENTRIES: usize = 10;
/// Grades go up to 1024 (`CItemDrop::Init`).
const MAX_GRADE: u16 = 1024;
/// Real type 27 (quest and similar items) never drops by grade.
const NO_DROP_REAL_TYPE: u8 = 27;
/// Chance of gold, in percent.
const GOLD_CHANCE: u32 = 30;
/// The make type of a monster's drop.
const DROP_MAKE_TYPE: u8 = b'D';
/// The make type of a sequence item (`docs/original/quest-drops.md` section 4.4).
const SEQUENCE_MAKE_TYPE: u8 = b'V';
/// `CItemDrop::m_pSequenceItem` holds at most 8 groups.
const MAX_SEQUENCE_GROUPS: usize = 8;

/// Something on the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loot {
    /// `kept`: what the item keeps of itself (`docs/original/item-drop.md`): a player's drop all of it, a monster's its make type and
    /// craft name.
    Item {
        item_type: u8,
        type_id: u8,
        count: u8,
        quality: u16,
        kept: Option<Kept>,
    },
    Gold(u32),
}

/// The items of each drop grade (`CItemDrop::Init`): items without a server-wide limit first, then the limited ones, each part by type
/// and type id. A limited item (`wDrop` above 0) drops only within its limits, which are not kept here: it never drops. Also the
/// boxes' sets (`Data/ItemCreate.ini`, `docs/original/item-origins.md` section 5), once loaded.
#[derive(Debug, Default)]
pub struct DropTables {
    grades: HashMap<u16, Vec<(u8, u8, bool)>>,
    boxes: HashMap<u8, BoxSet>,
    /// Item -> quest links by item type and type id: the quests it starts and the quests it ends (`CItemQuestData`, section 1.3).
    links: HashMap<(u8, u8), (Vec<u16>, Vec<u16>)>,
    /// The sequence groups: the monster id they divide and the grade per level tens (`stSequenceItemCreate`, section 4.1).
    sequences: Vec<(u32, [u16; 10])>,
}

/// One `[ItemCreate_n]` set: its grades in file order with the running total of the weights, and the total.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BoxSet {
    lines: Vec<(u16, u32)>,
    total: u32,
}

/// A row of `world.item_quest_links` as imported: item type, type id, quests started, quests ended.
pub type QuestLinkRow = (i16, i16, Vec<i16>, Vec<i16>);
/// A row of `world.sequence_groups` as imported: group number, sequence, grades.
pub type SequenceRow = (i32, i64, Vec<i32>);

/// A line of `world.item_create` as imported: set, grade, weight (in file order).
pub type BoxLine = (i32, i32, i32);

/// What a box gives (`docs/original/item-origins.md` section 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxRoll {
    /// No such set: the error answer, the box kept.
    NoSet,
    /// The roll gave grade 0: the box is used up for nothing.
    Nothing,
    /// The grade has no item: the error answer, the box kept.
    NoItem,
    Item(u8, u8),
}

impl DropTables {
    pub fn new(defs: &ItemDefs) -> Self {
        let mut eligible: Vec<_> = defs
            .iter()
            .filter(|d| (1..=MAX_GRADE).contains(&d.grade) && d.item_type != 0 && d.type_id != 0)
            .filter(|d| real_type(d.item_type) != NO_DROP_REAL_TYPE)
            .map(|d| (d.server != 0 || d.drop_limit != 0, d.item_type, d.type_id, d.grade, d.drop_limit != 0))
            .collect();
        eligible.sort_unstable();
        let mut grades: HashMap<u16, Vec<(u8, u8, bool)>> = HashMap::new();
        for (_, item_type, type_id, grade, limited) in eligible {
            grades.entry(grade).or_default().push((item_type, type_id, limited));
        }
        Self { grades, boxes: HashMap::new(), links: HashMap::new(), sequences: Vec::new() }
    }

    /// How many drop grades have an item that can drop.
    pub fn grade_count(&self) -> usize {
        self.grades.len()
    }

    /// How many box sets were loaded.
    pub fn box_count(&self) -> usize {
        self.boxes.len()
    }

    /// The boxes' sets from `world.item_create`, with the rules of `CItemDrop::InitItemCreate` (section 5.1): sets 1 to 255, grades 0
    /// to 1024 with a weight above 0, a grade without items counts as 0, and a grade given again adds its weight to its first line.
    /// Returns how many sets there are.
    pub fn load_boxes(&mut self, lines: &[BoxLine]) -> usize {
        let mut sets: HashMap<u8, Vec<(u16, u32)>> = HashMap::new();
        for &(set, grade, weight) in lines {
            let (Ok(set @ 1..=255), Ok(grade @ 0..=MAX_GRADE), Ok(weight @ 1..)) =
                (u8::try_from(set), u16::try_from(grade), u32::try_from(weight))
            else {
                continue;
            };
            let grade = if self.grades.get(&grade).is_some_and(|items| !items.is_empty()) { grade } else { 0 };
            let set = sets.entry(set).or_default();
            match set.iter_mut().find(|(g, _)| *g == grade) {
                Some((_, w)) => *w += weight,
                None => set.push((grade, weight)),
            }
        }
        self.boxes = sets
            .into_iter()
            .map(|(n, weights)| {
                let mut total = 0;
                let lines = weights
                    .into_iter()
                    .map(|(grade, w)| {
                        (grade, {
                            total += w;
                            total
                        })
                    })
                    .collect();
                (n, BoxSet { lines, total })
            })
            .collect();
        self.boxes.len()
    }

    /// The item -> quest links of `world.item_quest_links` (`CItemQuestData::Load`, `GetQuestLink`): item types and type ids 1 to 255
    /// only, quest ids above 0 (`docs/original/quest-drops.md` section 1.3). Returns how many items have a link.
    pub fn load_quest_links(&mut self, rows: &[QuestLinkRow]) -> usize {
        let ids = |list: &[i16]| list.iter().filter_map(|&q| u16::try_from(q).ok().filter(|&q| q > 0)).collect::<Vec<_>>();
        self.links = rows
            .iter()
            .filter_map(|(t, i, started, ended)| {
                let (t, i) = (u8::try_from(*t).ok().filter(|&t| t > 0)?, u8::try_from(*i).ok().filter(|&i| i > 0)?);
                let (started, ended) = (ids(started), ids(ended));
                (!started.is_empty() || !ended.is_empty()).then_some(((t, i), (started, ended)))
            })
            .collect();
        self.links.len()
    }

    /// Whether the item starts a quest (`bQuestStart`: a non-empty started list). Whether the item exists is the caller's.
    pub fn quest_start(&self, item_type: u8, type_id: u8) -> bool {
        self.links.get(&(item_type, type_id)).is_some_and(|(started, _)| !started.is_empty())
    }

    /// The quests the item ends, in file order.
    pub fn quest_ends(&self, item_type: u8, type_id: u8) -> &[u16] {
        self.links.get(&(item_type, type_id)).map_or(&[], |(_, ended)| ended)
    }

    /// The sequence groups of `world.sequence_groups` with the rules of `CItemDrop::InitSequenceItemCreate` (section 4.1): at most 8,
    /// and a group with sequence 0 turns all of them off. Returns how many groups are in use.
    pub fn load_sequences(&mut self, rows: &[SequenceRow]) -> usize {
        let mut rows: Vec<_> = rows.iter().collect();
        rows.sort_by_key(|r| r.0);
        let groups: Option<Vec<_>> = rows
            .into_iter()
            .take(MAX_SEQUENCE_GROUPS)
            .map(|(_, sequence, grades)| {
                let sequence = u32::try_from(*sequence).ok().filter(|&s| s != 0)?;
                Some((sequence, std::array::from_fn(|i| grades.get(i).and_then(|&g| u16::try_from(g).ok()).unwrap_or(0))))
            })
            .collect();
        self.sequences = groups.unwrap_or_else(|| {
            tracing::warn!("a sequence group has sequence 0 (or one too large): no sequence items");
            Vec::new()
        });
        self.sequences.len()
    }

    /// The sequence item of a monster (`CItemDrop::GetSequenceItem`, section 4.2): its level 1 to 99 and object id decide. The first
    /// group whose sequence divides the id gives the grade of the level's tens; a grade of 0 ends the search.
    pub fn sequence_item(&self, level: u16, mob_object_id: u32, rand: &mut impl FnMut() -> u32) -> Option<(u8, u8)> {
        if !(1..100).contains(&level) || mob_object_id == 0 {
            return None;
        }
        let (_, grades) = self.sequences.iter().find(|(sequence, _)| mob_object_id.is_multiple_of(*sequence))?;
        match grades[usize::from(level / 10)] {
            0 => None,
            grade => self.pick(grade, rand),
        }
    }

    /// A random item of the grade (`CItemDrop::GetItem`); `None` for an empty grade or a limited item.
    fn pick(&self, grade: u16, rand: &mut impl FnMut() -> u32) -> Option<(u8, u8)> {
        let items = self.grades.get(&grade).filter(|items| !items.is_empty())?;
        let (item_type, type_id, limited) = items[rand() as usize % items.len()];
        (!limited).then_some((item_type, type_id))
    }

    /// `CItemDrop::GetItemPlz`: `pick`, and when that gives nothing, another random item of the grade whatever its limits.
    fn pick_any(&self, grade: u16, rand: &mut impl FnMut() -> u32) -> Option<(u8, u8)> {
        if let Some(item) = self.pick(grade, rand) {
            return Some(item);
        }
        let items = self.grades.get(&grade).filter(|items| !items.is_empty())?;
        let (item_type, type_id, _) = items[rand() as usize % items.len()];
        Some((item_type, type_id))
    }

    /// What a box of set `set` (its `reqvg`) gives (section 5.2): `CItemDrop::GetItemCreateByGrade` with one `rand()`, then `GetItemPlz`.
    pub fn open_box(&self, set: i16, rand: &mut impl FnMut() -> u32) -> BoxRoll {
        let Some(set) = u8::try_from(set).ok().and_then(|n| self.boxes.get(&n)).filter(|s| !s.lines.is_empty()) else {
            return BoxRoll::NoSet;
        };
        let r = rand();
        let grade = set.lines.iter().find(|&&(_, running)| r % set.total < running).map_or(0, |&(grade, _)| grade);
        if grade == 0 {
            return BoxRoll::Nothing;
        }
        match self.pick_any(grade, rand) {
            Some((item_type, type_id)) => BoxRoll::Item(item_type, type_id),
            None => BoxRoll::NoItem,
        }
    }
}

/// A monster's item as `CItem::Init` and `CreateCraftExpansion` make it: one piece at the definition's quality.
fn made(item: &crate::inventory::ItemDef, make_type: u8, rand: &mut impl FnMut() -> u32) -> Loot {
    let kept = Kept { make_type, craft_name: crate::lapis::new_craft_name(item, rand), ..Kept::default() };
    Loot::Item { item_type: item.item_type, type_id: item.type_id, count: 1, quality: item.quality, kept: Some(kept) }
}

/// What a monster drops (`CMob::ItemDrop`): its quest item, its drop entries 1 to 9, a sequence item, then gold. Each item is made with
/// make type `'D'` (`'V'` for the sequence item) and random craft stats (`docs/original/item-origins.md` section 4). `has_quest` says
/// whether the killer or a party member has one of the given quests in progress. `rand` is like the C library's (0 to 32767).
pub fn roll(
    def: &MobDef,
    mob_object_id: u32,
    tables: &DropTables,
    items: &ItemDefs,
    has_quest: &mut impl FnMut(&[u16]) -> bool,
    rand: &mut impl FnMut() -> u32,
) -> Vec<Loot> {
    let mut loot = Vec::new();
    // The quest item (section 3.1): a quest-starting item rolls at its quality; any other only when the killer or a party member has
    // one of its quests in progress, and then once.
    if let Some((item_type, type_id)) = def.quest_item
        && let Some(item) = items.get(item_type, type_id)
    {
        let chance = u32::from(item.quality);
        let rolls = tables.quest_start(item_type, type_id) || has_quest(tables.quest_ends(item_type, type_id));
        if rolls && rand() % 100 < chance {
            loot.push(made(item, DROP_MAKE_TYPE, rand));
        }
    }
    for (entry, &(grade, rate)) in def.drops.iter().enumerate().take(DROP_ENTRIES).skip(1) {
        if grade == 0 {
            continue;
        }
        let roll = if PERCENT_ENTRIES.contains(&entry) { rand() % 100 } else { ((rand() << 15) | rand()) % 1_000_000 };
        if roll >= rate {
            continue;
        }
        if let Some((item_type, type_id)) = tables.pick(grade, rand)
            && let Some(item) = items.get(item_type, type_id)
        {
            loot.push(made(item, DROP_MAKE_TYPE, rand));
        }
    }
    if def.money != 0
        && let Some((item_type, type_id)) = tables.sequence_item(def.level, mob_object_id, rand)
        && let Some(item) = items.get(item_type, type_id)
    {
        tracing::debug!(mob = def.id, object = mob_object_id, item_type, type_id, "sequence item");
        loot.push(made(item, SEQUENCE_MAKE_TYPE, rand));
    }
    if rand() % 100 < GOLD_CHANCE {
        let mut gold = u32::from(def.money);
        if def.money_add > 0 {
            gold += rand() % u32::from(def.money_add);
        }
        if gold > 0 {
            loot.push(Loot::Gold(gold));
        }
    }
    loot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    fn defs() -> ItemDefs {
        let item =
            |item_type, type_id, grade, drop_limit| ItemDef { item_type, type_id, grade, drop_limit, quality: 50, ..Default::default() };
        ItemDefs::new([item(1, 1, 5, 0), item(1, 2, 5, 0), item(30, 1, 5, 3), item(27, 1, 5, 0), item(2, 1, 0, 0), item(2, 2, 6, 0)])
    }

    #[test]
    fn grades_list_their_droppable_items() {
        let t = DropTables::new(&defs());
        // Unlimited first by type and type id, then the limited one; real type 27 and grade 0 are left out.
        assert_eq!(t.grades[&5], [(1, 1, false), (1, 2, false), (30, 1, true)]);
        assert_eq!(t.grades[&6], [(2, 2, false)]);
        assert_eq!(t.pick(5, &mut || 2), None, "limited");
        assert_eq!(t.pick(5, &mut || 4), Some((1, 2)));
        assert_eq!(t.pick(7, &mut || 0), None);
    }

    #[test]
    fn entries_then_gold() {
        let (items, t) = (defs(), DropTables::new(&defs()));
        let mut drops = [(0, 0); 10];
        // Entry 0 is never used; entry 1 at 40 %, entry 7 at 500,000 in a million.
        drops[0] = (5, 100);
        drops[1] = (5, 40);
        drops[7] = (6, 500_000);
        let def = MobDef { drops, money: 10, money_add: 5, ..Default::default() };
        // Entry 1: 39 < 40, item 0 of grade 5; entry 7: (0 << 15 | 7) < 500,000, grade 6; gold 29 < 30: 10 + 3 % 5.
        let mut rolls = [39, 0, 0, 7, 0, 29, 3].into_iter();
        // Not craftable: make type 'D' and no craft name.
        let dropped = Kept { make_type: b'D', ..Kept::default() };
        assert_eq!(
            roll(&def, 0, &t, &items, &mut |_| false, &mut || rolls.next().unwrap()),
            [
                Loot::Item { item_type: 1, type_id: 1, count: 1, quality: 50, kept: Some(dropped) },
                Loot::Item { item_type: 2, type_id: 2, count: 1, quality: 50, kept: Some(dropped) },
                Loot::Gold(13)
            ]
        );
        // Nothing: entry 1 misses, entry 7 misses (16 << 15 = 524,288), no gold.
        let mut rolls = [40, 16, 0, 30].into_iter();
        assert_eq!(roll(&def, 0, &t, &items, &mut |_| false, &mut || rolls.next().unwrap()), []);
    }

    #[test]
    fn crafted_drops_roll_their_stats_after_the_pick() {
        let sword =
            ItemDef { item_type: 1, type_id: 1, grade: 5, quality: 50, craftable: true, server: 6, craft_max: 10, ..Default::default() };
        let items = ItemDefs::new([sword]);
        let t = DropTables::new(&items);
        let mut drops = [(0, 0); 10];
        drops[1] = (5, 100);
        let def = MobDef { drops, ..Default::default() };
        // Entry roll, the pick, then three draws for each of the six stats (all 9 % 11), then the gold roll misses.
        let mut rolls = [0u32, 0].into_iter().chain(std::iter::repeat_n(9, 18)).chain([99]);
        let loot = roll(&def, 0, &t, &items, &mut |_| false, &mut || rolls.next().unwrap());
        let Loot::Item { kept: Some(kept), .. } = loot[0] else { panic!("{loot:?}") };
        assert_eq!((kept.make_type, &kept.craft_name), (b'D', b"09090909090900000000"));
        assert_eq!(rolls.next(), None, "every draw used");
    }

    #[test]
    fn box_sets_weigh_their_grades() {
        let mut t = DropTables::new(&defs());
        // Set 3: grade 5 weight 10, grade 900 (no items: grade 0) 5, grade 5 again 2 (added to the first), grade 6 weight 3.
        // Lines out of range are dropped: set 0, set 300, grade 2000, weight 0.
        let lines = [(3, 5, 10), (3, 900, 5), (3, 5, 2), (3, 6, 3), (0, 5, 1), (300, 5, 1), (3, 2000, 1), (3, 6, 0), (4, 0, 7)];
        assert_eq!(t.load_boxes(&lines), 2);
        assert_eq!(t.boxes[&3], BoxSet { lines: vec![(5, 12), (0, 17), (6, 20)], total: 20 });
        // r % 20: 0 to 11 grade 5, 12 to 16 nothing, 17 to 19 grade 6.
        let mut rolls = [31u32, 1].into_iter();
        assert_eq!(t.open_box(3, &mut || rolls.next().unwrap()), BoxRoll::Item(1, 2));
        assert_eq!(t.open_box(3, &mut || 12), BoxRoll::Nothing);
        assert_eq!(t.open_box(3, &mut || 19), BoxRoll::Item(2, 2));
        assert_eq!(t.open_box(4, &mut || 0), BoxRoll::Nothing, "a set of grade 0 only");
        assert_eq!(t.open_box(5, &mut || 0), BoxRoll::NoSet);
        assert_eq!(t.open_box(-1, &mut || 0), BoxRoll::NoSet);
        // Grade 5's third item is limited: the first pick gives nothing, the second takes any item.
        let mut rolls = [0u32, 2, 2].into_iter();
        assert_eq!(t.open_box(3, &mut || rolls.next().unwrap()), BoxRoll::Item(30, 1));
    }

    /// Items for the quest and sequence tests: 27/1 starts quest 5 (quality 100), 27/2 ends quests 6 and 7 (quality 50), 27/3 ends
    /// quest 8 with quality 0; grade 9 holds 1/1 and 1/2, grade 8 holds 2/2 (a craftable one).
    fn quest_tables() -> (ItemDefs, DropTables) {
        let item = |item_type, type_id, grade, quality| ItemDef { item_type, type_id, grade, quality, ..Default::default() };
        let items = ItemDefs::new([
            item(27, 1, 0, 100),
            item(27, 2, 0, 50),
            item(27, 3, 0, 0),
            item(1, 1, 9, 10),
            item(1, 2, 9, 10),
            item(2, 2, 8, 10),
        ]);
        let mut t = DropTables::new(&items);
        assert_eq!(
            t.load_quest_links(&[
                (27, 1, vec![5], vec![]),
                (27, 2, vec![], vec![6, 7]),
                (27, 3, vec![], vec![8]),
                (0, 1, vec![1], vec![]),
                (28, 0, vec![], vec![1]),
                (28, 1, vec![0, -2], vec![])
            ]),
            3
        );
        (items, t)
    }

    fn quest_def(item: (u8, u8)) -> MobDef {
        MobDef { quest_item: Some(item), ..Default::default() }
    }

    fn quest_kept() -> Option<Kept> {
        Some(Kept { make_type: b'D', ..Kept::default() })
    }

    #[test]
    fn quest_links_keep_valid_ids_only() {
        let (_, t) = quest_tables();
        assert!(t.quest_start(27, 1));
        assert!(!t.quest_start(27, 2));
        assert_eq!(t.quest_ends(27, 2), [6, 7]);
        assert_eq!(t.quest_ends(27, 9), [] as [u16; 0]);
        assert!(!t.quest_start(28, 1), "quest ids of 0 and below are dropped, so nothing is left of the link");
    }

    #[test]
    fn a_quest_starting_item_drops_without_asking_for_quests() {
        let (items, t) = quest_tables();
        let def = quest_def((27, 1));
        let mut asked = 0;
        // 99 < 100: it drops. The gold roll then misses.
        let mut rolls = [99u32, 99].into_iter();
        let loot = roll(
            &def,
            0,
            &t,
            &items,
            &mut |_| {
                asked += 1;
                false
            },
            &mut || rolls.next().unwrap(),
        );
        assert_eq!(loot, [Loot::Item { item_type: 27, type_id: 1, count: 1, quality: 100, kept: quest_kept() }]);
        assert_eq!(asked, 0);
    }

    #[test]
    fn a_linked_quest_item_needs_a_quest_in_progress_and_rolls_once() {
        let (items, t) = quest_tables();
        let def = quest_def((27, 2));
        let mut asked = Vec::new();
        // No quest in progress: no roll for it at all (the first draw is the gold's, which misses).
        let mut rolls = [99u32].into_iter();
        assert_eq!(
            roll(
                &def,
                0,
                &t,
                &items,
                &mut |q| {
                    asked.push(q.to_vec());
                    false
                },
                &mut || rolls.next().unwrap()
            ),
            []
        );
        assert_eq!(asked, [vec![6, 7]]);
        assert_eq!(rolls.next(), None);
        // A quest in progress: one roll at quality 50. 49 drops; 50 does not and is not tried again.
        let mut rolls = [49u32, 99].into_iter();
        let loot = roll(&def, 0, &t, &items, &mut |_| true, &mut || rolls.next().unwrap());
        assert_eq!(loot, [Loot::Item { item_type: 27, type_id: 2, count: 1, quality: 50, kept: quest_kept() }]);
        let mut rolls = [50u32, 99].into_iter();
        assert_eq!(roll(&def, 0, &t, &items, &mut |_| true, &mut || rolls.next().unwrap()), []);
        assert_eq!(rolls.next(), None, "one roll for the item, one for the gold");
    }

    #[test]
    fn a_quest_item_of_quality_zero_never_drops() {
        let (items, t) = quest_tables();
        // Even a roll of 0 is not below a chance of 0 (and the gold, of 0, is not made).
        assert_eq!(roll(&quest_def((27, 3)), 0, &t, &items, &mut |_| true, &mut || 0), []);
        // A monster with a quest item that has no definition drops none either.
        assert_eq!(roll(&quest_def((27, 99)), 0, &t, &items, &mut |_| true, &mut || 99), []);
    }

    #[test]
    fn the_quest_item_comes_before_the_entries() {
        let (items, t) = quest_tables();
        let mut def = quest_def((27, 1));
        def.drops[1] = (9, 100);
        // Quest roll 0 drops; entry 1 rolls 0; the pick takes item 0 of grade 9 (1/1); gold misses.
        let mut rolls = [0u32, 0, 0, 99].into_iter();
        let loot = roll(&def, 0, &t, &items, &mut |_| false, &mut || rolls.next().unwrap());
        assert_eq!(
            loot.iter()
                .map(|l| match l {
                    Loot::Item { item_type, type_id, .. } => (*item_type, *type_id),
                    Loot::Gold(_) => (0, 0),
                })
                .collect::<Vec<_>>(),
            [(27, 1), (1, 1)]
        );
    }

    fn sequence_rows() -> Vec<SequenceRow> {
        vec![(2, 7, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), (1, 16001, vec![9, 9, 9, 9, 9, 9, 9, 0, 0, 0])]
    }

    #[test]
    fn sequence_items_follow_the_monsters_id() {
        let (items, mut t) = quest_tables();
        assert_eq!(t.load_sequences(&sequence_rows()), 2, "sorted by group number");
        let def = MobDef { level: 5, money: 1, ..Default::default() };
        // Id 16001 x 3 at level 5: group 1's grade 9 at tens 0; the pick takes item 1 (rand 1) of grade 9; gold misses.
        let mut rolls = [1u32, 99].into_iter();
        let loot = roll(&def, 16001 * 3, &t, &items, &mut |_| false, &mut || rolls.next().unwrap());
        let sequence = Some(Kept { make_type: b'V', ..Kept::default() });
        assert_eq!(loot, [Loot::Item { item_type: 1, type_id: 2, count: 1, quality: 10, kept: sequence }]);
        assert_eq!(t.sequence_item(0, 16001, &mut || 0), None);
        assert_eq!(t.sequence_item(100, 16001, &mut || 0), None);
        assert_eq!(t.sequence_item(5, 0, &mut || 0), None, "id 0");
        assert_eq!(t.sequence_item(5, 16000, &mut || 0), None, "not divisible");
        assert_eq!(t.sequence_item(75, 16001, &mut || 0), None, "grade 0 for tens 7");
        // A monster without gold makes none.
        let mut rolls = [99u32].into_iter();
        assert_eq!(roll(&MobDef { money: 0, ..def }, 16001, &t, &items, &mut |_| false, &mut || rolls.next().unwrap()), []);
        // A group whose grade is 0 stops the search even when a later group divides.
        t.load_sequences(&[(1, 7, vec![0; 10]), (2, 16001, vec![9; 10])]);
        assert_eq!(t.sequence_item(5, 7 * 16001, &mut || 0), None);
        assert_eq!(t.sequence_item(5, 16001, &mut || 0), Some((1, 1)));
    }

    #[test]
    fn sequence_groups_follow_the_loading_rules() {
        let (_, mut t) = quest_tables();
        // A zero sequence turns every group off.
        assert_eq!(t.load_sequences(&[(1, 5, vec![1; 10]), (2, 0, vec![1; 10])]), 0);
        assert!(t.sequences.is_empty());
        // Too large for a u32: the same.
        assert_eq!(t.load_sequences(&[(1, 5, vec![1; 10]), (2, 1 << 32, vec![1; 10])]), 0);
        // Out of range grades are 0; more than 8 groups keep the first 8.
        let rows: Vec<SequenceRow> = (1..=10).map(|n| (n, i64::from(n) + 1, vec![-1, 70000, 3])).collect();
        assert_eq!(t.load_sequences(&rows), 8);
        assert_eq!(t.sequences[0], (2, [0, 0, 3, 0, 0, 0, 0, 0, 0, 0]));
        // The 9th group's sequence (10) is not read, even when a zero sequence follows it.
        let mut rows = rows;
        rows[9].1 = 0;
        assert_eq!(t.load_sequences(&rows), 8);
    }
}
