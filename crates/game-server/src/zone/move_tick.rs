//! The move tick between the passes (`docs/original/chase-smoothing.md` deviation 2): a monster in the middle of a chase, of going
//! home or (Lua) of a walk, and an NPC chasing, going back or walking, takes its next step when that is due, and nothing else.
//! Every other outcome (in reach, giving up, home, arrival, no way) is left to its next pass, which decides it as before.

use super::*;
use crate::mob_lua as ml;
use crate::sync::Relock;

impl Zones {
    /// One move tick (every 100 ms between the passes, with smoothing on).
    pub fn move_steps(&self, now: Instant, world: &World, defs: &MobDefs) {
        if !self.smooth_chase() {
            return;
        }
        let maps = self.maps();
        let mut all = self.mobs.guard();
        let nobody = HashMap::new();
        for (&map, mobs) in all.iter_mut() {
            let players = maps.get(&map).unwrap_or(&nobody);
            let terrain = MapTerrain { world, map };
            let mut ids: Vec<u32> =
                mobs.mobs.values().filter(|m| m.moving != Moving::None && m.dead_until.is_none()).map(|m| m.id).collect();
            ids.sort_unstable();
            for id in ids {
                mob_step(mobs, players, id, defs, now, &terrain, world);
            }
        }
        let mut npcs = self.npcs.guard();
        let no_mobs = MobMap::default();
        for (&map, m) in npcs.iter_mut() {
            let players = maps.get(&map).unwrap_or(&nobody);
            npc::move_tick(world, map, m, all.get(&map).unwrap_or(&no_mobs), players, now);
        }
    }
}

/// A Lua monster's status (the top of its stack), `None` for any other.
fn lua_top(mobs: &MobMap, id: u32) -> Option<u8> {
    lua::lua_status(mobs, id)
}

/// One monster's step of the move tick.
fn mob_step(mobs: &mut MobMap, players: &HashMap<u32, Placed>, id: u32, defs: &MobDefs, now: Instant, terrain: &MapTerrain, world: &World) {
    let Some(mob) = mobs.mobs.get(&id).copied() else { return };
    let Some(def) = defs.get(mob.mob_id) else { return stop_moving(mobs, id) };
    let effects = mobs.effects(id);
    let speed = mobs.speeds(id).1;
    if effects.sleep > 0 || effects.stun > 0 || speed == buffs::CANNOT {
        return;
    }
    match mob.moving {
        Moving::None => {}
        Moving::Chase { target, range, wait } => {
            // Still after the same target, as the pass that set it out, and its attack time come (the pass looks only then).
            let ours = match mob.state {
                MobState::Fighting(t) | MobState::Scripted(Some(t)) => t == target && !mob.lua,
                MobState::Lua(Some(t)) => {
                    t == target && lua_top(mobs, id) == Some(ml::COMBAT) && !mobs.lua.get(&id).is_some_and(|l| l.fields.hold)
                }
                _ => false,
            };
            let at = players.get(&target).filter(|p| targetable(p, def) && (!mob.lua || p.form.shape == 0)).map(|p| p.occupant.position);
            let Some(at) = at.filter(|_| ours) else { return stop_moving(mobs, id) };
            if now < mob.next_chase || now < mob.next_attack {
                return;
            }
            let mut path = mobs.paths.get(&id).cloned().unwrap_or_default();
            match fight::chase(mob.position, mob.home, at, def, range, speed, mob.next_chase, now, terrain, &mut path, wait) {
                Chase::Wait => {}
                Chase::Step { to: Some(to), next, trail } if world.is_safe_pos(terrain.map, to) => {
                    mobs.paths.insert(id, path);
                    fight::add_trail(mobs.trails.entry(id).or_default(), trail, terrain);
                    let next = paced_next(mobs, id, def, Some(to), 1, next, now, true, mob.moving);
                    mobs.mobs.get_mut(&id).expect("listed").next_chase = next;
                    walk_mob(mobs, Some(players), id, to, 1);
                }
                _ => stop_moving(mobs, id),
            }
        }
        Moving::Home => {
            let lua = mob.lua;
            if !(if lua { lua_top(mobs, id) == Some(ml::HOME) } else { mob.state == MobState::Home }) {
                return stop_moving(mobs, id);
            }
            let due = if lua { mobs.lua.get(&id).map_or(now, |l| l.fields.next_step) } else { mob.next_chase };
            if now < due {
                return;
            }
            let mut trail = mobs.trails.get(&id).cloned().unwrap_or_default();
            let mut path = mobs.paths.get(&id).cloned().unwrap_or_default();
            match fight::homeward(mob.position, mob.home, def, &mut trail, &mut path, speed, due, now, terrain) {
                Homeward::Wait => {}
                Homeward::Step { to, next } if world.is_safe_pos(terrain.map, to) => {
                    mobs.trails.insert(id, trail);
                    mobs.paths.insert(id, path);
                    let next = paced_next(mobs, id, def, Some(to), 1, next, now, true, Moving::Home);
                    set_due(mobs, id, lua, next);
                    walk_mob(mobs, Some(players), id, to, 1);
                }
                _ => stop_moving(mobs, id),
            }
        }
        Moving::Walk { spot } => {
            if lua_top(mobs, id) != Some(ml::MOVING) {
                return stop_moving(mobs, id);
            }
            let due = mobs.lua.get(&id).map_or(now, |l| l.fields.next_step);
            // Within 10 of the spot the pass may call it there (`5 + rand() % 5`): left to the pass.
            if now < due || distance(mob.position, spot) < 10.0 {
                return;
            }
            let mut trail = mobs.trails.get(&id).cloned().unwrap_or_default();
            let mut path = mobs.paths.get(&id).cloned().unwrap_or_default();
            match fight::moving(mob.position, spot, def, &mut trail, &mut path, speed, due, now, terrain, &mut || 0) {
                Homeward::Wait => {}
                Homeward::Step { to, next } if world.is_safe_pos(terrain.map, to) => {
                    mobs.trails.insert(id, trail);
                    mobs.paths.insert(id, path);
                    let next = paced_next(mobs, id, def, Some(to), 0, next, now, true, mob.moving);
                    set_due(mobs, id, true, next);
                    walk_mob(mobs, Some(players), id, to, 0);
                }
                _ => stop_moving(mobs, id),
            }
        }
    }
}

/// The next step time of going home or of a Lua move: the Lua fields' (`+0xcb8`) or the chase time.
fn set_due(mobs: &mut MobMap, id: u32, lua: bool, next: Instant) {
    if lua {
        if let Some(l) = mobs.lua.get_mut(&id) {
            l.fields.next_step = next;
        }
    } else if let Some(m) = mobs.mobs.get_mut(&id) {
        m.next_chase = next;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::tests::{drain, fighting_ground, no_loot, player, step};
    use super::*;
    use crate::combat::{self, Gear};

    /// A monster of `fighting_ground` (chase step 8 every second: the client walks 8 − 0.3 = 7.7 a second) chasing a player 35 away,
    /// after its first step to (100, 113).
    fn chasing(smooth: bool) -> (Zones, World, MobDefs, crate::outbox::OutboxInbox, Instant) {
        let (world, _, defs) = fighting_ground();
        let zones = Zones::new(&world, &defs);
        zones.set_smooth_chase(smooth);
        let (a, mut a_in) = player(1, 100.0, 140.0);
        zones.enter(1, a);
        let now = Instant::now();
        zones.respawn(now, &world, &defs, &mut || 0);
        let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
        let attack =
            Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 40.0, range: 0, rec: 0, cross_road: false };
        let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
        let mut rolls = [0, 99].into_iter().cycle();
        zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap());
        drain(&mut a_in);
        zones.mob_fight(now, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 113.0]));
        assert_eq!(drain(&mut a_in), [(0x0603, 1)]);
        (zones, world, defs, a_in, now)
    }

    fn ms(now: Instant, t: Instant) -> u128 {
        t.duration_since(now).as_millis()
    }

    #[test]
    fn a_chasing_monster_steps_when_the_client_gets_there() {
        let (zones, world, defs, mut a_in, now) = chasing(true);
        let at = |t: u64| now + Duration::from_millis(t);
        // 8 at 7.7 a second: the client there after 1039 ms, the next step 100 ms before.
        let mob = zones.mob(1, 1).unwrap();
        assert_eq!(ms(now, mob.next_chase), 938);
        assert!(matches!(mob.moving, Moving::Chase { target: 1, range: 2, .. }));
        zones.move_steps(at(900), &world, &defs);
        assert!(drain(&mut a_in).is_empty(), "not yet");
        // The tick between the passes takes the step.
        zones.move_steps(at(1000), &world, &defs);
        assert_eq!(drain(&mut a_in), [(0x0603, 1)]);
        assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 121.0]));
        // Anchored on the client's arrival (2078 ms), not on when the step went.
        assert_eq!(ms(now, zones.mob(1, 1).unwrap().next_chase), 1977);
        zones.move_steps(at(2000), &world, &defs);
        assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 129.0]));
        // 11 left: the last, shorter step to 3.4 short of the player.
        zones.move_steps(at(3100), &world, &defs);
        let mob = zones.mob(1, 1).unwrap();
        assert!((mob.position[2] - 136.6).abs() < 1e-3, "{:?}", mob.position);
        drain(&mut a_in);
        // In reach: the tick does not strike, it leaves it to the pass.
        zones.move_steps(at(4200), &world, &defs);
        assert!(drain(&mut a_in).is_empty(), "no blow on a move tick");
        assert_eq!(zones.mob(1, 1).map(|m| m.moving), Some(Moving::None));
        zones.mob_fight(at(4500), &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut a_in).iter().any(|&(op, _)| op == 0x0605), "the pass strikes");
    }

    #[test]
    fn without_smoothing_the_original_waits() {
        let (zones, world, defs, mut a_in, now) = chasing(false);
        let mob = zones.mob(1, 1).unwrap();
        assert_eq!(ms(now, mob.next_chase), 1000);
        assert_eq!(mob.moving, Moving::None);
        zones.move_steps(now + Duration::from_millis(1000), &world, &defs);
        assert!(drain(&mut a_in).is_empty());
        assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 113.0]));
    }

    #[test]
    fn the_move_tick_leaves_every_decision_to_the_pass() {
        let (zones, world, defs, mut a_in, now) = chasing(true);
        // The player runs off beyond the leash: the tick neither steps nor sends the monster home.
        zones.move_to(1, 1, &step(100.0, 170.0));
        drain(&mut a_in);
        zones.move_steps(now + Duration::from_millis(1000), &world, &defs);
        assert!(drain(&mut a_in).is_empty());
        let mob = zones.mob(1, 1).unwrap();
        assert_eq!((mob.state, mob.moving), (MobState::Fighting(1), Moving::None));
        // The pass decides: home.
        zones.mob_fight(now + Duration::from_millis(1000), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Home));
    }
}
