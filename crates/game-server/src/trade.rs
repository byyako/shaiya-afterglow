//! Trades between two players (`docs/original/trade.md`): who trades with whom, what each side offers, ready, and the exchange
//! plan. Bookkeeping only: the zone sends the packets, each connection checks and changes its own bags.
//!
//! The exchange (section 5, deviation 2): the player who pressed ready last checks its side against its bags ([`prepare`]) and parks
//! it here; its partner's connection takes it, checks its own side, makes the [`plan`], writes it in one transaction and hands the
//! parked side its share of the result.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use shaiya_proto::ground as gp;
use shaiya_proto::trade as tp;
use tokio::sync::oneshot;

use crate::backend::{Handover, ItemUnit};
use crate::inventory::{Inventory, SlotChange};

/// How long a request waits for its answer (`CUser::PacketExchange`, 30000 ms).
pub const REQUEST_TIME: Duration = Duration::from_secs(30);
/// A step that leaves the two further apart than this ends the trade (`CUser::PacketPC` 0x46a5d1, x and z).
pub const MAX_APART: f32 = 10.0;
const SLOTS: usize = tp::SLOTS as usize;

/// Where a player is in a trade (the original's states 1 to 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// It asked the partner (1).
    Asking,
    /// The partner asked it (2).
    Asked,
    /// The window is open (3).
    Open,
    /// Both are ready; the exchange is running (4).
    Exchanging,
}

/// An item offered: where it is, how many, and what it was then (checked again at the exchange).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offered {
    pub bag: u8,
    pub slot: u8,
    pub count: u8,
    pub item_type: u8,
    pub type_id: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Side {
    pub partner: u32,
    pub stage: Stage,
    pub offers: [Option<Offered>; SLOTS],
    pub money: u32,
    pub ready: bool,
    /// The requester's deadline for the answer.
    deadline: Option<Instant>,
}

impl Side {
    fn new(partner: u32, stage: Stage, deadline: Option<Instant>) -> Self {
        Self { partner, stage, offers: [None; SLOTS], money: 0, ready: false, deadline }
    }
}

/// One side as its bags are when the exchange runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prepared {
    pub id: u32,
    /// The money it has and the money it offers.
    pub money: u32,
    pub offer: u32,
    /// The items it gives, in trade slot order, with the count offered of each.
    pub gives: Vec<(ItemUnit, u8)>,
    /// Its empty inventory slots in the original's search order.
    pub free: Vec<(u8, u8)>,
}

/// What the exchange does to one side.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settled {
    /// The money gained (negative: lost).
    pub money: i64,
    /// The slots it gave from: emptied or with less.
    pub given: Vec<SlotChange>,
    /// The items it got, in their new slots.
    pub received: Vec<ItemUnit>,
    /// Its packets, in order, the last being `0x0A04 0`.
    pub packets: Vec<Vec<u8>>,
    /// The gold it put in and the gold it got (the game log's `0x0804` and `0x0803`).
    pub sent: u32,
    pub got: u32,
    /// The other side's player id.
    pub partner: u32,
    /// Each item it gave or got, for the game log; the rows are filled in once the exchange is saved (`Traded::row`).
    pub traded: Vec<Traded>,
}

/// One item of an exchange as one side saw it (`docs/original/game-log.md` section 3.2, `0x0405` and `0x0406`): `item` with the count
/// that changed hands, in the slot it left (the giver) or now sits in (the receiver). `index` is its handover's place in the list the
/// exchange was saved with; `row` the row the save left for this side (the giver's remaining row, or the moved or copied one).
#[derive(Clone, Debug, PartialEq)]
pub struct Traded {
    pub index: usize,
    pub gave: bool,
    pub item: ItemUnit,
    pub row: Option<crate::backend::Stored>,
}

/// How the exchange ended for the parked side.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Done(Settled),
    /// The `0x0A04` result for it (its partner has had its own).
    Failed(u8),
}

/// The side of the player who pressed ready last, waiting for its partner's connection.
#[derive(Debug)]
pub struct Parked {
    pub side: Prepared,
    pub reply: oneshot::Sender<Outcome>,
}

/// What answering a request did (`0x0A02`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Not asked: nothing.
    Ignored,
    /// Asked by someone no longer asking: the answerer is cleared, silently.
    Stale,
    Accepted {
        requester: u32,
    },
    Refused {
        requester: u32,
    },
}

/// A trade that ended (`CUser::CancleActionExc`): who was in it and where the one ending it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ended {
    pub partner: u32,
    pub stage: Stage,
}

/// What a change to the offer did: the partner, and whether both ready flags were cleared first (`0x0A05 3,1` to both).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changed {
    pub partner: u32,
    pub unready: bool,
}

/// Every trade of the server.
#[derive(Debug, Default)]
pub struct Trades {
    sides: HashMap<u32, Side>,
    parked: HashMap<u32, Parked>,
}

impl Trades {
    pub fn side(&self, id: u32) -> Option<&Side> {
        self.sides.get(&id)
    }

    /// In any trade, asking or asked included.
    pub fn busy(&self, id: u32) -> bool {
        self.sides.contains_key(&id)
    }

    /// Links a requester and the player it asks (section 2.1 step 4); the caller has checked both are free.
    pub fn request(&mut self, requester: u32, target: u32, now: Instant) {
        self.sides.insert(requester, Side::new(target, Stage::Asking, Some(now + REQUEST_TIME)));
        self.sides.insert(target, Side::new(requester, Stage::Asked, None));
    }

    /// The other side of `id`'s trade when it is at `stage` and that side is at `partner_stage` with `id` as its partner.
    fn partner_at(&self, id: u32, stage: Stage, partner_stage: Stage) -> Option<u32> {
        let me = self.sides.get(&id).filter(|s| s.stage == stage)?;
        self.sides.get(&me.partner).filter(|p| p.stage == partner_stage && p.partner == id).map(|_| me.partner)
    }

    /// The partner of an open trade.
    fn open_partner(&self, id: u32) -> Option<u32> {
        self.partner_at(id, Stage::Open, Stage::Open)
    }

    fn remove_both(&mut self, id: u32, partner: u32) {
        self.sides.remove(&id);
        self.sides.remove(&partner);
        // A parked side's connection wakes with nothing: whoever ended the trade has told both clients.
        self.parked.remove(&id);
        self.parked.remove(&partner);
    }

    /// `0x0A02` (section 2.2).
    pub fn answer(&mut self, id: u32, yes: bool) -> Answer {
        if self.sides.get(&id).is_none_or(|s| s.stage != Stage::Asked) {
            return Answer::Ignored;
        }
        let Some(requester) = self.partner_at(id, Stage::Asked, Stage::Asking) else {
            self.sides.remove(&id);
            return Answer::Stale;
        };
        if yes {
            for side in [id, requester] {
                let side = self.sides.get_mut(&side).expect("both sides exist");
                side.stage = Stage::Open;
                side.deadline = None;
            }
            Answer::Accepted { requester }
        } else {
            self.remove_both(id, requester);
            Answer::Refused { requester }
        }
    }

    /// `CUser::ExchangeCancleReady`: whether either side was ready (both are not, now).
    fn unready(&mut self, id: u32, partner: u32) -> bool {
        let was = [id, partner].iter().any(|s| self.sides.get(s).is_some_and(|s| s.ready));
        for s in [id, partner] {
            if let Some(side) = self.sides.get_mut(&s) {
                side.ready = false;
            }
        }
        was
    }

    /// `0x0A06` once the caller checked the item (section 2.3): into an empty trade slot, the place not offered already.
    pub fn offer(&mut self, id: u32, trade_slot: u8, offered: Offered) -> Option<Changed> {
        let partner = self.open_partner(id)?;
        let side = &self.sides[&id];
        let slot = usize::from(trade_slot);
        if slot >= SLOTS
            || side.offers[slot].is_some()
            || side.offers.iter().flatten().any(|o| (o.bag, o.slot) == (offered.bag, offered.slot))
        {
            return None;
        }
        let unready = self.unready(id, partner);
        self.sides.get_mut(&id).expect("checked").offers[slot] = Some(offered);
        Some(Changed { partner, unready })
    }

    /// `0x0A07` (section 2.4).
    pub fn take_back(&mut self, id: u32, trade_slot: u8) -> Option<Changed> {
        let partner = self.open_partner(id)?;
        let slot = usize::from(trade_slot);
        self.sides[&id].offers.get(slot).copied().flatten()?;
        let unready = self.unready(id, partner);
        self.sides.get_mut(&id).expect("checked").offers[slot] = None;
        Some(Changed { partner, unready })
    }

    /// `0x0A08` once the caller checked the amount against the money (section 2.5).
    pub fn set_money(&mut self, id: u32, money: u32) -> Option<Changed> {
        let partner = self.open_partner(id)?;
        let unready = self.unready(id, partner);
        self.sides.get_mut(&id).expect("checked").money = money;
        Some(Changed { partner, unready })
    }

    /// `0x0A05 1`: the partner when either was ready.
    pub fn not_ready(&mut self, id: u32) -> Option<u32> {
        let partner = self.open_partner(id)?;
        self.unready(id, partner).then_some(partner)
    }

    /// `0x0A05 0`: the partner, and whether both are ready now; then both are exchanging (once: the step is taken under the
    /// caller's lock).
    pub fn ready(&mut self, id: u32) -> Option<(u32, bool)> {
        let partner = self.open_partner(id)?;
        self.sides.get_mut(&id).expect("checked").ready = true;
        let both = self.sides[&partner].ready;
        if both {
            for s in [id, partner] {
                self.sides.get_mut(&s).expect("both sides exist").stage = Stage::Exchanging;
            }
        }
        Some((partner, both))
    }

    /// `0x0A05 2`: the partner of the open trade cancelled (section 2.6).
    pub fn cancel(&mut self, id: u32) -> Option<u32> {
        let partner = self.open_partner(id)?;
        self.remove_both(id, partner);
        Some(partner)
    }

    /// `CUser::CancleActionExc` (section 4), at any stage; both sides are cleared (deviation 4 for a requester).
    pub fn end(&mut self, id: u32) -> Option<Ended> {
        let side = self.sides.get(&id)?;
        let ended = Ended { partner: side.partner, stage: side.stage };
        // Only a side that still points back is the partner's; a stale one is left to its own end.
        let linked = self.sides.get(&ended.partner).is_some_and(|p| p.partner == id);
        if linked {
            self.remove_both(id, ended.partner);
        } else {
            self.sides.remove(&id);
            self.parked.remove(&id);
        }
        Some(ended)
    }

    pub fn partner(&self, id: u32) -> Option<u32> {
        self.sides.get(&id).map(|s| s.partner)
    }

    /// Requests past their deadline: both sides cleared, silently (`CUser::UpdateTimeover`, deviation 4).
    pub fn expire(&mut self, now: Instant) -> usize {
        let due: Vec<(u32, u32)> = self
            .sides
            .iter()
            .filter(|(_, s)| s.stage == Stage::Asking && s.deadline.is_some_and(|d| d <= now))
            .map(|(&id, s)| (id, s.partner))
            .collect();
        for &(id, partner) in &due {
            self.sides.remove(&id);
            if self.sides.get(&partner).is_some_and(|p| p.partner == id) {
                self.sides.remove(&partner);
            }
        }
        due.len()
    }

    /// The offers and the money of an exchanging side.
    pub fn exchanging(&self, id: u32) -> Option<(&[Option<Offered>; SLOTS], u32)> {
        self.sides.get(&id).filter(|s| s.stage == Stage::Exchanging).map(|s| (&s.offers, s.money))
    }

    /// Parks the side of the player who pressed ready last; given back when the trade ended in the meantime. Returns the partner.
    pub fn park(&mut self, id: u32, parked: Parked) -> Result<u32, Parked> {
        match self.partner_at(id, Stage::Exchanging, Stage::Exchanging) {
            Some(partner) => {
                self.parked.insert(id, parked);
                Ok(partner)
            }
            None => Err(parked),
        }
    }

    /// The partner's parked side, for its exchange: both sides leave the book, the trade now belongs to the caller.
    pub fn take_parked(&mut self, id: u32) -> Option<Parked> {
        let partner = self.partner_at(id, Stage::Exchanging, Stage::Exchanging)?;
        let parked = self.parked.remove(&partner)?;
        self.sides.remove(&id);
        self.sides.remove(&partner);
        Some(parked)
    }

    /// Takes a parked side back that no one has taken (the wait ran out): both sides cleared. The partner, or `None` when the
    /// partner's connection has it.
    pub fn unpark(&mut self, id: u32) -> Option<u32> {
        self.parked.remove(&id)?;
        let partner = self.sides.get(&id).map(|s| s.partner);
        self.sides.remove(&id);
        if let Some(partner) = partner {
            self.sides.remove(&partner);
        }
        partner
    }
}

/// A side as its bags are now. `None` when any offer is gone or no longer what it was (another item, fewer than offered): the
/// exchange is then cancelled for both and nothing moves (audit R3; `docs/original/trade.md` section 5, deviation 7).
pub fn prepare(id: u32, money: u32, offers: &[Option<Offered>], offer: u32, inventory: &Inventory) -> Option<Prepared> {
    let gives = offers
        .iter()
        .flatten()
        .map(|o| {
            let item = inventory.get(o.bag, o.slot)?;
            (item.item_type == o.item_type && item.type_id == o.type_id && item.count >= o.count && o.count > 0)
                .then(|| (item.clone(), o.count))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Prepared { id, money, offer, gives, free: inventory.free_slots() })
}

/// The exchange (`CUser::Exchange`, section 3): `last` pressed ready last. The checks in the original's order, each failure giving
/// the `0x0A04` results for (`last`, `other`); then the money both ways, `other`'s items to `last`, `last`'s to `other`.
pub fn plan(last: &Prepared, other: &Prepared) -> Result<(Settled, Settled, Vec<Handover>), (u8, u8)> {
    let fits = |money: u32, gain: u32| money.checked_add(gain).is_some();
    if other.offer > other.money || !fits(last.money, other.offer) {
        return Err((tp::END_TOO_MUCH_GOLD, tp::END_PARTNER_TOO_MUCH_GOLD));
    }
    if last.offer > last.money || !fits(other.money, last.offer) {
        return Err((tp::END_PARTNER_TOO_MUCH_GOLD, tp::END_TOO_MUCH_GOLD));
    }
    if other.gives.len() > last.free.len() {
        return Err((tp::END_NO_ROOM, tp::END_PARTNER_NO_ROOM));
    }
    if last.gives.len() > other.free.len() {
        return Err((tp::END_PARTNER_NO_ROOM, tp::END_NO_ROOM));
    }
    let gain = |side: &Prepared, from: &Prepared| i64::from(from.offer) - i64::from(side.offer);
    let mut last_out = Settled { money: gain(last, other), sent: last.offer, got: other.offer, partner: other.id, ..Default::default() };
    let mut other_out = Settled { money: gain(other, last), sent: other.offer, got: last.offer, partner: last.id, ..Default::default() };
    let mut handovers = Vec::new();
    hand_over(other, last, &mut other_out, &mut last_out, &mut handovers);
    hand_over(last, other, &mut last_out, &mut other_out, &mut handovers);
    last_out.packets.push(tp::end(tp::END_DONE));
    other_out.packets.push(tp::end(tp::END_DONE));
    Ok((last_out, other_out, handovers))
}

/// `CUser::ExchangeItemAdd`: each item of `giver` into the next empty slot of `receiver`; all of it moves, or part of it as a copy.
fn hand_over(giver: &Prepared, receiver: &Prepared, giver_out: &mut Settled, receiver_out: &mut Settled, handovers: &mut Vec<Handover>) {
    for ((item, count), &to) in giver.gives.iter().zip(&receiver.free) {
        let (bag, slot) = (item.bag, item.slot);
        let left = item.count - count;
        let (change, rest) = if left == 0 {
            (SlotChange::Remove { bag, slot }, None)
        } else {
            (SlotChange::SetCount { bag, slot, count: left }, Some(ItemUnit { count: left, ..item.clone() }))
        };
        giver_out.given.push(change);
        giver_out.packets.push(gp::item_left(bag, slot, rest.as_ref()));
        let got = ItemUnit { bag: to.0, slot: to.1, count: *count, ..item.clone() };
        let index = handovers.len();
        giver_out.traded.push(Traded { index, gave: true, item: ItemUnit { count: *count, ..item.clone() }, row: None });
        receiver_out.traded.push(Traded { index, gave: false, item: got.clone(), row: None });
        receiver_out.packets.push(gp::picked_up(&got));
        receiver_out.received.push(got);
        handovers.push(Handover { giver: giver.id, from: (bag, slot), had: item.count, count: *count, receiver: receiver.id, to });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shaiya_proto::frame::opcode;

    fn item(bag: u8, slot: u8, item_type: u8, count: u8) -> ItemUnit {
        ItemUnit { bag, slot, item_type, type_id: 1, count, ..Default::default() }
    }

    fn offered(bag: u8, slot: u8, item_type: u8, count: u8) -> Offered {
        Offered { bag, slot, count, item_type, type_id: 1 }
    }

    fn open(trades: &mut Trades, a: u32, b: u32) {
        trades.request(a, b, Instant::now());
        assert_eq!(trades.answer(b, true), Answer::Accepted { requester: a });
    }

    #[test]
    fn a_request_is_answered_once() {
        let mut trades = Trades::default();
        trades.request(1, 2, Instant::now());
        assert!(trades.busy(1) && trades.busy(2));
        assert_eq!(trades.answer(1, true), Answer::Ignored, "the requester cannot answer");
        assert_eq!(trades.answer(2, false), Answer::Refused { requester: 1 });
        assert!(!trades.busy(1) && !trades.busy(2));
        assert_eq!(trades.answer(2, true), Answer::Ignored);
    }

    #[test]
    fn requests_expire_after_30_seconds_for_both() {
        let mut trades = Trades::default();
        let now = Instant::now();
        trades.request(1, 2, now);
        assert_eq!(trades.expire(now + Duration::from_secs(29)), 0);
        assert_eq!(trades.expire(now + REQUEST_TIME), 1);
        assert!(!trades.busy(2), "the asked player is free again (the original leaves it busy)");
        // An open trade has no deadline.
        open(&mut trades, 1, 2);
        assert_eq!(trades.expire(now + REQUEST_TIME * 4), 0);
    }

    #[test]
    fn an_answer_to_a_vanished_request_clears_the_answerer() {
        let mut trades = Trades::default();
        trades.request(1, 2, Instant::now());
        trades.sides.remove(&1);
        assert_eq!(trades.answer(2, true), Answer::Stale);
        assert!(!trades.busy(2));
    }

    #[test]
    fn every_change_clears_both_ready_flags_first() {
        let mut trades = Trades::default();
        open(&mut trades, 1, 2);
        assert_eq!(trades.offer(1, 0, offered(1, 0, 25, 5)), Some(Changed { partner: 2, unready: false }));
        assert_eq!(trades.offer(1, 0, offered(1, 1, 25, 5)), None, "trade slot taken");
        assert_eq!(trades.offer(1, 1, offered(1, 0, 25, 1)), None, "the same bag slot twice");
        assert_eq!(trades.offer(1, 8, offered(1, 2, 25, 1)), None, "only 8 trade slots");
        assert_eq!(trades.ready(2), Some((1, false)));
        assert_eq!(trades.set_money(1, 100), Some(Changed { partner: 2, unready: true }));
        assert!(!trades.side(2).unwrap().ready);
        assert_eq!(trades.ready(2), Some((1, false)));
        assert_eq!(trades.take_back(1, 0), Some(Changed { partner: 2, unready: true }));
        assert_eq!(trades.take_back(1, 0), None);
        assert_eq!(trades.not_ready(2), None, "nobody was ready");
        assert_eq!(trades.ready(1), Some((2, false)));
        assert_eq!(trades.not_ready(2), Some(1));
    }

    #[test]
    fn both_ready_starts_the_exchange_once() {
        let mut trades = Trades::default();
        open(&mut trades, 1, 2);
        assert_eq!(trades.ready(1), Some((2, false)));
        assert_eq!(trades.ready(2), Some((1, true)));
        assert_eq!(trades.ready(1), None, "exchanging: no second start");
        assert_eq!(trades.cancel(1), None, "the window's cancel does nothing while exchanging");
        let (tx, _rx) = oneshot::channel();
        assert_eq!(trades.park(2, Parked { side: Prepared::default(), reply: tx }).ok(), Some(1));
        let parked = trades.take_parked(1).expect("parked");
        assert_eq!(parked.side, Prepared::default());
        assert!(!trades.busy(1) && !trades.busy(2));
        assert_eq!(trades.unpark(2), None, "already taken");
    }

    #[test]
    fn ending_a_trade_drops_a_parked_side() {
        let mut trades = Trades::default();
        open(&mut trades, 1, 2);
        trades.ready(1);
        trades.ready(2);
        let (tx, mut rx) = oneshot::channel();
        trades.park(2, Parked { side: Prepared::default(), reply: tx }).unwrap();
        assert_eq!(trades.end(1), Some(Ended { partner: 2, stage: Stage::Exchanging }));
        assert!(rx.try_recv().is_err(), "the parked connection wakes with nothing");
        assert!(trades.take_parked(1).is_none());
        let (tx, _rx) = oneshot::channel();
        assert!(trades.park(2, Parked { side: Prepared::default(), reply: tx }).is_err());
    }

    #[test]
    fn unparking_after_the_wait_clears_both() {
        let mut trades = Trades::default();
        open(&mut trades, 1, 2);
        trades.ready(1);
        trades.ready(2);
        let (tx, _rx) = oneshot::channel();
        trades.park(2, Parked { side: Prepared::default(), reply: tx }).unwrap();
        assert_eq!(trades.unpark(2), Some(1));
        assert!(!trades.busy(1) && !trades.busy(2));
    }

    #[test]
    fn preparing_refuses_what_changed_since_the_offer() {
        let inventory = Inventory::new(&[item(1, 0, 25, 10), item(1, 1, 5, 1)]);
        let side = prepare(7, 50, &[Some(offered(1, 0, 25, 4)), Some(offered(1, 1, 5, 1)), None], 20, &inventory).expect("all there");
        assert_eq!(side.gives, vec![(item(1, 0, 25, 10), 4), (item(1, 1, 5, 1), 1)]);
        assert_eq!(side.free.len(), 2 * 24 - 2);
        assert_eq!(side.free[0], (1, 2));
        let refused = |offers: &[Option<Offered>]| prepare(7, 50, offers, 20, &inventory).is_none();
        assert!(refused(&[Some(offered(1, 0, 25, 4)), Some(offered(1, 1, 6, 1))]), "another item now");
        assert!(refused(&[Some(offered(1, 2, 25, 1))]), "an empty slot");
        assert!(refused(&[Some(offered(1, 0, 25, 11))]), "more than there is");
        assert!(refused(&[Some(Offered { type_id: 2, ..offered(1, 0, 25, 4) })]), "another type id");
        assert!(refused(&[Some(offered(1, 0, 25, 0))]), "nothing");
    }

    fn side(id: u32, money: u32, offer: u32, gives: Vec<(ItemUnit, u8)>, free: usize) -> Prepared {
        let free = (0..free as u8).map(|s| (2, s)).collect();
        Prepared { id, money, offer, gives, free }
    }

    #[test]
    fn the_checks_come_in_the_original_order() {
        let gives = || vec![(item(1, 0, 5, 1), 1)];
        // The other side offers more than it has: 6 for the last, 7 for the other.
        assert_eq!(plan(&side(1, 0, 0, gives(), 0), &side(2, 10, 11, gives(), 0)), Err((6, 7)));
        // The last side's money would pass the maximum.
        assert_eq!(plan(&side(1, u32::MAX, 0, vec![], 5), &side(2, 10, 1, vec![], 5)), Err((6, 7)));
        assert_eq!(plan(&side(1, 10, 11, vec![], 5), &side(2, 0, 0, vec![], 5)), Err((7, 6)));
        assert_eq!(plan(&side(1, 0, 0, vec![], 0), &side(2, 0, 0, gives(), 5)), Err((4, 5)), "no room for the other's item");
        assert_eq!(plan(&side(1, 0, 0, gives(), 5), &side(2, 0, 0, vec![], 0)), Err((5, 4)));
    }

    #[test]
    fn items_and_money_change_hands() {
        let sword = ItemUnit {
            bag: 1,
            slot: 3,
            item_type: 5,
            type_id: 2,
            quality: 300,
            gems: [1, 0, 0, 0, 0, 0],
            count: 1,
            craft_name: b"x".to_vec(),
        };
        let potions = item(1, 0, 25, 10);
        let last = side(1, 100, 30, vec![(sword.clone(), 1)], 3);
        let other = side(2, 50, 20, vec![(potions.clone(), 4)], 3);
        let (l, o, handovers) = plan(&last, &other).unwrap();
        assert_eq!((l.money, o.money), (-10, 10));
        // The other's potions first, part of the stack: a copy into the last's first empty slot.
        assert_eq!(o.given, vec![SlotChange::SetCount { bag: 1, slot: 0, count: 6 }]);
        assert_eq!(l.received, vec![ItemUnit { bag: 2, slot: 0, count: 4, ..potions }]);
        // Then the whole sword.
        assert_eq!(l.given, vec![SlotChange::Remove { bag: 1, slot: 3 }]);
        assert_eq!(o.received, vec![ItemUnit { bag: 2, slot: 0, ..sword }]);
        assert_eq!(
            handovers,
            vec![
                Handover { giver: 2, from: (1, 0), had: 10, count: 4, receiver: 1, to: (2, 0) },
                Handover { giver: 1, from: (1, 3), had: 1, count: 1, receiver: 2, to: (2, 0) },
            ]
        );
        let ops = |p: &[Vec<u8>]| p.iter().map(|p| opcode(p)).collect::<Vec<_>>();
        assert_eq!(ops(&l.packets), [gp::OP_PICK_UP, gp::OP_ITEM_DROP, tp::OP_END]);
        assert_eq!(ops(&o.packets), [gp::OP_ITEM_DROP, gp::OP_PICK_UP, tp::OP_END]);
    }
}
