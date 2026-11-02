//! A character's bags: the NPC shop rules (`docs/original/shop.md`) and moving and equipping items (`docs/original/items.md`).
//! The functions only decide; the caller saves the changes and then applies them.

use std::collections::HashMap;

pub use crate::backend::{CRAFT_NAME_MAX, ItemUnit, Kept, NewItem};

/// Bag 0 is the equipment, 1 to 5 the inventory.
pub const BAGS: usize = 6;
pub const SLOTS: usize = 24;
/// Inventory bags usable without the passive skill that adds more (`CUser::InitEquipment`).
pub const DEFAULT_MAX_BAG: u8 = 2;
/// The warehouse's bag number in the packets, its slots, and the first slot that needs the Double Warehouse buff
/// (`docs/original/warehouse.md`).
pub const BANK: u8 = 100;
pub const BANK_SLOTS: usize = 240;
pub const BANK_SECOND_HALF: u8 = 120;
/// Items taken out of the warehouse for nothing: item type 94, the guild bars (`CUser::ItemBankToBag`).
const FREE_WITHDRAWAL_TYPE: u8 = 94;
/// The make type of a bought item.
const BOUGHT_MAKE_TYPE: u8 = b'B';

/// What the server needs of an item definition (`defs.items`, the original's `stItemInfo`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemDef {
    pub item_id: u32,
    pub item_type: u8,
    pub type_id: u8,
    /// Most items in one slot (`byCount`); below 2 the item does not stack.
    pub max_count: u8,
    /// A new item's quality (`wQuality[0]`).
    pub quality: u16,
    pub buy: u32,
    pub sell: u32,
    /// Who may equip it (`docs/original/items.md` section 3): level, country (race rule), classes, mode, `bySpecial`.
    pub level: u16,
    pub country: u8,
    pub jobs: [bool; 6],
    pub grow: u8,
    pub special: u8,
    /// What equipping it adds (`docs/original/stats.md`): STR, DEX, REC, INT, WIS, LUC; HP, MP, SP; attack speed; move speed;
    /// whether its craft name holds crafted stats.
    pub const_stats: [u16; 6],
    pub const_health: [u16; 3],
    pub attack_time: u8,
    pub speed: u8,
    pub craftable: bool,
    /// Effects 1 to 4 at full quality (attack, attack spread, defence, magic defence; `docs/original/combat.md` section 4).
    pub effects: [u16; 4],
    pub range: u16,
    /// The element (`byAttrib`).
    pub attrib: u8,
    /// The drop grade (`wGrade`) and how often the item may drop server-wide (`wDrop`, 0 = no limit).
    pub grade: u16,
    pub drop_limit: u16,
    /// The `server` column (`byServer`).
    pub server: u8,
    /// Not tradeable: the `reqog` column (`byOG`) is not 0 (EP6 names its values 1 account bound, 2 character bound). The original
    /// refuses such items at the market and keeps them on a death drop, but not in trade (`docs/original/trade.md` section 2.3).
    pub bound: bool,
    /// The reuse group of a usable item (`byIG`, the `reqig` column; `docs/original/item-use.md` section 2).
    pub reuse_group: u8,
    /// The `reqvg` column (`shVG`): the honor an Increase Fame item adds; a box's set in `ItemCreate.ini`.
    pub vg: i16,
    /// The `slot` column (`bySlot`): an item's lapis room, or the room a lapis takes (`docs/original/lapis.md` section 1).
    pub room: u8,
    /// The `reqluc` column (`wLuc`): the `itemid` of an enchant's material.
    pub material: u16,
    /// The `reqwis` column: the highest crafted number.
    pub craft_max: u16,
    /// The `reqstr` column: for a lapis, whether it fits a bracelet.
    pub req_str: u16,
    /// The `reqdex` column (`wDex`): a union led for loot hands the leader only items where it is not 0 (`docs/original/union.md`
    /// section 7).
    pub req_dex: u16,
    /// The `exp` column (`byExp`): a lapis' absorption.
    pub absorption: u8,
}

impl ItemDef {
    /// Real type 27 (item types 27 to 29 in `LITEM_TYPE`): the original pays nothing for them.
    fn sells_for_nothing(&self) -> bool {
        real_type(self.item_type) == 27
    }
}

/// The real type of an item type: the number of the `LITEM_TYPE` table (ps_game.exe 0x542188) that lists it, 0 when none does
/// (`GetItemRType`). 1 to 15 are weapons, 16 to 25 armour and accessories.
pub fn real_type(item_type: u8) -> u8 {
    const TABLES: [&[u8]; 27] = [
        &[1, 45],
        &[2, 46],
        &[3, 47],
        &[4, 48],
        &[5, 49, 50],
        &[6, 51, 52],
        &[7, 53, 54],
        &[8, 55, 56],
        &[9, 57],
        &[10, 58],
        &[11, 59],
        &[12, 60, 61],
        &[13, 62, 63],
        &[14, 64],
        &[15, 65],
        &[16, 31, 66, 72, 81, 87],
        &[17, 32, 67, 73, 82, 88],
        &[18, 33, 68, 74, 83, 89],
        &[19, 34, 69, 75, 84, 90],
        &[20, 35, 70, 76, 85, 91],
        &[21, 36, 71, 77, 86, 92],
        &[22, 37],
        &[23, 38],
        &[24, 39],
        &[40, 41],
        &[25, 44, 100],
        &[27, 28, 29],
    ];
    TABLES.iter().position(|table| table.contains(&item_type)).map_or(0, |i| i as u8 + 1)
}

/// What equipping depends on about the character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wearer {
    pub level: u16,
    /// 0 human, 1 elf, 2 vail, 3 deatheater.
    pub family: u8,
    pub job: u8,
    /// 0 easy, 1 normal, 2 hard, 3 ultimate.
    pub grow: u8,
    pub sex: u8,
}

/// Equipment slots items can be equipped in (bag 0, 0 to 13); 0 to 7 show to other players.
pub const EQUIPMENT_SLOTS: u8 = 14;
pub const VISIBLE_SLOTS: u8 = 8;
const WEAPON_SLOT: u8 = 5;
const SHIELD_SLOT: u8 = 6;
/// Item type of mounts (slot 13).
const MOUNT_TYPE: u8 = 42;

/// Whether an item's country (`byCountry`) allows a race (`m_byFamily`: 0 human, 1 elf, 2 vail, 3 deatheater): 0 human, 1 elf,
/// 2 human or elf, 3 deatheater, 4 vail, 5 vail or deatheater, anything else everyone (`CUser::EnableEquipment`, `CUser::ItemUse`).
pub fn race_fits(country: u8, family: u8) -> bool {
    match country {
        0 => family == 0,
        1 => family == 1,
        2 => family <= 1,
        3 => family == 3,
        4 => family == 2,
        5 => matches!(family, 2 | 3),
        _ => true,
    }
}

/// Weapons that can be used with a shield (`CUser::EnableEquipment` → 0x45b1a0).
fn one_handed(def: &ItemDef) -> bool {
    matches!(real_type(def.item_type), 1 | 3 | 7 | 9)
}

#[derive(Debug, Default)]
pub struct ItemDefs(HashMap<(u8, u8), ItemDef>);

impl ItemDefs {
    pub fn new(defs: impl IntoIterator<Item = ItemDef>) -> Self {
        Self(defs.into_iter().map(|d| ((d.item_type, d.type_id), d)).collect())
    }

    /// `CGameData::GetItemInfo`: type and type id start at 1.
    pub fn get(&self, item_type: u8, type_id: u8) -> Option<&ItemDef> {
        if item_type == 0 || type_id == 0 {
            return None;
        }
        self.0.get(&(item_type, type_id))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemDef> {
        self.0.values()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One slot's change, saved together with the money.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotChange {
    Insert(NewItem),
    SetCount {
        bag: u8,
        slot: u8,
        count: u8,
    },
    /// Wear or a repair (`docs/original/repair.md`).
    SetQuality {
        bag: u8,
        slot: u8,
        quality: u16,
    },
    /// Linked or extracted lapis, or a cloak's badge (`docs/original/lapis.md`).
    SetGems {
        bag: u8,
        slot: u8,
        gems: [u8; 6],
    },
    /// New crafted numbers or an enchant step, padded with zeros.
    SetCraftName {
        bag: u8,
        slot: u8,
        craft_name: [u8; CRAFT_NAME_MAX],
    },
    Remove {
        bag: u8,
        slot: u8,
    },
    /// To an empty slot.
    Move {
        from: (u8, u8),
        to: (u8, u8),
    },
    /// Two taken slots exchange their items.
    Swap {
        a: (u8, u8),
        b: (u8, u8),
    },
}

/// Where a picked-up item goes (`CUser::ItemGet`, `docs/original/combat.md` section 8).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PickUp {
    /// The changes in order: stacks topped up, then the rest into an empty slot.
    pub changes: Vec<SlotChange>,
    /// What did not fit.
    pub left: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Buy {
    /// Pay `price`, make `change`; the slot then holds `count`.
    Done {
        price: u32,
        change: SlotChange,
        bag: u8,
        slot: u8,
        count: u8,
    },
    NoMoney,
    NoRoom,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Sale {
    pub gain: u32,
    pub change: SlotChange,
    pub item_type: u8,
    pub type_id: u8,
    /// What stays in the slot; 0 when sold out.
    pub left: u8,
}

/// A drop decided (`CUser::ItemDrop`, `docs/original/item-drop.md` section 2): `count` of the slot's `had` goes on the ground with
/// `quality` (0 for part of a stack); `change` is what the slot gets.
#[derive(Debug, PartialEq, Eq)]
pub struct Dropping {
    pub item_type: u8,
    pub type_id: u8,
    pub had: u8,
    pub count: u8,
    pub quality: u16,
    pub change: SlotChange,
}

/// A warehouse move decided (`docs/original/warehouse.md` sections 3 to 6): the fee to pay and the changes to save.
#[derive(Debug, PartialEq, Eq)]
pub struct BankMove {
    pub fee: u32,
    pub changes: Vec<SlotChange>,
}

/// The fee for taking `count` of `def` out of the warehouse (`CUser::ItemBankToBag`): nothing for the guild bars, otherwise the
/// 32-bit `count × nSell` made a `float`, times `0.05f`, truncated. A product of two `float`s is exact in `f64`, as in the x87.
pub fn withdrawal_fee(def: &ItemDef, count: u8) -> u32 {
    if def.item_type == FREE_WITHDRAWAL_TYPE {
        return 0;
    }
    let total = u32::from(count).wrapping_mul(def.sell) as f32;
    (f64::from(total) * f64::from(0.05f32)) as u32
}

#[derive(Clone)]
pub struct Inventory {
    slots: [[Option<ItemUnit>; SLOTS]; BAGS],
    /// The account's warehouse, bag 100.
    bank: Vec<Option<ItemUnit>>,
    max_bag: u8,
}

impl Inventory {
    pub fn new(items: &[ItemUnit]) -> Self {
        let mut inventory =
            Self { slots: std::array::from_fn(|_| std::array::from_fn(|_| None)), bank: vec![None; BANK_SLOTS], max_bag: DEFAULT_MAX_BAG };
        for item in items.iter().filter(|i| usize::from(i.bag) < BAGS && usize::from(i.slot) < SLOTS) {
            inventory.slots[usize::from(item.bag)][usize::from(item.slot)] = Some(item.clone());
        }
        inventory
    }

    /// With the account's warehouse (items of bag 100).
    pub fn with_bank(mut self, bank: &[ItemUnit]) -> Self {
        for item in bank.iter().filter(|i| i.bag == BANK && usize::from(i.slot) < BANK_SLOTS) {
            self.bank[usize::from(item.slot)] = Some(item.clone());
        }
        self
    }

    pub fn bank_get(&self, slot: u8) -> Option<&ItemUnit> {
        self.bank.get(usize::from(slot))?.as_ref()
    }

    /// The warehouse's items, by slot.
    pub fn bank_items(&self) -> impl Iterator<Item = &ItemUnit> {
        self.bank.iter().flatten()
    }

    /// A bag or warehouse place's item.
    pub fn at(&self, (bag, slot): (u8, u8)) -> Option<&ItemUnit> {
        if bag == BANK { self.bank_get(slot) } else { self.get(bag, slot) }
    }

    /// Whether a place is in the warehouse (`CUser::IsBank`): bag 100, slot below 240.
    pub fn is_bank((bag, slot): (u8, u8)) -> bool {
        bag == BANK && usize::from(slot) < BANK_SLOTS
    }

    /// A move into, out of or inside the warehouse (`CUser::PacketMaininterface`, `ItemBagToBank`, `ItemBankToBag`,
    /// `ItemBankToBank`). `None` when the original does nothing (no answer) or when it is not a warehouse move. `money` pays the
    /// fee; `second_half` is the Double Warehouse buff, needed for target slots from 120 on.
    pub fn bank_move(&self, defs: &ItemDefs, money: u32, second_half: bool, from: (u8, u8), to: (u8, u8)) -> Option<BankMove> {
        let (from_bank, to_bank) = (Self::is_bank(from), Self::is_bank(to));
        if from == to || !(from_bank || to_bank) || (to_bank && to.1 >= BANK_SECOND_HALF && !second_half) {
            return None;
        }
        let moving = self.at(from)?;
        let done = |changes| Some(BankMove { fee: 0, changes });
        if from_bank && to_bank {
            if self.at(to).is_none() {
                return done(vec![SlotChange::Move { from, to }]);
            }
            return done(self.combine(defs, from, to).unwrap_or_else(|| vec![SlotChange::Swap { a: from, b: to }]));
        }
        if from_bank {
            // Into an empty bag slot only, never straight into the equipment.
            if !self.is_bag(to) || to.0 == 0 || self.at(to).is_some() {
                return None;
            }
            let fee = withdrawal_fee(defs.get(moving.item_type, moving.type_id)?, moving.count);
            if money < fee {
                return None;
            }
            return Some(BankMove { fee, changes: vec![SlotChange::Move { from, to }] });
        }
        // Into an empty warehouse slot, not from the equipment, and not real type 27 (quest items and the like).
        if !self.is_bag(from) || from.0 == 0 || self.at(to).is_some() {
            return None;
        }
        if defs.get(moving.item_type, moving.type_id).is_none_or(|def| real_type(def.item_type) == 27) {
            return None;
        }
        done(vec![SlotChange::Move { from, to }])
    }

    /// Two places holding the same stacking item (`CItem::EnableCombine`: both stack limits above 1, same type and type id):
    /// everything into `to` when it fits the stack limit (`from` emptied), otherwise `to` full and `from` the rest. `None` when
    /// they do not combine.
    fn combine(&self, defs: &ItemDefs, from: (u8, u8), to: (u8, u8)) -> Option<Vec<SlotChange>> {
        let (moving, other) = (self.at(from)?, self.at(to)?);
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

    /// The usable bags (2 by default; a passive skill raises it, `docs/original/skills.md` section 5), at most the five there are.
    pub fn set_max_bag(&mut self, bags: u8) {
        self.max_bag = bags.clamp(1, BAGS as u8 - 1);
    }

    pub fn get(&self, bag: u8, slot: u8) -> Option<&ItemUnit> {
        self.slots.get(usize::from(bag))?.get(usize::from(slot))?.as_ref()
    }

    /// The usable inventory slots in the original's search order: bag by bag from 1, slot by slot.
    fn inventory_slots(&self) -> impl Iterator<Item = (u8, u8)> {
        (1..=self.max_bag).flat_map(|bag| (0..SLOTS as u8).map(move |slot| (bag, slot)))
    }

    /// Buying `count` of `def` with `money` (section 2): the money first, then a stack with room, then the first empty slot.
    /// The caller has checked that `count` is 1 to `max_count`.
    pub fn buy(&self, money: u32, def: &ItemDef, count: u8) -> Buy {
        // 64 bits: the original multiplies in 32 and could wrap to a small price.
        let price = u64::from(def.buy) * u64::from(count);
        let Ok(price) = u32::try_from(price) else { return Buy::NoMoney };
        if price > money {
            return Buy::NoMoney;
        }
        if def.max_count >= 2 {
            let stack = self.inventory_slots().find_map(|(bag, slot)| {
                let item = self.get(bag, slot)?;
                let total = u16::from(item.count) + u16::from(count);
                (item.item_type == def.item_type && item.type_id == def.type_id && total <= u16::from(def.max_count)).then_some((
                    bag,
                    slot,
                    total as u8,
                ))
            });
            if let Some((bag, slot, count)) = stack {
                return Buy::Done { price, change: SlotChange::SetCount { bag, slot, count }, bag, slot, count };
            }
        }
        let Some((bag, slot)) = self.inventory_slots().find(|&(bag, slot)| self.get(bag, slot).is_none()) else { return Buy::NoRoom };
        let count = if def.max_count < 2 { 1 } else { count };
        // Make type 'B' and, for a craftable item, twenty zeros (`docs/original/item-origins.md` sections 1 and 2).
        let kept = Kept { make_type: BOUGHT_MAKE_TYPE, craft_name: crate::lapis::zero_craft_name(def), ..Kept::default() };
        let item = NewItem {
            bag,
            slot,
            item_id: def.item_id,
            item_type: def.item_type,
            type_id: def.type_id,
            quality: def.quality,
            count,
            kept: Some(kept),
        };
        Buy::Done { price, change: SlotChange::Insert(item), bag, slot, count }
    }

    /// Picking up `count` of `def` with `quality` (`CUser::ItemGet`): stackable items first onto stacks of the same item with room,
    /// in bag and slot order, up to `max_count`; the rest into the first empty slot of the usable bags.
    pub fn pick_up(&self, def: &ItemDef, count: u8, quality: u16, kept: Option<Kept>) -> PickUp {
        let mut pick = PickUp { changes: Vec::new(), left: count };
        if def.max_count > 1 {
            for (bag, slot) in self.inventory_slots() {
                if pick.left == 0 {
                    break;
                }
                let Some(item) = self.get(bag, slot) else { continue };
                if item.item_type != def.item_type || item.type_id != def.type_id || item.count >= def.max_count {
                    continue;
                }
                let added = pick.left.min(def.max_count - item.count);
                pick.left -= added;
                pick.changes.push(SlotChange::SetCount { bag, slot, count: item.count + added });
            }
        }
        if pick.left > 0
            && let Some((bag, slot)) = self.inventory_slots().find(|&(bag, slot)| self.get(bag, slot).is_none())
        {
            let item = NewItem {
                bag,
                slot,
                item_id: def.item_id,
                item_type: def.item_type,
                type_id: def.type_id,
                quality,
                count: pick.left,
                kept,
            };
            pick.changes.push(SlotChange::Insert(item));
            pick.left = 0;
        }
        pick
    }

    /// Dropping `count` from a bag slot (`docs/original/item-drop.md` sections 1 and 2); `None` when the original ignores it: the
    /// equipment or not a bag, `count` 0, an empty slot, an unknown item, real type 27, or `count` above what the slot holds. An
    /// unstackable item goes whole whatever `count` says.
    pub fn drop_from(&self, defs: &ItemDefs, bag: u8, slot: u8, count: u8) -> Option<Dropping> {
        if bag == 0 || count == 0 || !self.is_bag((bag, slot)) {
            return None;
        }
        let item = self.get(bag, slot)?;
        let def = defs.get(item.item_type, item.type_id)?;
        if real_type(def.item_type) == 27 {
            return None;
        }
        let count = if def.max_count == 1 && item.count == 1 { 1 } else { count };
        if count > item.count {
            return None;
        }
        let whole = count == item.count;
        Some(Dropping {
            item_type: item.item_type,
            type_id: item.type_id,
            had: item.count,
            count,
            quality: if whole { item.quality } else { 0 },
            change: if whole { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: item.count - count } },
        })
    }

    /// Selling `count` from a slot with `money` (section 3); `None` when the original ignores the request.
    pub fn sell(&self, money: u32, defs: &ItemDefs, bag: u8, slot: u8, count: u8) -> Option<Sale> {
        if bag == 0 || bag > self.max_bag {
            return None;
        }
        let item = self.get(bag, slot)?;
        if count == 0 || count > item.count {
            return None;
        }
        let def = defs.get(item.item_type, item.type_id)?;
        let gain = if def.sells_for_nothing() { 0 } else { u64::from(def.sell) * u64::from(count) };
        // Past the u32 maximum the original does nothing.
        let gain = u32::try_from(gain).ok().filter(|gain| money.checked_add(*gain).is_some())?;
        let left = item.count - count;
        let change = if left == 0 { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: left } };
        Some(Sale { gain, change, item_type: item.item_type, type_id: item.type_id, left })
    }

    /// The items in the usable bags, in the original's search order (bag by bag from 1, slot by slot).
    pub fn usable_items(&self) -> impl Iterator<Item = &ItemUnit> {
        self.inventory_slots().filter_map(|(bag, slot)| self.get(bag, slot))
    }

    /// The empty usable inventory slots, in the original's search order (`CUser::ExchangeItemAdd`).
    pub fn free_slots(&self) -> Vec<(u8, u8)> {
        self.inventory_slots().filter(|&(bag, slot)| self.get(bag, slot).is_none()).collect()
    }

    /// The bag count in use (`+0x1304`).
    pub fn max_bag(&self) -> u8 {
        self.max_bag
    }

    /// Puts a saved item, whole, into its (empty) slot: an item from a trade.
    pub fn receive(&mut self, item: ItemUnit) {
        let (bag, slot) = (usize::from(item.bag), usize::from(item.slot));
        self.slots[bag][slot] = Some(item);
    }

    pub fn items(&self) -> impl Iterator<Item = &ItemUnit> {
        self.slots.iter().flatten().flatten()
    }

    /// Whether a place is a usable bag slot (`CUser::IsBag`): bag 0 (the equipment) to the last usable bag, slot below 24.
    pub fn is_bag(&self, (bag, slot): (u8, u8)) -> bool {
        bag <= self.max_bag && usize::from(slot) < SLOTS
    }

    /// Whether `def` may be equipped in `slot` by `wearer`, with what is equipped now (`CUser::EnableEquipment`).
    pub fn can_equip(&self, defs: &ItemDefs, def: &ItemDef, slot: u8, wearer: &Wearer) -> bool {
        let equipped = |slot| self.get(0, slot).and_then(|item| defs.get(item.item_type, item.type_id));
        if def.level > wearer.level {
            return false;
        }
        let real = real_type(def.item_type);
        let fits = match slot {
            0 => real == 16,
            1 => real == 17,
            2 => real == 18,
            3 => real == 20,
            4 => real == 21,
            // With a shield, only a one-handed weapon.
            WEAPON_SLOT => (1..=15).contains(&real) && (self.get(0, SHIELD_SLOT).is_none() || one_handed(def)),
            SHIELD_SLOT => real == 19 && equipped(WEAPON_SLOT).is_none_or(one_handed),
            7 => real == 24,
            8 => real == 23,
            9 | 10 => real == 22,
            11 | 12 => real == 25,
            13 => def.item_type == MOUNT_TYPE,
            _ => false,
        };
        let race = race_fits(def.country, wearer.family);
        let job = def.jobs.get(usize::from(wearer.job)).copied().unwrap_or(false);
        let grow = match wearer.grow {
            0 => def.grow == 0,
            1 => def.grow <= 1,
            2 => def.grow != 3,
            _ => true,
        };
        let sex = match def.special {
            64 => wearer.sex == 0,
            65 => wearer.sex == 1,
            _ => true,
        };
        fits && race && job && grow && sex
    }

    /// Moving an item between bag slots (`CUser::ItemBagToBag`, `docs/original/items.md` section 2). `None` when the original
    /// does not answer; otherwise the changes to save (empty when nothing moves; the original answers anyway).
    pub fn move_item(&self, defs: &ItemDefs, wearer: &Wearer, from: (u8, u8), to: (u8, u8)) -> Option<Vec<SlotChange>> {
        if from == to || !self.is_bag(from) || !self.is_bag(to) {
            return None;
        }
        let moving = self.get(from.0, from.1)?;
        let target = self.get(to.0, to.1);
        let def_of = |item: &ItemUnit| defs.get(item.item_type, item.type_id);
        let place = |target: Option<&ItemUnit>| match target {
            None => SlotChange::Move { from, to },
            Some(_) => SlotChange::Swap { a: from, b: to },
        };
        if from.0 == 0 {
            // Unequipping, into the inventory only.
            if from.1 >= EQUIPMENT_SLOTS || to.0 == 0 {
                return None;
            }
            return Some(match target {
                None => vec![place(None)],
                Some(other) if def_of(other).is_some_and(|def| self.can_equip(defs, def, from.1, wearer)) => vec![place(target)],
                Some(_) => Vec::new(),
            });
        }
        if to.0 == 0 {
            if to.1 >= EQUIPMENT_SLOTS {
                return None;
            }
            let allowed = def_of(moving).is_some_and(|def| self.can_equip(defs, def, to.1, wearer));
            return Some(if allowed { vec![place(target)] } else { Vec::new() });
        }
        // Inventory to inventory: combine stacks of the same item (`CItem::EnableCombine`), otherwise move or swap.
        if let Some(changes) = self.combine(defs, from, to) {
            return Some(changes);
        }
        Some(vec![place(target)])
    }

    /// Makes a saved change.
    pub fn apply(&mut self, change: &SlotChange) {
        match change {
            SlotChange::Insert(n) => *self.cell((n.bag, n.slot)) = Some(n.unit()),
            SlotChange::SetCount { bag, slot, count } => {
                if let Some(item) = self.cell((*bag, *slot)) {
                    item.count = *count;
                }
            }
            SlotChange::SetQuality { bag, slot, quality } => {
                if let Some(item) = self.cell((*bag, *slot)) {
                    item.quality = *quality;
                }
            }
            SlotChange::SetGems { bag, slot, gems } => {
                if let Some(item) = self.cell((*bag, *slot)) {
                    item.gems = *gems;
                }
            }
            SlotChange::SetCraftName { bag, slot, craft_name } => {
                if let Some(item) = self.cell((*bag, *slot)) {
                    item.craft_name = craft_name.iter().copied().take_while(|&b| b != 0).collect();
                }
            }
            SlotChange::Remove { bag, slot } => *self.cell((*bag, *slot)) = None,
            SlotChange::Move { from, to } => {
                let item = self.take(*from);
                self.put(*to, item);
            }
            SlotChange::Swap { a, b } => {
                let (first, second) = (self.take(*a), self.take(*b));
                self.put(*a, second);
                self.put(*b, first);
            }
        }
    }

    /// A bag or warehouse slot.
    fn cell(&mut self, (bag, slot): (u8, u8)) -> &mut Option<ItemUnit> {
        if bag == BANK { &mut self.bank[usize::from(slot)] } else { &mut self.slots[usize::from(bag)][usize::from(slot)] }
    }

    pub(crate) fn take(&mut self, place: (u8, u8)) -> Option<ItemUnit> {
        self.cell(place).take()
    }

    pub(crate) fn put(&mut self, (bag, slot): (u8, u8), item: Option<ItemUnit>) {
        *self.cell((bag, slot)) = item.map(|item| ItemUnit { bag, slot, ..item });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(item_type: u8, max_count: u8, buy: u32, sell: u32) -> ItemDef {
        ItemDef { item_id: u32::from(item_type) * 1000 + 1, item_type, type_id: 1, max_count, buy, sell, ..Default::default() }
    }

    fn item(bag: u8, slot: u8, item_type: u8, count: u8) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id: 1, count, ..Default::default() }
    }

    #[test]
    fn buying_checks_money_then_stacks_then_takes_the_first_empty_slot() {
        let potion = def(25, 100, 10, 2);
        // The equipment's sword in bag 0 is never used; bag 1 slot 0 holds 95 potions, slot 1 nine more.
        let inv = Inventory::new(&[item(0, 5, 5, 1), item(1, 0, 25, 95), item(1, 1, 25, 9)]);
        assert_eq!(inv.buy(99, &potion, 10), Buy::NoMoney);
        // 10 do not fit on 95, so they go on the 9.
        assert_eq!(
            inv.buy(100, &potion, 10),
            Buy::Done { price: 100, change: SlotChange::SetCount { bag: 1, slot: 1, count: 19 }, bag: 1, slot: 1, count: 19 }
        );
        assert_eq!(inv.buy(1000, &potion, 92).change_slot(), Some((1, 2)), "a new stack in the first empty slot");

        let sword = def(5, 1, 500, 100);
        assert_eq!(inv.buy(1000, &sword, 1).change_slot(), Some((1, 2)));
        // A new item is made 'B'; a craftable one gets twenty zeros.
        let crafted = ItemDef { craftable: true, ..sword };
        let Buy::Done { change: SlotChange::Insert(new), .. } = inv.buy(1000, &crafted, 1) else { panic!("bought") };
        assert_eq!(new.kept.map(|k| (k.make_type, k.craft_name)), Some((b'B', *b"00000000000000000000")));
        let Buy::Done { change: SlotChange::Insert(new), .. } = inv.buy(1000, &sword, 1) else { panic!("bought") };
        assert_eq!(new.kept.map(|k| (k.make_type, k.craft_name)), Some((b'B', [0; CRAFT_NAME_MAX])));
        // A price that does not fit 32 bits is not a small price.
        assert_eq!(inv.buy(u32::MAX, &def(25, 255, u32::MAX, 0), 2), Buy::NoMoney);
    }

    #[test]
    fn buying_needs_room_in_the_usable_bags() {
        let full: Vec<_> = (1..=2).flat_map(|bag| (0..24).map(move |slot| item(bag, slot, 5, 1))).collect();
        let inv = Inventory::new(&full);
        assert_eq!(inv.buy(1000, &def(5, 1, 1, 0), 1), Buy::NoRoom, "bags 3 to 5 are not usable yet");
    }

    #[test]
    fn selling_pays_and_empties_the_slot() {
        let defs = ItemDefs::new([def(25, 100, 10, 3), def(27, 100, 10, 3)]);
        let mut inv = Inventory::new(&[item(1, 0, 25, 10), item(1, 1, 27, 1), item(0, 5, 25, 1), item(3, 0, 25, 1)]);
        let sale = inv.sell(0, &defs, 1, 0, 4).unwrap();
        assert_eq!((sale.gain, sale.left, &sale.change), (12, 6, &SlotChange::SetCount { bag: 1, slot: 0, count: 6 }));
        inv.apply(&sale.change);
        let sale = inv.sell(0, &defs, 1, 0, 6).unwrap();
        assert_eq!((sale.gain, sale.left), (18, 0));
        inv.apply(&sale.change);
        assert!(inv.get(1, 0).is_none());

        assert_eq!(inv.sell(0, &defs, 1, 1, 1).unwrap().gain, 0, "real type 27 sells for nothing");
        assert!(inv.sell(0, &defs, 1, 0, 1).is_none(), "empty slot");
        assert!(inv.sell(0, &defs, 0, 5, 1).is_none(), "not the equipment");
        assert!(inv.sell(0, &defs, 3, 0, 1).is_none(), "not above the usable bags");
        assert!(inv.sell(0, &defs, 1, 1, 2).is_none(), "more than there is");
        assert!(inv.sell(0, &defs, 1, 1, 0).is_none());
        let mut rich = Inventory::new(&[item(1, 0, 25, 10)]);
        assert!(rich.sell(u32::MAX - 2, &defs, 1, 0, 1).is_none(), "the money would overflow");
        rich.apply(&SlotChange::Remove { bag: 1, slot: 0 });
        assert!(rich.get(1, 0).is_none());
    }

    #[test]
    fn equipping_follows_the_original_rules() {
        let wear =
            |item_type: u8| ItemDef { item_type, type_id: 1, max_count: 1, level: 10, country: 6, jobs: [true; 6], ..Default::default() };
        // Real types: 16 helmet, 1 (one-handed) and 2 (two-handed) weapons, 19 shield.
        let (helmet, one_hand, two_hand, shield) = (wear(16), wear(1), wear(2), wear(19));
        let defs = ItemDefs::new([helmet, one_hand, two_hand, shield]);
        let elf = Wearer { level: 10, family: 1, job: 0, grow: 1, sex: 0 };
        let empty = Inventory::new(&[]);
        assert!(empty.can_equip(&defs, &helmet, 0, &elf));
        assert!(!empty.can_equip(&defs, &helmet, 1, &elf), "a helmet only fits slot 0");
        assert!(!empty.can_equip(&defs, &helmet, 0, &Wearer { level: 9, ..elf }), "level");
        assert!(!empty.can_equip(&defs, &ItemDef { country: 0, ..helmet }, 0, &elf), "human only");
        assert!(empty.can_equip(&defs, &ItemDef { country: 2, ..helmet }, 0, &elf), "Light");
        assert!(!empty.can_equip(&defs, &ItemDef { country: 5, ..helmet }, 0, &elf), "Fury");
        assert!(!empty.can_equip(&defs, &ItemDef { jobs: [false, true, true, true, true, true], ..helmet }, 0, &elf), "class");
        assert!(!empty.can_equip(&defs, &ItemDef { grow: 2, ..helmet }, 0, &elf), "hard items need hard mode");
        assert!(empty.can_equip(&defs, &ItemDef { grow: 2, ..helmet }, 0, &Wearer { grow: 2, ..elf }));
        assert!(!empty.can_equip(&defs, &ItemDef { special: 65, ..helmet }, 0, &elf), "sex");

        // With a shield, only a one-handed weapon; a shield only with a one-handed weapon.
        let item = |slot, item_type| ItemUnit { bag: 0, slot, item_type, type_id: 1, count: 1, ..Default::default() };
        let with_shield = Inventory::new(&[item(6, 19)]);
        assert!(with_shield.can_equip(&defs, &one_hand, 5, &elf));
        assert!(!with_shield.can_equip(&defs, &two_hand, 5, &elf));
        assert!(!Inventory::new(&[item(5, 2)]).can_equip(&defs, &shield, 6, &elf));
        assert!(Inventory::new(&[item(5, 1)]).can_equip(&defs, &shield, 6, &elf));
    }

    #[test]
    fn moving_items_swaps_combines_and_equips() {
        let potion = def(25, 100, 0, 0);
        let helmet = ItemDef { item_type: 16, type_id: 1, max_count: 1, country: 6, jobs: [true; 6], ..Default::default() };
        let defs = ItemDefs::new([potion, helmet]);
        let wearer = Wearer::default();
        let inv = Inventory::new(&[item(1, 0, 25, 60), item(1, 1, 25, 60), item(1, 2, 16, 1), item(0, 0, 16, 1)]);
        assert_eq!(
            inv.move_item(&defs, &wearer, (1, 0), (1, 1)),
            Some(vec![SlotChange::SetCount { bag: 1, slot: 1, count: 100 }, SlotChange::SetCount { bag: 1, slot: 0, count: 20 }]),
            "combined up to the stack limit"
        );
        assert_eq!(inv.move_item(&defs, &wearer, (1, 0), (1, 5)), Some(vec![SlotChange::Move { from: (1, 0), to: (1, 5) }]));
        assert_eq!(inv.move_item(&defs, &wearer, (1, 0), (1, 2)), Some(vec![SlotChange::Swap { a: (1, 0), b: (1, 2) }]));
        // Equipping the helmet swaps with the one worn; a potion cannot be equipped (answered, nothing changes).
        assert_eq!(inv.move_item(&defs, &wearer, (1, 2), (0, 0)), Some(vec![SlotChange::Swap { a: (1, 2), b: (0, 0) }]));
        assert_eq!(inv.move_item(&defs, &wearer, (1, 0), (0, 1)), Some(Vec::new()));
        // Ignored: an empty source, the same place, bag 3 (not usable yet), an equipment slot above 13, equipment to equipment.
        for (from, to) in [((1, 9), (1, 10)), ((1, 0), (1, 0)), ((1, 0), (3, 0)), ((1, 0), (0, 14)), ((0, 0), (0, 1))] {
            assert_eq!(inv.move_item(&defs, &wearer, from, to), None, "{from:?} to {to:?}");
        }
        let mut moved = Inventory::new(&[item(1, 0, 25, 60), item(1, 2, 16, 1)]);
        moved.apply(&SlotChange::Swap { a: (1, 0), b: (1, 2) });
        assert_eq!(moved.get(1, 0).map(|i| (i.item_type, i.slot)), Some((16, 0)), "the item knows its new place");
        assert_eq!(moved.get(1, 2).map(|i| (i.item_type, i.slot)), Some((25, 2)));
    }

    impl Buy {
        fn change_slot(&self) -> Option<(u8, u8)> {
            match self {
                Buy::Done { change: SlotChange::Insert(n), .. } => Some((n.bag, n.slot)),
                _ => None,
            }
        }
    }

    #[test]
    fn picked_up_items_stack_then_take_an_empty_slot() {
        let potion = def(25, 100, 10, 2);
        let full = || Inventory::new(&[item(1, 0, 25, 99), item(1, 1, 25, 100), item(1, 2, 25, 50)]);
        // One onto the first stack with room.
        assert_eq!(
            full().pick_up(&potion, 1, 0, None),
            PickUp { changes: vec![SlotChange::SetCount { bag: 1, slot: 0, count: 100 }], left: 0 }
        );
        // 60: 1 tops up slot 0, 50 slot 2, 9 go to the first empty slot.
        let pick = full().pick_up(&potion, 60, 0, None);
        assert_eq!(
            pick.changes[..2],
            [SlotChange::SetCount { bag: 1, slot: 0, count: 100 }, SlotChange::SetCount { bag: 1, slot: 2, count: 100 }]
        );
        assert!(matches!(pick.changes[2], SlotChange::Insert(NewItem { bag: 1, slot: 3, count: 9, .. })));
        // A sword does not stack; with every slot taken nothing fits.
        let sword = def(5, 1, 500, 100);
        assert!(matches!(full().pick_up(&sword, 1, 1200, None).changes[..], [SlotChange::Insert(NewItem { slot: 3, quality: 1200, .. })]));
        let taken: Vec<_> = (0..2).flat_map(|b| (0..24).map(move |s| item(b + 1, s, 5, 1))).collect();
        assert_eq!(Inventory::new(&taken).pick_up(&sword, 1, 1200, None), PickUp { changes: vec![], left: 1 });
    }

    #[test]
    fn a_kept_item_comes_back_with_its_gems_and_craft_name() {
        let sword = def(5, 1, 500, 100);
        let kept = Kept { gems: [1, 2, 0, 0, 0, 0], craft_name: Kept::pad_craft_name(b"0102"), ..Default::default() };
        let mut inv = Inventory::new(&[]);
        let pick = inv.pick_up(&sword, 1, 900, Some(kept));
        inv.apply(&pick.changes[0]);
        let back = inv.get(1, 0).unwrap();
        assert_eq!((back.quality, back.gems, back.craft_name.as_slice()), (900, [1, 2, 0, 0, 0, 0], &b"0102"[..]));
    }

    #[test]
    fn dropping_takes_all_or_part_and_ignores_what_the_original_ignores() {
        let defs = ItemDefs::new([def(5, 1, 500, 100), def(25, 100, 10, 2), def(27, 100, 10, 3)]);
        let sword = ItemUnit { quality: 1100, ..item(1, 0, 5, 1) };
        let inv =
            Inventory::new(&[item(0, 5, 5, 1), sword, ItemUnit { quality: 7, ..item(1, 1, 25, 10) }, item(1, 2, 27, 5), item(3, 0, 25, 1)]);
        // An unstackable item goes whole, whatever the count.
        let whole = inv.drop_from(&defs, 1, 0, 3).unwrap();
        assert_eq!((whole.had, whole.count, whole.quality, whole.change), (1, 1, 1100, SlotChange::Remove { bag: 1, slot: 0 }));
        // Part of a stack: the rest stays, the part on the ground has quality 0; all of it empties the slot.
        let part = inv.drop_from(&defs, 1, 1, 4).unwrap();
        assert_eq!((part.had, part.count, part.quality, part.change), (10, 4, 0, SlotChange::SetCount { bag: 1, slot: 1, count: 6 }));
        let all = inv.drop_from(&defs, 1, 1, 10).unwrap();
        assert_eq!((all.count, all.quality, all.change), (10, 7, SlotChange::Remove { bag: 1, slot: 1 }));
        // Ignored: more than the slot holds, count 0, the equipment, an empty slot, real type 27, a bag past the bag count.
        for (bag, slot, count) in [(1, 1, 11), (1, 1, 0), (0, 5, 1), (1, 9, 1), (1, 2, 1), (3, 0, 1)] {
            assert_eq!(inv.drop_from(&defs, bag, slot, count), None, "bag {bag} slot {slot} count {count}");
        }
    }
}
