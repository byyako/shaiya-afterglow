//! Obelisks in the zone (`docs/original/obelisks.md`): each map's obelisks rise with their guards in the respawn pass, fall when
//! their monster dies and are reset when a faction reaches its top grade. Part of the zone: the obelisks live in their map's
//! `MobMap`; what the server must do with a fall (the kill counts, `0x2103` to the world, the saves) goes into `obelisk_events`,
//! taken after `maps` and `mobs`, held briefly.

use super::*;
use crate::mobs::Origin;
use crate::obelisk::{self as ob, Event, Obelisk, Phase};
use crate::sync::Relock;
use shaiya_proto::obelisk as op;

type Players = HashMap<u32, Placed>;

/// How an obelisk falls (`CMap::ObeliskDestroy`'s callers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FallKind {
    /// Its monster died (`CMob::Death`): all of section 4.
    Kill,
    /// An operator took it away (`CZone::MobRemove`, `MobRemoveByID`, section 11.5): `0x2102` to the map and the owner's save when
    /// it changed; no kill count, no `0x2103`.
    Removal,
    /// The reset: nothing told or saved here (`reset_obelisks` does both itself).
    Reset,
}

/// `0x0602` of a monster that goes without dying, to its view.
pub(super) fn leave_view(players: Option<&Players>, mob: &Mob) {
    let packet = w::mob_leave(mob.id);
    for p in players.into_iter().flat_map(|p| p.values()).filter(|p| in_view(p.cell, mob.cell)) {
        p.send(&packet);
    }
}

/// To every player of the map (`CZone::SendAll`).
fn send_map(players: Option<&Players>, packet: &[u8]) {
    for p in players.into_iter().flat_map(|p| p.values()) {
        p.send(packet);
    }
}

impl Zones {
    /// The obelisks of every map that has them, at their base countries, waiting to rise.
    pub(super) fn place_obelisks(mobs: &mut HashMap<u16, MobMap>, world: &World) {
        for (&map, defs) in &world.obelisks().maps {
            mobs.entry(map).or_default().obelisks = defs.iter().cloned().map(Obelisk::new).collect();
        }
    }

    /// The saved owners (`CWorld::SetKeepObelisk`), before anything has risen. Returns how many matched an obelisk.
    pub fn set_obelisk_owners(&self, owners: &[ob::Owner]) -> usize {
        let mut all = self.mobs.guard();
        let mut set = 0;
        for owner in owners.iter().filter(|o| o.country <= ob::NEUTRAL && o.last <= ob::NEUTRAL) {
            let Some(o) = all.get_mut(&owner.map).and_then(|m| m.obelisks.iter_mut().find(|o| o.def.number == owner.number)) else {
                continue;
            };
            (o.country, o.last) = (owner.country, owner.last);
            set += 1;
        }
        set
    }

    /// A monster that has just appeared (`CMob::Init`).
    pub(super) fn new_mob(
        &self,
        def: &MobDef,
        position: [f32; 3],
        entry: usize,
        origin: Origin,
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Mob {
        let id = self.last_mob_id.fetch_add(1, Ordering::Relaxed) + 1;
        let next_move = now + mobs::first_move_delay(rand);
        let base = Mob::new(id, def.id, position, def.hp, entry, cell_of(position), now, next_move);
        // A monster with a machine runs it from its first turn (`CMob::Init`, `docs/original/scripted-mobs.md` section 1).
        let scripted = self.scripted_ais.contains(&def.ai);
        // `AI` 100 and up: a Lua monster, run by the Lua pass, with or without a script (`docs/original/instances.md` section 9).
        let lua = def.ai >= crate::mob_ai::AI_LUA;
        let state = if lua {
            MobState::Lua(None)
        } else if scripted {
            MobState::Scripted(None)
        } else {
            MobState::Idle
        };
        Mob { sp: def.sp, mp: def.mp, origin, scripted, lua, state, ..base }
    }

    /// The obelisks' part of a map's respawn pass (`CMap::UpdateRespawnObelisk`), for those whose monster counts for their owner:
    /// one that is down and due rises with all its guards (`ObeliskRespawn`); one that stands gets one guard per group below its
    /// count and due.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn obelisk_respawn(
        &self,
        map: u16,
        mobs: &mut MobMap,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        rand: &mut impl FnMut() -> u32,
        spawned: &mut Vec<(u16, Mob, (i32, i32))>,
    ) {
        for i in 0..mobs.obelisks.len() {
            let o = &mobs.obelisks[i];
            let Some(def) = o.monster().and_then(|id| defs.get(id)) else { continue };
            let groups = o.def.guards.len();
            match o.phase {
                Phase::Due(due) if due.is_none_or(|due| due <= now) => {
                    let mob = self.new_mob(def, o.def.position, usize::MAX, Origin::Obelisk(i), now, rand);
                    let o = &mut mobs.obelisks[i];
                    o.phase = Phase::Standing(mob.id);
                    o.rise();
                    // `0x0A02` (`CMap::ObeliskRespawn`): the obelisk's map and place, the country holding it, its monster's name.
                    let log = self.game_log();
                    let header = crate::game_log::Header::map_only(map, o.def.position);
                    log.action(crate::game_log::obelisk_respawn(header, ob::packet_country(o.country), log.mob_name(def.id)));
                    mobs.insert(mob);
                    spawned.push((map, mob, mob.cell));
                    for g in 0..groups {
                        for _ in 0..mobs.obelisks[i].def.guards[g].count {
                            self.spawn_guard(map, mobs, (i, g), true, now, world, defs, rand, spawned);
                        }
                    }
                }
                Phase::Standing(_) => {
                    for g in 0..groups {
                        let o = &mobs.obelisks[i];
                        let (group, guards) = (&o.def.guards[g], &o.groups[g]);
                        if guards.current < group.count && guards.due.is_none_or(|due| due <= now) {
                            self.spawn_guard(map, mobs, (i, g), false, now, world, defs, rand, spawned);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// One guard of a group at a random spot, if that is on the map and free; outside a rising, the group is due again after the
    /// guard's respawn time.
    #[allow(clippy::too_many_arguments)]
    fn spawn_guard(
        &self,
        map: u16,
        mobs: &mut MobMap,
        (i, g): (usize, usize),
        rising: bool,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        rand: &mut impl FnMut() -> u32,
        spawned: &mut Vec<(u16, Mob, (i32, i32))>,
    ) {
        let o = &mobs.obelisks[i];
        let Some(def) = o.guard(g).and_then(|id| defs.get(id)) else { return };
        let position = o.def.guards[g].spot(rand(), rand());
        if world.is_blocked(map, position[0].round_ties_even() as i32, position[2].round_ties_even() as i32) {
            return;
        }
        let origin = Origin::Guard { obelisk: i, group: g, generation: o.groups[g].generation };
        let mob = self.new_mob(def, position, usize::MAX, origin, now, rand);
        let guards = &mut mobs.obelisks[i].groups[g];
        guards.alive.push(mob.id);
        guards.current += 1;
        if !rising {
            guards.due = Some(now + def.respawn);
        }
        mobs.insert(mob);
        spawned.push((map, mob, mob.cell));
    }

    /// An obelisk's or a guard's monster died (`CMob::Death`): an obelisk falls to the killer's country, or with no player to credit
    /// to the other side of its owner (section 4); a guard leaves its group's list.
    pub(super) fn obelisk_death(&self, map: u16, players: &Players, mobs: &mut MobMap, mob: &Mob, killer_country: Option<u8>) {
        match mob.origin {
            Origin::Obelisk(i) => {
                let Some(o) = mobs.obelisks.get_mut(i).filter(|o| o.phase == Phase::Standing(mob.id)) else { return };
                o.phase = Phase::Fallen(mob.id);
                let country = killer_country.unwrap_or(ob::other_side(o.country));
                self.obelisk_fall(map, Some(players), mobs, i, country, mob.mob_id, FallKind::Kill);
            }
            Origin::Guard { obelisk, group, generation } => {
                if let Some(g) = mobs.obelisks.get_mut(obelisk).and_then(|o| o.groups.get_mut(group)).filter(|g| g.generation == generation)
                {
                    g.alive.retain(|&id| id != mob.id);
                }
            }
            _ => {}
        }
    }

    /// An operator takes an obelisk's or a guard's monster away (`CZone::MobRemove` for `/mera`, `MobRemoveByID` for `/mera t`,
    /// section 11.5): a standing obelisk falls as a removal, to its current country or with `to_base` to its base; its monster's body
    /// then makes it due after its respawn time. A guard leaves its group's list.
    pub(super) fn obelisk_removed(&self, map: u16, players: Option<&Players>, mobs: &mut MobMap, mob: &Mob, to_base: bool) {
        match mob.origin {
            Origin::Obelisk(i) => {
                let Some(o) = mobs.obelisks.get_mut(i).filter(|o| o.phase == Phase::Standing(mob.id)) else { return };
                o.phase = Phase::Fallen(mob.id);
                let country = if to_base { o.def.base } else { o.country };
                self.obelisk_fall(map, players, mobs, i, country, mob.mob_id, FallKind::Removal);
            }
            Origin::Guard { obelisk, group, generation } => {
                if let Some(g) = mobs.obelisks.get_mut(obelisk).and_then(|o| o.groups.get_mut(group)).filter(|g| g.generation == generation)
                {
                    g.alive.retain(|&id| id != mob.id);
                }
            }
            _ => {}
        }
    }

    /// `CMap::ObeliskDestroy`: the guards go at once; the new owner; then, unless for the reset, `0x2102` to the map and the owner's
    /// save when it changed, and for a kill the fall for the server (the kill counts, `0x2103`).
    #[allow(clippy::too_many_arguments)]
    fn obelisk_fall(&self, map: u16, players: Option<&Players>, mobs: &mut MobMap, i: usize, country: u8, mob_id: u16, kind: FallKind) {
        let Some(o) = mobs.obelisks.get_mut(i) else { return };
        let fall = o.fall(country);
        let (change, owner) = (op::change(o.def.id, ob::packet_country(o.country)), o.owner(map));
        for id in &fall.guards {
            if let Some(guard) = mobs.mobs.get(id).copied() {
                leave_view(players, &guard);
            }
            mobs.remove(*id);
        }
        // `0x0A03` (`CMap::ObeliskDestroy`): the country that held it and the one that holds it next.
        let log = self.game_log();
        let header = crate::game_log::Header::map_only(map, mobs.obelisks.get(i).map_or([0.0; 3], |o| o.def.position));
        log.action(crate::game_log::obelisk_destroy(
            header,
            ob::packet_country(fall.from),
            ob::packet_country(fall.to),
            log.mob_name(mob_id),
        ));
        if kind == FallKind::Reset {
            return;
        }
        send_map(players, &change);
        let mut events = self.obelisk_events.guard();
        if kind == FallKind::Kill {
            events.push(Event::Fell { mob_id, from: fall.from, to: fall.to });
        }
        if fall.from != fall.to {
            events.push(Event::Owner(owner));
        }
    }

    /// An obelisk's or a guard's body went (`CZone::UpdateMobDeath`): a fallen obelisk is due after its monster's respawn time; a
    /// guard of the current rising counts its group down, a full group being due again after the respawn time. False for any other
    /// monster.
    pub(super) fn obelisk_body(mobs: &mut MobMap, mob: &Mob, defs: &MobDefs, now: Instant, rand: &mut impl FnMut() -> u32) -> bool {
        match mob.origin {
            Origin::Obelisk(i) => {
                if let Some(o) = mobs.obelisks.get_mut(i).filter(|o| o.phase == Phase::Fallen(mob.id)) {
                    o.phase = Phase::Due(Some(now + defs.get(mob.mob_id).map_or(Duration::ZERO, |d| d.respawn)));
                }
                true
            }
            Origin::Guard { obelisk, group, generation } => {
                let Some(o) = mobs.obelisks.get_mut(obelisk) else { return true };
                let count = o.def.guards.get(group).map_or(0, |g| g.count);
                if let Some(g) = o.groups.get_mut(group).filter(|g| g.generation == generation) {
                    if g.current >= count
                        && let Some(def) = defs.get(mob.mob_id)
                    {
                        g.due = Some(respawn_due(def, now, rand));
                    }
                    g.current = g.current.saturating_sub(1);
                }
                true
            }
            _ => false,
        }
    }

    /// `CMap::ObeliskReset` on every map of create type 0 (a faction reached its top grade): a standing obelisk's monster goes
    /// with its guards; every obelisk is down for ten minutes at its base country; `0x2102` to the map, and the owners are saved.
    pub fn reset_obelisks(&self, now: Instant, world: &World) {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        for (&map, mobs) in all.iter_mut().filter(|(map, mobs)| !mobs.obelisks.is_empty() && world.create_type(**map) == 0) {
            let players = maps.get(&map);
            for i in 0..mobs.obelisks.len() {
                if let Phase::Standing(id) = mobs.obelisks[i].phase
                    && let Some(mob) = mobs.mobs.get(&id).copied()
                {
                    leave_view(players, &mob);
                    mobs.remove(id);
                    let base = mobs.obelisks[i].def.base;
                    self.obelisk_fall(map, players, mobs, i, base, mob.mob_id, FallKind::Reset);
                }
                let o = &mut mobs.obelisks[i];
                o.reset(now);
                send_map(players, &op::change(o.def.id, ob::packet_country(o.country)));
                self.obelisk_events.guard().push(Event::Owner(o.owner(map)));
            }
        }
    }

    /// `0x2101` of a map's obelisks, standing or not (`CMap::SendObeliskList`); nothing when it has none.
    pub(super) fn obelisk_list(mobs: Option<&MobMap>) -> Vec<Vec<u8>> {
        let rows: Vec<_> = mobs.into_iter().flat_map(|m| m.obelisks.iter()).map(Obelisk::row).collect();
        op::list(&rows)
    }

    /// What the server has to do for the obelisks since it last looked.
    pub fn take_obelisk_events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.obelisk_events.guard())
    }

    /// Who holds a map's obelisks, in order.
    pub fn obelisk_owners(&self, map: u16) -> Vec<ob::Owner> {
        self.mobs.guard().get(&map).map(|m| m.obelisks.iter().map(|o| o.owner(map)).collect()).unwrap_or_default()
    }

    /// A packet for every player in the world (`CWorld::SendAll`).
    pub fn send_all(&self, packet: &[u8]) {
        for p in self.maps.guard().values().flat_map(|players| players.values()) {
            p.send(packet);
        }
    }
}
