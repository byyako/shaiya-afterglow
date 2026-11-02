//! Steps paced to the client's own walking (`docs/original/chase-smoothing.md`): the client walks every move packet at the
//! monster's or NPC's speed and stands still on arrival, so a step sent later than the client gets there shows a stop.

use std::time::{Duration, Instant};

/// The move tick between the passes (deviation 2); a paced step goes this long before the client gets to the last spot.
pub const TICK: Duration = Duration::from_millis(100);

/// Slower than this (units a second) the original's waits are kept.
const SLOWEST: f32 = 0.05;

/// The client's monster speed (`game.exe` 0x431aa0, section 1 step 3): `s / (T × 0.001) − 0.3` units a second, with `s` the step
/// after the move speed index and `T` the step's time; `None` when too slow to pace.
pub fn mob_speed(step: i32, time: Duration) -> Option<f32> {
    let t = time.as_millis() as f32;
    if t <= 0.0 {
        return None;
    }
    let v = step as f32 / (t * 0.001) - 0.3;
    (v > SLOWEST).then_some(v)
}

/// The client's NPC speed (`game.exe` 0x433e26, section 7): `nMoveDist / (nMoveSpeed × 0.001)` units a second, four times that
/// running; `None` when too slow to pace.
pub fn npc_speed(move_dist: u32, move_speed: u32, run: bool) -> Option<f32> {
    if move_speed == 0 {
        return None;
    }
    let v = move_dist as f32 / (move_speed as f32 * 0.001) * if run { 4.0 } else { 1.0 };
    (v > SLOWEST).then_some(v)
}

/// After a step of `length` at the client's `speed`: when the client gets there (it first finishes the step before, `eta`) and
/// when the next step may go, one tick before that (deviation 1).
pub fn paced(now: Instant, eta: Option<Instant>, length: f32, speed: f32) -> (Instant, Instant) {
    let start = eta.map_or(now, |e| e.max(now));
    let eta = start + Duration::from_secs_f32((length / speed).max(0.0));
    (eta, eta.checked_sub(TICK).map_or(now, |t| t.max(now)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_as_the_client() {
        let v = mob_speed(9, Duration::from_millis(1800)).unwrap();
        assert!((v - 4.7).abs() < 1e-4, "{v}");
        // Move speed index 0: the step 4 shorter.
        assert!((mob_speed(5, Duration::from_millis(1800)).unwrap() - 2.4778).abs() < 1e-3);
        assert_eq!(mob_speed(9, Duration::ZERO), None);
        assert_eq!(mob_speed(0, Duration::from_millis(1800)), None, "a step of 0 does not move");
        assert_eq!(npc_speed(16, 8000, false), Some(2.0));
        assert_eq!(npc_speed(16, 8000, true), Some(8.0));
        assert_eq!(npc_speed(16, 0, true), None);
        assert_eq!(npc_speed(0, 8000, true), None);
    }

    #[test]
    fn steps_follow_the_clients_arrival() {
        let now = Instant::now();
        let ms = |t: Instant| t.duration_since(now).as_millis();
        // 9 at 4.5 a second: there in 2 s, the next step 100 ms before.
        let (eta, next) = paced(now, None, 9.0, 4.5);
        assert_eq!((ms(eta), ms(next)), (2000, 1900));
        // The next step sent at 1.9 s: the client first finishes the last 0.1 s.
        let (eta, next) = paced(now + Duration::from_millis(1900), Some(eta), 9.0, 4.5);
        assert_eq!((ms(eta), ms(next)), (4000, 3900));
        // Sent late (the client already stands): from now.
        let (eta, _) = paced(now + Duration::from_millis(5000), Some(eta), 4.5, 4.5);
        assert_eq!(ms(eta), 6000);
        // A short step: never before now.
        let (_, next) = paced(now, None, 0.1, 4.5);
        assert_eq!(next, now);
    }
}
