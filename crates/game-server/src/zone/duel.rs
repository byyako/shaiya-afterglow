//! Duels in the zone (`docs/original/duel.md`): the request and the answer, the stakes window's packets, the countdown and the fight,
//! and every end with the stakes going on the ground. Part of the zone: each player keeps its side (`Placed::duel`); the partner is
//! looked up on the same map, as `CZone::FindUser` does. The bags are the connections' own: they check an item before staking it and
//! take their stakes out themselves (`ZoneEvent::DuelStakes`).

use super::*;
use crate::duel::{self as du, Held, Offered, Side, Stage};
use crate::sync::Relock;
use shaiya_proto::duel as dp;
use shaiya_proto::trade::Offer;

type Players = HashMap<u32, Placed>;

/// How a duel ends without a winner, for the packets each end sends (section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum How {
    /// Hit by someone else, or a death that was not the partner's win (`PvPChkLeaveByAttack`, `PvPChkLeaveByDeath`).
    Interrupted,
    /// Leaving the map (`PvPChkLeaveByZoneLeave`).
    Left,
    /// A step out of the area (`PvPChkLeaveByMove`): only the window, the countdown and the fight.
    OutOfArea,
    /// `0x2406` while fighting.
    GaveUp,
    /// Anything that ends a trade (`CUser::CancleActionExc`): only the window.
    Closed,
}

/// Whether `p` fights a duel with `other` (`CUser::IsTeam`'s first rule, section 5).
pub(super) fn fighting(p: &Placed, other: u32) -> bool {
    p.duel.as_ref().is_some_and(|d| d.stage == Stage::Fighting && d.partner == other)
}

/// Whether `p` fights a duel at all (`CUser::IsTeamUsable`).
pub(super) fn in_fight(p: &Placed) -> bool {
    p.duel.as_ref().is_some_and(|d| d.stage == Stage::Fighting)
}

/// The partner of `id`'s duel, in the zone and pointing back.
fn partner(players: &Players, id: u32) -> Option<u32> {
    let side = players.get(&id)?.duel.as_ref()?;
    players.get(&side.partner)?.duel.as_ref().filter(|s| s.partner == id).map(|_| side.partner)
}

/// Both sides of a duel, `id`'s first.
fn sides(players: &mut Players, id: u32) -> Option<(&mut Side, &mut Side)> {
    let other = partner(players, id)?;
    let [a, b] = players.get_disjoint_mut([&id, &other]);
    Some((a?.duel.as_mut()?, b?.duel.as_mut()?))
}

fn tell(players: &Players, id: u32, packet: &[u8]) {
    if let Some(p) = players.get(&id) {
        p.send(packet);
    }
}

/// A character name in the 21 bytes of the packets (the duel events carry it for the game log).
fn short_name(name: &[u8]) -> [u8; 21] {
    let mut short = [0; 21];
    let len = name.len().min(21);
    short[..len].copy_from_slice(&name[..len]);
    short
}

/// `CUser::PvPModeEnd`: the duel is over for one player (`pvp::mode_end`).
fn mode_end(players: &mut Players, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    p.duel = None;
    pvp::mode_end(players, id);
}

impl Zones {
    /// `CUser::ItemDropPvP`: what came out of `holder`'s bags goes on the ground at its feet ± 0.5 for `recipient`, kind 1, no party.
    fn drop_held(&self, map: u16, players: &Players, holder: u32, recipient: u32, held: Held, now: Instant) {
        let Some(at) = players.get(&holder).map(|p| p.occupant.position) else { return };
        let mut ground = self.ground.guard();
        let ground = ground.entry(map).or_default();
        let mut rand = || rand::random::<u32>() & 0x7fff;
        for loot in held.loot() {
            let position = pvp::drop_spot(at, &mut rand);
            self.place(ground, Some(players), loot, position, recipient, 0, now, gp::KIND_PLAYER_DROP, 0, Dropped::death(holder));
        }
    }

    /// The ends of section 6 without a winner, on `me`: both sides' stakes go back to themselves on the ground at their feet, the duel
    /// ends for both, and the packets of that end go out. With `How::Left` the leaver's stakes are returned instead (deviation 3).
    /// A side whose partner is not in the zone ends quietly. While the stakes come out (`Stage::Taking`, the original's countdown
    /// already) a side whose window its client still shows is told as for the window, the other as for the countdown.
    pub(super) fn end_duel(&self, map: u16, players: &mut Players, me: u32, how: How, now: Instant) -> Option<Held> {
        let stage = players.get(&me)?.duel.as_ref()?.stage;
        let ends = match stage {
            Stage::Asking | Stage::Asked => matches!(how, How::Interrupted | How::Left),
            Stage::Window => matches!(how, How::Interrupted | How::Left | How::OutOfArea | How::Closed),
            Stage::Taking | Stage::Countdown => matches!(how, How::Interrupted | How::Left | How::OutOfArea),
            Stage::Fighting => matches!(how, How::Interrupted | How::Left | How::OutOfArea | How::GaveUp),
        };
        if !ends {
            return None;
        }
        let other = partner(players, me);
        let mut take = |id: u32| players.get_mut(&id).and_then(|p| p.duel.as_mut()).and_then(|d| d.held.take());
        let (mine, theirs) = (take(me), other.and_then(&mut take));
        // Whose client still shows the stakes window (its stakes are not out yet).
        let window = |held: &Option<Held>| stage == Stage::Window || (stage == Stage::Taking && held.is_none());
        let (my_window, their_window) = (window(&mine), window(&theirs));
        if let (Some(other), Some(held)) = (other, theirs) {
            self.drop_held(map, players, other, other, held, now);
        }
        let kept = match mine {
            Some(held) if how == How::Left => Some(held),
            Some(held) => {
                self.drop_held(map, players, me, me, held, now);
                None
            }
            None => None,
        };
        if let Some(other) = other {
            mode_end(players, other);
        }
        mode_end(players, me);
        let Some(other) = other else { return kept };
        if matches!(stage, Stage::Asking | Stage::Asked) {
            let packet = dp::answer(dp::NO_ANSWER, if stage == Stage::Asked { me } else { other });
            tell(players, other, &packet);
            tell(players, me, &packet);
            return kept;
        }
        let reason = match how {
            How::Left => dp::CANCEL_LEFT,
            How::OutOfArea => dp::CANCEL_OUT_OF_AREA,
            How::GaveUp => dp::CANCEL_GAVE_UP,
            How::Interrupted | How::Closed => dp::CANCEL_INTERRUPTED,
        };
        // The game log's `0x0E03` for both sides (they find the other's name here: the leaver is leaving the map).
        let name = |id: u32| players.get(&id).map(|p| short_name(&p.occupant.shape.name)).unwrap_or_default();
        for (id, opponent) in [(me, other), (other, me)] {
            if let Some(p) = players.get(&id) {
                let _ = p.occupant.events.send(ZoneEvent::DuelCancelled { reason, opponent, name: name(opponent) });
            }
        }
        let cancel = dp::cancel(reason, me);
        tell(players, other, &if their_window { dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER) } else { cancel.clone() });
        // The leaver of a countdown or a fight is told nothing: it is leaving the map.
        if my_window {
            tell(players, me, &dp::window_closed(dp::WINDOW_CLOSED_BY_ME));
        } else if how != How::Left {
            tell(players, me, &cancel);
        }
        kept
    }

    /// `0x2401` (section 3.1): the target on the requester's map, both free (no duel, no guild war) and on foot, the same country
    /// unless on map 39 or 40, not oneself (deviation 1), neither with a debuff.
    pub fn duel_request(&self, map: u16, me: u32, target: u32, now: Instant) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(requester) = players.get(&me) else { return };
        if requester.duel.is_some() || requester.war.is_some() || requester.ride.status != Riding::OnFoot {
            return;
        }
        // A trade or a personal shop on either side excludes a duel (audit R3).
        let trading = {
            let trades = self.trades.guard();
            trades.busy(me) || trades.busy(target)
        };
        let fits = !trading
            && requester.shop.is_none()
            && players.get(&target).is_some_and(|t| {
                target != me
                    && t.shop.is_none()
                    && t.duel.is_none()
                    && t.war.is_none()
                    && t.ride.status == Riding::OnFoot
                    && (t.occupant.shape.country == requester.occupant.shape.country || du::ANY_COUNTRY_MAPS.contains(&map))
            });
        if !fits {
            requester.send(&dp::answer(dp::NOT_POSSIBLE, target));
            return;
        }
        if requester.occupant.buffs.has_debuff() || players[&target].occupant.buffs.has_debuff() {
            requester.send(&dp::answer(dp::DEBUFFED, target));
            return;
        }
        let (asking, asked) = Side::request(me, target, now);
        players.get_mut(&me).expect("found").duel = Some(asking);
        players.get_mut(&target).expect("found").duel = Some(asked);
        let packet = dp::request(me, target);
        tell(players, me, &packet);
        tell(players, target, &packet);
    }

    /// `0x2402` (section 3.2): a refusal clears both and tells the requester; an acceptance opens both stakes windows, the area's
    /// centre where the two stand (deviation 2).
    pub fn duel_answer(&self, map: u16, me: u32, accept: bool) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        if players.get(&me).and_then(|p| p.duel.as_ref()).is_none_or(|d| d.stage != Stage::Asked) {
            return;
        }
        let Some(requester) = partner(players, me) else { return };
        // A trade or a personal shop begun since the request: the duel is not possible (audit R3).
        let blocked = accept && {
            let trades = self.trades.guard();
            trades.busy(me) || trades.busy(requester) || players[&me].shop.is_some() || players[&requester].shop.is_some()
        };
        if !accept || blocked {
            players.get_mut(&me).expect("found").duel = None;
            players.get_mut(&requester).expect("found").duel = None;
            tell(players, requester, &dp::answer(if blocked { dp::NOT_POSSIBLE } else { dp::REFUSED }, me));
            return;
        }
        let centre = du::midpoint(players[&me].occupant.position, players[&requester].occupant.position);
        let (mine, theirs) = sides(players, me).expect("both found");
        mine.open(centre);
        theirs.open(centre);
        tell(players, requester, &dp::answer(dp::ACCEPTED, me));
        tell(players, me, &dp::window(requester));
        tell(players, requester, &dp::window(me));
    }

    /// The window's two sides, `me`'s first, when it is open.
    fn window_sides(players: &mut Players, me: u32) -> Option<(u32, &mut Side, &mut Side)> {
        let other = partner(players, me)?;
        let (mine, theirs) = sides(players, me)?;
        (mine.stage == Stage::Window).then_some((other, mine, theirs))
    }

    /// `0x240A` for an item the caller found in its bags as asked (section 4): the request echoed, the partner shown the item.
    pub fn duel_stake(&self, map: u16, me: u32, offer: Offer, item: &w::ItemUnit) -> bool {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return false };
        let Some((other, mine, theirs)) = Self::window_sides(players, me) else { return false };
        let offered = Offered { bag: offer.bag, slot: offer.slot, count: offer.count, item_type: item.item_type, type_id: item.type_id };
        let Some(unready) = du::offer(mine, theirs, offer.trade_slot, offered) else { return false };
        if unready {
            tell_unready(players, me, other);
        }
        tell(players, me, &dp::staked(&offer));
        tell(players, other, &dp::partner_staked(offer.trade_slot, item, offer.count));
        true
    }

    /// `0x240B`.
    pub fn duel_unstake(&self, map: u16, me: u32, stake_slot: u8) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some((other, mine, theirs)) = Self::window_sides(players, me) else { return };
        let Some(unready) = du::take_back(mine, theirs, stake_slot) else { return };
        if unready {
            tell_unready(players, me, other);
        }
        tell(players, me, &dp::unstaked(dp::WHO_ME, stake_slot));
        tell(players, other, &dp::unstaked(dp::WHO_PARTNER, stake_slot));
    }

    /// A side's stakes could not be taken as staked (an item gone or smaller, the gold short; audit R3): the duel ends for both, the
    /// stakes window closed on both screens, and whatever the partner's connection took already goes on the ground at its feet.
    pub fn duel_stakes_failed(&self, map: u16, me: u32, now: Instant) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        self.end_duel(map, players, me, How::Interrupted, now);
    }

    /// `0x240C` with an amount the caller has.
    pub fn duel_money(&self, map: u16, me: u32, money: u32) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some((other, mine, theirs)) = Self::window_sides(players, me) else { return };
        if du::set_money(mine, theirs, money) {
            tell_unready(players, me, other);
        }
        tell(players, me, &dp::stake_money(dp::WHO_ME, money));
        tell(players, other, &dp::stake_money(dp::WHO_PARTNER, money));
    }

    /// `0x2409` (section 4). True when both are ready now: the caller takes its stakes out, the partner's connection is woken to take
    /// its own (`ZoneEvent::DuelStakes`).
    pub fn duel_ready(&self, map: u16, me: u32, request: u8) -> bool {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return false };
        let Some((other, mine, theirs)) = Self::window_sides(players, me) else { return false };
        match request {
            dp::READY => {
                let both = du::ready(mine, theirs);
                tell(players, me, &dp::stake_ready(dp::WHO_ME, false));
                tell(players, other, &dp::stake_ready(dp::WHO_PARTNER, false));
                if both {
                    let _ = players[&other].occupant.events.send(ZoneEvent::DuelStakes);
                }
                both
            }
            dp::NOT_READY => {
                if du::unready(mine, theirs) {
                    tell_unready(players, me, other);
                }
                false
            }
            dp::CLOSE => {
                tell(players, me, &dp::window_closed(dp::WINDOW_CLOSED_BY_ME));
                tell(players, other, &dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER));
                mode_end(players, other);
                mode_end(players, me);
                false
            }
            _ => false,
        }
    }

    /// The other side of a player's duel, with its name (the game log's duel records).
    pub fn duel_partner(&self, map: u16, me: u32) -> Option<(u32, String)> {
        let maps = self.maps();
        let players = maps.get(&map)?;
        let other = partner(players, me)?;
        Some((other, String::from_utf8_lossy(&players[&other].occupant.shape.name).trim_end_matches('\0').to_owned()))
    }

    /// What a player taking its stakes out staked: the items and the money.
    pub fn duel_offers(&self, map: u16, me: u32) -> Option<(Vec<Option<Offered>>, u32)> {
        let maps = self.maps();
        let side = maps.get(&map)?.get(&me)?.duel.as_ref()?;
        (side.stage == Stage::Taking && side.held.is_none()).then(|| (side.offers.to_vec(), side.money))
    }

    /// A side's stakes are out of its bags (`ExchangePvP`). When the partner's are too, the countdown begins at the two players'
    /// midpoint: `0x2403` to both, 5 s. When the duel ended meanwhile they go back to it on the ground at its feet, as the original's
    /// ends do; when the player is no longer in the zone they are returned for its bags.
    pub fn duel_staked(&self, map: u16, me: u32, held: Held, now: Instant) -> Option<Held> {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map).filter(|players| players.contains_key(&me)) else { return Some(held) };
        let taking = partner(players, me).filter(|_| players[&me].duel.as_ref().is_some_and(|d| d.stage == Stage::Taking));
        let Some(other) = taking else {
            if held != Held::default() {
                self.drop_held(map, players, me, me, held, now);
            }
            return None;
        };
        let centre = du::midpoint(players[&me].occupant.position, players[&other].occupant.position);
        let (mine, theirs) = sides(players, me).expect("both found");
        mine.held = Some(held);
        // The countdown waits for the partner's stakes.
        theirs.held.as_ref()?;
        du::countdown(mine, centre, now);
        du::countdown(theirs, centre, now);
        let packet = dp::ready(centre[0], centre[2]);
        tell(players, me, &packet);
        tell(players, other, &packet);
        None
    }

    /// `0x2406` (section 6.3): only while fighting; both get their stakes back, no winner.
    pub fn duel_give_up(&self, map: u16, me: u32, now: Instant) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        self.end_duel(map, players, me, How::GaveUp, now);
    }

    /// After a step (section 6.4): more than 40 from the centre ends the window, the countdown or the fight.
    pub fn duel_moved(&self, map: u16, me: u32, now: Instant) {
        let mut maps = self.maps();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get(&me) else { return };
        let out = p
            .duel
            .as_ref()
            .is_some_and(|d| d.stage != Stage::Asking && d.stage != Stage::Asked && du::out_of_area(d.centre, p.occupant.position));
        if out {
            self.end_duel(map, players, me, How::OutOfArea, now);
        }
    }

    /// Every second (`CUser::PvPChkLeaveByTime`, section 5): a request without an answer ends, a countdown over starts the fight.
    pub fn duel_timers(&self, now: Instant) {
        let mut maps = self.maps();
        for players in maps.values_mut() {
            let due: Vec<u32> = players.values().filter(|p| p.duel.as_ref().is_some_and(|d| d.due(now))).map(|p| p.occupant.id).collect();
            for id in due {
                let Some(stage) = players.get(&id).and_then(|p| p.duel.as_ref()).filter(|d| d.due(now)).map(|d| d.stage) else { continue };
                let Some(other) = partner(players, id) else {
                    mode_end(players, id);
                    continue;
                };
                match stage {
                    Stage::Asking | Stage::Asked => {
                        let asked = if stage == Stage::Asked { id } else { other };
                        mode_end(players, id);
                        mode_end(players, other);
                        let packet = dp::answer(dp::NO_ANSWER, asked);
                        tell(players, other, &packet);
                        tell(players, id, &packet);
                    }
                    Stage::Countdown => {
                        for side in [id, other] {
                            let p = players.get_mut(&side).expect("found");
                            p.harm.clear();
                            let d = p.duel.as_mut().expect("in a duel");
                            d.stage = Stage::Fighting;
                            d.deadline = None;
                        }
                        let packet = dp::start();
                        tell(players, other, &packet);
                        tell(players, id, &packet);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Ends a stakes window as `CUser::CancleActionExc` does (section 6.8), wherever the player is.
    pub(super) fn close_duel_window(&self, maps: &mut HashMap<u16, Players>, me: u32, now: Instant) {
        let Some((&map, players)) = maps.iter_mut().find(|(_, players)| players.contains_key(&me)) else { return };
        self.end_duel(map, players, me, How::Closed, now);
    }
}

/// `CUser::ExchangeCancleReadyPvP`: both told neither is ready.
fn tell_unready(players: &Players, me: u32, partner: u32) {
    let packet = dp::stake_ready(dp::WHO_BOTH, true);
    tell(players, me, &packet);
    tell(players, partner, &packet);
}

/// After a hit broke the target's cast (section 6.5): a monster's hit, or a player's other than the duel partner, ends the target's
/// duel at any stage.
pub(super) fn hit(zones: &Zones, map: u16, players: &mut Players, target: u32, attacker: Option<u32>, now: Instant) {
    let Some(side) = players.get(&target).and_then(|p| p.duel.as_ref()) else { return };
    if attacker.is_some_and(|a| a == side.partner) {
        return;
    }
    zones.end_duel(map, players, target, How::Interrupted, now);
}

/// A duel won (`CUser::PvPChkLoseDeath(4)`, section 6.1): the winner's own stakes and the loser's go on the ground for the winner,
/// the duel ends for both, the victory and the defeat are counted (`ZoneEvent::DuelResult`) and `0x2405` tells each.
pub(super) fn won(zones: &Zones, map: u16, players: &mut Players, winner: u32, loser: u32, now: Instant) {
    let mut take = |id: u32| players.get_mut(&id).and_then(|p| p.duel.as_mut()).and_then(|d| d.held.take());
    let (own, lost) = (take(winner), take(loser));
    if let Some(held) = own {
        zones.drop_held(map, players, winner, winner, held, now);
    }
    if let Some(held) = lost {
        zones.drop_held(map, players, loser, winner, held, now);
    }
    mode_end(players, loser);
    mode_end(players, winner);
    let name = |id: u32| players.get(&id).map(|p| short_name(&p.occupant.shape.name)).unwrap_or_default();
    let (winner_name, loser_name) = (name(winner), name(loser));
    for (id, won, opponent, name) in [(winner, true, loser, loser_name), (loser, false, winner, winner_name)] {
        if let Some(p) = players.get(&id) {
            let _ = p.occupant.events.send(ZoneEvent::DuelResult { won, opponent, name });
            p.send(&dp::result(if won { dp::WON } else { dp::LOST }));
        }
    }
}

/// The duel partner a death hands the win to (section 6.1), with `CUser::Death`'s flag from the desperate counterattack: the killer
/// of record when the victim fights it; with the flag also the victim's partner when the victim killed itself. `None` otherwise.
pub(super) fn winner(players: &Players, victim: u32, (kind, killer): (u8, u32), flag: bool) -> Option<u32> {
    if kind != cp::KILLER_PLAYER {
        return None;
    }
    let side = players.get(&victim)?.duel.as_ref().filter(|d| d.stage == Stage::Fighting)?;
    let partner = side.partner;
    if !players.contains_key(&partner) {
        return None;
    }
    let mutual = || players.get(&killer).is_some_and(|k| fighting(k, victim));
    match (flag, killer == victim) {
        (true, true) => Some(partner),
        (true, false) => (killer == partner && mutual()).then_some(partner),
        (false, false) => (killer == partner).then_some(partner),
        (false, true) => None,
    }
}
