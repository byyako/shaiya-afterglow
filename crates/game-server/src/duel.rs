//! Duels (`docs/original/duel.md`): each side's stage, the stakes window and what came out of the bags for the fight. Bookkeeping
//! only: the zone keeps a side on each player and sends the packets; each connection checks and takes its own stakes.

use std::time::{Duration, Instant};

use shaiya_proto::duel as dp;

use crate::backend::{ItemUnit, Kept};
use crate::drops::Loot;
use crate::inventory::Inventory;
pub use crate::trade::Offered;

/// How long a request waits for its answer (`CUser::PacketPVP`, 10000 ms).
pub const REQUEST_TIME: Duration = Duration::from_secs(10);
/// The countdown before the fight (5000 ms).
pub const COUNTDOWN: Duration = Duration::from_secs(5);
/// A duellist further than this from the centre ends the duel (`CUser::PvPChkLeaveByMove`, x and z).
pub const AREA: f32 = 40.0;
/// Maps where the two countries may duel (`m_wMap` 39 and 40).
pub const ANY_COUNTRY_MAPS: [u16; 2] = [39, 40];
const SLOTS: usize = dp::SLOTS as usize;

/// Where a player is in a duel (the original's statuses; `Taking` is the stakes window's state 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// It asked the partner (1).
    Asking,
    /// The partner asked it (2).
    Asked,
    /// The stakes window is open (5).
    Window,
    /// Both are ready; the stakes are coming out of the bags (5, window state 4).
    Taking,
    /// The five seconds before the fight (3).
    Countdown,
    /// Fighting (4).
    Fighting,
}

/// What came out of a side's bags for the fight (`CExchangePvP` +0x15B8, +0x15BC).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Held {
    pub money: u32,
    pub items: Vec<Loot>,
}

impl Held {
    /// The ground loot, gold first (`CUser::ItemDropPvP`).
    pub fn loot(self) -> impl Iterator<Item = Loot> {
        (self.money > 0).then_some(Loot::Gold(self.money)).into_iter().chain(self.items)
    }
}

/// One player's side of a duel.
#[derive(Clone, Debug, PartialEq)]
pub struct Side {
    pub partner: u32,
    pub stage: Stage,
    pub deadline: Option<Instant>,
    /// The area's centre: where the two stood when the request was accepted (deviation 2), then when the countdown began.
    pub centre: [f32; 3],
    pub offers: [Option<Offered>; SLOTS],
    pub money: u32,
    pub ready: bool,
    /// Its stakes once out of the bags; `None` before (and when the window closed without a fight).
    pub held: Option<Held>,
}

impl Side {
    /// The two sides of a request (`0x2401`, section 3.1 step 4): both wait 10 s for the answer.
    pub fn request(requester: u32, target: u32, now: Instant) -> (Side, Side) {
        let side = |partner, stage| Side {
            partner,
            stage,
            deadline: Some(now + REQUEST_TIME),
            centre: [0.0; 3],
            offers: [None; SLOTS],
            money: 0,
            ready: false,
            held: None,
        };
        (side(target, Stage::Asking), side(requester, Stage::Asked))
    }

    /// The stakes window opens (`0x2402` accepted, section 3.2): nothing staked, no deadline.
    pub fn open(&mut self, centre: [f32; 3]) {
        self.stage = Stage::Window;
        self.deadline = None;
        self.centre = centre;
        self.offers = [None; SLOTS];
        self.money = 0;
        self.ready = false;
        self.held = None;
    }

    /// The original's status (`m_nPvPStatus`).
    pub fn status(&self) -> u8 {
        match self.stage {
            Stage::Asking => 1,
            Stage::Asked => 2,
            Stage::Window | Stage::Taking => 5,
            Stage::Countdown => 3,
            Stage::Fighting => 4,
        }
    }

    /// The stakes window is open or its stakes are coming out: no trade may be open with it (audit R3).
    pub fn staking(&self) -> bool {
        matches!(self.stage, Stage::Window | Stage::Taking)
    }

    pub fn due(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|d| d <= now)
    }
}

/// Victories and defeats stop below 65535 (`CUser::PvPChkLoseDeath`).
pub fn counted(count: u32) -> Option<u32> {
    (count < u32::from(u16::MAX)).then_some(count + 1)
}

/// The midpoint of two positions (`(a + b) / 2`, all three coordinates).
pub fn midpoint(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0]
}

/// More than 40 from the centre, x and z (`SVector::Distance2`).
pub fn out_of_area(centre: [f32; 3], at: [f32; 3]) -> bool {
    ((centre[0] - at[0]).powi(2) + (centre[2] - at[2]).powi(2)).sqrt() > AREA
}

/// `CUser::ExchangeCancleReadyPvP`: whether either was ready (neither is now).
pub fn unready(me: &mut Side, partner: &mut Side) -> bool {
    let was = me.ready || partner.ready;
    (me.ready, partner.ready) = (false, false);
    was
}

/// `0x240A` once the caller checked the item (section 4): into an empty stake slot, the place not staked already. Whether the
/// ready flags were cleared first; `None` when refused.
pub fn offer(me: &mut Side, partner: &mut Side, stake_slot: u8, offered: Offered) -> Option<bool> {
    let slot = usize::from(stake_slot);
    if slot >= SLOTS || me.offers[slot].is_some() || me.offers.iter().flatten().any(|o| (o.bag, o.slot) == (offered.bag, offered.slot)) {
        return None;
    }
    let was = unready(me, partner);
    me.offers[slot] = Some(offered);
    Some(was)
}

/// `0x240B`: whether the ready flags were cleared; `None` when the slot is empty.
pub fn take_back(me: &mut Side, partner: &mut Side, stake_slot: u8) -> Option<bool> {
    me.offers.get(usize::from(stake_slot)).copied().flatten()?;
    let was = unready(me, partner);
    me.offers[usize::from(stake_slot)] = None;
    Some(was)
}

/// `0x240C` once the caller checked the amount: whether the ready flags were cleared.
pub fn set_money(me: &mut Side, partner: &mut Side, money: u32) -> bool {
    let was = unready(me, partner);
    me.money = money;
    was
}

/// `0x2409 0`: whether both are ready now; then both are taking their stakes out (once: the step is taken under the zone's lock).
pub fn ready(me: &mut Side, partner: &mut Side) -> bool {
    me.ready = true;
    let both = partner.ready;
    if both {
        me.stage = Stage::Taking;
        partner.stage = Stage::Taking;
    }
    both
}

/// The countdown begins for one side (section 4, last step).
pub fn countdown(side: &mut Side, centre: [f32; 3], now: Instant) {
    side.stage = Stage::Countdown;
    side.centre = centre;
    side.deadline = Some(now + COUNTDOWN);
}

/// A side's stakes as its bags are now, with the count staked. `None` when any stake is gone or no longer what it was (audit R3,
/// deviation 5): the stake window is then closed for both and nothing is taken.
pub fn prepare(offers: &[Option<Offered>], inventory: &Inventory) -> Option<Vec<(ItemUnit, u8)>> {
    crate::trade::prepare(0, 0, offers, 0, inventory).map(|side| side.gives)
}

/// The stakes as `ItemStore::take_stakes` takes them: `(bag, slot, had, count)`.
pub fn taken(gives: &[(ItemUnit, u8)]) -> Vec<(u8, u8, u8, u8)> {
    gives.iter().map(|(item, count)| (item.bag, item.slot, item.count, *count)).collect()
}

/// What came out, from the stakes and what the store kept of each (`ExchangePvP`): a whole item keeps itself; part of a stack is a new
/// item with the count staked and the giver's quality, gems and craft name (trade deviation 3).
pub fn held(money: u32, gives: &[(ItemUnit, u8)], kept: Vec<Kept>) -> Held {
    let items = gives
        .iter()
        .zip(kept)
        .map(|((item, count), kept)| Loot::Item {
            item_type: item.item_type,
            type_id: item.type_id,
            count: *count,
            quality: item.quality,
            kept: Some(kept),
        })
        .collect();
    Held { money, items }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(bag: u8, slot: u8) -> Offered {
        Offered { bag, slot, count: 1, item_type: 25, type_id: 1 }
    }

    fn window() -> (Side, Side) {
        let (mut a, mut b) = Side::request(1, 2, Instant::now());
        a.open([10.0, 0.0, 10.0]);
        b.open([10.0, 0.0, 10.0]);
        (a, b)
    }

    #[test]
    fn a_request_waits_ten_seconds_on_both_sides() {
        let now = Instant::now();
        let (a, b) = Side::request(1, 2, now);
        assert_eq!((a.partner, a.stage, a.status()), (2, Stage::Asking, 1));
        assert_eq!((b.partner, b.stage, b.status()), (1, Stage::Asked, 2));
        assert!(!a.due(now + Duration::from_secs(9)) && a.due(now + REQUEST_TIME) && b.due(now + REQUEST_TIME));
    }

    #[test]
    fn the_window_opens_empty_with_its_centre() {
        let (a, _) = window();
        assert_eq!((a.partner, a.stage, a.status(), a.deadline, a.centre), (2, Stage::Window, 5, None, [10.0, 0.0, 10.0]));
        assert!(a.offers.iter().all(Option::is_none) && a.held.is_none());
    }

    #[test]
    fn every_change_clears_both_ready_flags_first() {
        let (mut a, mut b) = window();
        assert_eq!(offer(&mut a, &mut b, 0, offered(1, 0)), Some(false));
        assert_eq!(offer(&mut a, &mut b, 0, offered(1, 1)), None, "stake slot taken");
        assert_eq!(offer(&mut a, &mut b, 1, offered(1, 0)), None, "the same bag slot twice");
        assert_eq!(offer(&mut a, &mut b, 8, offered(1, 2)), None, "only 8 stake slots");
        assert!(!ready(&mut b, &mut a));
        assert!(set_money(&mut a, &mut b, 100), "b was ready");
        assert!(!b.ready);
        assert!(!ready(&mut b, &mut a));
        assert_eq!(take_back(&mut a, &mut b, 0), Some(true));
        assert_eq!(take_back(&mut a, &mut b, 0), None);
        assert!(!unready(&mut a, &mut b), "nobody was ready");
    }

    #[test]
    fn both_ready_takes_the_stakes_once() {
        let (mut a, mut b) = window();
        assert!(!ready(&mut a, &mut b));
        assert!(ready(&mut b, &mut a));
        assert_eq!((a.stage, b.stage), (Stage::Taking, Stage::Taking));
        assert_eq!(a.status(), 5, "still the window's status while the stakes come out");
        let now = Instant::now();
        countdown(&mut a, [1.0, 2.0, 3.0], now);
        assert_eq!((a.stage, a.status(), a.centre, a.deadline), (Stage::Countdown, 3, [1.0, 2.0, 3.0], Some(now + COUNTDOWN)));
    }

    #[test]
    fn the_area_is_forty_across_x_and_z() {
        assert_eq!(midpoint([0.0, 2.0, 0.0], [10.0, 4.0, 20.0]), [5.0, 3.0, 10.0]);
        assert!(!out_of_area([0.0, 0.0, 0.0], [40.0, 100.0, 0.0]), "y does not count");
        assert!(out_of_area([0.0, 0.0, 0.0], [30.0, 0.0, 30.0]));
    }

    #[test]
    fn stakes_refuse_what_changed_and_keep_what_they_are() {
        let potions = ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 10, quality: 0, ..Default::default() };
        let sword =
            ItemUnit { bag: 1, slot: 1, item_type: 5, type_id: 2, count: 1, quality: 300, gems: [1, 0, 0, 0, 0, 0], ..Default::default() };
        let inventory = Inventory::new(&[potions.clone(), sword.clone()]);
        let offers = [
            Some(Offered { bag: 1, slot: 0, count: 4, item_type: 25, type_id: 1 }),
            Some(Offered { bag: 1, slot: 1, count: 1, item_type: 6, type_id: 2 }),
            Some(Offered { bag: 1, slot: 1, count: 1, item_type: 5, type_id: 2 }),
            Some(Offered { bag: 1, slot: 2, count: 1, item_type: 5, type_id: 2 }),
        ];
        assert_eq!(prepare(&offers, &inventory), None, "another item now and an empty slot refuse the whole side");
        let gives = prepare(&[offers[0], offers[2]], &inventory).expect("both still there");
        assert_eq!(gives, vec![(potions.clone(), 4), (sword.clone(), 1)]);
        assert_eq!(taken(&gives), vec![(1, 0, 10, 4), (1, 1, 1, 1)]);
        let part = Kept { make_type: b'T', ..Kept::SPLIT };
        let whole = Kept { gems: sword.gems, origin: Some((7, 8)), ..Kept::SPLIT };
        let held = held(50, &gives, vec![part, whole]);
        assert_eq!(
            held.clone().loot().collect::<Vec<_>>(),
            vec![
                Loot::Gold(50),
                Loot::Item { item_type: 25, type_id: 1, count: 4, quality: 0, kept: Some(part) },
                Loot::Item { item_type: 5, type_id: 2, count: 1, quality: 300, kept: Some(whole) },
            ]
        );
        assert_eq!(Held { money: 0, items: Vec::new() }.loot().count(), 0, "no gold item without gold");
    }
}
