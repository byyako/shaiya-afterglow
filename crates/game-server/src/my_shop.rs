//! Personal shops (`docs/original/personal-shop.md`): a seller's shop slots and visitors, what may be placed, and the purchase as the
//! seller's connection carries it out. Bookkeeping only: the zone keeps the shop on the seller and the visit on the visitor and
//! sends the packets; each connection checks and changes its own bags.
//!
//! A purchase (section 7, deviations 7 to 9): the buyer's connection checks its side and queues a [`Purchase`] with the seller; the
//! seller's connection takes the queue in order, checks its side ([`plan`]), writes both in one transaction and answers the buyer.

use std::time::Duration;

use shaiya_proto::my_shop as mp;
use tokio::sync::oneshot;

use crate::backend::{Handover, ItemUnit};
use crate::inventory::{ItemDef, SlotChange, real_type};

pub const SLOTS: usize = mp::SLOTS as usize;
/// The most visitors a shop takes (`stMyShop::Use`: 16 → 2).
pub const MAX_VISITORS: usize = 16;
/// Where the seller pays a commission (`m_wMap` 42, the Auction House).
pub const COMMISSION_MAP: u16 = 42;
/// Where the two countries may visit each other's shops (the visitor's map 42).
pub const ANY_COUNTRY_MAP: u16 = 42;
/// How long a buyer waits for the seller's connection to carry its purchase out.
pub const SALE_WAIT: Duration = Duration::from_secs(30);

/// Setting up (status 1) or open (2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Setup,
    Open,
}

/// A shop slot: where the item is, its price of one, and the item as it was when placed (every item action of the seller ends the
/// shop, so it stays what is in the bag; a sale updates its count).
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub bag: u8,
    pub slot: u8,
    pub price: u32,
    pub item: ItemUnit,
}

/// A seller's shop (`stMyShop`).
#[derive(Clone, Debug, PartialEq)]
pub struct Shop {
    pub stage: Stage,
    pub slots: [Option<Listed>; SLOTS],
    /// The title's length as the client gave it (cut to 120), and the title.
    pub len: u8,
    pub title: Vec<u8>,
    pub visitors: Vec<u32>,
}

/// What `stMyShop::Use` says of a visitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    In,
    Full,
    Refused,
}

impl Default for Shop {
    fn default() -> Self {
        Self { stage: Stage::Setup, slots: Default::default(), len: 0, title: Vec::new(), visitors: Vec::new() }
    }
}

impl Shop {
    /// `0x2305` (section 3.3): into an empty shop slot, the place not listed already. The caller checked the item.
    pub fn place(&mut self, shop_slot: u8, listed: Listed) -> bool {
        let Some(slot) = self.slots.get(usize::from(shop_slot)) else { return false };
        if self.stage != Stage::Setup || slot.is_some() || self.slots.iter().flatten().any(|l| (l.bag, l.slot) == (listed.bag, listed.slot))
        {
            return false;
        }
        self.slots[usize::from(shop_slot)] = Some(listed);
        true
    }

    /// `0x2306` (section 3.4).
    pub fn take_off(&mut self, shop_slot: u8) -> bool {
        match self.slots.get_mut(usize::from(shop_slot)) {
            Some(slot @ Some(_)) if self.stage == Stage::Setup => {
                *slot = None;
                true
            }
            _ => false,
        }
    }

    /// `0x2307` (section 3.4): a filled slot, the total of all of it fitting in 32 bits (deviation 2).
    pub fn set_price(&mut self, shop_slot: u8, price: u32) -> bool {
        let stage = self.stage;
        match self.slots.get_mut(usize::from(shop_slot)) {
            Some(Some(listed)) if stage == Stage::Setup && total(listed.item.count, price).is_some() => {
                listed.price = price;
                true
            }
            _ => false,
        }
    }

    /// `stMyShop::Use`.
    pub fn admit(&mut self, visitor: u32) -> Admit {
        if self.visitors.len() >= MAX_VISITORS {
            Admit::Full
        } else if self.stage != Stage::Open || self.visitors.contains(&visitor) {
            Admit::Refused
        } else {
            self.visitors.push(visitor);
            Admit::In
        }
    }

    /// `stMyShop::Unuse`.
    pub fn let_go(&mut self, visitor: u32) {
        self.visitors.retain(|&v| v != visitor);
    }

    /// What `0x230B` shows: each filled shop slot in order with its price of one and the item.
    pub fn units(&self) -> Vec<(u8, u32, &ItemUnit)> {
        self.slots.iter().enumerate().filter_map(|(i, l)| l.as_ref().map(|l| (i as u8, l.price, &l.item))).collect()
    }

    /// A sale left `left` in a shop slot (0 empties it).
    pub fn sold(&mut self, shop_slot: u8, left: u8) {
        let Some(slot) = self.slots.get_mut(usize::from(shop_slot)) else { return };
        match slot {
            Some(listed) if left > 0 => listed.item.count = left,
            _ => *slot = None,
        }
    }
}

/// count × price of one, when it fits in 32 bits (deviation 3: the original's product wraps).
pub fn total(count: u8, price: u32) -> Option<u32> {
    price.checked_mul(u32::from(count))
}

/// What the seller pays of a sale: on map 42, price × 0.05 truncated (section 5 step 2); nothing elsewhere.
pub fn commission(map: u16, total: u32) -> u32 {
    if map == COMMISSION_MAP { (f64::from(total) * 0.05) as u32 } else { 0 }
}

/// Whether an item may be placed at that price (deviation 2): not a quest item (real type 27) nor type 99, not bound, and all of
/// it at that price fitting in 32 bits.
pub fn placeable(item: &ItemUnit, def: Option<&ItemDef>, price: u32) -> bool {
    let bound = def.is_none_or(|def| def.bound);
    real_type(item.item_type) != 27 && item.item_type != 99 && !bound && item.count > 0 && total(item.count, price).is_some()
}

/// What the buyer's connection hands the seller's: the shop slot and count, what it saw there, the total, the buyer's money and
/// empty slot.
#[derive(Debug)]
pub struct Purchase {
    pub buyer: u32,
    pub buyer_money: u32,
    pub shop_slot: u8,
    pub count: u8,
    pub seen: Listed,
    pub total: u32,
    pub to: (u8, u8),
    pub reply: oneshot::Sender<Result<Bought, u8>>,
}

/// The buyer's share of a sale: the item in its slot, and what it paid. Its `0x230D` has gone out already.
#[derive(Clone, Debug, PartialEq)]
pub struct Bought {
    pub item: ItemUnit,
    pub total: u32,
    /// The seller's player id and the row the buyer's item got (the game log's records).
    pub seller: u32,
    pub row: Option<crate::backend::Stored>,
}

/// The seller's side of a sale, once checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Sale {
    /// The money the seller gets (the total less any commission).
    pub gain: u32,
    /// What is left in the bag slot and the shop slot.
    pub left: u8,
    pub change: SlotChange,
    pub handover: Handover,
    /// The buyer's item: the seller's, with the count bought, in the buyer's slot.
    pub got: ItemUnit,
}

/// The seller's checks and the sale (section 5, deviations 7 and 8): the shop slot still lists what the buyer saw; the bag still has
/// that item with enough of it; the seller's money can take the total (else 3). Any other failure is 0.
pub fn plan(
    purchase: &Purchase,
    seller: u32,
    listed: Option<&Listed>,
    bag_item: Option<&ItemUnit>,
    money: u32,
    map: u16,
) -> Result<Sale, u8> {
    let seen = &purchase.seen;
    let same = |l: &Listed| {
        (l.bag, l.slot, l.price, l.item.item_type, l.item.type_id)
            == (seen.bag, seen.slot, seen.price, seen.item.item_type, seen.item.type_id)
    };
    let listed = listed.filter(|l| same(l)).ok_or(mp::BUY_FAILED)?;
    let item = bag_item
        .filter(|i| {
            (i.item_type, i.type_id) == (listed.item.item_type, listed.item.type_id) && i.count >= purchase.count && purchase.count > 0
        })
        .ok_or(mp::BUY_FAILED)?;
    if total(purchase.count, listed.price) != Some(purchase.total) {
        return Err(mp::BUY_FAILED);
    }
    if money.checked_add(purchase.total).is_none() {
        return Err(mp::BUY_SELLER_TOO_MUCH_GOLD);
    }
    let (bag, slot) = (listed.bag, listed.slot);
    let left = item.count - purchase.count;
    let change = if left == 0 { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: left } };
    let handover =
        Handover { giver: seller, from: (bag, slot), had: item.count, count: purchase.count, receiver: purchase.buyer, to: purchase.to };
    let got = ItemUnit { bag: purchase.to.0, slot: purchase.to.1, count: purchase.count, ..item.clone() };
    Ok(Sale { gain: purchase.total - commission(map, purchase.total), left, change, handover, got })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(count: u8) -> ItemUnit {
        ItemUnit { bag: 1, slot: 3, item_type: 100, type_id: 1, quality: 0, gems: [0; 6], count, craft_name: Vec::new() }
    }

    fn listed(count: u8, price: u32) -> Listed {
        Listed { bag: 1, slot: 3, price, item: item(count) }
    }

    fn purchase(count: u8, price: u32) -> (Purchase, oneshot::Receiver<Result<Bought, u8>>) {
        let (reply, rx) = oneshot::channel();
        let p = Purchase {
            buyer: 2,
            buyer_money: 1000,
            shop_slot: 0,
            count,
            seen: listed(10, price),
            total: total(count, price).unwrap(),
            to: (1, 7),
            reply,
        };
        (p, rx)
    }

    #[test]
    fn placing_follows_the_original_with_the_trade_checks() {
        let mut shop = Shop::default();
        assert!(shop.place(0, listed(5, 10)));
        assert!(!shop.place(0, Listed { slot: 4, ..listed(5, 10) }), "shop slot used");
        assert!(!shop.place(1, listed(5, 10)), "the same bag and slot already placed");
        assert!(!shop.place(12, Listed { slot: 4, ..listed(5, 10) }), "shop slot out of range");
        assert!(shop.set_price(0, 20));
        assert!(!shop.set_price(0, u32::MAX), "five at u32::MAX does not fit");
        assert!(!shop.set_price(1, 20), "empty slot");
        shop.stage = Stage::Open;
        assert!(!shop.take_off(0), "only while setting up");
        assert!(!shop.place(2, Listed { slot: 5, ..listed(5, 10) }));
        assert_eq!(shop.units().len(), 1);

        let def = ItemDef { bound: false, ..Default::default() };
        assert!(placeable(&item(5), Some(&def), 10));
        assert!(!placeable(&item(5), Some(&def), u32::MAX / 4), "deviation 2: all of it must fit");
        assert!(!placeable(&item(5), Some(&ItemDef { bound: true, ..def }), 10), "bound");
        assert!(!placeable(&ItemUnit { item_type: 99, ..item(1) }, Some(&def), 10));
        assert!(!placeable(&item(5), None, 10), "no definition");
    }

    #[test]
    fn visitors_as_stmyshop_use() {
        let mut shop = Shop::default();
        assert_eq!(shop.admit(5), Admit::Refused, "not open");
        shop.stage = Stage::Open;
        assert_eq!(shop.admit(5), Admit::In);
        assert_eq!(shop.admit(5), Admit::Refused, "already in");
        for v in 6..21 {
            assert_eq!(shop.admit(v), Admit::In);
        }
        assert_eq!(shop.admit(99), Admit::Full);
        shop.let_go(5);
        assert_eq!(shop.admit(99), Admit::In);
    }

    #[test]
    fn the_price_never_wraps_and_map_42_takes_five_percent() {
        assert_eq!(total(2, 1 << 31), None, "the original makes this 0");
        assert_eq!(total(255, 16_843_009), Some(4_294_967_295));
        assert_eq!(commission(42, 1000), 50);
        assert_eq!(commission(42, 19), 0, "0.95 truncated");
        assert_eq!(commission(42, 39), 1, "1.95 truncated");
        assert_eq!(commission(42, 20), 1);
        assert_eq!(commission(1, 1000), 0);
    }

    #[test]
    fn a_sale_checks_the_seller_side() {
        let (p, _rx) = purchase(4, 25);
        let sale = plan(&p, 1, Some(&listed(10, 25)), Some(&item(10)), 0, 42).unwrap();
        assert_eq!((sale.gain, sale.left), (95, 6));
        assert_eq!(sale.change, SlotChange::SetCount { bag: 1, slot: 3, count: 6 });
        assert_eq!(sale.handover, Handover { giver: 1, from: (1, 3), had: 10, count: 4, receiver: 2, to: (1, 7) });
        assert_eq!((sale.got.bag, sale.got.slot, sale.got.count), (1, 7, 4));

        let all = plan(&purchase(10, 25).0, 1, Some(&listed(10, 25)), Some(&item(10)), 0, 1).unwrap();
        assert_eq!((all.gain, all.left, all.change), (250, 0, SlotChange::Remove { bag: 1, slot: 3 }));

        assert_eq!(plan(&p, 1, None, Some(&item(10)), 0, 1), Err(mp::BUY_FAILED), "taken off meanwhile");
        assert_eq!(plan(&p, 1, Some(&listed(10, 30)), Some(&item(10)), 0, 1), Err(mp::BUY_FAILED), "another price");
        assert_eq!(plan(&p, 1, Some(&listed(10, 25)), Some(&item(3)), 0, 1), Err(mp::BUY_FAILED), "not enough left");
        assert_eq!(plan(&p, 1, Some(&listed(10, 25)), None, 0, 1), Err(mp::BUY_FAILED), "gone from the bag");
        assert_eq!(plan(&p, 1, Some(&listed(10, 25)), Some(&item(10)), u32::MAX - 99, 1), Err(mp::BUY_SELLER_TOO_MUCH_GOLD));
    }

    #[test]
    fn a_sale_updates_the_shop_slot() {
        let mut shop = Shop::default();
        shop.place(3, listed(10, 5));
        shop.sold(3, 4);
        assert_eq!(shop.slots[3].as_ref().unwrap().item.count, 4);
        shop.sold(3, 0);
        assert!(shop.slots[3].is_none());
    }
}
