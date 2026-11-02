//! Boss groups in the zone (`docs/original/obelisks.md` section 11.3): every zone of a map with `[bothmob_…]` groups runs their
//! chains: a stage's boss and its followers rise in the respawn pass, the boss's death or removal (`BothMobDestroy`) takes the
//! followers away and moves the chain on, the last stage's fall opens the group's portal and brings the chests, the next rising
//! shuts it. Part of the zone, mirrored on `zone/obelisk.rs`: the chains live in their zone's `MobMap`.

use super::obelisk::leave_view;
use super::*;
use crate::boss::{self, Chain};
use crate::mobs::Origin;
use crate::obelisk::Phase;
use crate::sync::Relock;

type Players = HashMap<u32, Placed>;

impl Zones {
    /// A zone's monsters (`CZone::MapInit`): the respawn entries of its map's areas and the map's boss chains, nothing spawned yet.
    pub(super) fn new_mob_map(zone: u16, world: &World, defs: &MobDefs) -> MobMap {
        let mut mobs = MobMap::new(world.spawn_areas_of(zone), defs);
        mobs.bosses = world.boss_groups().of(crate::world::map_of(zone)).iter().cloned().map(Chain::new).collect();
        mobs
    }

    /// A boss group's portal takes a country (`stMapPortal::nCountry` and `UpdateInsZonePortalCountry`): on a static map the map's
    /// portal, which the whole server sees; in an instance zone the zone's pair.
    fn boss_portal(&self, zone: u16, portal: Option<(u16, u8, u32)>, country: u32) {
        let Some((map, place, id)) = portal else { return };
        if crate::world::map_of(zone) == zone {
            self.portal_countries.guard().insert((map, place), country);
        } else {
            self.set_instance_portal(zone, id, country);
        }
    }

    /// A portal's country on a static map: a boss group's change, else the map file's (`portal.country`).
    pub fn portal_country(&self, map: u16, place: u8, file: u32) -> u32 {
        self.portal_countries.guard().get(&(crate::world::map_of(map), place)).copied().unwrap_or(file)
    }

    /// The boss chains' part of a zone's respawn pass (`CZone::UpdateBothMobRespawn`): a chain set up at its first pass; the chests
    /// of a last fall placed; no boss standing and due → `BothMobRespawn` (at stage 0 the portal shuts and the followers and chests
    /// still there go, into `left`; then the stage's boss and each follower group filled); a boss standing → one follower per group
    /// below its count and due.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn boss_respawn(
        &self,
        zone: u16,
        mobs: &mut MobMap,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        rand: &mut impl FnMut() -> u32,
        spawned: &mut Vec<(u16, Mob, (i32, i32))>,
        left: &mut Vec<(u16, Mob)>,
    ) {
        for i in 0..mobs.bosses.len() {
            if !mobs.bosses[i].started {
                mobs.bosses[i].start(now, rand);
            }
            if mobs.bosses[i].chests_due {
                mobs.bosses[i].chests_due = false;
                for set in 0..mobs.bosses[i].def.followers.len() {
                    let f = &mobs.bosses[i].def.followers[set];
                    let Some(def) = f.chest.and_then(|id| defs.get(id)) else { continue };
                    for _ in 0..f.count {
                        let position = mobs.bosses[i].def.followers[set].spot(rand(), rand());
                        if world.is_blocked(zone, position[0].round_ties_even() as i32, position[2].round_ties_even() as i32) {
                            continue;
                        }
                        let mob = self.new_mob(def, position, usize::MAX, Origin::Chest(i), now, rand);
                        mobs.bosses[i].chests.push(mob.id);
                        mobs.insert(mob);
                        spawned.push((zone, mob, mob.cell));
                    }
                }
            }
            let c = &mobs.bosses[i];
            match c.phase {
                Phase::Due(due) if due.is_none_or(|due| due <= now) => {
                    let Some(def) = c.boss().and_then(|id| defs.get(id)) else { continue };
                    if c.stage == 0 {
                        self.boss_portal(zone, c.def.portal, boss::PORTAL_SHUT);
                        let c = &mut mobs.bosses[i];
                        let mut gone: Vec<u32> = c.groups.iter_mut().flat_map(|g| std::mem::take(&mut g.alive)).collect();
                        gone.append(&mut c.chests);
                        for id in gone {
                            if let Some(mob) = mobs.mobs.get(&id).copied() {
                                left.push((zone, mob));
                                mobs.remove(id);
                            }
                        }
                    }
                    let c = &mobs.bosses[i];
                    let position = if c.def.random.is_some() { c.def.spot(rand()) } else { c.def.position };
                    let mob = self.new_mob(def, position, usize::MAX, Origin::Boss(i), now, rand);
                    let c = &mut mobs.bosses[i];
                    c.phase = Phase::Standing(mob.id);
                    c.removed = false;
                    c.rise();
                    tracing::info!(zone, group = c.def.number, stage = c.stage, mob = def.id, "boss rose");
                    mobs.insert(mob);
                    spawned.push((zone, mob, mob.cell));
                    for set in 0..mobs.bosses[i].groups.len() {
                        for _ in 0..mobs.bosses[i].def.followers[set].count {
                            self.spawn_follower(zone, mobs, (i, set), true, now, world, defs, rand, spawned);
                        }
                    }
                }
                Phase::Standing(_) => {
                    for set in 0..c.groups.len() {
                        let c = &mobs.bosses[i];
                        let (line, group) = (&c.def.followers[set], &c.groups[set]);
                        if group.current < line.count && group.due.is_none_or(|due| due <= now) {
                            self.spawn_follower(zone, mobs, (i, set), false, now, world, defs, rand, spawned);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// One follower of the current stage at a random spot, if that is on the map and free (deviation 7: else tried again on the next
    /// pass); outside a rising, the group is due again after the follower's respawn time.
    #[allow(clippy::too_many_arguments)]
    fn spawn_follower(
        &self,
        zone: u16,
        mobs: &mut MobMap,
        (i, set): (usize, usize),
        rising: bool,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        rand: &mut impl FnMut() -> u32,
        spawned: &mut Vec<(u16, Mob, (i32, i32))>,
    ) {
        let c = &mobs.bosses[i];
        let Some(def) = c.follower(set).and_then(|id| defs.get(id)) else { return };
        let position = c.def.followers[set].spot(rand(), rand());
        if world.is_blocked(zone, position[0].round_ties_even() as i32, position[2].round_ties_even() as i32) {
            return;
        }
        let origin = Origin::Follower { chain: i, set, generation: c.groups[set].generation };
        let mob = self.new_mob(def, position, usize::MAX, origin, now, rand);
        let group = &mut mobs.bosses[i].groups[set];
        group.alive.push(mob.id);
        group.current += 1;
        if !rising {
            group.due = Some(now + def.respawn);
        }
        mobs.insert(mob);
        spawned.push((zone, mob, mob.cell));
    }

    /// A boss group's monster died (`CMob::Death`, killer or not) or was taken away (`removed`: `/mera`, `LuaDeleteMob`): a standing
    /// boss → `BothMobDestroy` (its followers go, the chain moves on; past the last stage the portal opens and the chests are due);
    /// a follower leaves its group's list. Nothing for any other monster.
    pub(super) fn boss_death(&self, zone: u16, players: Option<&Players>, mobs: &mut MobMap, mob: &Mob, removed: bool) {
        match mob.origin {
            Origin::Boss(i) => {
                let Some(c) = mobs.bosses.get_mut(i).filter(|c| c.phase == Phase::Standing(mob.id)) else { return };
                c.phase = Phase::Fallen(mob.id);
                c.removed = removed;
                let stage = c.stage;
                let destroy = c.destroy();
                let (number, portal) = (c.def.number, c.def.portal);
                for id in &destroy.followers {
                    if let Some(follower) = mobs.mobs.get(id).copied() {
                        leave_view(players, &follower);
                    }
                    mobs.remove(*id);
                }
                if destroy.ended {
                    self.boss_portal(zone, portal, boss::PORTAL_OPEN);
                }
                tracing::info!(zone, group = number, stage, ended = destroy.ended, removed, "boss fell");
            }
            Origin::Follower { chain, set, generation } => {
                if let Some(g) = mobs.bosses.get_mut(chain).and_then(|c| c.groups.get_mut(set)).filter(|g| g.generation == generation) {
                    g.alive.retain(|&id| id != mob.id);
                }
            }
            _ => {}
        }
    }

    /// A boss group's monster's body went (`CZone::UpdateMobDeath`): a fallen boss makes its chain due (its own respawn time, or the
    /// chain's wait after a killed last stage); a follower of the current stage counts its group down, a full group due again after
    /// the respawn time; a chest nothing. False for any other monster.
    pub(super) fn boss_body(zone: u16, mobs: &mut MobMap, mob: &Mob, defs: &MobDefs, now: Instant, rand: &mut impl FnMut() -> u32) -> bool {
        match mob.origin {
            Origin::Boss(i) => {
                if let Some(c) = mobs.bosses.get_mut(i).filter(|c| c.phase == Phase::Fallen(mob.id)) {
                    let own = defs.get(mob.mob_id).map_or(Duration::ZERO, |d| d.respawn);
                    let wait = c.body_gone(own, now, rand);
                    tracing::info!(zone, group = c.def.number, stage = c.stage, in_secs = wait.as_secs(), "boss due");
                }
                true
            }
            Origin::Follower { chain, set, generation } => {
                let Some(c) = mobs.bosses.get_mut(chain) else { return true };
                let count = c.def.followers.get(set).map_or(0, |f| f.count);
                if let Some(g) = c.groups.get_mut(set).filter(|g| g.generation == generation) {
                    if g.current >= count
                        && let Some(def) = defs.get(mob.mob_id)
                    {
                        g.due = Some(respawn_due(def, now, rand));
                    }
                    g.current = g.current.saturating_sub(1);
                }
                true
            }
            Origin::Chest(_) => true,
            _ => false,
        }
    }
}
