//! Trades in the zone (`docs/original/trade.md`): finding the partner, the faction check and every packet to the two sides.
//! Part of the zone: `trades` is taken after `maps` and held briefly. The bags are the connections' own: the caller checks an item
//! before offering it and runs the exchange (`crate::trade`).

use super::*;
use crate::sync::Relock;
use crate::trade::{Changed, Ended, Offered, Parked, Stage, Trades};
use shaiya_proto::trade as tp;

type Maps = HashMap<u16, HashMap<u32, Placed>>;

/// Sends to a player wherever it is (the partner is on the same map unless it is leaving it).
fn tell(maps: &Maps, id: u32, packet: &[u8]) {
    if let Some(p) = maps.values().find_map(|players| players.get(&id)) {
        p.send(packet);
    }
}

/// Whether the player's duel stakes window is open (audit R3: a trade and a duel stake window exclude each other).
fn staking(maps: &Maps, id: u32) -> bool {
    maps.values().find_map(|players| players.get(&id)).is_some_and(|p| p.duel.as_ref().is_some_and(|d| d.staking()))
}

/// `CUser::ExchangeCancleReady`: both told neither is ready.
fn tell_unready(maps: &Maps, me: u32, partner: u32) {
    let packet = tp::ready(tp::WHO_BOTH, true);
    tell(maps, me, &packet);
    tell(maps, partner, &packet);
}

/// The ending of a trade as `CUser::CancleActionExc` tells it (section 4).
fn tell_ended(maps: &Maps, me: u32, ended: Ended) {
    match ended.stage {
        Stage::Asking => {}
        Stage::Asked => tell(maps, ended.partner, &tp::start(me, tp::START_REFUSED)),
        Stage::Open | Stage::Exchanging => tell_cancelled(maps, me, ended.partner),
    }
}

/// `0x0A04 2` to the one ending the trade, `3` to its partner.
fn tell_cancelled(maps: &Maps, me: u32, partner: u32) {
    tell(maps, me, &tp::end(tp::END_CANCELLED));
    tell(maps, partner, &tp::end(tp::END_CANCELLED_BY_PARTNER));
}

impl Zones {
    fn trades(&self) -> std::sync::MutexGuard<'_, Trades> {
        self.trades.guard()
    }

    /// `0x0A01` (section 2.1): the target on the requester's map, both free, the same faction unless `any_faction` (a map listed in
    /// `GAME_CROSS_FACTION_TRADE_MAPS`), not oneself (deviation 5).
    pub fn trade_request(&self, map: u16, me: u32, target: u32, any_faction: bool, now: Instant) {
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return };
        let Some(requester) = players.get(&me) else { return };
        let mut trades = self.trades();
        if trades.busy(me) {
            requester.send(&tp::start(target, tp::START_BUSY));
            return;
        }
        let Some(asked) = players.get(&target) else { return };
        let faction_ok = any_faction || asked.occupant.shape.country == requester.occupant.shape.country;
        if target == me || trades.busy(target) || !faction_ok || staking(&maps, me) || staking(&maps, target) {
            requester.send(&tp::start(target, tp::START_BUSY));
            return;
        }
        trades.request(me, target, now);
        asked.send(&tp::request(me));
    }

    /// `0x0A02` (section 2.2): yes opens both windows; no tells the requester with its own id, as the original.
    pub fn trade_answer(&self, me: u32, yes: bool) {
        let maps = self.maps.guard();
        let mut trades = self.trades();
        // A duel stakes window opened meanwhile on either side: the request is cancelled as busy (audit R3).
        let busy = yes && trades.partner(me).is_some_and(|requester| staking(&maps, me) || staking(&maps, requester));
        match trades.answer(me, yes && !busy) {
            crate::trade::Answer::Refused { requester } if busy => tell(&maps, requester, &tp::start(me, tp::START_BUSY)),
            crate::trade::Answer::Accepted { requester } => {
                tell(&maps, me, &tp::start(requester, tp::START_OK));
                tell(&maps, requester, &tp::start(me, tp::START_OK));
            }
            crate::trade::Answer::Refused { requester } => tell(&maps, requester, &tp::start(requester, tp::START_REFUSED)),
            crate::trade::Answer::Ignored | crate::trade::Answer::Stale => {}
        }
    }

    /// `0x0A06` for an item the caller found in its bags as asked (section 2.3): the request echoed, the partner shown the item.
    pub fn trade_offer(&self, me: u32, offer: tp::Offer, item: &w::ItemUnit) -> bool {
        let maps = self.maps.guard();
        let offered = Offered { bag: offer.bag, slot: offer.slot, count: offer.count, item_type: item.item_type, type_id: item.type_id };
        let Some(Changed { partner, unready }) = self.trades().offer(me, offer.trade_slot, offered) else { return false };
        if unready {
            tell_unready(&maps, me, partner);
        }
        tell(&maps, me, &tp::added(&offer));
        tell(&maps, partner, &tp::partner_added(offer.trade_slot, item, offer.count));
        true
    }

    /// `0x0A07` (section 2.4).
    pub fn trade_take_back(&self, me: u32, trade_slot: u8) {
        let maps = self.maps.guard();
        let Some(Changed { partner, unready }) = self.trades().take_back(me, trade_slot) else { return };
        if unready {
            tell_unready(&maps, me, partner);
        }
        tell(&maps, me, &tp::removed(tp::WHO_ME, trade_slot));
        tell(&maps, partner, &tp::removed(tp::WHO_PARTNER, trade_slot));
    }

    /// `0x0A08` with an amount the caller has (section 2.5).
    pub fn trade_money(&self, me: u32, money: u32) {
        let maps = self.maps.guard();
        let Some(Changed { partner, unready }) = self.trades().set_money(me, money) else { return };
        if unready {
            tell_unready(&maps, me, partner);
        }
        tell(&maps, me, &tp::money(tp::WHO_ME, money));
        tell(&maps, partner, &tp::money(tp::WHO_PARTNER, money));
    }

    /// `0x0A05` (section 2.6). Returns the partner when both are ready now: the caller runs the exchange.
    pub fn trade_ready(&self, me: u32, request: u8) -> Option<u32> {
        let maps = self.maps.guard();
        let mut trades = self.trades();
        match request {
            tp::READY => {
                let (partner, both) = trades.ready(me)?;
                tell(&maps, me, &tp::ready(tp::WHO_ME, false));
                tell(&maps, partner, &tp::ready(tp::WHO_PARTNER, false));
                both.then_some(partner)
            }
            tp::NOT_READY => {
                if let Some(partner) = trades.not_ready(me) {
                    tell_unready(&maps, me, partner);
                }
                None
            }
            tp::CANCEL => {
                if let Some(partner) = trades.cancel(me) {
                    tell_cancelled(&maps, me, partner);
                }
                None
            }
            _ => None,
        }
    }

    /// `CUser::CancleActionExc` (section 4): death, item actions, shops, leaving the map or the world. Whether a trade ended.
    /// An open duel stakes window closes too (`docs/original/duel.md` section 6.8).
    pub fn trade_end(&self, me: u32) -> bool {
        let mut maps = self.maps.guard();
        let ended = self.trades().end(me);
        if let Some(ended) = ended {
            tell_ended(&maps, me, ended);
        }
        self.close_duel_window(&mut maps, me, Instant::now());
        ended.is_some()
    }

    /// After a step (`CUser::PacketPC` 0x46a5d1): more than 10 from the partner ends the trade at any stage, `0x0A04 2` and `3`.
    pub fn trade_moved(&self, map: u16, me: u32) {
        let maps = self.maps.guard();
        let mut trades = self.trades();
        let Some(partner) = trades.partner(me) else { return };
        let Some(players) = maps.get(&map) else { return };
        let (Some(a), Some(b)) = (players.get(&me), players.get(&partner)) else { return };
        let (a, b) = (a.occupant.position, b.occupant.position);
        if ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() <= crate::trade::MAX_APART {
            return;
        }
        if trades.end(me).is_some() {
            tell_cancelled(&maps, me, partner);
        }
    }

    /// Requests past their 30 s, silently (`CUser::UpdateTimeover`).
    pub fn expire_trades(&self, now: Instant) {
        self.trades().expire(now);
    }

    /// What an exchanging player offers: the items and the money.
    pub fn trade_offers(&self, me: u32) -> Option<(Vec<Option<Offered>>, u32)> {
        self.trades().exchanging(me).map(|(offers, money)| (offers.to_vec(), money))
    }

    /// Parks the side of the player who pressed ready last and wakes its partner's connection (`ZoneEvent::Trade`). Given back
    /// when the trade ended meanwhile or the partner is gone.
    pub fn trade_park(&self, me: u32, parked: Parked) -> Result<(), Parked> {
        let maps = self.maps.guard();
        let mut trades = self.trades();
        let partner = trades.park(me, parked)?;
        let woken =
            maps.values().find_map(|players| players.get(&partner)).is_some_and(|p| p.occupant.events.send(ZoneEvent::Trade).is_ok());
        if woken {
            return Ok(());
        }
        let parked = trades.take_parked(partner).expect("just parked");
        tell_cancelled(&maps, me, partner);
        Err(parked)
    }

    /// The partner's parked side, with what this player offers, for the exchange (the trade leaves the book).
    pub fn trade_take_parked(&self, me: u32) -> Option<(Parked, Vec<Option<Offered>>, u32)> {
        let mut trades = self.trades();
        let (offers, money) = trades.exchanging(me).map(|(offers, money)| (offers.to_vec(), money))?;
        let parked = trades.take_parked(me)?;
        Some((parked, offers, money))
    }

    /// Takes back a side nobody took in time: both told it was cancelled. False when the partner's connection has it.
    pub fn trade_unpark(&self, me: u32) -> bool {
        let maps = self.maps.guard();
        let Some(partner) = self.trades().unpark(me) else { return false };
        tell_cancelled(&maps, me, partner);
        true
    }

    /// `0x0A04` to a player, from the connection running an exchange for its partner.
    pub fn trade_tell(&self, id: u32, packet: &[u8]) {
        tell(&self.maps.guard(), id, packet);
    }
}
