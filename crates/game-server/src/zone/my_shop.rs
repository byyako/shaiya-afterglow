//! Personal shops in the zone (`docs/original/personal-shop.md`): the seller's shop (`Placed::shop`) and the visitor's visit
//! (`Placed::visiting`) on the same map, the sign, the visitors' packets, and the purchases waiting for the seller's connection.
//! The bags are the connections' own: the seller's checks an item before placing it and carries each purchase out.

use super::*;
use crate::backend::ItemUnit;
use crate::my_shop::{Admit, Listed, Purchase, Shop, Stage};
use crate::sync::Relock;
use shaiya_proto::my_shop as mp;

type Players = HashMap<u32, Placed>;

/// The sign of an open shop for an asker of `country` (section 3.7): only to the seller's country, or anyone on map 42.
pub(super) fn sign(p: &Placed, map: u16, country: Option<u8>) -> Option<Vec<u8>> {
    let shop = p.shop.as_ref().filter(|s| s.stage == Stage::Open)?;
    let seen = map == crate::my_shop::ANY_COUNTRY_MAP || country == Some(p.occupant.shape.country);
    seen.then(|| mp::sign(p.occupant.id, shop.len, &shop.title))
}

/// To the seller's view, the seller included (`CObject::SendView`).
fn tell_view(players: &Players, id: u32, packet: &[u8]) {
    let Some(cell) = players.get(&id).map(|p| p.cell) else { return };
    for p in players.values().filter(|p| in_view(p.cell, cell)) {
        p.send(packet);
    }
}

/// `stMyShop::SendStopped`: every visitor on the map gets `0x230C` and its visit ends; the list empties.
fn stop_visitors(players: &mut Players, seller: u32) {
    let Some(visitors) = players.get_mut(&seller).and_then(|p| p.shop.as_mut()).map(|s| std::mem::take(&mut s.visitors)) else { return };
    for v in visitors {
        if let Some(p) = players.get_mut(&v).filter(|p| p.visiting == Some(seller)) {
            p.visiting = None;
            p.send(&mp::closed_to_visitor());
        }
    }
}

impl Zones {
    fn sales(&self) -> std::sync::MutexGuard<'_, HashMap<u32, std::collections::VecDeque<Purchase>>> {
        self.sales.guard()
    }

    /// Purchases waiting for a shop that ends: their buyers' connections are answered `0x230D 0` by the dropped replies.
    fn drop_sales(&self, seller: u32) {
        self.sales().remove(&seller);
    }

    /// A shop ends (`stMyShop::Ended` and the shop packets' own ends): visitors stopped, waiting purchases dropped, `0x2302` to
    /// the seller when `tell_end`, the sign down when `sign_down`.
    fn close_shop(&self, players: &mut Players, id: u32, tell_end: bool, sign_down: bool) {
        if players.get(&id).is_none_or(|p| p.shop.is_none()) {
            return;
        }
        stop_visitors(players, id);
        self.drop_sales(id);
        let p = players.get_mut(&id).expect("checked");
        p.shop = None;
        if tell_end {
            p.send(&mp::end());
        }
        if sign_down {
            tell_view(players, id, &mp::sign_down(id));
        }
    }

    /// Leaving the map (`CUser::LeaveZone`): the shop ends (the sign down for the others) and the visit is taken out of the
    /// seller's list (deviation 4).
    pub(super) fn shop_left(&self, players: &mut Players, id: u32) {
        let was_open = players.get(&id).and_then(|p| p.shop.as_ref()).is_some_and(|s| s.stage == Stage::Open);
        self.close_shop(players, id, false, was_open);
        if let Some(seller) = players.get_mut(&id).and_then(|p| p.visiting.take())
            && let Some(shop) = players.get_mut(&seller).and_then(|p| p.shop.as_mut())
        {
            shop.let_go(id);
        }
    }

    /// `stMyShop::Ended` from outside the shop packets (section 6): anything that ends a trade, a trade packet, standing up, a move
    /// carried out. Nothing without a shop.
    pub fn shop_end(&self, me: u32) {
        let mut maps = self.maps();
        if let Some(players) = maps.values_mut().find(|players| players.get(&me).is_some_and(|p| p.shop.is_some())) {
            self.close_shop(players, me, true, true);
        }
    }

    /// `CUser::CancleActionExc` then `stMyShop::Ended` (`docs/original/trade.md` section 4, `docs/original/personal-shop.md`
    /// section 6): death, item actions, NPC shops, gems, quests, leaving. Whether a trade ended.
    pub fn cancel_actions(&self, me: u32) -> bool {
        let ended = self.trade_end(me);
        self.shop_end(me);
        ended
    }

    /// Whether the player has a shop, set up or open.
    pub fn has_shop(&self, map: u16, me: u32) -> bool {
        self.maps().get(&map).and_then(|players| players.get(&me)).is_some_and(|p| p.shop.is_some())
    }

    /// `0x2301` (section 3.1); the caller checked that the player sits on a market map. A shop already there starts afresh, its
    /// visitors stopped and its sign down (deviation 6).
    pub fn shop_begin(&self, map: u16, me: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get(&me) else { return };
        if let Some(shop) = &p.shop {
            let was_open = shop.stage == Stage::Open;
            stop_visitors(players, me);
            self.drop_sales(me);
            if was_open {
                tell_view(players, me, &mp::sign_down(me));
            }
        }
        let p = players.get_mut(&me).expect("checked");
        p.shop = Some(Shop::default());
        p.send(&mp::begin());
    }

    /// `0x2302` (section 3.2): the sign down when the shop was open (deviation 6).
    pub fn shop_finish(&self, map: u16, me: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let was_open = players.get(&me).and_then(|p| p.shop.as_ref()).is_some_and(|s| s.stage == Stage::Open);
        self.close_shop(players, me, true, was_open);
    }

    /// `0x2303` (section 3.5): open, the sign to the view.
    pub fn shop_open(&self, map: u16, me: u32, len: u8, title: Vec<u8>) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get_mut(&me) else { return };
        let Some(shop) = p.shop.as_mut().filter(|s| s.stage == Stage::Setup) else { return };
        shop.stage = Stage::Open;
        shop.len = len;
        shop.title = title;
        let sign = mp::sign(me, shop.len, &shop.title);
        p.send(&mp::started());
        tell_view(players, me, &sign);
        tracing::debug!(seller = me, "personal shop opened");
    }

    /// `0x2304` (section 3.6): back to setting up, the items still placed.
    pub fn shop_close(&self, map: u16, me: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get_mut(&me) else { return };
        let Some(shop) = p.shop.as_mut().filter(|s| s.stage == Stage::Open) else { return };
        shop.stage = Stage::Setup;
        p.send(&mp::stopped());
        tell_view(players, me, &mp::sign_down(me));
        stop_visitors(players, me);
        self.drop_sales(me);
    }

    /// `0x2305` for an item the caller found placeable (section 3.3, deviation 2): the request echoed.
    pub fn shop_place(&self, map: u16, me: u32, add: mp::Add, item: ItemUnit) -> bool {
        let mut maps = self.maps();
        let Some(p) = maps.get_mut(&map).and_then(|players| players.get_mut(&me)) else { return false };
        let Some(shop) = p.shop.as_mut() else { return false };
        if !shop.place(add.shop_slot, Listed { bag: add.bag, slot: add.slot, price: add.price, item }) {
            return false;
        }
        p.send(&mp::added(&add));
        true
    }

    /// `0x2306` (section 3.4): the request echoed.
    pub fn shop_take_off(&self, map: u16, me: u32, shop_slot: u8) {
        let mut maps = self.maps();
        let Some(p) = maps.get_mut(&map).and_then(|players| players.get_mut(&me)) else { return };
        if p.shop.as_mut().is_some_and(|s| s.take_off(shop_slot)) {
            p.send(&mp::removed(shop_slot));
        }
    }

    /// `0x2307` (section 3.4): no answer.
    pub fn shop_set_price(&self, map: u16, me: u32, shop_slot: u8, price: u32) {
        let mut maps = self.maps();
        if let Some(shop) = maps.get_mut(&map).and_then(|players| players.get_mut(&me)).and_then(|p| p.shop.as_mut()) {
            shop.set_price(shop_slot, price);
        }
    }

    /// `0x2309` (section 4.1): the seller on the visitor's map, the same country unless on map 42, the shop open with room; then
    /// `0x2309 1` and the items. A refused visit is answered nothing (deviation 5), oneself included.
    pub fn shop_visit(&self, map: u16, me: u32, seller: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(visitor) = players.get(&me) else { return };
        if visitor.visiting.is_some() || seller == me {
            return;
        }
        let country = visitor.occupant.shape.country;
        let Some(s) = players.get_mut(&seller) else { return };
        if map != crate::my_shop::ANY_COUNTRY_MAP && s.occupant.shape.country != country {
            return;
        }
        let Some(shop) = s.shop.as_mut() else { return };
        match shop.admit(me) {
            Admit::In => {
                let list = mp::list(&shop.units());
                let visitor = players.get_mut(&me).expect("checked");
                visitor.visiting = Some(seller);
                visitor.send(&mp::visiting(seller));
                visitor.send(&list);
            }
            refused => tracing::debug!(visitor = me, seller, ?refused, "shop visit refused"),
        }
    }

    /// `0x230A` (section 4.2): out of the seller's list, answered even when the shop is gone (deviation 4).
    pub fn shop_leave(&self, map: u16, me: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(seller) = players.get_mut(&me).and_then(|p| p.visiting.take()) else { return };
        if let Some(shop) = players.get_mut(&seller).and_then(|p| p.shop.as_mut()) {
            shop.let_go(me);
        }
        players[&me].send(&mp::left());
    }

    /// The seller the player visits, and what its shop shows in a shop slot (section 5's first check, on the buyer's side).
    pub fn shop_listing(&self, map: u16, me: u32, shop_slot: u8) -> Option<(u32, Option<Listed>)> {
        let maps = self.maps();
        let players = maps.get(&map)?;
        let seller = players.get(&me)?.visiting?;
        let shop = players.get(&seller).and_then(|s| s.shop.as_ref());
        Some((seller, shop.and_then(|s| s.slots.get(usize::from(shop_slot))?.clone())))
    }

    /// The seller's own shop slot now, for its checks of a purchase.
    pub fn shop_slot(&self, map: u16, me: u32, shop_slot: u8) -> Option<Listed> {
        let maps = self.maps();
        let shop = maps.get(&map)?.get(&me)?.shop.as_ref()?;
        shop.slots.get(usize::from(shop_slot))?.clone()
    }

    /// Queues a purchase with an open shop on the map and wakes the seller's connection (`ZoneEvent::Sale`). Given back when the
    /// shop is gone or its connection cannot be woken.
    pub fn shop_queue(&self, map: u16, seller: u32, purchase: Purchase) -> Result<(), Purchase> {
        let maps = self.maps();
        let Some(s) =
            maps.get(&map).and_then(|players| players.get(&seller)).filter(|s| s.shop.as_ref().is_some_and(|s| s.stage == Stage::Open))
        else {
            return Err(purchase);
        };
        let mut sales = self.sales();
        sales.entry(seller).or_default().push_back(purchase);
        if s.occupant.events.send(ZoneEvent::Sale).is_err() {
            let queue = sales.get_mut(&seller).expect("just queued");
            let purchase = queue.pop_back().expect("just queued");
            if queue.is_empty() {
                sales.remove(&seller);
            }
            return Err(purchase);
        }
        Ok(())
    }

    /// Takes a purchase back that the seller has not taken (the wait ran out). False when the seller's connection has it.
    pub fn shop_withdraw(&self, seller: u32, buyer: u32) -> bool {
        let mut sales = self.sales();
        let Some(queue) = sales.get_mut(&seller) else { return false };
        let before = queue.len();
        queue.retain(|p| p.buyer != buyer);
        let withdrawn = queue.len() < before;
        if queue.is_empty() {
            sales.remove(&seller);
        }
        withdrawn
    }

    /// The next purchase waiting for this seller.
    pub fn shop_next_sale(&self, me: u32) -> Option<Purchase> {
        let mut sales = self.sales();
        let queue = sales.get_mut(&me)?;
        let next = queue.pop_front();
        if queue.is_empty() {
            sales.remove(&me);
        }
        next
    }

    /// A sale was written (section 5 steps 5 and 7): `bought` (`0x230D 1`) to the buyer wherever it is, the shop slot keeps `left`,
    /// and every visitor gets `0x230E`.
    pub fn shop_sold(&self, map: u16, me: u32, shop_slot: u8, left: u8, buyer: u32, bought: &[u8]) {
        let mut maps = self.maps();
        if let Some(b) = maps.values().find_map(|players| players.get(&buyer)) {
            b.send(bought);
        }
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(shop) = players.get_mut(&me).and_then(|p| p.shop.as_mut()) else { return };
        shop.sold(shop_slot, left);
        let visitors = shop.visitors.clone();
        let packet = mp::slot_count(shop_slot, left);
        for v in visitors {
            if let Some(p) = players.get(&v) {
                p.send(&packet);
            }
        }
    }
}
