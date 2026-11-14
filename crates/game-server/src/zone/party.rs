//! Parties in the zone (`docs/original/party.md`): invitations, joining, leaving, kicking, the leader, the member units and status,
//! party chat and what a kill shares; and unions (`docs/original/union.md`): making one, its settings, `/join`, the sub-leader, group
//! moves, dismantling. Part of the zone: `parties` is taken after `maps` and `mobs` and held briefly.

use super::*;
use crate::party::{self as pt, Departure, Parties, Shown};
use crate::sync::Relock;
use shaiya_proto::party as pp;

/// Whom an invitation is for: `0x0B02` names an id, `0x0B09` a name.
#[derive(Clone, Copy, Debug)]
pub enum Invitee<'a> {
    Id(u32),
    Name(&'a [u8]),
}

type Maps = HashMap<u16, HashMap<u32, Placed>>;

/// A player wherever it is, with its map.
fn locate(maps: &Maps, id: u32) -> Option<(u16, &Placed)> {
    maps.iter().find_map(|(&map, players)| players.get(&id).map(|p| (map, p)))
}

/// A record for the game log from a player's own connection (`ZoneEvent::Record`, which puts the header on).
fn record(maps: &Maps, who: u32, row: crate::game_log::Row) {
    if let Some((_, p)) = locate(maps, who) {
        let _ = p.occupant.events.send(ZoneEvent::Record(Box::new(row)));
    }
}

fn send_anywhere(maps: &Maps, id: u32, packet: &[u8]) {
    if let Some((_, p)) = locate(maps, id) {
        p.send(packet);
    }
}

fn send_all(maps: &Maps, members: &[u32], packet: &[u8]) {
    for &m in members {
        send_anywhere(maps, m, packet);
    }
}

/// `CUser::SendPartyType`: `0x0520` to the player's view, itself included; its looks (`0x0303`) carry the type from now on.
fn tell_party_type(maps: &mut Maps, id: u32, kind: u8) {
    let Some(players) = maps.values_mut().find(|players| players.contains_key(&id)) else { return };
    let me = players.get_mut(&id).expect("found");
    me.occupant.shape.party_type = kind;
    let cell = me.cell;
    let packet = pp::party_type(id, kind);
    for p in players.values().filter(|p| in_view(p.cell, cell)) {
        p.send(&packet);
    }
}

/// HP, SP, MP from the zone's HP, MP, SP.
fn hp_sp_mp(v: [u16; 3]) -> [u16; 3] {
    [v[0], v[2], v[1]]
}

/// The applied skills the party is shown: not those kept through death (`CParty::EnterNormalParty`, `CUser::SendSkillApply`).
fn shown_buffs(p: &Placed) -> impl Iterator<Item = &Applied> {
    p.occupant.buffs.list.iter().filter(|a| !a.def.keep_on_death)
}

/// `PARTY_USER_INFO_UNIT` (section 3).
fn unit(map: u16, p: &Placed, now: Instant) -> pp::MemberUnit {
    let o = &p.occupant;
    pp::MemberUnit {
        id: o.id,
        name: o.shape.name.clone(),
        level: o.vitals.guard.level,
        job: o.shape.job,
        max: hp_sp_mp(o.vitals.max),
        health: hp_sp_mp(o.vitals.health),
        map: crate::world::map_of(map),
        position: o.position,
        buffs: shown_buffs(p)
            .map(|a| (a.def.id, a.def.level, a.ends.saturating_duration_since(now).as_secs().min(u64::from(u32::MAX)) as u32))
            .collect(),
    }
}

/// What the others are shown of a member as it joins.
fn shown_now(p: &Placed) -> Shown {
    let v = &p.occupant.vitals;
    Shown {
        health: hp_sp_mp(v.health),
        max: hp_sp_mp(v.max),
        buffs: shown_buffs(p).map(|a| (a.def.id, a.def.level)).collect(),
        dead: p.dead,
    }
}

/// `CParty::ChkPartyExpByLevel`: the highest member level, kept while the party has two or more.
fn refresh_max_level(maps: &Maps, parties: &mut Parties, party: u32) {
    let Some(p) = parties.get_mut(party) else { return };
    if p.members.len() > 1 {
        p.max_level = p.members.iter().filter_map(|&m| locate(maps, m)).map(|(_, m)| m.occupant.vitals.guard.level).max().unwrap_or(0);
    }
}

/// A kill as the killer's party shares it (sections 8 and 9).
pub(super) struct Share {
    pub party: u32,
    pub size: u8,
    pub max_level: u16,
    /// The members alive on the killer's map within 100 of it, the killer included, in slot order.
    pub near: Vec<u32>,
    pub union: bool,
    /// The loot rule (always 1 in a normal party) and the leader.
    pub rule: u32,
    pub leader: u32,
}

impl Zones {
    /// The player's party id, 0 without one.
    pub(super) fn party_id(&self, player: u32) -> u32 {
        self.parties.guard().party_of(player).map_or(0, |p| p.id)
    }

    /// The killer's party as a kill on this map shares it; `None` without a party.
    pub(super) fn kill_share(&self, players: &HashMap<u32, Placed>, killer: u32) -> Option<Share> {
        let at = players.get(&killer)?.occupant.position;
        let parties = self.parties.guard();
        let p = parties.party_of(killer)?;
        let near = p
            .members
            .iter()
            .copied()
            .filter(|m| players.get(m).is_some_and(|m| !m.dead && distance(m.occupant.position, at) <= pt::SHARE_RANGE))
            .collect();
        let rule = p.union.as_ref().map_or(pt::RULE_IN_TURN, |u| u.rule);
        Some(Share {
            party: p.id,
            size: p.members.len() as u8,
            max_level: p.max_level,
            near,
            union: p.is_union(),
            rule,
            leader: p.leader_id(),
        })
    }

    /// Sets the quests in progress of an online character (`docs/original/quest-drops.md` section 6).
    pub fn set_quests(&self, id: u32, ids: Vec<u16>) {
        self.quests.guard().insert(id, ids);
    }

    /// Forgets them when the character leaves the world.
    pub fn forget_quests(&self, id: u32) {
        self.quests.guard().remove(&id);
    }

    /// Whether the killer, or a member of its party present in `players` (the killer's zone, deviation 1 of the spec), has any of
    /// the quests `wanted` in progress (`CUser::QuestFind` on each).
    pub(super) fn has_quest(&self, players: &HashMap<u32, Placed>, killer: u32, wanted: &[u16]) -> bool {
        if wanted.is_empty() {
            return false;
        }
        let mut who = vec![killer];
        if let Some(p) = self.parties.guard().party_of(killer) {
            who.extend(p.members.iter().copied().filter(|&m| m != killer && players.contains_key(&m)));
        }
        let quests = self.quests.guard();
        who.iter().any(|m| quests.get(m).is_some_and(|ids| ids.iter().any(|q| wanted.contains(q))))
    }

    /// `0x0C07` of a player to the members on its map (`CParty::SendMove`).
    pub(super) fn tell_position(&self, players: &HashMap<u32, Placed>, map: u16, id: u32, at: [f32; 3]) {
        let parties = self.parties.guard();
        let Some(p) = parties.party_of(id) else { return };
        let packet = pp::position(p.is_union(), id, crate::world::map_of(map), at);
        for other in p.others(id).filter_map(|m| players.get(&m)) {
            other.send(&packet);
        }
    }

    /// The party type a player entering a map shows (`0x0303`).
    pub(super) fn party_type(&self, id: u32) -> u8 {
        self.parties.guard().party_type(id)
    }

    /// `0x0B02` / `0x0B09` (section 2), or for a union (`union`) `0x0B14` / `0x0B18` (`docs/original/union.md` section 4): the
    /// checks in the original's order; the invited player gets the invitation, the inviter a failure code (with a real id:
    /// deviation), `0x0B19` for `0x0B14`, `0x0B03` otherwise.
    pub fn party_invite(&self, map: u16, me: u32, invitee: Invitee, union: bool, now: Instant) {
        let maps = self.maps();
        let Some(inviter) = maps.get(&map).and_then(|players| players.get(&me)) else { return };
        let by_id = matches!(invitee, Invitee::Id(_));
        let fail = |code: u8, id: u32| {
            inviter.send(&if union && by_id { pp::union_failed(u32::from(code), id) } else { pp::failed(code, id) });
        };
        let mut parties = self.parties.guard();
        // A union's leader or sub-leader brings up to 30 (the leader test does not look at the union flag).
        let may_bring = |parties: &Parties| {
            parties.party_of(me).is_none_or(|p| {
                if union { p.leads(me) && p.members.len() < pt::MAX_UNION_MEMBERS } else { p.leader_id() == me && !p.is_full() }
            })
        };
        let country = inviter.occupant.shape.country;
        let target = match invitee {
            Invitee::Id(id) => {
                if id == me {
                    return fail(pp::NO_SUCH_CHARACTER, me);
                }
                // "Busy": a personal shop set up or open (`docs/original/personal-shop.md` section 6).
                if inviter.shop.is_some() {
                    return fail(pp::CANNOT_INVITE, me);
                }
                if !may_bring(&parties) {
                    return fail(pp::NOT_LEADER, me);
                }
                // In the inviter's zone only (`CZone::FindUser`).
                match maps[&map].get(&id) {
                    Some(t) if t.occupant.shape.country == country => t,
                    _ => return fail(pp::NO_SUCH_CHARACTER, me),
                }
            }
            Invitee::Name(name) => {
                let found = maps
                    .iter()
                    .find_map(|(&m, players)| players.values().find(|p| p.occupant.shape.name.eq_ignore_ascii_case(name)).map(|p| (m, p)));
                let Some((target_map, t)) = found else { return fail(pp::NO_SUCH_CHARACTER, me) };
                if !may_bring(&parties) {
                    return fail(pp::NOT_LEADER, me);
                }
                if inviter.shop.is_some() {
                    return fail(pp::CANNOT_INVITE, me);
                }
                // `CUser::EqualZone`, the same country.
                if t.occupant.id == me || target_map != map || t.occupant.shape.country != country {
                    return fail(pp::NO_SUCH_CHARACTER, me);
                }
                t
            }
        };
        let target_id = target.occupant.id;
        if parties.engaged(target_id) {
            let same = parties.party_of(me).is_some_and(|p| p.members.contains(&target_id));
            return fail(if same { pp::IN_THE_PARTY } else { pp::IN_ANOTHER_PARTY }, target_id);
        }
        if target.shop.is_some() {
            return fail(pp::CANNOT_INVITE, target_id);
        }
        parties.invite(me, target_id, now);
        target.send(&match (invitee, union) {
            (Invitee::Id(_), false) => pp::invitation(me),
            (Invitee::Name(_), false) => pp::invitation_by_name(&inviter.occupant.shape.name),
            (Invitee::Id(_), true) => pp::union_by_id(pp::OP_UNION_INVITE, me),
            (Invitee::Name(_), true) => pp::union_invitation_by_name(&inviter.occupant.shape.name),
        });
        tracing::debug!(inviter = me, target = target_id, union, "party invitation");
    }

    /// `0x0B03` (section 3), or `0x0B15` (`union`, `docs/original/union.md` section 4: failures as `0x0B19`, the sub-leader's
    /// invitation too): no, or the inviter's party (created when it has none: a normal party) takes the answerer in.
    pub fn party_answer(&self, map: u16, me: u32, yes: bool, union: bool, now: Instant) {
        let mut maps = self.maps();
        let mut parties = self.parties.guard();
        if parties.party_of(me).is_some() {
            return;
        }
        let Some(inviter) = parties.take_invite(me) else { return };
        // In the answerer's zone only.
        let Some(inv) = maps.get(&map).and_then(|players| players.get(&inviter)) else { return };
        let failed = |code: u8| if union { pp::union_failed(u32::from(code), me) } else { pp::failed(code, me) };
        // Either of them busy (a personal shop): 1.
        if inv.shop.is_some() || maps[&map].get(&me).is_some_and(|p| p.shop.is_some()) {
            return inv.send(&failed(pp::CANNOT_INVITE));
        }
        if !yes {
            return inv.send(&failed(pp::REJECTED));
        }
        let Some((party, created)) = parties.party_for(inviter, union) else { return inv.send(&failed(pp::CANNOT_INVITE)) };
        // Joining a party ends the party search registration (`CParty::EnterNormalParty`, `docs/original/friends.md` section 8).
        if created {
            enter_party(&mut maps, &mut parties, party, inviter, now);
            self.friends().party_joined(inviter);
        }
        if parties.join(party, me) {
            enter_party(&mut maps, &mut parties, party, me, now);
            self.friends().party_joined(me);
            tracing::info!(party, inviter, member = me, "joined a party");
        } else {
            maps.get(&map).and_then(|players| players.get(&inviter)).inspect(|inv| inv.send(&failed(pp::CANNOT_INVITE)));
        }
    }

    /// `0x0B05` (section 4).
    pub fn party_leave(&self, me: u32) {
        let mut maps = self.maps();
        let mut parties = self.parties.guard();
        let party = parties.party_of(me).map(|p| p.id);
        if let (Some(party), Some(out)) = (party, parties.leave(me)) {
            depart(&mut maps, &mut parties, party, me, &out, false);
            tracing::info!(party, member = me, "left the party");
        }
    }

    /// Leaving the world (`CUser::LeaveWorld`): out of the party, its invitations forgotten. Before `Zones::leave`.
    pub fn party_leave_world(&self, me: u32) {
        self.party_leave(me);
        self.parties.guard().forget_invites(me);
    }

    /// `0x0B06` (section 4): only by the leader, never itself (deviation: the original's leader index is kept right). In a union
    /// `0x0B1A`, the leader itself included, which `0x0B06` there acts as (`docs/original/union.md` section 5, deviation 5).
    pub fn party_kick(&self, me: u32, target: u32) {
        self.kick(me, target, false);
    }

    /// `0x0B1A` (`docs/original/union.md` section 5): only in a union.
    pub fn union_kick(&self, me: u32, target: u32) {
        self.kick(me, target, true);
    }

    fn kick(&self, me: u32, target: u32, union_only: bool) {
        if union_only && !self.parties.guard().party_of(me).is_some_and(|p| p.is_union()) {
            return;
        }
        let mut maps = self.maps();
        let mut parties = self.parties.guard();
        let party = parties.party_of(me).map(|p| p.id);
        if let (Some(party), Some(out)) = (party, parties.kick(me, target)) {
            depart(&mut maps, &mut parties, party, target, &out, true);
            tracing::info!(party, leader = me, member = target, "kicked from the party");
        }
    }

    /// `0x0B07` (section 4): `0x0B07` to every member, or with id 0 to the requester when it cannot be done. In a union `0x0B17`,
    /// which `0x0B07` there acts as (`docs/original/union.md` section 6, deviation 5).
    pub fn party_leader(&self, me: u32, target: u32) {
        if self.parties.guard().party_of(me).is_some_and(|p| p.is_union()) {
            return self.union_leader(me, target);
        }
        let mut maps = self.maps();
        let mut parties = self.parties.guard();
        let Some((members, old, new)) = parties.set_leader(me, target) else {
            return send_anywhere(&maps, me, &pp::leader(0));
        };
        let packet = pp::leader(new);
        for &m in &members {
            send_anywhere(&maps, m, &packet);
        }
        tell_party_type(&mut maps, new, pp::TYPE_LEADER);
        tell_party_type(&mut maps, old, pp::TYPE_MEMBER);
    }

    /// `0x1105` (section 6) to every member, the sender included; false without a party.
    pub fn party_chat(&self, me: u32, packet: &[u8]) -> bool {
        let maps = self.maps();
        let parties = self.parties.guard();
        let Some(p) = parties.party_of(me) else { return false };
        for &m in &p.members {
            send_anywhere(&maps, m, packet);
        }
        true
    }

    /// A member levelled up (`CUser::LevelUp`): `0x0C09` to every member, itself included; the highest level again.
    pub fn party_level(&self, me: u32, level: u16) {
        let maps = self.maps();
        let mut parties = self.parties.guard();
        let Some(p) = parties.party_of(me) else { return };
        let (party, packet) = (p.id, pp::level(p.is_union(), me, level));
        for &m in &p.members {
            send_anywhere(&maps, m, &packet);
        }
        refresh_max_level(&maps, &mut parties, party);
    }

    pub fn in_party(&self, me: u32) -> bool {
        self.parties.guard().party_of(me).is_some()
    }

    /// Where the dead player comes back to life with its party (`CUser::KillSoulParty`, section 11): at its leader, when that is
    /// another player, alive, on the same map.
    pub fn party_leader_spot(&self, map: u16, me: u32) -> Option<[f32; 3]> {
        let maps = self.maps();
        let leader = self.parties.guard().party_of(me)?.leader_id();
        let p = maps.get(&map)?.get(&leader).filter(|p| leader != me && !p.dead)?;
        Some(p.occupant.position)
    }

    /// `0x0B08` (section 9): the members on the player's map within 100 of it hear of what it got.
    pub fn party_loot_notice(&self, map: u16, me: u32, item_type: u8, type_id: u8) {
        let maps = self.maps();
        let Some(players) = maps.get(&map) else { return };
        let Some(at) = players.get(&me).map(|p| p.occupant.position) else { return };
        let parties = self.parties.guard();
        let Some(p) = parties.party_of(me) else { return };
        let packet = pp::loot(p.is_union(), me, item_type, type_id);
        for other in p.others(me).filter_map(|m| players.get(&m)) {
            if distance(other.occupant.position, at) <= pt::LOOT_NOTICE_RANGE {
                other.send(&packet);
            }
        }
    }

    /// `0x0B0E` (`docs/original/union.md` section 2): the leader makes its party a union (not again, deviation 8); `0x0B0E` to every member.
    pub fn union_set(&self, me: u32, union: u8, auto_join: bool, rule: u32) {
        let maps = self.maps();
        let mut parties = self.parties.guard();
        let Some((members, settings)) = parties.make_union(me, union, auto_join, rule) else {
            return tracing::debug!(id = me, union, rule, "union set-up refused");
        };
        send_all(&maps, &members, &pp::union_set(settings));
        tracing::info!(leader = me, members = members.len(), rule, auto_join, "union set up");
    }

    /// `0x0B0F`: the union's loot rule; `0x0B0F` to every member.
    pub fn union_rule(&self, me: u32, rule: u32) {
        let maps = self.maps();
        if let Some(members) = self.parties.guard().set_union_rule(me, rule) {
            send_all(&maps, &members, &pp::union_rule(rule));
        }
    }

    /// `0x0B10`: auto-join; `0x0B10` to every member.
    pub fn union_auto_join(&self, me: u32, on: bool) {
        let maps = self.maps();
        if let Some(members) = self.parties.guard().set_auto_join(me, on) {
            send_all(&maps, &members, &pp::union_auto_join(on));
        }
    }

    /// `0x0B11` `/join <name>` (`docs/original/union.md` section 4): into the named player's union, from anywhere, when it takes
    /// anyone (auto-join) and is not full; else `0x0B19` 12, 13 or 14. Ignored when the player is in a party.
    pub fn union_join(&self, me: u32, name: &[u8], now: Instant) {
        let mut maps = self.maps();
        let mut parties = self.parties.guard();
        if parties.party_of(me).is_some() {
            return;
        }
        let Some((_, mine)) = locate(&maps, me) else { return };
        let country = mine.occupant.shape.country;
        let target = maps.values().flat_map(HashMap::values).find(|p| p.occupant.shape.name.eq_ignore_ascii_case(name));
        let joined = match target {
            Some(t) if t.occupant.shape.country == country => parties.auto_join(me, t.occupant.id),
            _ => Err(pp::UNION_NOT_FOUND),
        };
        match joined {
            Ok(party) => {
                enter_party(&mut maps, &mut parties, party, me, now);
                self.friends().party_joined(me);
                tracing::info!(party, member = me, "joined a union");
            }
            Err(code) => send_anywhere(&maps, me, &pp::union_failed(code, 0)),
        }
    }

    /// `0x0B12` (`docs/original/union.md` section 6): a group move by the leader or the sub-leader; `0x0B12` to every member.
    pub fn union_move(&self, me: u32, from: u32, to: u32) {
        let maps = self.maps();
        let Some((members, settings)) = self.parties.guard().move_in_union(me, from, to) else {
            return tracing::debug!(id = me, from, to, "union move refused");
        };
        let packet = pp::union_move(from as u8, to as u8, settings.leader_slot, settings.sub_leader_slot);
        send_all(&maps, &members, &packet);
    }

    /// `0x0B17` (`docs/original/union.md` section 6): the union's leader hands over; `0x0B17` to every member, `0x0B13` when the
    /// sub-leader changed with it, the two leaders' party types to their views. Nothing outside a union.
    pub fn union_leader(&self, me: u32, target: u32) {
        let mut maps = self.maps();
        let Some(change) = self.parties.guard().set_union_leader(me, target) else { return };
        send_all(&maps, &change.members, &pp::union_by_id(pp::OP_UNION_LEADER, change.new));
        if let Some(sub) = change.sub {
            send_all(&maps, &change.members, &pp::union_by_id(pp::OP_UNION_SUB_LEADER, sub));
        }
        tell_party_type(&mut maps, change.new, pp::TYPE_LEADER);
        tell_party_type(&mut maps, change.old, pp::TYPE_MEMBER);
        tracing::info!(old = me, new = target, "union leader");
    }

    /// `0x0B13` (`docs/original/union.md` section 6): the leader names the sub-leader; `0x0B13` with the sub-leader to every member.
    pub fn union_sub_leader(&self, me: u32, target: u32) {
        let maps = self.maps();
        if let Some((members, sub)) = self.parties.guard().set_sub_leader(me, target) {
            send_all(&maps, &members, &pp::union_by_id(pp::OP_UNION_SUB_LEADER, sub));
        }
    }

    /// `0x0B16` (`docs/original/union.md` section 6): the leader dismantles the union, which ends the party too: `0x0B16` to every
    /// member, then each one's party type 0 to its view.
    pub fn union_break(&self, me: u32) {
        let mut maps = self.maps();
        let party = self.parties.guard().party_of(me).map(|p| p.id);
        let Some(members) = self.parties.guard().dismantle(me) else { return };
        // `0x0F02` (`CParty::BreakParty`) for every member.
        for &m in &members {
            record(&maps, m, crate::game_log::party_leave(crate::game_log::Header::none(), party.unwrap_or(0)));
        }
        send_all(&maps, &members, &pp::union_break());
        for &m in &members {
            tell_party_type(&mut maps, m, pp::TYPE_NONE);
        }
        tracing::info!(leader = me, members = members.len(), "union dismantled");
    }

    /// `0x1112` (`docs/original/union.md` section 9): the union's leader or sub-leader to every member (as `0xF10A`, deviation 7);
    /// false otherwise.
    pub fn union_chat(&self, me: u32, packet: &[u8]) -> bool {
        let maps = self.maps();
        let parties = self.parties.guard();
        let Some(p) = parties.party_of(me).filter(|p| p.is_union() && p.leads(me)) else { return false };
        send_all(&maps, &p.members, packet);
        true
    }

    /// The members of the player's union group (`CParty::GetGroup`), `None` outside a union.
    pub(super) fn union_group(&self, me: u32) -> Option<Vec<u32>> {
        let parties = self.parties.guard();
        let p = parties.party_of(me).filter(|p| p.is_union())?;
        let Some(group) = p.group_of(me) else { return Some(Vec::new()) };
        Some(p.members.iter().copied().filter(|&m| p.group_of(m) == Some(group)).collect())
    }

    /// Drops the invitations past their 30 s, silently (`CUser::UpdateTimeover`).
    pub fn expire_invites(&self, now: Instant) {
        self.parties.guard().expire(now);
    }

    /// The party pass (every 500 ms; deviation, section 13): what changed of each member since the others were last told: death
    /// (`0x0C06`, the client clears its buffs), maxima (`0x0C08`), HP, SP and MP (`0x0C03`), applied skills (`0x0C05`, `0x0C04`).
    pub fn party_status(&self) {
        let maps = self.maps();
        let mut parties = self.parties.guard();
        for party in parties.all_mut() {
            let union = party.is_union();
            for i in 0..party.members.len() {
                let id = party.members[i];
                let Some((_, p)) = locate(&maps, id) else { continue };
                let now = shown_now(p);
                let shown = party.shown.entry(id).or_insert_with(|| now.clone());
                if *shown == now {
                    continue;
                }
                let mut packets = Vec::new();
                if now.dead && !shown.dead {
                    packets.push(pp::death(union, id));
                    shown.buffs.clear();
                }
                if now.max != shown.max {
                    packets.push(pp::status_all(union, true, id, now.max));
                }
                if now.health != shown.health {
                    packets.push(pp::status_all(union, false, id, now.health));
                }
                for &(skill, level) in shown.buffs.iter().filter(|b| !now.buffs.contains(b)) {
                    packets.push(pp::skill(union, false, id, skill, level));
                }
                for &(skill, level) in now.buffs.iter().filter(|b| !shown.buffs.contains(b)) {
                    packets.push(pp::skill(union, true, id, skill, level));
                }
                *shown = now;
                for other in party.others(id) {
                    for packet in &packets {
                        send_anywhere(&maps, other, packet);
                    }
                }
            }
        }
    }
}

/// `CParty::EnterNormalParty` (section 3) for a player already added to the party's slots: `0x0B04` to the members before it,
/// `0x0B01` to it, its party type to its view. In a union `CParty::EnterRaidParty` (`docs/original/union.md` section 3): `0x0B0C`
/// to the members before it, the `0x0B0B` lists of everyone, itself included, to it, then its own `0x0B0C` (deviation 9).
fn enter_party(maps: &mut Maps, parties: &mut Parties, party: u32, who: u32, now: Instant) {
    let Some(p) = parties.get(party) else { return };
    // `0x0F01` (`CParty::EnterNormalParty`, `EnterRaidParty`): the party's index.
    record(maps, who, crate::game_log::party_enter(crate::game_log::Header::none(), party));
    let before: Vec<u32> = p.others(who).collect();
    let leader = p.leader as u8;
    if let Some((map, me)) = locate(maps, who) {
        if let (Some(settings), Some(slot)) = (p.settings(), p.slot_of(who)) {
            let mine = pp::union_enter(slot, &unit(map, me, now));
            send_all(maps, &before, &mine);
            let units: Vec<(u8, pp::MemberUnit)> =
                p.slotted().into_iter().filter_map(|(slot, m)| locate(maps, m).map(|(map, m)| (slot, unit(map, m, now)))).collect();
            for list in pp::union_lists(settings, &units) {
                me.send(&list);
            }
            me.send(&mine);
        } else {
            let mine = pp::enter(&unit(map, me, now));
            send_all(maps, &before, &mine);
            let units: Vec<pp::MemberUnit> = before.iter().filter_map(|&m| locate(maps, m)).map(|(map, m)| unit(map, m, now)).collect();
            me.send(&pp::list(leader, &units));
        }
        let shown = shown_now(me);
        parties.get_mut(party).expect("listed").shown.insert(who, shown);
    }
    refresh_max_level(maps, parties, party);
    tell_party_type(maps, who, parties.party_type(who));
}

/// After `Parties::leave` or `kick` (section 4): `0x0B05` (or `0x0B06`) to every member it was in the party with, itself
/// included; a last member alone leaves too; a new leader is announced; party types to the views. A union's the same with
/// `0x0B0D`, `0x0B1A`, `0x0B17`, and `0x0B13` for a new sub-leader (`docs/original/union.md` section 5).
fn depart(maps: &mut Maps, parties: &mut Parties, party: u32, who: u32, out: &Departure, kicked: bool) {
    // `0x0F02` (`LeaveNormalParty`, `ExpelUser` and the union's): under the member who goes, and the last one left alone.
    for gone in std::iter::once(who).chain(out.last) {
        record(maps, gone, crate::game_log::party_leave(crate::game_log::Header::none(), party));
    }
    let packet = match (out.union, kicked) {
        (false, false) => pp::left(who),
        (false, true) => pp::kicked(who),
        (true, false) => pp::union_by_id(pp::OP_UNION_LEAVE, who),
        (true, true) => pp::union_by_id(pp::OP_UNION_KICK, who),
    };
    send_all(maps, &out.told, &packet);
    if let Some(last) = out.last {
        send_anywhere(maps, last, &if out.union { pp::union_by_id(pp::OP_UNION_LEAVE, last) } else { pp::left(last) });
        tell_party_type(maps, last, pp::TYPE_NONE);
    }
    if let Some(new) = out.new_leader {
        let packet = if out.union { pp::union_by_id(pp::OP_UNION_LEADER, new) } else { pp::leader(new) };
        send_all(maps, &out.remaining, &packet);
        tell_party_type(maps, new, pp::TYPE_LEADER);
    }
    if let Some(sub) = out.new_sub {
        send_all(maps, &out.remaining, &pp::union_by_id(pp::OP_UNION_SUB_LEADER, sub));
    }
    refresh_max_level(maps, parties, party);
    tell_party_type(maps, who, pp::TYPE_NONE);
}
