//! Lapis and enchanting (`docs/original/lapis.md`): linking (`0x0801`), extraction (`0x0802`), enchanting (`0x0805`), new craft
//! stats (`0x0806`, `0x0808`), remake (`0x0807`), cloak badges (`0x0803`, `0x0804`), and what lapis and enchant steps add to the
//! wearer. The functions only decide; the caller rolls, saves the changes with the money and then applies them.

use crate::inventory::{CRAFT_NAME_MAX, Inventory, ItemDef, ItemDefs, ItemUnit, Kept, NewItem, SlotChange, real_type};

/// Lapis are item type 30, their type id the gem byte; Lapisia item type 95.
pub const LAPIS_TYPE: u8 = 30;
pub const LAPISIA_TYPE: u8 = 95;
const CLOAK_REAL_TYPE: u8 = 24;
/// Hammers, charms and runes are real type 26 (item types 25, 44, 100).
const TOOL_REAL_TYPE: u8 = 26;
const QUEST_REAL_TYPE: u8 = 27;
const LINKING_HAMMER: u8 = 36;
const EXTRACTION_HAMMER: u8 = 58;
const LUCKY_CHARM: u8 = 57;
const RECREATION_RUNE: u8 = 62;
/// The cloak's gems are its badge, and lapis there add nothing.
pub const CLOAK_SLOT: u8 = 7;
const WEAPON_SLOT: u8 = 5;
const GEMS: usize = 6;
/// Extraction: bags below 6, slots below 13 in the equipment (`CUser::ItemGemRemove`).
const BAGS: u8 = 6;
const EQUIPMENT_SLOTS: u8 = 13;
/// Link and extraction rates are kept within 1 to 50 per cent.
const MAX_RATE: i32 = 50;
/// A Linking Hammer of 1000 % or more always links (deviation 4, approved): in the data only the Operator's Exclusive (100/82, 2000 %),
/// which the original's 50 % limit holds back like any other.
const SURE_HAMMER: i16 = 1000;
const SURE: i32 = 100;
/// Make types: extracted lapis, a remade item, quest items, and the other kind remake and new stats refuse.
const LAPIS_MAKE_TYPE: u8 = b'G';
const REMAKE_MAKE_TYPE: u8 = b'C';
const QUEST_MAKE_TYPE: u8 = crate::quests::QUEST_MAKE_TYPE;
const B_MAKE_TYPE: u8 = b'B';
/// A cloak's badge costs 450000 (`CUser::ItemCloakAdd`).
pub const BADGE_PRICE: u32 = 450_000;
/// An enchant step of armour is written + 50; steps go to 20.
const ARMOUR_STEP: u8 = 50;
pub const MAX_STEP: u8 = 20;
/// The material counts for at most 100 in the enchant roll.
const MAX_MATERIAL: u8 = 100;
/// The enchant roll draws below one million.
pub const ENCHANT_DRAW: u32 = 1_000_000;
/// The jump a draw below `ENCHANT_DRAW` picks from a table of `(steps, weight)` whose weights add up to `total` (the weights are
/// proportional: `draw * total / 1,000,000` falls in one entry's span). Shared by the `enchant` rules and the Lapisia effects
/// (`docs/plans/ext-3-effects.md` section 5.2); 1 for an empty table.
pub fn weighted_jump(jumps: &[(u8, u32)], total: u64, draw: u32) -> u8 {
    let target = u64::from(draw.min(ENCHANT_DRAW - 1)) * total / u64::from(ENCHANT_DRAW);
    let mut upto = 0u64;
    for &(steps, weight) in jumps {
        upto += u64::from(weight);
        if target < upto {
            return steps;
        }
    }
    1
}

/// `g_LapisianEnchantPercentRate` by step: weapon, armour, in millionths.
const ENCHANT_RATES: [[u32; 2]; MAX_STEP as usize] = [
    [700_000, 990_000],
    [500_000, 980_000],
    [300_000, 970_000],
    [90_000, 300_000],
    [30_000, 100_000],
    [10_000, 30_000],
    [3_000, 10_000],
    [1_000, 3_000],
    [300, 1_000],
    [100, 300],
    [30, 100],
    [10, 30],
    [1, 10],
    [1, 3],
    [1, 1],
    [1, 1],
    [1, 1],
    [1, 1],
    [1, 1],
    [1, 1],
];
/// `g_LapisianEnchantAddValue` per step: weapon attack, armour absorption.
const ENCHANT_ADD: [u16; 2] = [7, 5];
/// What `CItem::InitCraftExpansion` gives a craftable item.
const ZEROS: &[u8; 20] = b"00000000000000000000";

/// `GetGemMoneyByGrade`: a lapis' or Lapisia's price by its grade (`reqig`).
pub fn gem_price(grade: u8) -> u32 {
    const LOW: [u32; 11] = [1_000, 4_095, 11_250, 22_965, 41_280, 137_900, 365_000, 480_000, 627_000, 814_000, 1_040_000];
    const HIGH: [u32; 5] = [2_500_000, 3_500_000, 5_500_000, 7_500_000, 9_800_000];
    match grade {
        31..=40 => LOW[usize::from(grade - 30)],
        96..=100 => HIGH[usize::from(grade - 96)],
        _ => 0,
    }
}

/// The link and extraction rate before tools: `trunc(f32(289 − 8g) + f32(KC) + f32(level) × 0.05f)`, at least 1; `kc` is the
/// faction grade's bonus type 24 (`docs/original/pvp-2.md` section 8).
pub fn base_rate(grade: u8, level: u16, kc: i32) -> i32 {
    let fixed = f64::from(((289 - 8 * i32::from(grade)) as f32) + kc as f32);
    let rate = (fixed + f64::from(f32::from(level)) * f64::from(0.05f32)) as i32;
    rate.max(1)
}

/// The link price: `trunc(f32(price) − f32(VG) / 10)`, wrapping as the original's unsigned compare sees a negative one.
fn link_price(grade: u8, vg: i16) -> u32 {
    let price = f64::from(gem_price(grade) as f32) - f64::from(f32::from(vg)) / 10.0;
    (price as i32) as u32
}

/// The extraction price of one lapis: `max(0, price − VG / 10)` in integers.
fn extraction_price(grade: u8, vg: i16) -> u32 {
    (gem_price(grade) as i32 - i32::from(vg) / 10).max(0) as u32
}

fn is_weapon(def: &ItemDef) -> bool {
    (1..=15).contains(&real_type(def.item_type))
}

/// Where a lapis or Lapisia fits, by its placement columns (`CUser::EnableGemAdd`, section 1). No column means a cloak: neither is
/// put into one (deviation 1).
pub fn fits(lapis: &ItemDef, item: &ItemDef) -> bool {
    let [j0, j1, j2, j3, j4, j5] = lapis.jobs;
    match real_type(item.item_type) {
        1..=15 => lapis.level != 0,
        16 => lapis.country != 0,
        17 => j0,
        18 => j1,
        19 => j2,
        20 => j3,
        21 => j4,
        22 => j5,
        23 => lapis.grow != 0,
        25 => lapis.req_str != 0,
        _ => false,
    }
}

/// A known lapis of a gem byte.
pub fn lapis(defs: &ItemDefs, gem: u8) -> Option<&ItemDef> {
    defs.get(LAPIS_TYPE, gem)
}

/// The enchant level, the tenth two-digit number of a 20-character craft name (0 otherwise; `CItem::GetEnchantStep`).
pub fn enchant_value(craft_name: &[u8]) -> u8 {
    match craft_name {
        [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, a @ b'0'..=b'9', b @ b'0'..=b'9'] => (a - b'0') * 10 + (b - b'0'),
        _ => 0,
    }
}

/// What an enchant level adds (`CItem::CopyCraftExpassion`): `(attack, absorption)`; 1 to 20 a weapon step, 51 to 70 an armour step.
pub fn enchant_bonus(craft_name: &[u8]) -> (i32, i32) {
    match enchant_value(craft_name) {
        v @ 1..=MAX_STEP => (i32::from(v) * i32::from(ENCHANT_ADD[0]), 0),
        v @ 51..=70 => (0, i32::from(v - ARMOUR_STEP) * i32::from(ENCHANT_ADD[1])),
        _ => (0, 0),
    }
}

/// An item's element with its lapis (`CUser::SetAttrAtk`, `SetAttrDef`): the item's, then each gem's that has one: the same is kept, a
/// first one is taken, a different one makes 0 and ends it.
pub fn element(base: u8, gems: &[u8; 6], defs: &ItemDefs) -> u8 {
    let mut element = base;
    for lapis in gems.iter().filter_map(|&gem| lapis(defs, gem)).filter(|l| l.attrib != 0) {
        if element == 0 {
            element = lapis.attrib;
        } else if element != lapis.attrib {
            return 0;
        }
    }
    element
}

/// The lapis of an equipped item that count (not the cloak's badge).
pub fn equipped_lapis<'a>(item: &'a ItemUnit, defs: &'a ItemDefs) -> impl Iterator<Item = &'a ItemDef> + 'a {
    let counted = item.bag == 0 && item.slot != CLOAK_SLOT;
    item.gems.iter().filter(move |_| counted).filter_map(move |&gem| lapis(defs, gem))
}

/// The state a lapis blocks outside the weapon (`m_pStateBlock`): its special, 1 to 15.
pub fn state_block(lapis: &ItemDef, slot: u8) -> Option<u8> {
    (slot != WEAPON_SLOT && (1..16).contains(&lapis.special)).then_some(lapis.special)
}

/// The armour slots whose lapis strike back at an attacker, in the original's order (`CUser::BeAttackedItemOption`): shield, helmet,
/// upper armour, gloves, lower armour, boots.
const OPTION_ARMOUR_SLOTS: [u8; 6] = [6, 0, 1, 3, 2, 4];
/// The recovery lapis' special, which is no skill (`docs/original/lapis.md` section 2).
const RECOVERY_SPECIAL: u8 = 16;

/// The level-100 skills a player's lapis set off on hits (`docs/original/item-options.md`): the weapon's six gems, and the armour's
/// in slot order, six per slot; 0 for a gem that names none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionSkills {
    pub weapon: [u16; GEMS],
    pub armour: [u16; GEMS * 6],
}

impl Default for OptionSkills {
    fn default() -> Self {
        Self { weapon: [0; GEMS], armour: [0; GEMS * 6] }
    }
}

impl OptionSkills {
    /// The equipped items' lapis skills (section 1): a known lapis whose special is not 0 or 16 and is a skill at level 100 (`known`).
    /// An empty armour slot is skipped (deviation 1).
    pub fn new<'a>(items: impl IntoIterator<Item = &'a ItemUnit>, defs: &ItemDefs, known: impl Fn(u16) -> bool) -> Self {
        let skills = |item: &ItemUnit| {
            item.gems.map(|gem| {
                let special = lapis(defs, gem).map_or(0, |l| l.special);
                let id = u16::from(special);
                if special == RECOVERY_SPECIAL || id == 0 || !known(id) { 0 } else { id }
            })
        };
        let mut o = Self::default();
        for item in items.into_iter().filter(|i| i.bag == 0) {
            if item.slot == WEAPON_SLOT {
                o.weapon = skills(item);
            } else if let Some(i) = OPTION_ARMOUR_SLOTS.iter().position(|&s| s == item.slot) {
                o.armour[i * GEMS..(i + 1) * GEMS].copy_from_slice(&skills(item));
            }
        }
        o
    }
}

/// One of the item at a place used up (`CUser::ItemUseNSend`): what is left, `0x050A` follows.
fn use_one(inv: &Inventory, (bag, slot): (u8, u8)) -> Option<SlotChange> {
    let item = inv.get(bag, slot)?;
    Some(if item.count <= 1 { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: item.count - 1 } })
}

/// A tool at a place: real type 26 with `special`.
fn tool<'a>(inv: &Inventory, defs: &'a ItemDefs, place: (u8, u8), special: u8) -> Option<&'a ItemDef> {
    if place.0 == 0 || !inv.is_bag(place) {
        return None;
    }
    let item = inv.get(place.0, place.1)?;
    defs.get(item.item_type, item.type_id).filter(|d| real_type(d.item_type) == TOOL_REAL_TYPE && d.special == special)
}

/// The first Lucky Charm in the bags (`CUser::FindItemGemAddItemProtect`).
fn lucky_charm(inv: &Inventory, defs: &ItemDefs) -> Option<(u8, u8)> {
    inv.usable_items()
        .find(|i| defs.get(i.item_type, i.type_id).is_some_and(|d| real_type(d.item_type) == TOOL_REAL_TYPE && d.special == LUCKY_CHARM))
        .map(|i| (i.bag, i.slot))
}

/// What a request comes to once rolled: the changes in order (saved with `-price` money in one transaction), the places of which one
/// item was used up (`0x050A` each: `before` ones ahead of the answer, the others after it), the item destroyed (section 9).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub success: bool,
    pub price: u32,
    pub changes: Vec<SlotChange>,
    pub used_before: Vec<(u8, u8)>,
    pub used_after: Vec<(u8, u8)>,
    pub destroyed: Option<(u8, u8)>,
    /// Lapis taken out: their new places and type ids (`0x0802`).
    pub extracted: Vec<((u8, u8), u8)>,
}

impl Outcome {
    /// Whether the numbers must be worked out again: something in the equipment changed.
    pub fn touches_equipment(&self) -> bool {
        self.changes.iter().any(|c| match c {
            SlotChange::SetGems { bag, .. }
            | SlotChange::SetCraftName { bag, .. }
            | SlotChange::SetQuality { bag, .. }
            | SlotChange::Remove { bag, .. } => *bag == 0,
            _ => false,
        })
    }
}

fn destroy(outcome: &mut Outcome, place: (u8, u8)) {
    outcome.changes.push(SlotChange::Remove { bag: place.0, slot: place.1 });
    outcome.destroyed = Some(place);
}

/// The item's quality cut to its definition's once lapis leave it.
fn cut_quality(item: &ItemUnit, def: &ItemDef) -> Option<SlotChange> {
    (item.quality > def.quality).then_some(SlotChange::SetQuality { bag: item.bag, slot: item.slot, quality: def.quality })
}

/// `0x0801`: lapis, item and hammer places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkRequest {
    pub lapis: (u8, u8),
    pub item: (u8, u8),
    pub hammer: (u8, u8),
}

/// A link that will be rolled (section 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub request: LinkRequest,
    pub price: u32,
    /// Per cent, 1 to 50 (100 with a hammer of 1000 % or more, deviation 4): success when `rand() % 100` is below.
    pub rate: i32,
    /// The hammer used, if one counted (the answer echoes its place, or 0, 0).
    pub hammer: Option<(u8, u8)>,
    pub position: u8,
    pub lapis: u8,
    pub dangerous: bool,
    pub charm: Option<(u8, u8)>,
}

/// A guild house's blacksmith (`docs/original/guild-house.md` section 9): the price rate and, linking, the bonus to the rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HouseRates {
    pub price: f64,
    pub bonus: i32,
}

/// Outside a guild house.
pub const NO_HOUSE: HouseRates = HouseRates { price: 1.0, bonus: 0 };

impl HouseRates {
    fn price(&self, price: u32) -> u32 {
        if *self == NO_HOUSE { price } else { crate::guild_house::scaled(price, self.price) }
    }
}

/// `CUser::ItemGemAdd` before the roll; `None` when the original ignores the request. `premium`: billing type 2 or 3.
#[allow(clippy::too_many_arguments)]
pub fn link(
    inv: &Inventory,
    defs: &ItemDefs,
    money: u32,
    vg: i16,
    (level, kc): (u16, i32),
    request: LinkRequest,
    house: HouseRates,
    premium: bool,
) -> Option<Link> {
    let LinkRequest { lapis: lapis_at, item: item_at, .. } = request;
    if !inv.is_bag(lapis_at) || !inv.is_bag(item_at) {
        return None;
    }
    let item = inv.get(item_at.0, item_at.1).filter(|i| i.count == 1)?;
    let gem = inv.get(lapis_at.0, lapis_at.1).filter(|g| g.item_type == LAPIS_TYPE)?;
    let item_def = defs.get(item.item_type, item.type_id)?;
    let lapis_def = lapis(defs, gem.type_id)?;
    // CItem::EnableGem: not the same lapis twice, and room for it.
    let used: i32 = item.gems.iter().filter_map(|&g| lapis(defs, g)).map(|l| i32::from(l.room)).sum();
    if !fits(lapis_def, item_def) || item.gems.contains(&gem.type_id) || i32::from(item_def.room) - used < i32::from(lapis_def.room) {
        return None;
    }
    let position = item.gems.iter().position(|&g| g == 0)? as u8;
    let price = house.price(link_price(lapis_def.reuse_group, vg));
    if money < price {
        return None;
    }
    let mut rate = base_rate(lapis_def.reuse_group, level, kc);
    let hammer = tool(inv, defs, request.hammer, LINKING_HAMMER);
    if let Some(h) = hammer {
        rate = (f64::from(f32::from(h.vg)) / 100.0 * f64::from(rate as f32)) as i32;
    }
    // Premium billing types 2 and 3: × 1.5 in f32, truncated (`ItemGemAdd` 0x46165a, `docs/original/item-mall.md` section 7).
    if premium {
        rate = (rate as f32 * 1.5) as i32;
    }
    rate += house.bonus;
    Some(Link {
        request,
        price,
        rate: if hammer.is_some_and(|h| h.vg >= SURE_HAMMER) { SURE } else { rate.clamp(1, MAX_RATE) },
        hammer: hammer.map(|_| request.hammer),
        position,
        lapis: gem.type_id,
        dangerous: lapis_def.vg != 0,
        charm: (lapis_def.vg != 0).then(|| lucky_charm(inv, defs)).flatten(),
    })
}

impl Link {
    /// The changes after the roll (section 3).
    pub fn outcome(&self, inv: &Inventory, success: bool) -> Outcome {
        let mut o = Outcome { success, price: self.price, ..Outcome::default() };
        if let Some(h) = self.hammer {
            o.changes.extend(use_one(inv, h));
            o.used_before.push(h);
        }
        o.changes.extend(use_one(inv, self.request.lapis));
        let item_at = self.request.item;
        if success && let Some(item) = inv.get(item_at.0, item_at.1) {
            let mut gems = item.gems;
            gems[usize::from(self.position)] = self.lapis;
            o.changes.push(SlotChange::SetGems { bag: item_at.0, slot: item_at.1, gems });
        }
        if self.dangerous {
            match self.charm {
                Some(charm) => {
                    o.changes.extend(use_one(inv, charm));
                    o.used_after.push(charm);
                }
                None if !success => destroy(&mut o, item_at),
                None => {}
            }
        }
        o
    }
}

/// `0x0802`'s two forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractRequest {
    All { place: (u8, u8) },
    One { place: (u8, u8), position: u8, hammer: (u8, u8) },
}

/// An extraction that will be rolled (section 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extract {
    pub place: (u8, u8),
    /// 255 for all gems.
    pub position: u8,
    pub price: u32,
    pub rate: i32,
    pub hammer: Option<(u8, u8)>,
    /// The known lapis taken out: position and type id.
    pub lapis: Vec<(u8, u8)>,
    /// Gems in all (places needed for all of them).
    pub gems: usize,
    pub dangerous: bool,
}

pub const ALL_GEMS: u8 = 255;

/// `CUser::ItemGemRemove` before the roll; `None` when the original ignores the request.
pub fn extract(
    inv: &Inventory,
    defs: &ItemDefs,
    money: u32,
    vg: i16,
    (level, kc): (u16, i32),
    request: ExtractRequest,
    house: HouseRates,
) -> Option<Extract> {
    let place = match request {
        ExtractRequest::All { place } | ExtractRequest::One { place, .. } => place,
    };
    let (bag, slot) = place;
    if !inv.is_bag(place) || bag >= BAGS || (bag == 0 && slot >= EQUIPMENT_SLOTS) {
        return None;
    }
    let item = inv.get(bag, slot).filter(|i| i.count == 1)?;
    let def = defs.get(item.item_type, item.type_id)?;
    if real_type(def.item_type) == CLOAK_REAL_TYPE || item.gems.iter().all(|&g| g == 0) {
        return None;
    }
    match request {
        ExtractRequest::All { .. } => {
            let known: Vec<(u8, &ItemDef)> =
                item.gems.iter().enumerate().filter_map(|(i, &g)| lapis(defs, g).map(|l| (i as u8, l))).collect();
            let price = house.price(known.iter().map(|(_, l)| extraction_price(l.reuse_group, vg)).sum());
            if money < price {
                return None;
            }
            let first = item.gems.iter().find(|&&g| g != 0).and_then(|&g| lapis(defs, g))?;
            Some(Extract {
                place,
                position: ALL_GEMS,
                price,
                rate: base_rate(first.reuse_group, level, kc).clamp(1, MAX_RATE),
                hammer: None,
                lapis: known.iter().map(|&(i, l)| (i, l.type_id)).collect(),
                gems: item.gems.iter().filter(|&&g| g != 0).count(),
                dangerous: known.iter().any(|(_, l)| l.vg != 0),
            })
        }
        ExtractRequest::One { position, hammer, .. } => {
            let gem = *item.gems.get(usize::from(position))?;
            let l = lapis(defs, gem)?;
            let price = house.price(extraction_price(l.reuse_group, vg));
            let h = tool(inv, defs, hammer, EXTRACTION_HAMMER).filter(|_| money >= price)?;
            Some(Extract {
                place,
                position,
                price,
                rate: i32::from(h.vg),
                hammer: Some(hammer),
                lapis: vec![(position, gem)],
                gems: 1,
                dangerous: l.vg != 0,
            })
        }
    }
}

impl Extract {
    /// The changes after the roll; `None` when it succeeded without enough free places: nothing more happens (no answer), though a
    /// hammer is gone (returned in `Err`).
    pub fn outcome(&self, inv: &Inventory, defs: &ItemDefs, success: bool) -> Result<Outcome, Option<SlotChange>> {
        let hammer = self.hammer.and_then(|h| use_one(inv, h));
        let (bag, slot) = self.place;
        let Some(item) = inv.get(bag, slot) else { return Err(hammer) };
        let Some(def) = defs.get(item.item_type, item.type_id) else { return Err(hammer) };
        let mut o = Outcome { success, price: self.price, ..Outcome::default() };
        if let Some(h) = self.hammer {
            o.changes.extend(hammer);
            o.used_before.push(h);
        }
        if success {
            let free = inv.free_slots();
            if free.len() < self.gems {
                return Err(o.changes.into_iter().next());
            }
            for (&(_, type_id), &at) in self.lapis.iter().zip(&free) {
                let Some(l) = lapis(defs, type_id) else { continue };
                let kept = Some(Kept { make_type: LAPIS_MAKE_TYPE, ..Kept::default() });
                o.changes.push(SlotChange::Insert(NewItem {
                    bag: at.0,
                    slot: at.1,
                    item_id: l.item_id,
                    item_type: LAPIS_TYPE,
                    type_id,
                    quality: l.quality,
                    count: 1,
                    kept,
                }));
                o.extracted.push((at, type_id));
            }
        }
        let mut gems = item.gems;
        match self.position {
            ALL_GEMS => gems = [0; GEMS],
            p => gems[usize::from(p)] = 0,
        }
        o.changes.push(SlotChange::SetGems { bag, slot, gems });
        o.changes.extend(cut_quality(item, def));
        if !success && self.dangerous {
            destroy(&mut o, self.place);
        }
        Ok(o)
    }
}

/// `0x0805`: Lapisia, material (with its count) and equipment places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnchantRequest {
    pub lapisia: (u8, u8),
    pub material: (u8, u8),
    pub count: u8,
    pub item: (u8, u8),
}

/// An enchant that will be rolled (section 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enchant {
    pub request: EnchantRequest,
    pub price: u32,
    /// Success when the material count plus a draw below one million is below this.
    pub rate: u32,
    /// The material used and how many (at most 100).
    pub material: Option<((u8, u8), u8)>,
    pub armour: bool,
    pub step: u8,
    pub dangerous: bool,
    /// How many steps a success rises: 1, or what an `enchant` rule drew (`docs/plans/ext-2-rules.md` point 8); the step never passes
    /// `MAX_STEP`.
    pub jump: u8,
    /// A rate of 1,000,000 from the Lapisia's `enchant` effect: succeeds without a draw (`docs/plans/ext-3-effects.md` section 5.2).
    pub always: bool,
    /// The Lapisia's own jump table (its `enchant` effect's `jumps`): drawn on success in place of the `enchant` rules.
    pub own_jumps: Option<crate::effects::Jumps>,
}

/// `CUser::ItemLapisianAdd` before the roll; `None` when the original ignores the request.
pub fn enchant(inv: &Inventory, defs: &ItemDefs, money: u32, request: EnchantRequest) -> Option<Enchant> {
    enchant_with(inv, defs, money, request, &crate::effects::ItemEffects::default())
}

/// As [`enchant`], for a Lapisia that may have an `enchant` effect (section 5.2): its rate replaces the step's, its jump table the rules',
/// and `safe` keeps a failure from destroying the item. Everything else (the checks, the price, the material) is the core's.
pub fn enchant_with(
    inv: &Inventory,
    defs: &ItemDefs,
    money: u32,
    request: EnchantRequest,
    effects: &crate::effects::ItemEffects,
) -> Option<Enchant> {
    let EnchantRequest { lapisia: lapisia_at, material: material_at, count, item: item_at } = request;
    if !inv.is_bag(lapisia_at) || !inv.is_bag(item_at) {
        return None;
    }
    let item = inv.get(item_at.0, item_at.1)?;
    let lapisia = inv.get(lapisia_at.0, lapisia_at.1)?;
    // The original reads the material's place unchecked; out of range here is no material.
    let material = if inv.is_bag(material_at) { inv.get(material_at.0, material_at.1) } else { None };
    if let Some(m) = material
        && (count == 0 || count > m.count)
    {
        return None;
    }
    let item_def = defs.get(item.item_type, item.type_id)?;
    let lapisia_def = defs.get(lapisia.item_type, lapisia.type_id)?;
    // CUser::EnableLapisianAdd, without the cloak's free pass (deviation 1).
    let material_fits = material
        .and_then(|m| defs.get(m.item_type, m.type_id))
        .is_some_and(|m| item_def.material != 0 && m.item_id == u32::from(item_def.material));
    if item.count != 1 || lapisia.item_type != LAPISIA_TYPE || item_def.server == 0 || !material_fits || !fits(lapisia_def, item_def) {
        return None;
    }
    let value = enchant_value(&item.craft_name);
    let step = if value > ARMOUR_STEP { value - ARMOUR_STEP } else { value };
    if step >= MAX_STEP {
        return None;
    }
    let price = gem_price(lapisia_def.reuse_group);
    if money < price {
        return None;
    }
    let armour = !is_weapon(item_def);
    let own = effects.get(lapisia_def.item_id).and_then(|e| match &**e {
        crate::effects::ItemEffect::Enchant(own) => Some(own),
        _ => None,
    });
    Some(Enchant {
        request,
        price,
        rate: own.map_or(ENCHANT_RATES[usize::from(step)][usize::from(armour)], |own| own.rate),
        material: material.map(|_| (material_at, count.min(MAX_MATERIAL))),
        armour,
        step,
        dangerous: lapisia_def.vg != 0 && !own.is_some_and(|own| own.safe),
        jump: 1,
        always: own.is_some_and(|own| own.rate() == crate::effects::Rate::Always),
        own_jumps: own.and_then(|own| own.jumps.clone()),
    })
}

impl Enchant {
    /// The jump a draw below one million gives from the Lapisia's own table, `None` when it has none (the rules are asked then).
    pub fn own_jump(&self, draw: u32) -> Option<u8> {
        self.own_jumps.as_ref().map(|j| weighted_jump(&j.list, j.total, draw))
    }

    /// Whether a draw below one million succeeds.
    pub fn succeeds(&self, draw: u32) -> bool {
        if self.always {
            return true;
        }
        let added = self.material.map_or(0, |(_, n)| u32::from(n));
        added + draw < self.rate
    }

    /// The craft name after a successful step (`CItem::SetEnchantStep`): a craftable item without a 20-character name gets twenty
    /// zeros first; characters 18 and 19 become the step, + 50 for armour. The name is kept up to its first zero byte.
    pub fn new_name(&self, item: &ItemUnit, def: &ItemDef) -> [u8; CRAFT_NAME_MAX] {
        let mut buffer = [0u8; CRAFT_NAME_MAX + 1];
        if item.craft_name.len() == CRAFT_NAME_MAX {
            buffer[..CRAFT_NAME_MAX].copy_from_slice(&item.craft_name);
        } else if def.craftable {
            buffer[..CRAFT_NAME_MAX].copy_from_slice(ZEROS);
        } else {
            let len = item.craft_name.len().min(CRAFT_NAME_MAX);
            buffer[..len].copy_from_slice(&item.craft_name[..len]);
        }
        let value = (self.step + self.jump).min(MAX_STEP) + if self.armour { ARMOUR_STEP } else { 0 };
        buffer[18] = b'0' + value / 10;
        buffer[19] = b'0' + value % 10;
        Kept::pad_craft_name(&buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(CRAFT_NAME_MAX)])
    }

    /// The changes after the roll (section 5).
    pub fn outcome(&self, inv: &Inventory, defs: &ItemDefs, success: bool) -> Outcome {
        let mut o = Outcome { success, price: self.price, ..Outcome::default() };
        o.changes.extend(use_one(inv, self.request.lapisia));
        if let Some(((bag, slot), n)) = self.material
            && let Some(m) = inv.get(bag, slot)
        {
            let left = m.count.saturating_sub(n);
            o.changes.push(if left == 0 { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: left } });
        }
        let (bag, slot) = self.request.item;
        if success {
            if let Some(item) = inv.get(bag, slot)
                && let Some(def) = defs.get(item.item_type, item.type_id)
            {
                o.changes.push(SlotChange::SetCraftName { bag, slot, craft_name: self.new_name(item, def) });
            }
        } else if self.dangerous {
            destroy(&mut o, self.request.item);
        }
        o
    }
}

/// The nine crafted numbers a roll gives (`CItem::ReGenerationCraftExpansion` with `draws` 2, `CItem::CreateCraftExpansion` and
/// `ItemConsider` with 3; section 6). `rand` is the original's `rand()` (0 to 32767).
pub fn roll_craft(def: &ItemDef, draws: u8, rand: &mut impl FnMut() -> u32) -> [u8; 9] {
    let six = is_weapon(def) || matches!(real_type(def.item_type), 22 | 23 | 25);
    let s = usize::from(def.server);
    let mut chosen = [false; 9];
    let pick = |chosen: &mut [bool; 9], n: usize, count: usize, rand: &mut dyn FnMut() -> u32| {
        let mut order: Vec<usize> = (0..n).collect();
        for k in 0..count {
            let j = rand() as usize % (n - k);
            chosen[order[j]] = true;
            order[j] = order[n - 1 - k];
        }
    };
    if six {
        match s {
            1 => chosen[rand() as usize % 6] = true,
            2..=4 => pick(&mut chosen, 6, s, &mut *rand),
            5 => {
                chosen[..6].fill(true);
                chosen[rand() as usize % 6] = false;
            }
            _ => chosen[..6].fill(true),
        }
    } else {
        match s {
            1 => chosen[rand() as usize % 9] = true,
            2..=7 => pick(&mut chosen, 9, s, &mut *rand),
            8 => {
                chosen.fill(true);
                chosen[rand() as usize % 9] = false;
            }
            9 => chosen.fill(true),
            _ => {}
        }
    }
    let span = u32::from(def.craft_max) + 1;
    chosen.map(|on| {
        if !on {
            return 0;
        }
        (0..draws).map(|_| rand() % span).min().unwrap_or(0) as u8
    })
}

/// A craft name of nine numbers and an enchant level.
pub fn craft_name(values: [u8; 9], enchant: u8) -> [u8; CRAFT_NAME_MAX] {
    let mut name = [0u8; CRAFT_NAME_MAX];
    for (pair, v) in name.chunks_mut(2).zip(values.iter().chain(std::iter::once(&enchant))) {
        pair[0] = b'0' + v / 10;
        pair[1] = b'0' + v % 10;
    }
    name
}

/// The nine crafted numbers of an item after a Recreation Rune with a `recreate` effect (`docs/plans/ext-3-effects.md` section 5.3):
/// only the named numbers the item already has (not 0) change, each to the item's `reqwis` (`max`) or the smaller of two draws up to
/// it (`random`, as the core's rune); every other number stays. `None` when none of the named numbers is present. The enchant step
/// is the caller's (`rerolled_name`).
pub fn rerolled_values(item: &ItemUnit, def: &ItemDef, rune: &crate::effects::Recreate, rand: &mut impl FnMut() -> u32) -> Option<[u8; 9]> {
    use crate::effects::RecreateValue;
    let mut values = crafted(&item.craft_name);
    let span = u32::from(def.craft_max) + 1;
    let top = def.craft_max.min(99) as u8;
    let mut changed = false;
    for (value, &named) in values.iter_mut().zip(&rune.stats) {
        if !named || *value == 0 {
            continue;
        }
        changed = true;
        *value = match rune.value {
            RecreateValue::Max => top,
            RecreateValue::Random => (0..2).map(|_| rand() % span).min().unwrap_or(0).min(99) as u8,
        };
    }
    changed.then_some(values)
}

/// A new item's craft name from `CItem::CreateCraftExpansion` (`docs/original/item-origins.md` section 3): random numbers, the smallest
/// of three draws each, and no enchant level for a craftable item; empty for any other.
pub fn new_craft_name(def: &ItemDef, rand: &mut impl FnMut() -> u32) -> [u8; CRAFT_NAME_MAX] {
    if !def.craftable {
        return [0; CRAFT_NAME_MAX];
    }
    craft_name(roll_craft(def, 3, rand), 0)
}

/// A new item's craft name from `CItem::InitCraftExpansion` (`docs/original/item-origins.md` section 2): twenty zeros for a craftable
/// item, empty for any other.
pub fn zero_craft_name(def: &ItemDef) -> [u8; CRAFT_NAME_MAX] {
    if def.craftable { *ZEROS } else { [0; CRAFT_NAME_MAX] }
}

/// Why `0x0806`, `0x0807` or `0x0808` is not done: silently, or with the answer's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Ignored,
    Result(u8),
}

/// `0x0806` (`CUser::ItemCompose`): the rune's and the item's places and the item's make type; the item's place on success.
pub fn compose(inv: &Inventory, defs: &ItemDefs, rune: (u8, u8), item: (u8, u8), make_type: u8) -> Result<(u8, u8), Refused> {
    if !inv.is_bag(rune) || !inv.is_bag(item) {
        return Err(Refused::Ignored);
    }
    let (Some(r), Some(i)) = (inv.get(rune.0, rune.1), inv.get(item.0, item.1)) else { return Err(Refused::Ignored) };
    let craftable = defs.get(i.item_type, i.type_id).is_some_and(|d| d.craftable);
    let rune_ok = defs.get(r.item_type, r.type_id).is_some_and(|d| d.special == RECREATION_RUNE);
    if !craftable || !rune_ok || make_type == QUEST_MAKE_TYPE {
        return Err(Refused::Result(1));
    }
    Ok(item)
}

/// The nine crafted numbers of a name (all 0 for one that is not 20 characters).
fn crafted(craft_name: &[u8]) -> [u8; 9] {
    crate::stats::craft_values(craft_name).map_or([0; 9], |v| v.map(|n| n as u8))
}

/// `0x0808` (`CUser::ItemConsider`): an unidentified craftable item, not a quest item.
pub fn consider(inv: &Inventory, defs: &ItemDefs, place: (u8, u8), make_type: u8) -> Result<(), Refused> {
    if !inv.is_bag(place) {
        return Err(Refused::Ignored);
    }
    let Some(item) = inv.get(place.0, place.1) else { return Err(Refused::Ignored) };
    let def = defs.get(item.item_type, item.type_id);
    let allowed = def.is_some_and(|d| real_type(d.item_type) != QUEST_REAL_TYPE && d.craftable)
        && crafted(&item.craft_name) == [0; 9]
        && make_type != QUEST_MAKE_TYPE
        && make_type != B_MAKE_TYPE;
    if allowed { Ok(()) } else { Err(Refused::Result(1)) }
}

/// The craft name after new numbers: the enchant level stays (a name that is not 20 characters counts as twenty zeros).
pub fn rerolled_name(item: &ItemUnit, values: [u8; 9]) -> [u8; CRAFT_NAME_MAX] {
    craft_name(values, enchant_value(&item.craft_name))
}

/// `0x0807` (`CUser::ItemRemake`): three places and their make types; on success the place the new item goes to.
pub fn remake(inv: &Inventory, defs: &ItemDefs, places: [(u8, u8); 3], make_types: [u8; 3]) -> Result<(u8, u8), Refused> {
    if places.iter().any(|&p| !inv.is_bag(p) || inv.get(p.0, p.1).is_none()) {
        return Err(Refused::Ignored);
    }
    // Deviation 2: three different places.
    if places[0] == places[1] || places[1] == places[2] || places[0] == places[2] {
        return Err(Refused::Ignored);
    }
    if make_types.contains(&QUEST_MAKE_TYPE) {
        return Err(Refused::Result(2));
    }
    if make_types.contains(&B_MAKE_TYPE) {
        return Err(Refused::Result(3));
    }
    let defs_of: Vec<Option<&ItemDef>> =
        places.iter().map(|&(b, s)| inv.get(b, s).and_then(|i| defs.get(i.item_type, i.type_id))).collect();
    let first = defs_of[0].filter(|d| d.craftable);
    match first {
        Some(d) if defs_of.iter().all(|o| o.is_some_and(|o| o.item_id == d.item_id)) => {}
        _ => return Err(Refused::Result(1)),
    }
    Ok(places.iter().copied().find(|&(bag, _)| bag != 0).unwrap_or((0, 0)))
}

/// The new item of a remake: count 1, full quality, make type `'C'`, new craft stats.
pub fn remade(def: &ItemDef, at: (u8, u8), values: [u8; 9]) -> NewItem {
    let kept = Kept { make_type: REMAKE_MAKE_TYPE, craft_name: craft_name(values, 0), ..Kept::default() };
    NewItem {
        bag: at.0,
        slot: at.1,
        item_id: def.item_id,
        item_type: def.item_type,
        type_id: def.type_id,
        quality: def.quality,
        count: 1,
        kept: Some(kept),
    }
}

/// `0x0803`/`0x0804`: a cloak at a place (`CUser::ItemCloakAdd`, `ItemCloakRemove`).
pub fn cloak_at(inv: &Inventory, defs: &ItemDefs, place: (u8, u8)) -> bool {
    inv.is_bag(place)
        && inv
            .get(place.0, place.1)
            .and_then(|i| defs.get(i.item_type, i.type_id))
            .is_some_and(|d| real_type(d.item_type) == CLOAK_REAL_TYPE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(bag: u8, slot: u8, item_type: u8, type_id: u8, count: u8) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id, count, quality: 100, gems: [0; 6], craft_name: Vec::new() }
    }

    fn defs() -> ItemDefs {
        ItemDefs::new([
            // A sword with two lapis rooms and material 25043, craftable stats up to 10.
            ItemDef {
                item_id: 1001,
                item_type: 1,
                type_id: 1,
                quality: 100,
                room: 2,
                material: 25043,
                server: 6,
                craft_max: 10,
                craftable: true,
                ..Default::default()
            },
            ItemDef {
                item_id: 17001,
                item_type: 17,
                type_id: 1,
                quality: 100,
                room: 1,
                server: 3,
                craft_max: 10,
                craftable: true,
                ..Default::default()
            },
            ItemDef { item_id: 24001, item_type: 24, type_id: 1, ..Default::default() },
            // A weapon lapis of grade 31 adding STR 3, an armour lapis that is dangerous, of grade 33.
            ItemDef { item_id: 30001, item_type: 30, type_id: 1, level: 1, room: 1, reuse_group: 31, quality: 10, ..Default::default() },
            ItemDef {
                item_id: 30002,
                item_type: 30,
                type_id: 2,
                jobs: [true, false, false, false, false, false],
                room: 1,
                reuse_group: 33,
                vg: 1,
                attrib: 2,
                ..Default::default()
            },
            ItemDef { item_id: 30003, item_type: 30, type_id: 3, level: 1, room: 1, reuse_group: 31, attrib: 3, ..Default::default() },
            ItemDef { item_id: 95001, item_type: 95, type_id: 1, level: 1, reuse_group: 96, vg: 1, ..Default::default() },
            ItemDef { item_id: 25043, item_type: 25, type_id: 43, max_count: 255, ..Default::default() },
            ItemDef { item_id: 100199, item_type: 100, type_id: 199, special: 36, vg: 200, ..Default::default() },
            ItemDef { item_id: 100082, item_type: 100, type_id: 82, special: 36, vg: 2000, ..Default::default() },
            ItemDef { item_id: 100140, item_type: 100, type_id: 140, special: 58, vg: 30, ..Default::default() },
            ItemDef { item_id: 100139, item_type: 100, type_id: 139, special: 57, ..Default::default() },
            ItemDef { item_id: 100171, item_type: 100, type_id: 171, special: 62, ..Default::default() },
        ])
    }

    fn inv(items: &[ItemUnit]) -> Inventory {
        let mut inv = Inventory::new(items);
        inv.set_max_bag(2);
        inv
    }

    #[test]
    fn prices_and_rates() {
        assert_eq!(
            (gem_price(31), gem_price(40), gem_price(96), gem_price(100), gem_price(41)),
            (4_095, 1_040_000, 2_500_000, 9_800_000, 0)
        );
        assert_eq!(base_rate(31, 60, 0), 44);
        assert_eq!(base_rate(31, 1, 0), 41);
        assert_eq!(base_rate(36, 60, 0), 4);
        assert_eq!(base_rate(38, 1, 0), 1, "never below 1");
        assert_eq!(base_rate(31, 60, 5), 49, "the faction grade's bonus");
        assert_eq!(link_price(31, 15), 4093, "4095 − 1.5, truncated");
        assert_eq!(extraction_price(31, 15), 4094, "integer division");
        assert_eq!(link_price(0, 100), (-10i32) as u32, "negative: refused by the unsigned compare");
    }

    #[test]
    fn premium_raises_the_link_rate_by_half() {
        let defs = defs();
        let inv = inv(&[unit(1, 0, 30, 1, 1), unit(1, 1, 1, 1, 1)]);
        let req = LinkRequest { lapis: (1, 0), item: (1, 1), hammer: (1, 2) };
        let rate = |premium| link(&inv, &defs, 10_000, 0, (60, -20), req, NO_HOUSE, premium).unwrap().rate;
        assert_eq!((rate(false), rate(true)), (24, 36), "41 − 20 + 3 = 24; × 1.5");
    }

    #[test]
    fn linking_rolls_takes_the_lapis_and_protects_with_a_charm() {
        let defs = defs();
        let items = [unit(1, 0, 30, 1, 3), unit(1, 1, 1, 1, 1), unit(1, 2, 100, 199, 2), unit(1, 3, 30, 2, 1), unit(1, 4, 17, 1, 1)];
        let inv = inv(&items);
        let req = LinkRequest { lapis: (1, 0), item: (1, 1), hammer: (1, 2) };
        let link = link(&inv, &defs, 10_000, 0, (60, 0), req, NO_HOUSE, false).unwrap();
        assert_eq!((link.price, link.rate, link.hammer, link.position), (4_095, 50, Some((1, 2)), 0), "44 × 200 % = 88, at most 50");
        // The Operator's Exclusive (2000 %) always links (deviation 4), even the worst lapis.
        let sure = super::tests::inv(&[unit(1, 0, 30, 1, 1), unit(1, 1, 1, 1, 1), unit(1, 2, 100, 82, 1)]);
        let link_sure = super::link(&sure, &defs, 10_000, 0, (60, 0), req, NO_HOUSE, false).unwrap();
        assert_eq!((link_sure.rate, link_sure.hammer), (100, Some((1, 2))));
        let o = link.outcome(&inv, true);
        assert_eq!(
            o.changes,
            [
                SlotChange::SetCount { bag: 1, slot: 2, count: 1 },
                SlotChange::SetCount { bag: 1, slot: 0, count: 2 },
                SlotChange::SetGems { bag: 1, slot: 1, gems: [1, 0, 0, 0, 0, 0] }
            ]
        );
        assert!(super::link(&inv, &defs, 4_000, 0, (60, 0), req, NO_HOUSE, false).is_none(), "short of money");
        let house = HouseRates { price: 0.9, bonus: 2 };
        let cheaper = super::link(&inv, &defs, 4_000, 0, (60, 0), req, house, false).expect("4095 × 0.9 = 3685");
        assert_eq!(cheaper.price, 3_685);
        assert!(
            super::link(&inv, &defs, 10_000, 0, (60, 0), LinkRequest { lapis: (1, 0), item: (1, 4), hammer: (0, 0) }, NO_HOUSE, false)
                .is_none(),
            "a weapon lapis"
        );
        // The dangerous armour lapis without a charm: the armour goes on failure.
        let danger =
            super::link(&inv, &defs, 100_000, 0, (60, 0), LinkRequest { lapis: (1, 3), item: (1, 4), hammer: (0, 0) }, NO_HOUSE, false)
                .unwrap();
        assert_eq!((danger.rate, danger.hammer, danger.charm), (28, None, None));
        let o = danger.outcome(&inv, false);
        assert_eq!(o.destroyed, Some((1, 4)));
        assert_eq!(o.changes, [SlotChange::Remove { bag: 1, slot: 3 }, SlotChange::Remove { bag: 1, slot: 4 }]);
        // With a charm it is used up, success or not.
        let with_charm = inv_with(&items, unit(2, 0, 100, 139, 5));
        let danger = super::link(
            &with_charm,
            &defs,
            100_000,
            0,
            (60, 0),
            LinkRequest { lapis: (1, 3), item: (1, 4), hammer: (0, 0) },
            NO_HOUSE,
            false,
        )
        .unwrap();
        let o = danger.outcome(&with_charm, true);
        assert_eq!(o.used_after, [(2, 0)]);
        assert_eq!(o.destroyed, None);
    }

    fn inv_with(items: &[ItemUnit], extra: ItemUnit) -> Inventory {
        let mut all = items.to_vec();
        all.push(extra);
        inv(&all)
    }

    #[test]
    fn linking_needs_room_and_no_repeat() {
        let defs = defs();
        let mut sword = unit(1, 1, 1, 1, 1);
        sword.gems = [1, 0, 0, 0, 0, 0];
        let inv1 = inv(&[unit(1, 0, 30, 1, 1), sword.clone(), unit(1, 2, 30, 3, 1)]);
        assert!(
            link(&inv1, &defs, 10_000, 0, (1, 0), LinkRequest { lapis: (1, 0), item: (1, 1), hammer: (0, 0) }, NO_HOUSE, false).is_none(),
            "the same lapis"
        );
        assert_eq!(
            link(&inv1, &defs, 10_000, 0, (1, 0), LinkRequest { lapis: (1, 2), item: (1, 1), hammer: (0, 0) }, NO_HOUSE, false)
                .unwrap()
                .position,
            1
        );
        sword.gems = [1, 3, 0, 0, 0, 0];
        let full = inv(&[unit(1, 0, 30, 1, 1), sword, unit(1, 2, 30, 3, 1)]);
        assert!(
            link(&full, &defs, 10_000, 0, (1, 0), LinkRequest { lapis: (1, 2), item: (1, 1), hammer: (0, 0) }, NO_HOUSE, false).is_none(),
            "no room"
        );
        // Deviation 1: nothing goes into a cloak.
        let cloak = inv(&[unit(1, 0, 30, 1, 1), unit(1, 1, 24, 1, 1)]);
        assert!(
            link(&cloak, &defs, 10_000, 0, (1, 0), LinkRequest { lapis: (1, 0), item: (1, 1), hammer: (0, 0) }, NO_HOUSE, false).is_none()
        );
    }

    #[test]
    fn extracting_all_needs_places_and_loses_dangerous_items() {
        let defs = defs();
        let mut armour = unit(1, 0, 17, 1, 1);
        armour.gems = [2, 0, 0, 0, 0, 0];
        armour.quality = 110;
        let inv1 = inv(&[armour]);
        let e = extract(&inv1, &defs, 1_000_000, 0, (60, 0), ExtractRequest::All { place: (1, 0) }, NO_HOUSE).unwrap();
        assert_eq!((e.price, e.rate, e.gems, e.dangerous), (gem_price(33), 28, 1, true));
        let o = e.outcome(&inv1, &defs, true).unwrap();
        assert_eq!(o.extracted, [((1, 1), 2)]);
        assert!(o.changes.contains(&SlotChange::SetQuality { bag: 1, slot: 0, quality: 100 }), "quality cut");
        let o = e.outcome(&inv1, &defs, false).unwrap();
        assert_eq!(o.destroyed, Some((1, 0)));
        assert!(extract(&inv1, &defs, 10, 0, (60, 0), ExtractRequest::All { place: (1, 0) }, NO_HOUSE).is_none(), "short of money");
    }

    #[test]
    fn extracting_one_uses_the_hammer_first() {
        let defs = defs();
        let mut sword = unit(1, 0, 1, 1, 1);
        sword.gems = [1, 3, 0, 0, 0, 0];
        let inv1 = inv(&[sword, unit(1, 1, 100, 140, 1)]);
        let req = ExtractRequest::One { place: (1, 0), position: 1, hammer: (1, 1) };
        let e = extract(&inv1, &defs, 1_000_000, 0, (60, 0), req, NO_HOUSE).unwrap();
        assert_eq!((e.rate, e.lapis.clone()), (30, vec![(1, 3)]));
        let o = e.outcome(&inv1, &defs, false).unwrap();
        assert_eq!(o.changes[0], SlotChange::Remove { bag: 1, slot: 1 });
        assert!(o.changes.contains(&SlotChange::SetGems { bag: 1, slot: 0, gems: [1, 0, 0, 0, 0, 0] }));
        assert_eq!(o.destroyed, None);
        assert!(
            extract(&inv1, &defs, 1_000_000, 0, (60, 0), ExtractRequest::One { place: (1, 0), position: 2, hammer: (1, 1) }, NO_HOUSE)
                .is_none()
        );
    }

    #[test]
    fn enchanting_steps_and_names() {
        let defs = defs();
        let mut sword = unit(1, 0, 1, 1, 1);
        sword.craft_name = b"01000000000000000000".to_vec();
        let inv1 = inv(&[sword.clone(), unit(1, 1, 95, 1, 2), unit(1, 2, 25, 43, 5)]);
        let req = EnchantRequest { lapisia: (1, 1), material: (1, 2), count: 2, item: (1, 0) };
        let e = enchant(&inv1, &defs, 3_000_000, req).unwrap();
        assert_eq!((e.price, e.rate, e.armour, e.step, e.material), (2_500_000, 700_000, false, 0, Some(((1, 2), 2))));
        assert!(e.succeeds(699_997) && !e.succeeds(699_998), "the material count is added to the draw");
        assert_eq!(&e.new_name(&sword, defs.get(1, 1).unwrap()), b"01000000000000000001");
        let o = e.outcome(&inv1, &defs, false);
        assert_eq!(o.destroyed, Some((1, 0)), "a dangerous Lapisia");
        assert!(o.changes.contains(&SlotChange::SetCount { bag: 1, slot: 2, count: 3 }));
        assert!(enchant(&inv1, &defs, 3_000_000, EnchantRequest { material: (2, 0), ..req }).is_none(), "the material is needed");
        assert!(enchant(&inv1, &defs, 3_000_000, EnchantRequest { count: 6, ..req }).is_none(), "more than the slot holds");
        sword.craft_name = b"00000000000000000020".to_vec();
        let at_twenty = inv(&[sword, unit(1, 1, 95, 1, 1), unit(1, 2, 25, 43, 1)]);
        assert!(enchant(&at_twenty, &defs, 3_000_000, EnchantRequest { count: 1, ..req }).is_none(), "step 20");
        let blank = unit(1, 0, 1, 1, 1);
        assert_eq!(&e.new_name(&blank, defs.get(1, 1).unwrap()), b"00000000000000000001", "twenty zeros first");
        assert_eq!(e.jump, 1, "one step without a rule");
        assert_eq!(enchant_bonus(b"00000000000000000020"), (140, 0));
        assert_eq!(enchant_bonus(b"00000000000000000055"), (0, 25));
        assert_eq!(enchant_bonus(b"00000000000000000030"), (0, 0));
    }

    #[test]
    fn a_lapisias_enchant_effect_sets_the_rate_the_jump_table_and_the_danger() {
        use crate::effects::{ItemEffects, check_effect};
        let defs = defs();
        let mut sword = unit(1, 0, 1, 1, 1);
        sword.craft_name = b"01000000000000000000".to_vec();
        // The Lapisia 95/1 is dangerous (VG 1); the largest material of 100.
        let inv1 = inv(&[sword, unit(1, 1, 95, 1, 2), unit(1, 2, 25, 43, 100)]);
        let req = EnchantRequest { lapisia: (1, 1), material: (1, 2), count: 100, item: (1, 0) };
        let with = |effect: serde_json::Value| ItemEffects::new([(95001, check_effect(&effect).unwrap())]);
        let core = enchant(&inv1, &defs, 3_000_000, req).unwrap();
        assert!(!core.succeeds(999_999) && core.dangerous && core.own_jump(0).is_none() && !core.always, "the core's rate, danger and rules");
        // A rate of 1,000,000 always succeeds, even with 100 of material and the highest draw.
        let e = enchant_with(&inv1, &defs, 3_000_000, req, &with(serde_json::json!({ "type": "enchant", "rate": 1000000 }))).unwrap();
        assert!(e.always && e.succeeds(999_999) && e.succeeds(0));
        assert_eq!((e.price, e.step, e.dangerous), (2_500_000, 0, true), "the price and the danger stay the core's");
        assert_eq!(e.own_jump(0), None, "no table: the rules are asked");
        // A lower rate replaces the step's and still adds the material to the draw.
        let half = enchant_with(&inv1, &defs, 3_000_000, req, &with(serde_json::json!({ "type": "enchant", "rate": 500000 }))).unwrap();
        assert_eq!((half.rate, half.always), (500_000, false));
        assert!(half.succeeds(499_899) && !half.succeeds(499_900), "100 of material + the draw below 500,000");
        // The Lapisia's own table: used on success, with its weights.
        let table = with(serde_json::json!({ "type": "enchant", "jumps": [[1, 60], [2, 30], [3, 10]] }));
        let t = enchant_with(&inv1, &defs, 3_000_000, req, &table).unwrap();
        assert_eq!((t.own_jump(0), t.own_jump(600_000), t.own_jump(900_000), t.own_jump(999_999)), (Some(1), Some(2), Some(3), Some(3)));
        assert!(t.always, "an effect with a table alone has the default rate: always");
        // `safe` keeps a failed item whatever VG says; without it the core's rule stays.
        let safe = enchant_with(&inv1, &defs, 3_000_000, req, &with(serde_json::json!({ "type": "enchant", "rate": 1, "safe": true }))).unwrap();
        assert!(!safe.dangerous && safe.outcome(&inv1, &defs, false).destroyed.is_none(), "kept");
        let unsafe_ = enchant_with(&inv1, &defs, 3_000_000, req, &with(serde_json::json!({ "type": "enchant", "rate": 1 }))).unwrap();
        assert!(unsafe_.dangerous && unsafe_.outcome(&inv1, &defs, false).destroyed == Some((1, 0)));
        // An effect of another type, or on another item, changes nothing.
        let other = ItemEffects::new([(95001, check_effect(&serde_json::json!({ "type": "level_up", "levels": 1 })).unwrap()), (95002, check_effect(&serde_json::json!({ "type": "enchant", "rate": 1000000 })).unwrap())]);
        let same = enchant_with(&inv1, &defs, 3_000_000, req, &other).unwrap();
        assert_eq!((same.rate, same.always, same.dangerous), (core.rate, false, true));
        // The step still stops at 20 with a table that jumps further.
        let mut item = unit(1, 0, 1, 1, 1);
        item.craft_name = b"01000000000000000018".to_vec();
        let far = Enchant { step: 18, jump: 19, ..t };
        assert_eq!(&far.new_name(&item, defs.get(1, 1).unwrap())[18..], b"20");
    }

    #[test]
    fn a_jump_raises_the_step_by_more_than_one_and_stops_at_twenty() {
        let defs = defs();
        let mut sword = unit(1, 0, 1, 1, 1);
        sword.craft_name = b"01000000000000000000".to_vec();
        let inv1 = inv(&[sword.clone(), unit(1, 1, 95, 1, 2), unit(1, 2, 25, 43, 5)]);
        let req = EnchantRequest { lapisia: (1, 1), material: (1, 2), count: 2, item: (1, 0) };
        let e = enchant(&inv1, &defs, 3_000_000, req).unwrap();
        let named = |step: u8, jump: u8, armour: bool| {
            let mut item = unit(1, 0, 1, 1, 1);
            item.craft_name = format!("010000000000000000{:02}", step).into_bytes();
            let e = Enchant { step, jump, armour, ..e.clone() };
            String::from_utf8(e.new_name(&item, defs.get(1, 1).unwrap()).to_vec()).unwrap()
        };
        // A weapon: +1 as always, +3, and the cap at 20.
        assert_eq!(&named(0, 1, false)[18..], "01");
        assert_eq!(&named(0, 3, false)[18..], "03");
        assert_eq!(&named(7, 3, false)[18..], "10");
        assert_eq!(&named(17, 3, false)[18..], "20", "17 + 3 is 20");
        assert_eq!(&named(18, 3, false)[18..], "20", "18 + 3 stops at 20");
        assert_eq!(&named(19, 19, false)[18..], "20");
        // Armour is written + 50: 51 to 70.
        assert_eq!(&named(0, 1, true)[18..], "51");
        assert_eq!(&named(4, 3, true)[18..], "57");
        assert_eq!(&named(18, 3, true)[18..], "70", "the cap is step 20, written 70");
        assert_eq!(&named(19, 2, true)[18..], "70");
        // The bonus the client and the server read back follows the name.
        assert_eq!(enchant_bonus(named(18, 3, false).as_bytes()), (140, 0));
        assert_eq!(enchant_bonus(named(18, 3, true).as_bytes()), (0, 100));
    }

    #[test]
    fn elements_conflict_to_none() {
        let defs = defs();
        assert_eq!(element(0, &[3, 0, 0, 0, 0, 0], &defs), 3);
        assert_eq!(element(3, &[3, 1, 0, 0, 0, 0], &defs), 3, "lapis 1 has no element");
        assert_eq!(element(3, &[2, 0, 0, 0, 0, 0], &defs), 0);
        assert_eq!(element(0, &[2, 3, 0, 0, 0, 0], &defs), 0);
    }

    #[test]
    fn craft_rolls_choose_by_the_server_column() {
        let defs = defs();
        let mut draws = [1u32, 0, 7, 9, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4].into_iter().cycle();
        // A weapon with server 6: all six stats, the smaller of two draws % 11 each.
        let values = roll_craft(defs.get(1, 1).unwrap(), 2, &mut || draws.next().unwrap());
        assert_eq!(values, [0, 7, 4, 4, 4, 4, 0, 0, 0]);
        // Armour with server 3: three different numbers of nine.
        let mut seq = [0u32, 0, 0, 5, 5, 5, 5, 5, 5].into_iter();
        let values = roll_craft(defs.get(17, 1).unwrap(), 2, &mut || seq.next().unwrap_or(5));
        assert_eq!(values.iter().filter(|&&v| v != 0).count(), 3);
        assert_eq!((values[0], values[8], values[7]), (5, 5, 5), "0, then 8 moved to 0's place, then 7");
        assert_eq!(&craft_name([1, 2, 3, 4, 5, 6, 7, 8, 9], 55), b"01020304050607080955");
    }

    #[test]
    fn new_items_get_rolled_or_zero_names_when_craftable() {
        let defs = defs();
        let (sword, rune) = (defs.get(1, 1).unwrap(), defs.get(100, 171).unwrap());
        // Three draws each, the smallest kept; the enchant level is "00".
        let mut draws = [9u32, 3, 7].into_iter().cycle();
        assert_eq!(&new_craft_name(sword, &mut || draws.next().unwrap()), b"03030303030300000000");
        assert_eq!(&zero_craft_name(sword), b"00000000000000000000");
        assert_eq!(new_craft_name(rune, &mut || 1), [0; CRAFT_NAME_MAX], "not craftable: empty");
        assert_eq!(zero_craft_name(rune), [0; CRAFT_NAME_MAX]);
    }

    #[test]
    fn compose_consider_and_remake_rules() {
        let defs = defs();
        let mut identified = unit(1, 1, 1, 1, 1);
        identified.craft_name = b"01000000000000000000".to_vec();
        let inv1 = inv(&[unit(1, 0, 100, 171, 3), identified, unit(1, 2, 1, 1, 1), unit(1, 3, 1, 1, 1), unit(0, 5, 1, 1, 1)]);
        assert_eq!(compose(&inv1, &defs, (1, 0), (1, 1), 0), Ok((1, 1)));
        assert_eq!(compose(&inv1, &defs, (1, 0), (1, 1), b'Q'), Err(Refused::Result(1)));
        assert_eq!(compose(&inv1, &defs, (1, 2), (1, 1), 0), Err(Refused::Result(1)), "not a rune");
        assert_eq!(consider(&inv1, &defs, (1, 1), 0), Err(Refused::Result(1)), "already has numbers");
        assert_eq!(consider(&inv1, &defs, (1, 2), 0), Ok(()));
        assert_eq!(consider(&inv1, &defs, (1, 2), b'B'), Err(Refused::Result(1)));
        assert_eq!(remake(&inv1, &defs, [(0, 5), (1, 2), (1, 3)], [0; 3]), Ok((1, 2)), "the first not equipped");
        assert_eq!(remake(&inv1, &defs, [(1, 2), (1, 2), (1, 3)], [0; 3]), Err(Refused::Ignored), "deviation 2");
        assert_eq!(remake(&inv1, &defs, [(1, 0), (1, 2), (1, 3)], [0; 3]), Err(Refused::Result(1)), "not the same item");
        assert_eq!(remake(&inv1, &defs, [(1, 1), (1, 2), (1, 3)], [0, b'Q', 0]), Err(Refused::Result(2)));
        assert!(cloak_at(&inv(&[unit(1, 0, 24, 1, 1)]), &defs, (1, 0)));
    }

    #[test]
    fn a_stat_rune_redraws_only_the_named_numbers_the_item_has() {
        use crate::effects::{Recreate, RecreateValue};
        let defs = defs();
        let sword = defs.get(1, 1).unwrap();
        // STR 3, DEX 0, REC 7, the rest 0, at enchant step 4.
        let item = ItemUnit { craft_name: b"03000700000000000004".to_vec(), ..unit(1, 1, 1, 1, 1) };
        let named = |names: &[usize], value| {
            let mut stats = [false; 9];
            for &n in names {
                stats[n] = true;
            }
            Recreate { stats, value }
        };
        // "random": the smaller of two draws up to `craft_max` 10 (a span of 11): 9 % 11 and 4 % 11.
        let mut draws = [9u32, 4].into_iter();
        let mut rand = || draws.next().unwrap();
        let values = rerolled_values(&item, sword, &named(&[0], RecreateValue::Random), &mut rand).unwrap();
        assert_eq!(values, [4, 0, 7, 0, 0, 0, 0, 0, 0], "STR redrawn, REC stays");
        assert!(draws.next().is_none(), "two draws for one number");
        assert_eq!(rerolled_name(&item, values), craft_name([4, 0, 7, 0, 0, 0, 0, 0, 0], 4), "the enchant step is kept");
        // "max": the item's `reqwis`.
        let values = rerolled_values(&item, sword, &named(&[0, 2], RecreateValue::Max), &mut || unreachable!("no draw for max")).unwrap();
        assert_eq!(values, [10, 0, 10, 0, 0, 0, 0, 0, 0]);
        // A named number the item lacks (DEX is 0) does not appear; one that is present changes.
        let values = rerolled_values(&item, sword, &named(&[1, 2], RecreateValue::Max), &mut || 0).unwrap();
        assert_eq!(values, [3, 0, 10, 0, 0, 0, 0, 0, 0], "DEX stays 0, STR is not named");
        // None of the named numbers present: nothing to do.
        assert_eq!(rerolled_values(&item, sword, &named(&[1, 3, 8], RecreateValue::Max), &mut || 0), None);
        // A name that is not 20 characters has none.
        let bare = unit(1, 1, 1, 1, 1);
        assert_eq!(rerolled_values(&bare, sword, &named(&[0], RecreateValue::Max), &mut || 0), None);
        // A draw of 0 can bring a number to 0, as the core's rune can.
        let values = rerolled_values(&item, sword, &named(&[0], RecreateValue::Random), &mut || 0).unwrap();
        assert_eq!(values[0], 0);
    }

    #[test]
    fn the_jump_draw_follows_the_weights() {
        let jumps = [(1u8, 70u32), (2, 20), (3, 10)];
        let draw = |d| weighted_jump(&jumps, 100, d);
        assert_eq!((draw(0), draw(699_999), draw(700_000), draw(899_999), draw(900_000), draw(999_999)), (1, 1, 2, 2, 3, 3));
        assert_eq!(draw(ENCHANT_DRAW + 5), 3, "above the range counts as the last draw");
        assert_eq!(weighted_jump(&[], 0, 5), 1, "no table, one step");
        // Weights up to 1,000,000 each: 19 entries do not overflow.
        let big = [(1u8, 1_000_000u32), (19, 1_000_000)];
        assert_eq!((weighted_jump(&big, 2_000_000, 499_999), weighted_jump(&big, 2_000_000, 500_000)), (1, 19));
    }

    #[test]
    fn option_skills_come_from_the_weapon_and_the_armour_in_slot_order() {
        let lapis = |type_id: u8, special: u8| ItemDef {
            item_id: 30000 + u32::from(type_id),
            item_type: 30,
            type_id,
            special,
            ..Default::default()
        };
        let defs = ItemDefs::new([lapis(110, 1), lapis(111, 1), lapis(151, 205), lapis(152, 16), lapis(153, 99)]);
        let gems = |bag: u8, slot: u8, gems: [u8; 6]| ItemUnit { gems, ..unit(bag, slot, 1, 1, 1) };
        let items = [
            gems(0, 5, [110, 0, 151, 152, 153, 200]),
            // The upper armour without a shield or helmet: its lapis still count (deviation 1).
            gems(0, 1, [111, 0, 0, 0, 0, 0]),
            gems(0, 4, [0, 0, 0, 0, 0, 110]),
            // A cloak, a ring and a bag are not looked at.
            gems(0, 7, [110; 6]),
            gems(0, 9, [110; 6]),
            gems(1, 0, [110; 6]),
        ];
        let o = OptionSkills::new(&items, &defs, |id| id != 99);
        assert_eq!(o.weapon, [1, 0, 205, 0, 0, 0], "16 is recovery, 99 no skill at level 100, 200 no lapis");
        let mut armour = [0; 36];
        (armour[12], armour[35]) = (1, 1);
        assert_eq!(o.armour, armour, "the upper armour third, the boots last");
        assert_eq!(OptionSkills::new(&[], &defs, |_| true), OptionSkills::default());
    }
}
