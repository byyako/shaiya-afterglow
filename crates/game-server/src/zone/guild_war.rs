//! Guild wars in the zone (`docs/original/guild-war.md`): the request and the answer, who takes part, the countdown and the fight,
//! and every way out of a war. Part of the zone: `wars` is taken after `maps` and `guilds`, held briefly; each player in a war
//! carries its part (`Placed::war`). A war lives on one map: its officers and players are all there, as the original looks them up
//! in the player's own zone.

use super::*;
use crate::guild_war::{self as gw, Part, Stage, War, Wars};
use crate::sync::Relock;
use shaiya_proto::guild_war as wp;

type Players = HashMap<u32, Placed>;

fn tell(players: &Players, id: u32, packet: &[u8]) {
    if let Some(p) = players.get(&id) {
        p.send(packet);
    }
}

fn tell_all(players: &Players, ids: &[u32], packet: &[u8]) {
    for &id in ids {
        tell(players, id, packet);
    }
}

/// `CUser::PvPGModeEnd`: the war is over for one player.
fn mode_end(players: &mut Players, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    p.war = None;
    pvp::mode_end(players, id);
}

/// A request ends without an answer, or its officer died, left the map or the guild (sections 5 and 6): both officers' war ends
/// and both are told `0x2502 3` naming the asked one.
fn cancel_request(players: &mut Players, wars: &mut Wars, guild: u32) {
    let Some((war, _)) = wars.end(guild) else { return };
    let asked = if war.stage == Stage::Asked { war.officer } else { war.other_officer };
    mode_end(players, war.officer);
    mode_end(players, war.other_officer);
    let packet = wp::answer(wp::NO_ANSWER, asked);
    tell(players, war.other_officer, &packet);
    tell(players, war.officer, &packet);
}

/// One side's end (section 6.1): the result to everyone who took part, wherever it is now, and the war over for its players still
/// in it (deviation 4: not the guild's other members).
fn finish(players: &mut Players, war: &War, result: u8) {
    let packet = wp::result(result);
    for (_, outbox) in &war.joined {
        let _ = outbox.send(packet.clone());
    }
    for &id in &war.fighters {
        mode_end(players, id);
    }
}

/// `CGuild::PvPUserLeave` (section 6.1) for a player in a war's countdown or fight: `0x2506` to both sides' players, the player
/// itself included; its guild loses when it was the last one, or its own officer leaving (any event but a defeat), and then the
/// enemy guild gets `0x2505 1`, its own `0x2505 2`.
fn leave(players: &mut Players, wars: &mut Wars, id: u32, event: u8) {
    let Some(guild) = players.get(&id).map(|p| p.occupant.guild_id) else { return };
    let Some(war) = wars.get_mut(guild) else {
        mode_end(players, id);
        return;
    };
    war.fighters.retain(|&f| f != id);
    let lost = war.fighters.is_empty() || (id == war.officer && event != wp::DEFEATED);
    let enemy = war.enemy;
    let packet = wp::left(event, id);
    if let Some(other) = wars.get(enemy) {
        tell_all(players, &other.fighters, &packet);
    }
    if let Some(war) = wars.get(guild) {
        tell_all(players, &war.fighters, &packet);
    }
    tell(players, id, &packet);
    mode_end(players, id);
    if !lost {
        return;
    }
    let Some((war, other)) = wars.end(guild) else { return };
    tracing::info!(winner = enemy, loser = guild, "guild war over");
    if let Some(other) = other {
        finish(players, &other, wp::WON);
    }
    finish(players, &war, wp::LOST);
}

/// A player leaves its war (map, world, guild, death): a defeat or an exit with `event` from a countdown or fight, the end of a
/// request for its officer.
fn out(players: &mut Players, wars: &mut Wars, id: u32, event: u8) {
    let Some((part, guild)) = players.get(&id).and_then(|p| p.war.map(|w| (w, p.occupant.guild_id))) else { return };
    if part.ready {
        leave(players, wars, id, event);
    } else {
        cancel_request(players, wars, guild);
    }
}

/// Whether a player's death is a guild war defeat (`CUser::PvPGetDeathType` 5, `CUser::Death`'s flag; section 6.2): a player in the
/// war killed by an enemy in it, or with the desperate counterattack's flag by a player of record in the zone (itself included).
pub(super) fn defeat(players: &Players, victim: u32, (kind, killer): (u8, u32), flag: bool) -> bool {
    if kind != cp::KILLER_PLAYER {
        return false;
    }
    let Some(v) = players.get(&victim).filter(|v| v.war.is_some_and(|w| w.ready)) else { return false };
    if flag {
        return killer == victim || players.contains_key(&killer);
    }
    killer != victim
        && players.get(&killer).is_some_and(|k| gw::enemies((k.occupant.guild_id, k.war.as_ref()), (v.occupant.guild_id, v.war.as_ref())))
}

impl Zones {
    fn wars(&self) -> std::sync::MutexGuard<'_, Wars> {
        self.wars.guard()
    }

    /// `0x2501` (section 3): the requester ranked 1 to 4, in no duel, its guild at peace; the target on its map, ranked 1 to 4 in
    /// another guild at peace, in no duel. Without a guild nothing at all.
    pub fn war_request(&self, map: u16, me: u32, target: u32, now: Instant) {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(requester) = players.get(&me) else { return };
        let guild = requester.occupant.guild_id;
        let (my_rank, their_rank) = {
            let guilds = self.guilds.guard();
            (guilds.rank_of(me), guilds.rank_of(target))
        };
        let mut wars = self.wars();
        if guild == 0 || !my_rank.is_some_and(gw::may_lead) || wars.at_war(guild) || requester.duel.is_some() {
            return;
        }
        let other = players.get(&target).map_or(0, |t| t.occupant.guild_id);
        let fits = players.get(&target).is_some_and(|t| {
            other != 0 && other != guild && their_rank.is_some_and(gw::may_lead) && t.duel.is_none() && !wars.at_war(other)
        });
        if !fits {
            requester.send(&wp::answer(wp::NOT_POSSIBLE, target));
            return;
        }
        wars.request(map, (guild, me), (other, target), now);
        for (id, enemy) in [(me, other), (target, guild)] {
            players.get_mut(&id).expect("found").war = Some(Part { enemy, centre: [0.0; 3], ready: false });
        }
        let packet = wp::request(me, target);
        tell(players, me, &packet);
        tell(players, target, &packet);
    }

    /// `0x2502` (section 4) from the asked player (deviation 2), the requester on its map: a refusal ends the request; an acceptance
    /// starts the countdown with every member of each guild on the map closer than 40 to the officers' midpoint, unless the two stand
    /// 80 or more apart (deviation 1).
    pub fn war_answer(&self, map: u16, me: u32, accept: bool, now: Instant) {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(guild) = players.get(&me).map(|p| p.occupant.guild_id) else { return };
        // Every use of `wars` holds `maps`: nothing changes between the two takes.
        let asked = self.wars().get(guild).filter(|w| w.stage == Stage::Asked && w.officer == me).map(|w| (w.other_officer, w.enemy));
        let Some((requester, enemy)) = asked else { return };
        let ranks = {
            let guilds = self.guilds.guard();
            [guilds.rank_of(me), guilds.rank_of(requester)]
        };
        let mut wars = self.wars();
        if players.get(&requester).is_none_or(|r| r.occupant.guild_id != enemy) || !ranks.iter().all(|r| r.is_some_and(gw::may_lead)) {
            return;
        }
        let (mine, theirs) = (players[&me].occupant.position, players[&requester].occupant.position);
        if !accept || gw::too_far_apart(mine, theirs) {
            wars.end(guild);
            for id in [me, requester] {
                players.get_mut(&id).expect("found").war = None;
            }
            if accept {
                tell(players, requester, &wp::answer(wp::NOT_POSSIBLE, me));
                tell(players, me, &wp::answer(wp::NO_ANSWER, me));
            } else {
                tell(players, requester, &wp::answer(wp::REFUSED, me));
            }
            return;
        }
        // `CGuild::PvPUserSet`, the answerer's guild first.
        let centre = crate::duel::midpoint(mine, theirs);
        for (g, other) in [(guild, enemy), (enemy, guild)] {
            let fighters: Vec<u32> = players
                .values()
                .filter(|p| p.occupant.guild_id == g && gw::in_area(centre, p.occupant.position))
                .map(|p| p.occupant.id)
                .collect();
            for p in players.values_mut().filter(|p| p.occupant.guild_id == g) {
                p.war = fighters.contains(&p.occupant.id).then_some(Part { enemy: other, centre, ready: true });
                if p.war.is_some() {
                    p.harm.clear();
                }
            }
            let joined = fighters.iter().map(|id| (*id, players[id].occupant.outbox.clone())).collect();
            let w = wars.get_mut(g).expect("at war");
            (w.stage, w.map, w.centre, w.deadline) = (Stage::Countdown, map, centre, Some(now + gw::COUNTDOWN));
            (w.fighters, w.joined) = (fighters, joined);
        }
        tracing::info!(guild, enemy, ?centre, "guild war accepted");
        tell(players, requester, &wp::answer(wp::ACCEPTED, me));
        for (g, other) in [(guild, enemy), (enemy, guild)] {
            tell_all(players, &wars.get(g).expect("at war").fighters, &wp::ready(centre[0], centre[2], other));
        }
    }

    /// Every second (`CUser::PvPChkLeaveByTime`, section 5): a request without an answer ends, a countdown over starts the fight.
    pub fn war_timers(&self, now: Instant) {
        let mut maps = self.maps.guard();
        let mut wars = self.wars();
        for guild in wars.due(now) {
            let Some(war) = wars.get(guild) else { continue };
            let (map, enemy, stage) = (war.map, war.enemy, war.stage);
            let mut nobody = Players::new();
            let players = maps.get_mut(&map).unwrap_or(&mut nobody);
            match stage {
                Stage::Asking | Stage::Asked => cancel_request(players, &mut wars, guild),
                Stage::Countdown => {
                    for g in [guild, enemy] {
                        if let Some(w) = wars.get_mut(g) {
                            (w.stage, w.deadline) = (Stage::Fighting, None);
                            tell_all(players, &w.fighters, &wp::start());
                        }
                    }
                }
                Stage::Fighting => {}
            }
        }
    }

    /// After a step (`CUser::PvPChkLeaveByMove`, section 6.3): more than 40 from the centre takes a player out of the countdown or
    /// the fight; the officers are checked from the fight on.
    pub fn war_moved(&self, map: u16, me: u32) {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get(&me) else { return };
        let out = p.war.is_some_and(|w| w.ready && gw::out_of_area(w.centre, p.occupant.position));
        if !out {
            return;
        }
        let guild = p.occupant.guild_id;
        let mut wars = self.wars();
        if wars.get(guild).is_some_and(|w| w.stage == Stage::Countdown && w.officer == me) {
            return;
        }
        leave(players, &mut wars, me, wp::OUT_OF_AREA);
    }

    /// Any death of a player in a war (`CUser::PvPChkLeaveByDeath`, `PvPChkLoseDeath(5)`, section 6.2): defeated, or its request
    /// cancelled (deviations 3 and 7).
    pub(super) fn war_died(&self, players: &mut Players, victim: u32) {
        if players.get(&victim).is_some_and(|p| p.war.is_some()) {
            out(players, &mut self.wars(), victim, wp::DEFEATED);
        }
    }

    /// Leaving the map (`CUser::PvPChkLeaveByZoneLeave`, section 6.4) with `event`, the player still in `players`.
    pub(super) fn war_left(&self, players: &mut Players, id: u32, event: u8) {
        if players.get(&id).is_some_and(|p| p.war.is_some()) {
            out(players, &mut self.wars(), id, event);
        }
    }

    /// Leaving the world: `0x2506 0` "disconnected" (deviation 5), before the player leaves its map.
    pub fn war_left_world(&self, map: u16, id: u32) {
        let mut maps = self.maps.guard();
        if let Some(players) = maps.get_mut(&map) {
            self.war_left(players, id, wp::LEFT_WORLD);
        }
    }

    /// Players about to leave their guild (`CUser::PvPChkLeaveByGuildLeave`, section 6.5): each leaves its war while still a member
    /// (deviation 6).
    pub(super) fn war_guild_left(&self, maps: &mut HashMap<u16, Players>, ids: &[u32]) {
        for &id in ids {
            if let Some(players) = maps.values_mut().find(|players| players.get(&id).is_some_and(|p| p.war.is_some())) {
                self.war_left(players, id, wp::OUT_OF_AREA);
            }
        }
    }
}
