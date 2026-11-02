//! Walking NPCs and guards (`docs/original/guards.md`): an NPC's walk along its positions, a guard's chase and its way back. Pure
//! functions; the zone applies them (`Zones::npc_attack`, `Zones::npc_idle`).

use std::time::{Duration, Instant};

use crate::astar;
use crate::fight::{Path, Terrain, add_trail, after, cell, towards};
use crate::world::NpcDef;
use crate::zone::distance;

/// The NPC type that fights.
pub const GUARD: u8 = 8;
/// A guard takes a target within this of itself (`CNpc::FindTarget`).
pub const SIGHT: f32 = 20.0;
/// A target further than this from the guard's position, or from the guard, is given up; a new one must be this close to the
/// guard's position.
pub const LEASH: f32 = 30.0;
/// Within this of its position a guard going back is there (`CNpc::ReturnHome`).
const BACK: f32 = 1.5;
/// The range a guard chases into (`UpdateAttackAI` passes 2; in range below 2 + 1).
const RANGE: f32 = 2.0;
/// How far short of the cell a guard's last chase step stops (it is then put on the cell itself).
const SHORT: f32 = 0.3;
/// A target stays taken for this long (`now + 5000` on the player or the monster).
pub const TAKEN: Duration = Duration::from_secs(5);
/// A guard that finds no way waits this long (`m_dwDisableTime = now + 3000`).
pub const STUCK: Duration = Duration::from_secs(3);
/// The waits after a guard's blow: a kill (or a dead target), a player who lived.
pub const AFTER_KILL: Duration = Duration::from_millis(1000);
pub const AFTER_BLOW: Duration = Duration::from_millis(500);
/// More than this from its current NPC after a move, a player lets go of it (`CUser::PacketPC`, section 2).
pub const USE_RANGE: f32 = 10.0;

/// One turn of a walk (`CNpc::UpdateMove`, section 3): the spot to move to, whether it is the position itself (then, once the move
/// is made, the next position), and the next turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkTurn {
    pub to: [f32; 3],
    pub arrival: bool,
    pub next: Instant,
}

/// A walk's turn towards `point` from `position`. `None` for an NPC with move distance 0 (deviation 2: the original makes a
/// zero-length move every turn).
pub fn walk(position: [f32; 3], point: [f32; 3], def: &NpcDef, now: Instant, rand: &mut impl FnMut() -> u32) -> Option<WalkTurn> {
    if def.move_dist == 0 {
        return None;
    }
    let goal = [point[0] - (position[0] as i32) as f32, 0.0, point[2] - (position[2] as i32) as f32];
    let length = (goal[0] * goal[0] + goal[2] * goal[2]).sqrt();
    let step = def.move_dist as f32;
    if length <= step {
        let wait = u64::from(rand() % 10_000) + u64::from(def.move_speed) + 10_000;
        return Some(WalkTurn { to: point, arrival: true, next: now + Duration::from_millis(wait) });
    }
    let n = [goal[0] / length, 0.0, goal[2] / length];
    let to = [position[0] + n[0] * step, position[1], position[2] + n[2] * step];
    Some(WalkTurn { to, arrival: false, next: now + Duration::from_millis(u64::from(def.move_speed)) })
}

/// The position after `index` on a walk of `count` positions, back and forth: forwards to the end, then from `count − 2` backwards
/// to 0, then from 1 forwards again.
pub fn next_point(index: usize, forwards: bool, count: usize) -> (usize, bool) {
    if forwards {
        if index + 1 < count { (index + 1, true) } else { (count.saturating_sub(2), false) }
    } else if index >= 1 {
        (index - 1, false)
    } else {
        (1, true)
    }
}

/// The wait of a running step: a quarter of the move time (an integer division).
fn run_wait(def: &NpcDef) -> Duration {
    Duration::from_millis(u64::from(def.move_speed / 4))
}

/// `now + trunc(part × run_wait)`, where `part` may be negative.
fn part_wait(now: Instant, part: f32, def: &NpcDef) -> Instant {
    after(now, (f64::from(part) * f64::from(def.move_speed / 4)) as i64)
}

/// What a chase turn does (`CNpc::Chase`, section 8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chase {
    /// Before the next chase time.
    Wait,
    /// Close enough: the blow goes ahead (the path ended).
    InRange,
    /// The square under the guard is blocked: idle without a target (the path kept).
    Blocked,
    /// `CNpc::StatusAttackStop`: idle without a target (the path ended).
    Stop,
    /// Out of reach: back to its position without a target (the path ended); `stuck`: no way was found, nothing for 3 s.
    GiveUp { stuck: bool },
    /// A running step (`to` None: none, deviation 2); once it is made the guard stands at `then` when given (the original's quiet
    /// 0.3 more); `trail` goes on the chase path first; the next chase at `next`.
    Step { to: Option<[f32; 3]>, then: Option<[f32; 3]>, next: Instant },
}

/// One chase turn from `position` towards `target` with the guard's position `point`. Without a path the straight line decides: the
/// target's cell (no path kept), a blocked target's end, or a searched way; with one, its current cell. The guard's cell goes on
/// its chase path (`trail`) whenever it steps.
#[allow(clippy::too_many_arguments)]
pub fn chase(
    position: [f32; 3],
    target: [f32; 3],
    point: [f32; 3],
    def: &NpcDef,
    next_chase: Instant,
    now: Instant,
    terrain: &impl Terrain,
    path: &mut Path,
    trail: &mut Vec<(i32, i32)>,
) -> Chase {
    if now < next_chase {
        return Chase::Wait;
    }
    let m = cell(position);
    if terrain.blocked(m) {
        return Chase::Blocked;
    }
    let d = distance(position, target);
    if RANGE + 1.0 > d {
        path.clear();
        return Chase::InRange;
    }
    if distance(target, point) > LEASH || d > LEASH {
        path.clear();
        return Chase::GiveUp { stuck: false };
    }
    let t = cell(target);
    let goal = match path.first() {
        Some(&at) => at,
        None => match terrain.line(m, t) {
            Ok(()) => t,
            Err(free) if terrain.blocked(t) => return if free == m { Chase::GiveUp { stuck: false } } else { Chase::Stop },
            Err(_) => match astar::find_path(m, t, terrain) {
                Some(way) if !way.is_empty() => {
                    *path = way;
                    path[0]
                }
                _ => return Chase::GiveUp { stuck: true },
            },
        },
    };
    let (v, length, n) = towards(position, goal);
    let gap = length - 1.0;
    add_trail(trail, m, terrain);
    let step = def.move_dist as f32;
    let [x, y, z] = position;
    if def.move_dist == 0 {
        return Chase::Step { to: None, then: None, next: now + run_wait(def) };
    }
    if gap >= step {
        return Chase::Step { to: Some([x + n[0] * step, y, z + n[2] * step]), then: None, next: now + run_wait(def) };
    }
    if !path.is_empty() {
        path.remove(0);
    }
    let to = [x + v[0] - n[0] * SHORT, y, z + v[2] - n[2] * SHORT];
    Chase::Step { to: Some(to), then: Some([x + v[0], y, z + v[2]]), next: part_wait(now, gap / step, def) }
}

/// What a turn of going back does (`CNpc::ReturnHome`, section 9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Back {
    Wait,
    /// There: idle, both paths ended.
    Arrived,
    /// Its position is blocked, or (`stuck`) no way was found and nothing for 3 s: idle, both paths ended.
    Idle {
        stuck: bool,
    },
    /// A running step (`to` None: none, deviation 2) and the next turn.
    Step {
        to: Option<[f32; 3]>,
        next: Instant,
    },
}

/// One turn of going back to the guard's position `point`.
#[allow(clippy::too_many_arguments)]
pub fn back(
    position: [f32; 3],
    point: [f32; 3],
    def: &NpcDef,
    next_chase: Instant,
    now: Instant,
    terrain: &impl Terrain,
    path: &mut Path,
    trail: &mut Vec<(i32, i32)>,
) -> Back {
    if now < next_chase {
        return Back::Wait;
    }
    if distance(point, position) < BACK {
        path.clear();
        trail.clear();
        return Back::Arrived;
    }
    let (m, h) = (cell(position), cell(point));
    if path.is_empty() {
        match terrain.line(m, h) {
            Ok(()) => {}
            Err(_) if terrain.blocked(h) => {
                trail.clear();
                return Back::Idle { stuck: false };
            }
            Err(_) => {
                if let Some(at) = trail.pop() {
                    *path = vec![at];
                } else {
                    match astar::find_path(m, h, terrain) {
                        Some(way) if !way.is_empty() => *path = way,
                        _ => {
                            path.clear();
                            trail.clear();
                            return Back::Idle { stuck: true };
                        }
                    }
                }
            }
        }
    }
    let goal = path.first().copied().unwrap_or(h);
    let (v, length, n) = towards(position, goal);
    let step = def.move_dist as f32;
    let [x, y, z] = position;
    if def.move_dist == 0 {
        return Back::Step { to: None, next: now + run_wait(def) };
    }
    if length >= step {
        return Back::Step { to: Some([x + n[0] * step, y, z + n[2] * step]), next: now + run_wait(def) };
    }
    if !path.is_empty() {
        path.remove(0);
    }
    Back::Step { to: Some([x + v[0], y, z + v[2]]), next: part_wait(now, length / step, def) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An open field, with an optional wall of blocked cells from x = `wall` on.
    struct Field {
        wall: Option<i32>,
    }

    impl Terrain for Field {
        fn line(&self, from: (i32, i32), to: (i32, i32)) -> Result<(), (i32, i32)> {
            let Some(wall) = self.wall else { return Ok(()) };
            if (from.0 < wall) == (to.0 < wall) {
                return Ok(());
            }
            Err((wall - 1, from.1))
        }
        fn blocked(&self, at: (i32, i32)) -> bool {
            self.wall.is_some_and(|wall| at.0 >= wall)
        }
    }

    const DEF: NpcDef = NpcDef { move_dist: 16, move_speed: 8000, country: 0 };
    const OPEN: Field = Field { wall: None };

    #[test]
    fn a_walk_steps_then_waits_at_each_position() {
        let now = Instant::now();
        // 40 away on x from the truncated 100: a step of 16 from the real position, the next in 8 s.
        let t = walk([100.5, 7.0, 200.5], [140.0, 9.0, 200.0], &DEF, now, &mut || 0).unwrap();
        assert_eq!(t, WalkTurn { to: [116.5, 7.0, 200.5], arrival: false, next: now + Duration::from_secs(8) });
        // Within 16 of the truncated position: onto the position itself (its height too), then 10 + 8 s and the random part.
        let t = walk([130.9, 7.0, 200.9], [146.0, 9.0, 200.0], &DEF, now, &mut || 12_345).unwrap();
        assert_eq!(t, WalkTurn { to: [146.0, 9.0, 200.0], arrival: true, next: now + Duration::from_millis(2345 + 8000 + 10_000) });
        assert_eq!(walk([100.5, 7.0, 200.5], [140.0, 9.0, 200.0], &NpcDef { move_dist: 0, ..DEF }, now, &mut || 0), None, "deviation 2");
    }

    #[test]
    fn walks_go_back_and_forth() {
        let mut at = (0, true);
        let mut seen = vec![];
        for _ in 0..8 {
            at = next_point(at.0, at.1, 4);
            seen.push(at.0);
        }
        assert_eq!(seen, [1, 2, 3, 2, 1, 0, 1, 2]);
        assert_eq!(next_point(1, true, 2), (0, false));
        assert_eq!(next_point(0, false, 2), (1, true));
    }

    #[test]
    fn a_chase_runs_up_and_stops_short() {
        let now = Instant::now();
        let point = [100.0, 0.0, 100.0];
        let (mut path, mut trail) = (Path::new(), Vec::new());
        // 25 away: |v| - 1 = 24 >= 16, a full step; the next in a quarter of 8 s.
        let c = chase([100.5, 4.0, 100.5], [125.5, 0.0, 100.5], point, &DEF, now, now, &OPEN, &mut path, &mut trail);
        assert_eq!(c, Chase::Step { to: Some([116.5, 4.0, 100.5]), then: None, next: now + Duration::from_secs(2) });
        assert!(path.is_empty(), "a free line keeps no path");
        assert_eq!(trail, [(100, 100)]);
        // 10 away: 9 < 16: 0.3 short of the target's cell, then on it; 9/16 of 2 s.
        let c = chase([115.5, 4.0, 100.5], [125.5, 0.0, 100.5], point, &DEF, now, now, &OPEN, &mut path, &mut trail);
        let Chase::Step { to: Some(to), then: Some(then), next } = c else { panic!("{c:?}") };
        assert!((to[0] - 125.2).abs() < 1e-4 && to[2] == 100.5, "{to:?}");
        assert_eq!(then, [125.5, 4.0, 100.5]);
        assert_eq!(next, now + Duration::from_millis(1125));
        assert_eq!(chase([123.0, 0.0, 100.0], [125.5, 0.0, 100.0], point, &DEF, now, now, &OPEN, &mut path, &mut trail), Chase::InRange);
        assert_eq!(
            chase([100.0, 0.0, 100.0], [110.0, 0.0, 100.0], point, &DEF, now + Duration::from_secs(1), now, &OPEN, &mut path, &mut trail),
            Chase::Wait
        );
    }

    #[test]
    fn a_chase_is_given_up_beyond_its_reach() {
        let now = Instant::now();
        let (mut path, mut trail) = (Path::new(), Vec::new());
        // The target 31 from the guard's position: given up, wherever the guard is.
        let c = chase([120.0, 0.0, 100.0], [131.0, 0.0, 100.0], [100.0, 0.0, 100.0], &DEF, now, now, &OPEN, &mut path, &mut trail);
        assert_eq!(c, Chase::GiveUp { stuck: false });
        // 31 from the guard itself.
        let c = chase([70.0, 0.0, 100.0], [101.0, 0.0, 100.0], [100.0, 0.0, 100.0], &DEF, now, now, &OPEN, &mut path, &mut trail);
        assert_eq!(c, Chase::GiveUp { stuck: false });
    }

    #[test]
    fn a_wall_stops_or_sends_the_guard_back() {
        let now = Instant::now();
        let wall = Field { wall: Some(110) };
        let (mut path, mut trail) = (Path::new(), Vec::new());
        // A target behind the wall (on a blocked cell); the line's last free cell 109 is not the guard's: it stops.
        let c = chase([100.0, 0.0, 100.0], [115.0, 0.0, 100.0], [100.0, 0.0, 100.0], &DEF, now, now, &wall, &mut path, &mut trail);
        assert_eq!(c, Chase::Stop);
        // Right at the wall: back.
        let c = chase([109.5, 0.0, 100.0], [115.0, 0.0, 100.0], [100.0, 0.0, 100.0], &DEF, now, now, &wall, &mut path, &mut trail);
        assert_eq!(c, Chase::GiveUp { stuck: false });
        // Standing in the wall.
        let c = chase([111.0, 0.0, 100.0], [115.0, 0.0, 100.0], [100.0, 0.0, 100.0], &DEF, now, now, &wall, &mut path, &mut trail);
        assert_eq!(c, Chase::Blocked);
    }

    #[test]
    fn going_back_walks_to_the_position() {
        let now = Instant::now();
        let point = [100.0, 0.0, 100.0];
        let (mut path, mut trail) = (Path::new(), vec![(120, 100)]);
        let b = back([130.5, 2.0, 100.5], point, &DEF, now, now, &OPEN, &mut path, &mut trail);
        assert_eq!(b, Back::Step { to: Some([114.5, 2.0, 100.5]), next: now + Duration::from_secs(2) });
        // 14 away (from the truncated 114): onto the cell, 14/16 of 2 s.
        let b = back([114.5, 2.0, 100.5], point, &DEF, now, now, &OPEN, &mut path, &mut trail);
        assert_eq!(b, Back::Step { to: Some([100.5, 2.0, 100.5]), next: now + Duration::from_millis(1750) });
        assert_eq!(back([100.5, 2.0, 100.5], point, &DEF, now, now, &OPEN, &mut path, &mut trail), Back::Arrived);
        assert!(trail.is_empty());
        // Its position in a wall; then behind one: the chase path's newest cell first.
        let wall = Field { wall: Some(110) };
        let mut trail = vec![(105, 90), (108, 95)];
        let b = back([95.5, 0.0, 100.5], [100.0, 0.0, 100.0], &DEF, now, now, &Field { wall: Some(100) }, &mut path, &mut trail);
        assert!(matches!(b, Back::Idle { stuck: false }), "its position is in the wall: {b:?}");
        assert!(trail.is_empty(), "both paths ended");
        let mut trail = vec![(105, 90), (108, 95)];
        let b = back([115.5, 0.0, 100.5], [100.0, 0.0, 100.0], &DEF, now, now, &wall, &mut path, &mut trail);
        assert!(matches!(b, Back::Step { .. }), "{b:?}");
        assert!(path.is_empty() && trail == [(105, 90)], "the newest cell was taken and walked onto: {path:?} {trail:?}");
    }
}
