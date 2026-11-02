//! Equipment wear and repairs (`docs/original/repair.md`). The functions only decide; the caller saves the changes and then applies
//! them.

use crate::combat::wear_level;
use crate::inventory::{Inventory, ItemDefs, ItemUnit, SlotChange};

/// The weapon's slot, worn by critical blows (`CUser::QualityByAttack`).
pub const WEAPON_SLOT: u8 = 5;
/// The armour worn by blows taken, in order (`CUser::QualityByDefense`): shield, helmet, upper armour, gloves, lower armour, boots.
const ARMOUR_ORDER: [u8; 6] = [6, 0, 1, 3, 2, 4];
/// A repair of everything covers equipment slots 0 to 12 (not the mount); one item may be in slots below 13 of bag 0.
const REPAIRED_SLOTS: u8 = 13;
const BAG_SLOTS: u8 = 24;
/// Bags up to 5 may be repaired, whatever the bag count (`CUser::ItemRepair`).
const LAST_BAG: u8 = 5;
/// Bag and slot 255: everything equipped.
pub const ALL: u8 = 255;
/// A gem is the type id of a lapis, item type 30.
const LAPIS_TYPE: u8 = 30;

/// An equipped item worn by 1 (`CUser::QualityDec`): its new quality, and whether its wear level changed (the numbers again).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Worn {
    pub slot: u8,
    pub quality: u16,
    pub level_changed: bool,
}

impl Worn {
    pub fn change(&self) -> SlotChange {
        SlotChange::SetQuality { bag: 0, slot: self.slot, quality: self.quality }
    }
}

/// `CUser::QualityDec(item, 1)`: `None` for an empty slot or an item at quality 0.
fn wear(inv: &Inventory, defs: &ItemDefs, slot: u8) -> Option<Worn> {
    let item = inv.get(0, slot).filter(|item| item.quality > 0)?;
    let quality = item.quality - 1;
    let level_changed =
        defs.get(item.item_type, item.type_id).is_some_and(|def| wear_level(item.quality, def.quality) != wear_level(quality, def.quality));
    Some(Worn { slot, quality, level_changed })
}

/// The weapon after a critical blow on a monster.
pub fn wear_weapon(inv: &Inventory, defs: &ItemDefs) -> Option<Worn> {
    wear(inv, defs, WEAPON_SLOT)
}

/// Which armour piece the next blow taken wears (`m_nQualityDec`, 0 at first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmourWear {
    next: usize,
}

impl ArmourWear {
    /// A blow taken: from the index to the end of the order, the first piece that can wear does, and the index moves past it; when
    /// none can, the index goes back to the start and nothing wears.
    pub fn wear(&mut self, inv: &Inventory, defs: &ItemDefs) -> Option<Worn> {
        for (i, &slot) in ARMOUR_ORDER.iter().enumerate().skip(self.next) {
            if let Some(worn) = wear(inv, defs, slot) {
                self.next = (i + 1) % ARMOUR_ORDER.len();
                return Some(worn);
            }
        }
        self.next = 0;
        None
    }
}

/// An item's full quality: its definition's plus that of each lapis in its gems.
pub fn full_quality(item: &ItemUnit, defs: &ItemDefs) -> Option<u32> {
    let def = defs.get(item.item_type, item.type_id)?;
    let lapis = item.gems.iter().filter_map(|&gem| defs.get(LAPIS_TYPE, gem)).map(|lapis| u32::from(lapis.quality));
    Some(u32::from(def.quality) + lapis.sum::<u32>())
}

/// The price of bringing one item back to full quality, before truncation: `f32(full − quality) / 400 × f32(f32(nBuy) / 30)` in the
/// x87 at double precision.
fn price(full: u32, quality: u16, buy: u32) -> f64 {
    let missing = (full as i32).wrapping_sub(i32::from(quality)) as f32;
    let per_thirty = buy as f32 / 30.0;
    f64::from(missing) / 400.0 * f64::from(per_thirty)
}

/// The price charged: the guild house rate (1 elsewhere) applied, truncated, at least 1.
fn charge(total: i32, rate: f64) -> u32 {
    let total = if rate == 1.0 { total } else { (f64::from(total) * rate) as i32 };
    total.max(1) as u32
}

/// A repair decided: what it costs and the qualities it sets; `renumber` when an equipped item's wear level changes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Repair {
    pub cost: u32,
    pub changes: Vec<SlotChange>,
    pub renumber: bool,
}

/// `0x0554` (`CUser::ItemRepair`, section 5): one item, or everything equipped with bag and slot 255. `None` when the original does
/// nothing: not a place it repairs, an empty slot, an item already at full quality, or `money` short of the price.
pub fn repair(inv: &Inventory, defs: &ItemDefs, money: u32, bag: u8, slot: u8, rate: f64) -> Option<Repair> {
    if bag == ALL {
        return (slot == ALL).then(|| repair_all(inv, defs, rate)).filter(|r| r.cost <= money);
    }
    let last_slot = if bag == 0 { REPAIRED_SLOTS } else { BAG_SLOTS };
    if bag > LAST_BAG || slot >= last_slot {
        return None;
    }
    let item = inv.get(bag, slot)?;
    let full = full_quality(item, defs)?;
    if full as i32 <= i32::from(item.quality) {
        return None;
    }
    let cost = charge(price(full, item.quality, defs.get(item.item_type, item.type_id)?.buy) as i32, rate);
    if money < cost {
        return None;
    }
    let (quality, renumber) = restored(item, full, defs);
    Some(Repair { cost, changes: vec![SlotChange::SetQuality { bag, slot, quality }], renumber: bag == 0 && renumber })
}

/// Everything in equipment slots 0 to 12 back to full quality: the price is summed item by item (each step truncated, an item at full
/// quality adding 0), at least 1 even when nothing is worn.
fn repair_all(inv: &Inventory, defs: &ItemDefs, rate: f64) -> Repair {
    let mut total = 0i32;
    let mut repair = Repair::default();
    for slot in 0..REPAIRED_SLOTS {
        let Some(item) = inv.get(0, slot) else { continue };
        let (Some(full), Some(def)) = (full_quality(item, defs), defs.get(item.item_type, item.type_id)) else { continue };
        total = (f64::from(total as f32) + price(full, item.quality, def.buy)) as i32;
        if u32::from(item.quality) != full {
            let (quality, renumber) = restored(item, full, defs);
            repair.changes.push(SlotChange::SetQuality { bag: 0, slot, quality });
            repair.renumber |= renumber;
        }
    }
    repair.cost = charge(total, rate);
    repair
}

/// The Repair Hammer (item special 40): everything equipped back to full quality, for nothing.
pub fn repair_all_free(inv: &Inventory, defs: &ItemDefs) -> Repair {
    Repair { cost: 0, ..repair_all(inv, defs, 1.0) }
}

/// The quality an item is repaired to (the `u16` of its full quality) and whether its wear level changes.
fn restored(item: &ItemUnit, full: u32, defs: &ItemDefs) -> (u16, bool) {
    let quality = full as u16;
    let changed =
        defs.get(item.item_type, item.type_id).is_some_and(|def| wear_level(item.quality, def.quality) != wear_level(quality, def.quality));
    (quality, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    fn defs() -> ItemDefs {
        ItemDefs::new([
            // A sword: quality 100, bought for 3000; armour pieces; a lapis adding 20 quality.
            ItemDef { item_id: 1001, item_type: 1, type_id: 1, quality: 100, buy: 3000, max_count: 1, ..Default::default() },
            ItemDef { item_id: 16001, item_type: 16, type_id: 1, quality: 50, buy: 900, max_count: 1, ..Default::default() },
            ItemDef { item_id: 30005, item_type: 30, type_id: 5, quality: 20, max_count: 1, ..Default::default() },
        ])
    }

    fn item(bag: u8, slot: u8, item_type: u8, quality: u16) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id: 1, quality, count: 1, ..Default::default() }
    }

    #[test]
    fn blows_wear_the_armour_in_order_from_the_index() {
        let defs = defs();
        // Shield (6) at 0, helmet (0) at 30, gloves (3) at 50; no upper armour.
        let mut inv = Inventory::new(&[item(0, 6, 16, 0), item(0, 0, 16, 30), item(0, 3, 16, 50)]);
        let mut wear = ArmourWear::default();
        let worn = |inv: &mut Inventory, wear: &mut ArmourWear| {
            let w = wear.wear(inv, &defs)?;
            inv.apply(&w.change());
            Some(w.slot)
        };
        // The shield is at 0: the helmet, then the gloves (upper armour missing), then nothing to the end: back to the start.
        assert_eq!(worn(&mut inv, &mut wear), Some(0));
        assert_eq!(worn(&mut inv, &mut wear), Some(3));
        assert_eq!(worn(&mut inv, &mut wear), None);
        assert_eq!(worn(&mut inv, &mut wear), Some(0));
        assert_eq!(inv.get(0, 0).unwrap().quality, 28);
        assert_eq!(inv.get(0, 3).unwrap().quality, 49);
    }

    #[test]
    fn wear_levels_change_at_half_and_a_tenth() {
        let defs = defs();
        // Quality 51 → 50 is half of 100: level 0 → 1. 11 → 10: level 1 → 2. 0 does not wear.
        for (quality, changed) in [(51, true), (50, false), (11, true), (10, false)] {
            let inv = Inventory::new(&[item(0, 5, 1, quality)]);
            assert_eq!(wear_weapon(&inv, &defs), Some(Worn { slot: 5, quality: quality - 1, level_changed: changed }), "{quality}");
        }
        assert_eq!(wear_weapon(&Inventory::new(&[item(0, 5, 1, 0)]), &defs), None);
    }

    #[test]
    fn one_item_costs_by_what_it_misses_and_its_price() {
        let defs = defs();
        // A sword at 60 of 100: 40 / 400 × 3000 / 30 = 10. With a lapis its full quality is 120: 60 / 400 × 100 = 15.
        let worn = Inventory::new(&[item(0, 5, 1, 60), ItemUnit { gems: [5, 0, 0, 0, 0, 0], ..item(1, 0, 1, 60) }]);
        let r = repair(&worn, &defs, 1000, 0, 5, 1.0).unwrap();
        assert_eq!((r.cost, r.renumber), (10, false));
        assert_eq!(r.changes, [SlotChange::SetQuality { bag: 0, slot: 5, quality: 100 }]);
        assert_eq!(repair(&worn, &defs, 1000, 1, 0, 1.0).unwrap().cost, 15);
        assert_eq!(repair(&worn, &defs, 1000, 1, 0, 0.9).unwrap().cost, 13, "a guild house's rate, truncated");
        assert_eq!(repair(&worn, &defs, 1000, 1, 0, 0.0).unwrap().cost, 1, "at least 1");
        // Short of money, at full quality, an empty slot, the mount slot, a bag past 5: nothing.
        assert_eq!(repair(&worn, &defs, 9, 0, 5, 1.0), None);
        assert_eq!(repair(&Inventory::new(&[item(0, 5, 1, 100)]), &defs, 1000, 0, 5, 1.0), None);
        for (bag, slot) in [(0, 6), (0, 13), (6, 0), (1, 24)] {
            assert_eq!(repair(&worn, &defs, 1000, bag, slot, 1.0), None, "{bag} {slot}");
        }
        // A tiny repair costs at least 1; a worn-out sword brought back changes its wear level.
        assert_eq!(repair(&Inventory::new(&[item(0, 5, 1, 99)]), &defs, 1000, 0, 5, 1.0).unwrap().cost, 1);
        assert!(repair(&Inventory::new(&[item(0, 5, 1, 5)]), &defs, 1000, 0, 5, 1.0).unwrap().renumber);
    }

    #[test]
    fn everything_sums_the_prices_and_costs_at_least_1() {
        let defs = defs();
        // In slot order, each step truncated: helmet 10 short of 50 (10 / 400 × 30 = 0.75 → 0), boots full (0), sword 40 short (10).
        // A shield in bag 1 is not counted.
        let inv = Inventory::new(&[item(0, 5, 1, 60), item(0, 0, 16, 40), item(0, 4, 16, 50), item(1, 0, 16, 0)]);
        let r = repair(&inv, &defs, 1000, ALL, ALL, 1.0).unwrap();
        assert_eq!(r.cost, 10);
        assert_eq!(
            r.changes,
            [SlotChange::SetQuality { bag: 0, slot: 0, quality: 50 }, SlotChange::SetQuality { bag: 0, slot: 5, quality: 100 }]
        );
        // Nothing worn still costs 1; bag 255 with another slot does nothing; the hammer is free.
        let fine = Inventory::new(&[item(0, 5, 1, 100)]);
        assert_eq!(repair(&fine, &defs, 1000, ALL, ALL, 1.0), Some(Repair { cost: 1, changes: vec![], renumber: false }));
        assert_eq!(repair(&fine, &defs, 0, ALL, ALL, 1.0), None);
        assert_eq!(repair(&inv, &defs, 1000, ALL, 3, 1.0), None);
        assert_eq!(repair_all_free(&inv, &defs).cost, 0);
    }
}
