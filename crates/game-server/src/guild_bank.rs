//! The guild bank (`docs/original/guild-bank.md`): 240 places, bag 255, shared by a guild's members. The functions only decide;
//! the caller saves the changes and then applies them (as the warehouse's in `inventory.rs`).

use crate::backend::StoredGuild;
use crate::guild_house::{self as gh, House, HouseRules, REFUSED_LEVEL, REFUSED_NOT_AUTHORIZED, REFUSED_RANKING};
use crate::inventory::{Inventory, ItemDefs, ItemUnit, SlotChange, real_type, withdrawal_fee};
use shaiya_proto::world::GUILD_BANK;

/// The bank's places (`CUser::IsGuildBank`: bag 255, slot below 240).
pub const SLOTS: usize = 240;
/// Guild ranks (`CUser +0x174c`): the master and the officers may do everything, from rank 3 on a member only fills empty places,
/// and the newest (9) nothing.
const RANK_OFFICER: u8 = 2;
const RANK_NEW: u8 = 9;

/// What a refused move answers: `None` nothing at all, `Some((type, require))` a `0x0D28`.
pub type Refusal = Option<(u8, u8)>;

/// A guild's bank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildBank {
    slots: Vec<Option<ItemUnit>>,
}

impl Default for GuildBank {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl GuildBank {
    /// From the stored items (bag 255, slot below 240).
    pub fn new(items: &[ItemUnit]) -> Self {
        let mut slots = vec![None; SLOTS];
        for item in items.iter().filter(|i| i.bag == GUILD_BANK && usize::from(i.slot) < SLOTS) {
            slots[usize::from(item.slot)] = Some(item.clone());
        }
        Self { slots }
    }

    pub fn get(&self, slot: u8) -> Option<&ItemUnit> {
        self.slots.get(usize::from(slot))?.as_ref()
    }

    /// The items, by slot.
    pub fn items(&self) -> impl Iterator<Item = &ItemUnit> {
        self.slots.iter().flatten()
    }

    pub fn is_empty(&self) -> bool {
        self.items().next().is_none()
    }

    fn take(&mut self, slot: u8) -> Option<ItemUnit> {
        self.slots.get_mut(usize::from(slot))?.take()
    }

    fn put(&mut self, slot: u8, item: Option<ItemUnit>) {
        if let Some(cell) = self.slots.get_mut(usize::from(slot)) {
            *cell = item.map(|item| ItemUnit { bag: GUILD_BANK, slot, ..item });
        }
    }
}

/// How many places may be used (`CGuild::GetGuildHouseBankAllowSlotCount`): the warehouse NPC's level, at most 5, picks the entry
/// of `[Bank] PermitSlot`. The house need not be bought or open.
pub fn allowed_slots(house: &House, rules: &HouseRules) -> usize {
    let level = usize::from(house.level(gh::KEY_BANK).min(5));
    rules.bank_slots.get(level).map_or(0, |&slots| slots as usize)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Deposit,
    Withdraw,
    Rearrange,
}

/// A move decided: the fee to pay and the changes to save.
#[derive(Debug, PartialEq, Eq)]
pub struct Planned {
    pub kind: Kind,
    pub fee: u32,
    pub changes: Vec<SlotChange>,
}

/// A move with bag 255 on one side (`CUser::PacketMaininterface`, `CGuild::ItemBagToGuildBank`, `ItemGuildBankToBag`,
/// `ItemGuildBankToGuildBank`; sections 5 to 8, with the deviations 2 and 3 of section 14). `ranking` is the guild's place (31
/// unranked), `allowed` the usable places, `rank` the member's guild rank, `money` pays the fee. `Err(None)` when nothing happens.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    bank: &GuildBank,
    inventory: &Inventory,
    defs: &ItemDefs,
    ranking: u8,
    allowed: usize,
    rank: u8,
    money: u32,
    from: (u8, u8),
    to: (u8, u8),
) -> Result<Planned, Refusal> {
    let level_needed = |slot: u8| (REFUSED_LEVEL, slot / 40);
    let ranked = || if ranking > gh::ANY_RANKING { Err(Some((REFUSED_RANKING, gh::ANY_RANKING))) } else { Ok(()) };
    let officer = || if rank > RANK_OFFICER { Err(Some((REFUSED_NOT_AUTHORIZED, RANK_OFFICER))) } else { Ok(()) };
    let in_range = |slot: u8| usize::from(slot) < SLOTS;
    // Items that may enter the bank: a definition, not real type 27, not bound.
    let may_enter =
        |item: &ItemUnit| defs.get(item.item_type, item.type_id).is_some_and(|def| real_type(def.item_type) != 27 && !def.bound);
    match (from.0 == GUILD_BANK, to.0 == GUILD_BANK) {
        (false, true) => {
            if !inventory.is_bag(from) || !in_range(to.1) {
                return Err(None);
            }
            let moving = inventory.get(from.0, from.1).ok_or(None)?;
            if !may_enter(moving) || from.0 == 0 {
                return Err(None);
            }
            ranked()?;
            if rank >= RANK_NEW {
                return Err(Some((REFUSED_NOT_AUTHORIZED, RANK_NEW - 1)));
            }
            if usize::from(to.1) >= allowed {
                return Err(Some(level_needed(to.1)));
            }
            let taken = bank.get(to.1).is_some();
            if taken {
                officer()?;
            }
            let change = if taken { SlotChange::Swap { a: from, b: to } } else { SlotChange::Move { from, to } };
            Ok(Planned { kind: Kind::Deposit, fee: 0, changes: vec![change] })
        }
        (true, false) => {
            if !inventory.is_bag(to) || !in_range(from.1) {
                return Err(None);
            }
            let moving = bank.get(from.1).ok_or(None)?;
            if to.0 == 0 {
                return Err(None);
            }
            ranked()?;
            officer()?;
            if usize::from(from.1) >= allowed {
                return Err(Some(level_needed(from.1)));
            }
            let fee = withdrawal_fee(defs.get(moving.item_type, moving.type_id).ok_or(None)?, moving.count);
            if money < fee {
                return Err(None);
            }
            let change = match inventory.get(to.0, to.1) {
                None => SlotChange::Move { from, to },
                Some(other) if may_enter(other) => SlotChange::Swap { a: to, b: from },
                Some(_) => return Err(None),
            };
            Ok(Planned { kind: Kind::Withdraw, fee, changes: vec![change] })
        }
        (true, true) => {
            if !in_range(from.1) || !in_range(to.1) || from == to {
                return Err(None);
            }
            let moving = bank.get(from.1).ok_or(None)?;
            ranked()?;
            officer()?;
            if usize::from(to.1) >= allowed {
                return Err(Some(level_needed(to.1)));
            }
            let changes = match bank.get(to.1) {
                None => vec![SlotChange::Move { from, to }],
                Some(other) => combine(defs, moving, other, from, to).unwrap_or_else(|| vec![SlotChange::Swap { a: from, b: to }]),
            };
            Ok(Planned { kind: Kind::Rearrange, fee: 0, changes })
        }
        (false, false) => Err(None),
    }
}

/// Two stacks of the same item (`CItem::EnableCombine`, as `Inventory::combine`): everything into `to` when it fits the stack
/// limit, otherwise `to` full and `from` the rest. `None` when they do not combine.
fn combine(defs: &ItemDefs, moving: &ItemUnit, other: &ItemUnit, from: (u8, u8), to: (u8, u8)) -> Option<Vec<SlotChange>> {
    let stack = |item: &ItemUnit| defs.get(item.item_type, item.type_id).filter(|def| def.max_count > 1);
    let (def, _) = (stack(moving)?, stack(other)?);
    if (moving.item_type, moving.type_id) != (other.item_type, other.type_id) {
        return None;
    }
    let total = u16::from(moving.count) + u16::from(other.count);
    let max = u16::from(def.max_count);
    Some(if total > max {
        vec![
            SlotChange::SetCount { bag: to.0, slot: to.1, count: def.max_count },
            SlotChange::SetCount { bag: from.0, slot: from.1, count: (total - max) as u8 },
        ]
    } else {
        vec![SlotChange::SetCount { bag: to.0, slot: to.1, count: total as u8 }, SlotChange::Remove { bag: from.0, slot: from.1 }]
    })
}

/// The stored banks at start (section 2): an item whose type has no definition is left out (the original crashes), and an equipment
/// item (real type 1 to 25) whose count is above its stack limit is lowered to it. Returns the fixed counts as `(guild, slot,
/// count)` for the database.
pub fn check_loaded(stored: &mut [StoredGuild], defs: &ItemDefs) -> Vec<(u32, u8, u8)> {
    let mut fixes = Vec::new();
    for guild in stored {
        let id = guild.id;
        guild.bank.retain_mut(|item| {
            let Some(def) = defs.get(item.item_type, item.type_id) else {
                tracing::error!(
                    guild = id,
                    slot = item.slot,
                    item_type = item.item_type,
                    type_id = item.type_id,
                    "guild bank item without a definition left out"
                );
                return false;
            };
            if (1..=25).contains(&real_type(def.item_type)) && item.count > def.max_count {
                let count = def.max_count.max(1);
                tracing::warn!(
                    guild = id,
                    slot = item.slot,
                    item_type = item.item_type,
                    type_id = item.type_id,
                    had = item.count,
                    count,
                    "guild bank item count above its stack limit lowered"
                );
                item.count = count;
                fixes.push((id, item.slot, count));
            }
            true
        });
    }
    fixes
}

fn take(bank: &mut GuildBank, inventory: &mut Inventory, place: (u8, u8)) -> Option<ItemUnit> {
    if place.0 == GUILD_BANK { bank.take(place.1) } else { inventory.take(place) }
}

fn put(bank: &mut GuildBank, inventory: &mut Inventory, place: (u8, u8), item: Option<ItemUnit>) {
    if place.0 == GUILD_BANK { bank.put(place.1, item) } else { inventory.put(place, item) }
}

/// Makes a saved change: moves and swaps between a bag place and a bank place cross, changes at bag 255 touch only the bank,
/// anything else is the inventory's.
pub fn apply(bank: &mut GuildBank, inventory: &mut Inventory, change: &SlotChange) {
    match change {
        SlotChange::Move { from, to } if from.0 == GUILD_BANK || to.0 == GUILD_BANK => {
            let item = take(bank, inventory, *from);
            put(bank, inventory, *to, item);
        }
        SlotChange::Swap { a, b } if a.0 == GUILD_BANK || b.0 == GUILD_BANK => {
            let (first, second) = (take(bank, inventory, *a), take(bank, inventory, *b));
            put(bank, inventory, *a, second);
            put(bank, inventory, *b, first);
        }
        SlotChange::SetCount { bag: GUILD_BANK, slot, count } => {
            if let Some(Some(item)) = bank.slots.get_mut(usize::from(*slot)) {
                item.count = *count;
            }
        }
        SlotChange::Remove { bag: GUILD_BANK, slot } => {
            bank.take(*slot);
        }
        _ => inventory.apply(change),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    fn def(item_type: u8, type_id: u8, max_count: u8, sell: u32, bound: bool) -> ItemDef {
        ItemDef {
            item_id: u32::from(item_type) * 1000 + u32::from(type_id),
            item_type,
            type_id,
            max_count,
            sell,
            bound,
            ..Default::default()
        }
    }

    fn defs() -> ItemDefs {
        ItemDefs::new([
            def(25, 3, 10, 110, false),
            def(25, 1, 10, 10, false),
            def(94, 5, 1, 5000, false),
            def(44, 42, 10, 1, true),
            def(27, 1, 1, 1, false),
            def(5, 1, 1, 100, false),
        ])
    }

    fn unit(bag: u8, slot: u8, item_type: u8, type_id: u8, count: u8) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id, count, ..Default::default() }
    }

    fn bank(items: &[ItemUnit]) -> GuildBank {
        GuildBank::new(items)
    }

    /// Rank 1 in a ranked guild with 240 usable places and money for any fee, as the tests start.
    fn run(b: &GuildBank, inv: &Inventory, rank: u8, from: (u8, u8), to: (u8, u8)) -> Result<Planned, Refusal> {
        plan(b, inv, &defs(), 5, 240, rank, 1000, from, to)
    }

    #[test]
    fn the_slot_count_follows_the_warehouse_level() {
        let rules = HouseRules::default();
        let mut house = House::default();
        assert_eq!(allowed_slots(&house, &rules), 40, "no slot for the key: level 0");
        house.slots[0] = (gh::KEY_BANK, 1);
        assert_eq!(allowed_slots(&house, &rules), 80);
        house.slots[0] = (gh::KEY_BANK, 5);
        assert_eq!(allowed_slots(&house, &rules), 240);
        house.slots[0] = (gh::KEY_BANK, 7);
        assert_eq!(allowed_slots(&house, &rules), 240, "capped at 5");
        let short = HouseRules { bank_slots: vec![40], ..HouseRules::default() };
        assert_eq!(allowed_slots(&house, &short), 0, "a shorter list gives 0");
    }

    #[test]
    fn deposits_follow_the_checks_in_order() {
        let inv =
            Inventory::new(&[unit(1, 2, 25, 3, 5), unit(1, 3, 25, 3, 5), unit(1, 4, 44, 42, 3), unit(1, 5, 27, 1, 1), unit(0, 5, 5, 1, 1)]);
        let empty = bank(&[]);
        let full = bank(&[unit(255, 0, 25, 3, 2)]);
        let go = |b: &GuildBank, rank, to| run(b, &inv, rank, (1, 2), (255, to));
        assert_eq!(
            go(&empty, 1, 0),
            Ok(Planned { kind: Kind::Deposit, fee: 0, changes: vec![SlotChange::Move { from: (1, 2), to: (255, 0) }] })
        );
        assert_eq!(go(&empty, 3, 0).map(|p| p.fee), Ok(0), "a member fills an empty place");
        assert_eq!(go(&full, 1, 0).map(|p| p.changes), Ok(vec![SlotChange::Swap { a: (1, 2), b: (255, 0) }]));
        assert_eq!(go(&full, 2, 0).map(|p| p.kind), Ok(Kind::Deposit));
        assert_eq!(go(&full, 3, 0), Err(Some((4, 2))), "deviation 3: not authorized to swap");
        assert_eq!(go(&empty, 9, 0), Err(Some((4, 8))), "rank 9 puts nothing in");
        assert_eq!(plan(&empty, &inv, &defs(), 5, 40, 1, 0, (1, 2), (255, 45)), Err(Some((2, 1))), "slot 45 at level 0");
        assert_eq!(go(&empty, 1, 240), Err(None));
        assert_eq!(plan(&empty, &inv, &defs(), 31, 240, 1, 0, (1, 2), (255, 0)), Err(Some((1, 30))), "unranked");
        assert_eq!(plan(&empty, &inv, &defs(), 30, 240, 1, 0, (1, 2), (255, 0)).map(|p| p.kind), Ok(Kind::Deposit), "30 is ranked");
        // The ranking comes before the rank, the rank before the slot count.
        assert_eq!(plan(&empty, &inv, &defs(), 31, 40, 9, 0, (1, 2), (255, 100)), Err(Some((1, 30))));
        assert_eq!(plan(&empty, &inv, &defs(), 5, 40, 9, 0, (1, 2), (255, 100)), Err(Some((4, 8))));
        assert_eq!(run(&empty, &inv, 1, (1, 4), (255, 0)), Err(None), "a bound item");
        assert_eq!(run(&empty, &inv, 1, (1, 5), (255, 0)), Err(None), "real type 27");
        assert_eq!(run(&empty, &inv, 1, (0, 5), (255, 0)), Err(None), "from the equipment");
        assert_eq!(run(&empty, &inv, 1, (1, 9), (255, 0)), Err(None), "an empty bag place");
        assert_eq!(run(&empty, &inv, 1, (100, 0), (255, 0)), Err(None), "the warehouse");
    }

    #[test]
    fn withdrawals_pay_the_fee_and_swap_onto_a_taken_place() {
        let inv = Inventory::new(&[unit(1, 0, 25, 1, 1), unit(1, 4, 44, 42, 3)]);
        let b = bank(&[unit(255, 0, 25, 3, 5), unit(255, 1, 94, 5, 1), unit(255, 50, 25, 3, 1)]);
        let out = |rank, money, from, to| plan(&b, &inv, &defs(), 5, 240, rank, money, from, to);
        let moved = out(1, 100, (255, 0), (1, 2)).unwrap();
        assert_eq!((moved.kind, moved.fee), (Kind::Withdraw, 27), "5 x 110 = 550, 5 % rounded down");
        assert_eq!(moved.changes, vec![SlotChange::Move { from: (255, 0), to: (1, 2) }]);
        assert_eq!(out(1, 0, (255, 1), (1, 2)).map(|p| p.fee), Ok(0), "a guild bar is free");
        assert_eq!(out(1, 26, (255, 0), (1, 2)), Err(None), "too little money");
        assert_eq!(out(1, 27, (255, 0), (1, 2)).map(|p| p.fee), Ok(27));
        assert_eq!(out(1, 100, (255, 0), (1, 0)).map(|p| p.changes), Ok(vec![SlotChange::Swap { a: (1, 0), b: (255, 0) }]));
        assert_eq!(out(1, 100, (255, 0), (1, 4)), Err(None), "deviation 2: onto a bound item");
        assert_eq!(out(1, 100, (255, 0), (0, 3)), Err(None), "into the equipment");
        assert_eq!(out(1, 100, (255, 7), (1, 2)), Err(None), "an empty bank place");
        assert_eq!(out(3, 100, (255, 0), (1, 2)), Err(Some((4, 2))), "deviation 3");
        assert_eq!(out(2, 100, (255, 0), (1, 2)).map(|p| p.kind), Ok(Kind::Withdraw));
        assert_eq!(plan(&b, &inv, &defs(), 31, 240, 1, 100, (255, 0), (1, 2)), Err(Some((1, 30))));
        assert_eq!(plan(&b, &inv, &defs(), 5, 40, 1, 100, (255, 50), (1, 2)), Err(Some((2, 1))), "slot 50 needs level 1");
        let unknown = bank(&[unit(255, 0, 99, 99, 1)]);
        assert_eq!(plan(&unknown, &inv, &defs(), 5, 240, 1, 100, (255, 0), (1, 2)), Err(None), "no definition, no fee");
    }

    #[test]
    fn rearranging_moves_swaps_combines_and_splits() {
        let inv = Inventory::new(&[]);
        let b =
            bank(&[unit(255, 0, 25, 3, 5), unit(255, 1, 25, 3, 5), unit(255, 2, 25, 3, 8), unit(255, 3, 5, 1, 1), unit(255, 4, 94, 5, 1)]);
        let run = |rank, from, to| plan(&b, &inv, &defs(), 5, 240, rank, 0, (255, from), (255, to));
        assert_eq!(
            run(1, 0, 9).map(|p| (p.kind, p.fee, p.changes)),
            Ok((Kind::Rearrange, 0, vec![SlotChange::Move { from: (255, 0), to: (255, 9) }]))
        );
        assert_eq!(
            run(1, 0, 1).map(|p| p.changes),
            Ok(vec![SlotChange::SetCount { bag: 255, slot: 1, count: 10 }, SlotChange::Remove { bag: 255, slot: 0 },])
        );
        assert_eq!(
            run(1, 2, 0).map(|p| p.changes),
            Ok(vec![SlotChange::SetCount { bag: 255, slot: 0, count: 10 }, SlotChange::SetCount { bag: 255, slot: 2, count: 3 },])
        );
        assert_eq!(run(1, 3, 4).map(|p| p.changes), Ok(vec![SlotChange::Swap { a: (255, 3), b: (255, 4) }]));
        assert_eq!(run(1, 0, 3).map(|p| p.changes), Ok(vec![SlotChange::Swap { a: (255, 0), b: (255, 3) }]), "stack onto another item");
        assert_eq!(run(3, 0, 9), Err(Some((4, 2))));
        assert_eq!(run(1, 0, 0), Err(None));
        assert_eq!(run(1, 9, 0), Err(None), "an empty source");
        assert_eq!(run(1, 0, 240), Err(None));
        assert_eq!(plan(&b, &inv, &defs(), 31, 240, 1, 0, (255, 0), (255, 9)), Err(Some((1, 30))));
        assert_eq!(plan(&b, &inv, &defs(), 5, 40, 1, 0, (255, 0), (255, 90)), Err(Some((2, 2))), "the target is checked");
        assert_eq!(plan(&b, &inv, &defs(), 5, 1, 1, 0, (255, 3), (255, 1)), Err(Some((2, 0))));
        assert_eq!(plan(&b, &inv, &defs(), 5, 1, 1, 0, (255, 100), (255, 0)), Err(None));
        assert_eq!(plan(&b, &inv, &defs(), 5, 240, 1, 0, (255, 0), (100, 0)), Err(None), "bank to warehouse");
    }

    #[test]
    fn loading_drops_unknown_items_and_lowers_equipment_counts() {
        let stored = |bank| StoredGuild {
            id: 7,
            name: "G".into(),
            country: 0,
            remark: String::new(),
            members: Vec::new(),
            house: Default::default(),
            bank,
        };
        // Type 5 is a sword (real type 5, limit 1), 25 food (limit 10), 99 has no definition.
        let mut guilds = [stored(vec![unit(255, 0, 5, 1, 4), unit(255, 1, 25, 1, 200), unit(255, 2, 99, 9, 1), unit(255, 3, 25, 3, 10)])];
        let fixes = check_loaded(&mut guilds, &defs());
        assert_eq!(fixes, [(7, 0, 1)], "only the equipment is lowered");
        let kept: Vec<_> = guilds[0].bank.iter().map(|i| (i.slot, i.count)).collect();
        assert_eq!(kept, [(0, 1), (1, 200), (3, 10)]);
    }

    #[test]
    fn apply_moves_items_and_sets_bag_and_slot() {
        let mut inv = Inventory::new(&[unit(1, 2, 25, 3, 5), unit(1, 0, 25, 1, 1)]);
        let mut b = bank(&[unit(255, 7, 5, 1, 1), unit(255, 8, 25, 3, 5)]);
        apply(&mut b, &mut inv, &SlotChange::Move { from: (1, 2), to: (255, 0) });
        assert!(inv.get(1, 2).is_none());
        let put = b.get(0).unwrap();
        assert_eq!((put.bag, put.slot, put.count), (255, 0, 5));
        apply(&mut b, &mut inv, &SlotChange::Swap { a: (1, 0), b: (255, 7) });
        assert_eq!(inv.get(1, 0).map(|i| (i.bag, i.slot, i.item_type)), Some((1, 0, 5)));
        assert_eq!(b.get(7).map(|i| (i.bag, i.slot, i.item_type)), Some((255, 7, 25)));
        apply(&mut b, &mut inv, &SlotChange::Move { from: (255, 8), to: (1, 5) });
        assert!(b.get(8).is_none());
        assert_eq!(inv.get(1, 5).map(|i| (i.bag, i.slot, i.count)), Some((1, 5, 5)));
        apply(&mut b, &mut inv, &SlotChange::Move { from: (255, 0), to: (255, 9) });
        assert_eq!(b.get(9).map(|i| (i.bag, i.slot)), Some((255, 9)));
        apply(&mut b, &mut inv, &SlotChange::Swap { a: (255, 9), b: (255, 7) });
        assert_eq!(b.get(9).map(|i| i.item_type), Some(25));
        assert_eq!(b.get(7).map(|i| (i.slot, i.count)), Some((7, 5)));
        apply(&mut b, &mut inv, &SlotChange::SetCount { bag: 255, slot: 7, count: 9 });
        assert_eq!(b.get(7).map(|i| i.count), Some(9));
        apply(&mut b, &mut inv, &SlotChange::Remove { bag: 255, slot: 7 });
        assert!(b.get(7).is_none());
        // A bag change is the inventory's.
        apply(&mut b, &mut inv, &SlotChange::Move { from: (1, 5), to: (1, 6) });
        assert!(inv.get(1, 6).is_some() && inv.get(1, 5).is_none());
        assert_eq!(b.items().map(|i| i.slot).collect::<Vec<_>>(), [9]);
        assert!(!b.is_empty());
        assert!(GuildBank::default().is_empty());
    }
}
