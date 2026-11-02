//! Monsters fighting back (`docs/original/mob-fight.md`): which attack entry a monster uses, chasing its target and going
//! home. Pure functions; the zone applies them (`Zones::mob_fight`).

use std::time::{Duration, Instant};

use crate::astar;
use crate::buffs::CANNOT;
use crate::mobs::{MOVE_SPEED_STEP, MobAttack, MobDef};
use crate::zone::distance;

/// `g_ConstMobAttackSpeed` by attack speed 0 to 6 (added to the wait after an attack).
const ATTACK_SPEED_MS: [i64; 7] = [1500, 1000, 500, 0, -500, -1000, -1500];
/// The wait when no entry can be used, on top of the speed's (`CMob::AttackAI`).
const NO_ENTRY_MS: i64 = 2500;
/// A target further than this from the monster's home sends it home; one further than this from the monster is dropped
/// (`CMob::Chase`); a new target by aggro must be this close (`CMob::ReTargetByAggro`).
pub const LEASH: f32 = 45.0;
/// How often a monster may change target by aggro.
pub const RETARGET_EVERY: Duration = Duration::from_secs(5);
/// A monster within this of home is home (`CMob::ReturnHome`).
const HOME: f32 = 10.0;

/// `now + ms`, where `ms` may be negative.
pub fn after(now: Instant, ms: i64) -> Instant {
    if ms >= 0 { now + Duration::from_millis(ms as u64) } else { now.checked_sub(Duration::from_millis(ms.unsigned_abs())).unwrap_or(now) }
}

/// `g_ConstMobAttackSpeed[speed]`, the speed clamped to 0..6.
pub fn attack_speed_ms(speed: u8) -> i64 {
    ATTACK_SPEED_MS[usize::from(speed.min(6))]
}

/// The wait when no entry can be used: `now + speed + 2500`.
pub fn idle_wait(now: Instant, speed: u8) -> Instant {
    after(now, attack_speed_ms(speed) + NO_ENTRY_MS)
}

/// When an entry other than the first may be used again after a normal attack: `now + speed + dwTime`.
pub fn entry_wait(now: Instant, speed: u8, entry: &MobAttack) -> Instant {
    after(now, attack_speed_ms(speed) + entry.time.as_millis() as i64)
}

/// The monster's next attack after a normal one (`CMob::Attack`): `now + trunc((speed + entry 0's dwTime) / m_fAttackSpeed)`, the
/// factor 1.0 unless a script changed it.
pub fn next_attack(now: Instant, speed: u8, def: &MobDef, factor: f32) -> Instant {
    let wait = (attack_speed_ms(speed) + def.attacks[0].time.as_millis() as i64) as f32 / factor;
    after(now, wait as i64)
}

/// `trunc(max × f)` as the original computes it: the maximum stored as a float, times the float constant.
fn share(max: u32, f: f32) -> u32 {
    (f64::from(max as f32) * f64::from(f)) as u32
}

/// Whether an attack entry may be used with this HP (`CMob::AttackAI`, the `byOK` table of section 3). Skills and `byOK` 6 are
/// not covered yet: such entries are skipped.
pub fn entry_usable(entry: &MobAttack, hp: u32, max: u32) -> bool {
    if entry.ok == 0 {
        return false;
    }
    match entry.ok {
        3 => share(max, 0.2) <= hp && hp <= share(max, 0.4),
        4 => hp <= share(max, 0.05),
        5 => hp < share(max, 0.2),
        6 => false,
        _ => true,
    }
}

/// Whether a monster that does not walk reaches the player (`CMob::EnableAttackRange`).
pub fn stationary_reach(position: [f32; 3], target: [f32; 3], def: &MobDef, range: u8) -> bool {
    distance(position, target) < f32::from(def.size) + f32::from(range) + 0.3
}

/// Whether a monster walks when it fights (`AI` 4, no chase step and obelisk guards do not).
pub fn walks(def: &MobDef) -> bool {
    def.ai != 4 && def.chase_step != 0 && !def.obelisk
}

/// The map as chasing sees it: the straight line between whole-number spots (`Err` with the last free cell before a blocked
/// one), and whether a cell is blocked or off the map (`CMap::IsBlockSafe`).
pub trait Terrain {
    fn line(&self, from: (i32, i32), to: (i32, i32)) -> Result<(), (i32, i32)>;
    fn blocked(&self, at: (i32, i32)) -> bool;
}

/// What a chase turn does (`CMob::Chase`, section 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chase {
    /// Close enough: the attack goes ahead.
    InRange,
    /// Before the next chase time: nothing.
    Wait,
    /// The target is too far from home: going home without it.
    GoHome,
    /// Out of reach: idle again without the target.
    Drop,
    /// Stuck at the edge of a blocked line: going home, and a kill gives nothing (`m_bFairCombat` cleared).
    Unfair,
    /// A step towards the target (motion 1) or none (`to` None); `trail` goes on the chase path; the next chase at `next`.
    Step { to: Option<[f32; 3]>, next: Instant, trail: (i32, i32) },
}

pub(crate) fn cell(position: [f32; 3]) -> (i32, i32) {
    (position[0] as i32, position[2] as i32)
}

/// The path a monster walks (`CMob+0x7c`, `docs/original/pathing.md` section 2): the cells still to reach, the current one first;
/// empty when it has none.
pub type Path = Vec<(i32, i32)>;

/// The step vector towards a whole-number goal from the monster's truncated position: the vector, its length, its direction.
pub(crate) fn towards(position: [f32; 3], goal: (i32, i32)) -> ([f32; 3], f32, [f32; 3]) {
    let m = cell(position);
    let v = [(goal.0 - m.0) as f32, 0.0, (goal.1 - m.1) as f32];
    let length = (v[0] * v[0] + v[2] * v[2]).sqrt();
    // SVector::Normalize leaves the zero vector as it is.
    let n = if v == [0.0; 3] { v } else { [v[0] / length, 0.0, v[2] / length] };
    (v, length, n)
}

/// `g_ConstMobMoveSpeed[move speed] + step` (`byChaseStep` or `byNormalStep`), a whole number.
fn step_of(step: u8, move_speed: u8) -> i32 {
    MOVE_SPEED_STEP[usize::from(move_speed.min(4))] + i32::from(step)
}

/// `now + trunc(gap / step × wait)`.
fn partial_wait(now: Instant, gap: f32, step: f32, wait: Duration) -> Instant {
    after(now, (f64::from(gap) / f64::from(step) * wait.as_millis() as f64) as i64)
}

/// One chase turn towards `target` with an entry's `range` (section 4, `docs/original/pathing.md` section 3). Without a path, the
/// straight line decides one: the target's cell, the last free cell before a blocked target, or a searched way; then the monster
/// walks the path's cells and does not look again until it ends. Full steps wait `full_wait` (`dwChaseTime`, 400 ms less for
/// `ChaseExp`). At move speed 255 (stopped, stunned, asleep) a monster out of range waits.
#[allow(clippy::too_many_arguments)]
pub fn chase(
    position: [f32; 3],
    home: [f32; 3],
    target: [f32; 3],
    def: &MobDef,
    range: u8,
    move_speed: u8,
    next_chase: Instant,
    now: Instant,
    terrain: &impl Terrain,
    path: &mut Path,
    full_wait: Duration,
) -> Chase {
    if now < next_chase {
        return Chase::Wait;
    }
    let (size, range) = (f32::from(def.size), f32::from(range));
    let d = distance(position, target);
    if size + range + 1.5 > d {
        path.clear();
        return Chase::InRange;
    }
    if move_speed == CANNOT {
        return Chase::Wait;
    }
    if distance(target, home) > LEASH {
        path.clear();
        return Chase::GoHome;
    }
    if d > LEASH {
        path.clear();
        return Chase::Drop;
    }
    let (m, t) = (cell(position), cell(target));
    if path.is_empty() {
        match terrain.line(m, t) {
            Ok(()) => *path = vec![t],
            Err(free) if terrain.blocked(t) => {
                if (position[0] - free.0 as f32).abs() <= 1.0 && (position[2] - free.1 as f32).abs() <= 1.0 {
                    return Chase::Unfair;
                }
                let reach = size + f32::from(def.attacks[0].range) + 0.3;
                if distance([free.0 as f32, 0.0, free.1 as f32], target) > reach {
                    return Chase::GoHome;
                }
                *path = vec![free];
            }
            // No way: `CMob::StatusAttackStop`.
            Err(_) => match astar::find_path(m, t, terrain) {
                Some(way) if !way.is_empty() => *path = way,
                _ => return Chase::Drop,
            },
        }
    }
    let goal = path[0];
    let (v, length, n) = towards(position, goal);
    let whole = step_of(def.chase_step, move_speed);
    let step = whole as f32;
    let [x, y, z] = position;
    if path.len() > 1 {
        // A turn of a searched way: onto it when less than a step away (the next turn at once), else a full step.
        if (length as i32) < whole {
            path.remove(0);
            return Chase::Step { to: Some([goal.0 as f32, y, goal.1 as f32]), next: now, trail: m };
        }
        return Chase::Step { to: Some([x + n[0] * step, y, z + n[2] * step]), next: now + full_wait, trail: m };
    }
    // Computed in extended precision and stored as a float.
    let gap = (f64::from(length) - (f64::from(size) + f64::from(0.4f32)) - f64::from(range)) as f32;
    if gap >= step {
        // The path stays: the next turns head for the same cell.
        Chase::Step { to: Some([x + n[0] * step, y, z + n[2] * step]), next: now + full_wait, trail: m }
    } else if size + 0.6 - range >= length {
        path.clear();
        Chase::Step { to: None, next: now, trail: m }
    } else {
        path.clear();
        let back = size + range + 0.4;
        let to = [x + v[0] - n[0] * back, y, z + v[2] - n[2] * back];
        Chase::Step { to: Some(to), next: partial_wait(now, gap, step, def.chase_time), trail: m }
    }
}

/// Adds a cell to a chase path (`ChasePathList::AddPath`): it replaces the newest cell when the line from the one before is
/// free, is not added again when it is the newest, and is pushed otherwise.
pub fn add_trail(trail: &mut Vec<(i32, i32)>, at: (i32, i32), terrain: &impl Terrain) {
    if trail.len() >= 2 && terrain.line(trail[trail.len() - 2], at).is_ok() {
        *trail.last_mut().expect("two cells") = at;
        return;
    }
    if trail.last() != Some(&at) {
        trail.push(at);
    }
}

/// What a turn of going home (or of a Lua move) does (`CMob::ReturnHome`, section 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Homeward {
    Wait,
    /// There (home: within 10, or 0.5 with a chase path left): both paths cleared.
    Arrived,
    /// A step and the next turn.
    Step {
        to: [f32; 3],
        next: Instant,
    },
    /// No way: home becomes where it stands, both paths cleared.
    Settle,
    /// Home is blocked and the way is too: both paths cleared, home unchanged.
    Idle,
}

/// The cell to walk to when there is no path (`docs/original/pathing.md` section 4 item 1): the goal on a free line (no path set);
/// blocked, the chase path's newest cell (taken off) or a searched way. `check_goal`: a blocked goal cell ends it first (going
/// home; a Lua move does not test it).
fn way_to(
    m: (i32, i32),
    goal: (i32, i32),
    trail: &mut Vec<(i32, i32)>,
    path: &mut Path,
    terrain: &impl Terrain,
    check_goal: bool,
) -> Result<(), Homeward> {
    if !path.is_empty() {
        return Ok(());
    }
    match terrain.line(m, goal) {
        Ok(()) => Ok(()),
        Err(free) if check_goal && terrain.blocked(goal) => Err(if free == m { Homeward::Settle } else { Homeward::Idle }),
        Err(_) => {
            if let Some(at) = trail.pop() {
                *path = vec![at];
                return Ok(());
            }
            match astar::find_path(m, goal, terrain) {
                Some(way) if !way.is_empty() => {
                    *path = way;
                    Ok(())
                }
                _ => Err(Homeward::Settle),
            }
        }
    }
}

/// A step towards the path's current cell (or `goal` without a path): a full one when the gap `L − (bySize + 1)` allows it, else onto
/// the cell, a share of the wait, and the path's next cell.
fn walk_step(position: [f32; 3], goal: (i32, i32), path: &mut Path, size: u8, whole: i32, wait: Duration, now: Instant) -> Homeward {
    let goal = path.first().copied().unwrap_or(goal);
    let (_, length, n) = towards(position, goal);
    let step = whole as f32;
    let gap = length - (f32::from(size) + 1.0);
    let [x, y, z] = position;
    if gap >= step {
        return Homeward::Step { to: [x + n[0] * step, y, z + n[2] * step], next: now + wait };
    }
    if !path.is_empty() {
        path.remove(0);
    }
    Homeward::Step { to: [goal.0 as f32, y, goal.1 as f32], next: partial_wait(now, gap, step, wait) }
}

/// One turn of going home (`CMob::ReturnHome`, also `CMob::WhileReturnHome`): running steps (`byChaseStep`, `dwChaseTime`). At move
/// speed 255 it waits.
#[allow(clippy::too_many_arguments)]
pub fn homeward(
    position: [f32; 3],
    home: [f32; 3],
    def: &MobDef,
    trail: &mut Vec<(i32, i32)>,
    path: &mut Path,
    move_speed: u8,
    next_chase: Instant,
    now: Instant,
    terrain: &impl Terrain,
) -> Homeward {
    if now < next_chase {
        return Homeward::Wait;
    }
    let d = distance(position, home);
    if (!trail.is_empty() && d < 0.5) || d < HOME {
        trail.clear();
        path.clear();
        return Homeward::Arrived;
    }
    if move_speed == CANNOT {
        return Homeward::Wait;
    }
    let h = cell(home);
    if let Err(end) = way_to(cell(position), h, trail, path, terrain, true) {
        trail.clear();
        path.clear();
        return end;
    }
    walk_step(position, h, path, def.size, step_of(def.chase_step, move_speed), def.chase_time, now)
}

/// One turn of a Lua move to `spot` (`CMob::WhileMove`): walking steps (`byNormalStep`, `dwNormalTime`, motion 0), there when within
/// 0.5 with a chase path left or within `5 + rand() % 5` (drawn only then); no way: `Settle` (the move ends as on arrival).
#[allow(clippy::too_many_arguments)]
pub fn moving(
    position: [f32; 3],
    spot: [f32; 3],
    def: &MobDef,
    trail: &mut Vec<(i32, i32)>,
    path: &mut Path,
    move_speed: u8,
    next_step: Instant,
    now: Instant,
    terrain: &impl Terrain,
    rand: &mut impl FnMut() -> u32,
) -> Homeward {
    if now < next_step {
        return Homeward::Wait;
    }
    let d = distance(position, spot);
    if (!trail.is_empty() && d < 0.5) || d < (rand() % 5 + 5) as f32 {
        trail.clear();
        path.clear();
        return Homeward::Arrived;
    }
    if move_speed == CANNOT {
        return Homeward::Wait;
    }
    let goal = cell(spot);
    if let Err(end) = way_to(cell(position), goal, trail, path, terrain, false) {
        trail.clear();
        path.clear();
        return end;
    }
    walk_step(position, goal, path, def.size, step_of(def.normal_step, move_speed), def.normal_time, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The move speed of a monster without debuffs.
    const MOVE: u8 = crate::mobs::MOVE_SPEED;
    const SEC: Duration = Duration::from_secs(1);

    /// An open field, with an optional wall of blocked cells at x = `wall` (cells at and beyond `beyond` blocked too).
    struct Field {
        wall: Option<i32>,
    }

    impl Terrain for Field {
        fn line(&self, from: (i32, i32), to: (i32, i32)) -> Result<(), (i32, i32)> {
            let Some(wall) = self.wall else { return Ok(()) };
            if (from.0 < wall) == (to.0 < wall) && to.0 != wall {
                return Ok(());
            }
            Err((wall - 1, from.1))
        }
        fn blocked(&self, at: (i32, i32)) -> bool {
            self.wall.is_some_and(|wall| at.0 >= wall)
        }
    }

    fn def() -> MobDef {
        MobDef { size: 1, chase_step: 8, chase_time: Duration::from_millis(1000), ..Default::default() }
    }

    #[test]
    fn entries_follow_their_conditions() {
        let e = |ok, skill| MobAttack { ok, skill, ..Default::default() };
        assert!(entry_usable(&e(1, 0), 1, 100));
        assert!(!entry_usable(&e(0, 0), 1, 100));
        assert!(entry_usable(&e(1, 139), 1, 100), "skill entries follow the same conditions");
        assert!(entry_usable(&e(3, 0), 40, 100) && entry_usable(&e(3, 0), 20, 100) && !entry_usable(&e(3, 0), 41, 100));
        assert!(!entry_usable(&e(3, 0), 19, 100));
        assert!(entry_usable(&e(4, 0), 5, 100) && !entry_usable(&e(4, 0), 6, 100));
        assert!(entry_usable(&e(5, 0), 19, 100) && !entry_usable(&e(5, 0), 20, 100));
        assert!(!entry_usable(&e(6, 0), 1, 100), "6 is the zone's (it needs the damage record)");
        assert_eq!(attack_speed_ms(3), 0);
        assert_eq!(attack_speed_ms(9), -1500);
    }

    #[test]
    fn a_chase_steps_closes_in_and_attacks() {
        let now = Instant::now();
        let open = Field { wall: None };
        let home = [100.0, 0.0, 100.0];
        // 20 away on x: gap 20 - 1.4 - 2 = 16.6 >= 8: a full step of 8.
        let c = chase([100.5, 3.0, 100.5], home, [120.5, 0.0, 100.5], &def(), 2, MOVE, now, now, &open, &mut Path::new(), SEC);
        assert_eq!(c, Chase::Step { to: Some([108.5, 3.0, 100.5]), next: now + Duration::from_secs(1), trail: (100, 100) });
        // 10 away: gap 6.6 < 8, so it stops 3.4 short of the target's cell: 110 - 3.4 + 0.5.
        let Chase::Step { to: Some(to), next, .. } =
            chase([100.5, 0.0, 100.5], home, [110.5, 0.0, 100.5], &def(), 2, MOVE, now, now, &open, &mut Path::new(), SEC)
        else {
            panic!()
        };
        assert!((to[0] - 107.1).abs() < 1e-4 && to[2] == 100.5, "{to:?}");
        assert_eq!(next, now + Duration::from_millis(824), "6.6f / 8 of the chase time is 824.99998 ms");
        assert_eq!(
            chase([100.0, 0.0, 100.0], home, [104.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &open, &mut Path::new(), SEC),
            Chase::InRange,
            "1 + 2 + 1.5 > 4"
        );
        assert_eq!(
            chase(
                [100.0, 0.0, 100.0],
                home,
                [104.0, 0.0, 100.0],
                &def(),
                2,
                MOVE,
                now + Duration::from_secs(1),
                now,
                &open,
                &mut Path::new(),
                SEC
            ),
            Chase::Wait
        );
        assert_eq!(
            chase([100.0, 0.0, 100.0], home, [146.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &open, &mut Path::new(), SEC),
            Chase::GoHome,
            "46 from home"
        );
        assert_eq!(
            chase([140.0, 0.0, 100.0], home, [94.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &open, &mut Path::new(), SEC),
            Chase::Drop
        );
        // A slower monster (move speed 0) steps 4 less.
        let slow = chase([100.5, 3.0, 100.5], home, [120.5, 0.0, 100.5], &def(), 2, 0, now, now, &open, &mut Path::new(), SEC);
        assert!(matches!(slow, Chase::Step { to: Some([104.5, ..]), .. }), "{slow:?}");
        // Stopped: it still strikes in range but does not move.
        assert_eq!(
            chase([100.0, 0.0, 100.0], home, [104.0, 0.0, 100.0], &def(), 2, CANNOT, now, now, &open, &mut Path::new(), SEC),
            Chase::InRange
        );
        assert_eq!(
            chase([100.5, 0.0, 100.5], home, [120.5, 0.0, 100.5], &def(), 2, CANNOT, now, now, &open, &mut Path::new(), SEC),
            Chase::Wait
        );
    }

    #[test]
    fn walls_stop_a_chase() {
        let now = Instant::now();
        let home = [100.0, 0.0, 100.0];
        let wall = Field { wall: Some(110) };
        // Target behind the wall, standing right at the last free cell: unfair.
        assert_eq!(
            chase([109.2, 0.0, 100.0], home, [115.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &wall, &mut Path::new(), SEC),
            Chase::Unfair
        );
        // Further back, the target too far from the last free cell (109, 100): home.
        assert_eq!(
            chase([100.0, 0.0, 100.0], home, [125.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &wall, &mut Path::new(), SEC),
            Chase::GoHome
        );
    }

    #[test]
    fn going_home_steps_and_arrives() {
        let now = Instant::now();
        let open = Field { wall: None };
        let home = [100.0, 0.0, 100.0];
        assert_eq!(homeward([109.0, 0.0, 100.0], home, &def(), &mut vec![], &mut Path::new(), MOVE, now, now, &open), Homeward::Arrived);
        // 30 away: gap 30 - 2 = 28 >= 8.
        assert_eq!(
            homeward([130.0, 0.0, 100.0], home, &def(), &mut vec![], &mut Path::new(), MOVE, now, now, &open),
            Homeward::Step { to: [122.0, 0.0, 100.0], next: now + Duration::from_secs(1) }
        );
        assert_eq!(
            homeward([130.0, 0.0, 100.0], home, &def(), &mut vec![], &mut Path::new(), CANNOT, now, now, &open),
            Homeward::Wait,
            "stopped"
        );
        // 12 away: gap 10 >= 8, still a full step; at 10.5 (cell 110): gap 8 >= 8.
        let Homeward::Step { to, .. } = homeward([112.0, 0.0, 100.0], home, &def(), &mut vec![], &mut Path::new(), MOVE, now, now, &open)
        else {
            panic!()
        };
        assert_eq!(to, [104.0, 0.0, 100.0]);
        let mut trail = Vec::new();
        for at in [(100, 100), (105, 100), (110, 100), (110, 100)] {
            add_trail(&mut trail, at, &open);
        }
        assert_eq!(trail, [(100, 100), (110, 100)], "free lines merge cells");
    }

    /// Map 1, 256 cells a side, with the listed cells blocked.
    fn walled(walls: impl IntoIterator<Item = (i32, i32)>) -> crate::world::World {
        let mut world = crate::world::World::build([(1, 256)], &std::collections::HashSet::new(), []);
        let mut bits = vec![0u8; 256 * 256 / 8];
        for (x, z) in walls {
            let bit = (z * 256 + x) as usize;
            bits[bit / 8] |= 1 << (bit % 8);
        }
        world.set_mob_data([(1, bits)].into(), std::collections::HashMap::new());
        world
    }

    #[test]
    fn a_chase_keeps_the_spot_it_set_out_for() {
        let now = Instant::now();
        let world = walled([]);
        let terrain = crate::world::MapTerrain { world: &world, map: 1 };
        let home = [100.0, 0.0, 100.0];
        let mut path = Path::new();
        let c = chase([100.5, 0.0, 100.5], home, [120.5, 0.0, 100.5], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC);
        assert!(matches!(c, Chase::Step { to: Some([108.5, ..]), .. }), "{c:?}");
        assert_eq!(path, [(120, 100)]);
        // The target has gone north; the monster still heads for (120, 100).
        let c = chase([108.5, 0.0, 100.5], home, [100.5, 0.0, 130.5], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC);
        assert!(matches!(c, Chase::Step { to: Some([116.5, _, 100.5]), .. }), "{c:?}");
        // Short of the cell it stands off from it and the path ends; the next turn aims at the target again.
        let c = chase([116.5, 0.0, 100.5], home, [100.5, 0.0, 130.5], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC);
        assert!(matches!(c, Chase::Step { to: Some(_), .. }) && path.is_empty(), "{c:?} {path:?}");
        // In range ends the path at once, wherever it led.
        let mut path = vec![(150, 150)];
        assert_eq!(
            chase([100.0, 0.0, 100.0], home, [104.0, 0.0, 100.0], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC),
            Chase::InRange
        );
        assert!(path.is_empty());
    }

    #[test]
    fn a_blocked_chase_walks_a_searched_way() {
        let now = Instant::now();
        // A wall at x = 105 from z = 90 to 104; the box from (100, 100) to (115, 104) reaches z 98..106, so round its north end.
        let world = walled((90..=104).map(|z| (105, z)));
        let terrain = crate::world::MapTerrain { world: &world, map: 1 };
        let home = [100.0, 0.0, 100.0];
        let way = astar::find_path((100, 100), (115, 104), &terrain).expect("round the north end");
        assert!(way.len() > 1 && way.last() == Some(&(115, 104)), "{way:?}");
        let mut path = Path::new();
        let c = chase([100.5, 0.0, 100.5], home, [115.5, 0.0, 104.5], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC);
        let Chase::Step { to: Some(to), next, trail } = c else { panic!("{c:?}") };
        assert_eq!(trail, (100, 100));
        let (dx, dz) = (way[0].0 - 100, way[0].1 - 100);
        if (((dx * dx + dz * dz) as f32).sqrt() as i32) < 8 {
            assert_eq!((path.as_slice(), next, to), (&way[1..], now, [way[0].0 as f32, 0.0, way[0].1 as f32]), "onto the first turn");
        } else {
            assert_eq!((path, next), (way, now + SEC), "a full step towards the first turn");
        }
        // Without a way (the target's row walled across, start and goal on one row): the monster stops.
        let world = walled((90..=110).map(|z| (105, z)));
        let terrain = crate::world::MapTerrain { world: &world, map: 1 };
        let mut path = Path::new();
        assert_eq!(chase([100.5, 0.0, 100.5], home, [110.5, 0.0, 100.5], &def(), 2, MOVE, now, now, &terrain, &mut path, SEC), Chase::Drop);
        assert!(path.is_empty());
    }

    #[test]
    fn going_home_round_a_wall() {
        let now = Instant::now();
        let world = walled((90..=110).map(|z| (115, z)));
        let terrain = crate::world::MapTerrain { world: &world, map: 1 };
        let home = [100.0, 0.0, 100.0];
        // Blocked with a chase path: its newest cell is taken off as the monster sets out for it.
        let (mut trail, mut path) = (vec![(110, 100), (114, 112)], Path::new());
        let h = homeward([120.5, 0.0, 100.5], home, &def(), &mut trail, &mut path, MOVE, now, now, &terrain);
        assert_eq!((trail.as_slice(), path.as_slice()), (&[(110, 100)][..], &[(114, 112)][..]), "{h:?}");
        // Blocked, no chase path, home on the same row: no way within the box, home becomes where it stands.
        let (mut trail, mut path) = (vec![], Path::new());
        assert_eq!(homeward([120.5, 0.0, 100.5], home, &def(), &mut trail, &mut path, MOVE, now, now, &terrain), Homeward::Settle);
        // From (120, 112) the line crosses the wall at z 109 and the box (z 94..118) reaches past its end at z 110.
        let h = homeward([120.5, 0.0, 112.5], home, &def(), &mut trail, &mut path, MOVE, now, now, &terrain);
        assert!(matches!(h, Homeward::Step { .. }) && path.last() == Some(&(100, 100)), "{h:?} {path:?}");
    }

    #[test]
    fn a_lua_move_walks_and_ends() {
        let now = Instant::now();
        let world = walled((90..=110).map(|z| (115, z)));
        let terrain = crate::world::MapTerrain { world: &world, map: 1 };
        let mut def = def();
        (def.normal_step, def.normal_time) = (4, Duration::from_millis(2000));
        let mut rand = || 4u32;
        // Within 5 + 4 = 9: there.
        let (mut trail, mut path) = (vec![], Path::new());
        let spot = [130.0, 0.0, 100.0];
        assert_eq!(moving([122.0, 0.0, 100.0], spot, &def, &mut trail, &mut path, MOVE, now, now, &terrain, &mut rand), Homeward::Arrived);
        // A walking step of 4 every 2 s on a free line.
        assert_eq!(
            moving([140.0, 0.0, 100.0], [160.0, 0.0, 100.0], &def, &mut trail, &mut path, MOVE, now, now, &terrain, &mut rand),
            Homeward::Step { to: [144.0, 0.0, 100.0], next: now + Duration::from_millis(2000) }
        );
        // Blocked on one row with no chase path: the move ends.
        assert_eq!(
            moving([110.5, 0.0, 100.5], [130.0, 0.0, 100.0], &def, &mut trail, &mut path, MOVE, now, now, &terrain, &mut rand),
            Homeward::Settle
        );
    }
}
