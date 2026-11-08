//! Capture (`docs/plans/ext-4-pets.md` section 3): which monster a capture item is used on, whether it can be caught, the chance, and
//! the taking of the monster out of the zone as a death without rewards. The connection's side (the item, the egg, the whispers) is
//! `server/pets.rs`.

use super::*;
use crate::effects::{Capture, PetEgg, in_ranges};

/// A monster's HP in percent, 1 to 100: a live monster is at least 1.
pub fn hp_percent(hp: u32, max_hp: u32) -> u32 {
    (u64::from(hp) * 100 / u64::from(max_hp.max(1))).clamp(1, 100) as u32
}

/// The chance of a catch in percent (section 3.3): `c0 + (c1 − c0) × (below − p) / below`, rounded down, at the HP percent `p`
/// (counted as `below` at the most, so the chance is `c0` at the threshold).
pub fn catch_chance(chance: [u8; 2], below: u8, percent: u32) -> u32 {
    let [c0, c1] = chance.map(i64::from);
    let b = i64::from(below.max(1));
    let p = i64::from(percent).min(b);
    (c0 + ((c1 - c0) * (b - p)).div_euclid(b)).clamp(0, 100) as u32
}

/// Whether a roll (`rand() % 100`) catches at this chance.
pub fn catch_roll(chance: u32, rand: u32) -> bool {
    rand % 100 < chance
}

impl Zones {
    /// The monster a capture item is used on (section 3.1 steps 2 and 3): of the monsters on the map that are alive, fighting `user` and
    /// within `range` of it, the one with the lowest HP in percent, the nearest on a tie.
    pub fn weakest_fighting(&self, map: u16, user: u32, range: f32) -> Option<Mob> {
        let at = self.maps.guard().get(&map)?.get(&user)?.occupant.position;
        let all = self.mobs.guard();
        all.get(&map)?
            .mobs
            .values()
            .filter(|m| m.dead_until.is_none() && m.hp > 0 && m.state == MobState::Fighting(user) && distance(m.position, at) <= range)
            .min_by(|a, b| {
                let key = |m: &Mob| (hp_percent(m.hp, m.max_hp), distance(m.position, at));
                let (x, y) = (key(a), key(b));
                x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(a.id.cmp(&b.id))
            })
            .copied()
    }

    /// Whether a monster can be caught (section 3.2): from a spawn area, not a Lua, scripted, unbeatable or script-spawned monster, not
    /// a boss or a boss's follower or chest, no quest item, a pet record in the egg, not excepted, and not too far above the user's level.
    pub fn can_catch(&self, mob: &Mob, def: &MobDef, world: &World, capture: &Capture, egg: &PetEgg, user_level: u16) -> bool {
        if !matches!(mob.origin, Origin::Area) || mob.lua || mob.scripted || mob.unbeatable || self.ext_spawns.guard().contains_key(&mob.id)
        {
            return false;
        }
        let in_group = world.boss_groups().maps.values().flatten().any(|group| {
            let followers = group.followers.iter().flat_map(|f| f.mobs.iter().copied().chain([f.chest]));
            group.stages.iter().copied().chain(followers).flatten().any(|id| id == mob.mob_id)
        });
        if in_group || def.quest_item.is_some() || !in_ranges(&egg.monsters, mob.mob_id) || in_ranges(&capture.except, mob.mob_id) {
            return false;
        }
        capture.levels_above.is_none_or(|more| def.level <= user_level.saturating_add(u16::from(more)))
    }

    /// Takes the monster out as a death without rewards (section 3.4), if it is still alive, still fighting `user` and still at most
    /// `below` percent: `0x0602` to its view at once, its records cleared as `MobMap::remove` does but the monster itself kept, HP 0,
    /// idle, its body due now. The next `remove_bodies` pass takes it away and counts its spawn entry down, so it respawns as usual.
    /// Nothing a death gives: no experience, kill, drop, gold, kill count, quest progress or event. Returns the monster's id.
    pub fn capture_take(&self, map: u16, user: u32, mob_id: u32, below: u8, now: Instant) -> Option<u16> {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mobs = all.get_mut(&map)?;
        let mob = mobs.mobs.get(&mob_id).copied()?;
        if mob.dead_until.is_some()
            || mob.hp == 0
            || mob.state != MobState::Fighting(user)
            || hp_percent(mob.hp, mob.max_hp) > u32::from(below)
        {
            return None;
        }
        let packet = w::mob_leave(mob_id);
        maps.get(&map).into_iter().flat_map(HashMap::values).filter(|p| in_view(p.cell, mob.cell)).for_each(|p| p.send(&packet));
        mobs.debuffs.remove(&mob_id);
        mobs.damage.remove(&mob_id);
        mobs.trails.remove(&mob_id);
        mobs.paths.remove(&mob_id);
        mobs.scripts.remove(&mob_id);
        mobs.lua.remove(&mob_id);
        mobs.lua_events.retain(|&(id, _, _)| id != mob_id);
        let mob = mobs.mobs.get_mut(&mob_id)?;
        mob.hp = 0;
        mob.state = MobState::Idle;
        mob.dead_until = Some(now);
        Some(mob.mob_id)
    }

    /// Puts a monster in a fight with a player at an HP (the integration tests have no way to make a monster aggro).
    #[doc(hidden)]
    pub fn force_fight(&self, map: u16, mob_id: u32, player: u32, hp: u32) {
        if let Some(mob) = self.mobs.guard().get_mut(&map).and_then(|m| m.mobs.get_mut(&mob_id)) {
            mob.state = MobState::Fighting(player);
            mob.hp = hp;
        }
    }

    /// The next counter for an egg's craft name (section 4.1): two catches in one second get different names.
    pub fn next_egg_counter(&self) -> u8 {
        self.last_pet_stamp.fetch_add(1, Ordering::Relaxed) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chance_rises_as_the_monster_weakens() {
        let c = |p| catch_chance([10, 60], 30, p);
        // 10 at the threshold, 43 at 10 %, 58 at 1 %; 20 % is 26.67, rounded down (the spec's table says 27: see the build notes).
        assert_eq!((c(30), c(20), c(10), c(1)), (10, 26, 43, 58));
        assert_eq!(c(100), 10, "above the threshold counts as the threshold");
        assert_eq!(catch_chance([0, 100], 100, 1), 99);
        assert_eq!(catch_chance([100, 100], 30, 5), 100);
        assert_eq!(catch_chance([0, 0], 30, 5), 0);
        assert_eq!(catch_chance([60, 10], 30, 1), 11, "a falling chance rounds down too: 60 - 50 * 29 / 30 = 11.67");
        assert_eq!(catch_chance([10, 60], 1, 1), 10);
    }

    #[test]
    fn a_roll_is_below_the_chance() {
        assert!(catch_roll(10, 0) && catch_roll(10, 9) && !catch_roll(10, 10));
        assert!(catch_roll(10, 109) && !catch_roll(10, 110), "the roll is the random number modulo 100");
        assert!(!catch_roll(0, 0) && catch_roll(100, 99));
    }

    #[test]
    fn the_hp_percent_is_one_to_a_hundred() {
        assert_eq!((hp_percent(1000, 1000), hp_percent(300, 1000), hp_percent(1, 1000), hp_percent(5, 0)), (100, 30, 1, 100));
        assert_eq!(hp_percent(299, 1000), 29, "rounded down");
    }
}
