//! The move speed check (`stUserMoveChk`, `docs/original/move-check.md`): the distance walked in a window of at least five
//! seconds against the fastest speed the player had in it.

use std::time::{Duration, Instant};

/// A window lasts at least this long (`stUserMoveChk::Reset`: start + 5000).
pub const WINDOW: Duration = Duration::from_millis(5000);
/// Units a window always allows on top of its speed (`stUserMoveChk::Update`).
pub const SLACK: f32 = 50.0;
/// `g_ConstUserMoveSpeed` by move speed 0 to 4, units a second (section 2).
pub const SPEEDS: [f32; 5] = [3.0, 4.0, 5.0, 6.0, 7.0];
/// The speed while riding (`CUser::PacketPC` 0x46a378).
pub const RIDING: f32 = 8.0;
/// Charge, whose reach is taken off the distance walked when it reaches its target (`CUser::AttackMob`, `AttackUser`).
pub const CHARGE: u16 = 56;

/// The speed a step is checked at on foot (or summoning): the table at the move speed, clamped to 0..4 (`GetMoveSpeedNow`).
pub fn foot_speed(move_speed: u8) -> f32 {
    SPEEDS[usize::from(move_speed.min(4))]
}

/// How a closed window went: the distance walked, the allowance and the window's length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub walked: f32,
    pub allowed: f32,
    pub seconds: f32,
    pub fastest: f32,
}

/// What a step does to the check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Checked {
    /// Within the window: accepted.
    Open,
    /// The window closed and the step passed; a new one starts at the step.
    Passed(Window),
    /// The window closed with more walked than allowed: refused, and the player goes back to `to`, the window's start, where the
    /// new window starts too.
    Refused { window: Window, to: [f32; 3] },
}

#[derive(Clone, Copy, Debug)]
pub struct MoveCheck {
    start: Instant,
    start_pos: [f32; 3],
    last_pos: [f32; 3],
    walked: f32,
    fastest: f32,
}

impl MoveCheck {
    pub fn new(now: Instant, position: [f32; 3]) -> MoveCheck {
        MoveCheck { start: now, start_pos: position, last_pos: position, walked: 0.0, fastest: 0.0 }
    }

    /// A new window from `position` (`stUserMoveChk::Reset`): entering a zone, a move by the server.
    pub fn reset(&mut self, now: Instant, position: [f32; 3]) {
        *self = MoveCheck::new(now, position);
    }

    /// Takes `n` off the distance walked (`stUserMoveChk::Add`).
    pub fn shorten(&mut self, n: f32) {
        self.walked -= n;
    }

    /// A step to `position` at `speed` (`stUserMoveChk::Update`): the horizontal distance from the last step counts, the
    /// fastest speed is kept, and from the window's end on the window closes.
    pub fn update(&mut self, now: Instant, position: [f32; 3], speed: f32) -> Checked {
        let [x, _, z] = self.last_pos;
        self.walked += ((x - position[0]).powi(2) + (z - position[2]).powi(2)).sqrt();
        self.fastest = self.fastest.max(speed);
        self.last_pos = position;
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed < WINDOW {
            return Checked::Open;
        }
        let seconds = elapsed.as_millis() as f32 / 1000.0;
        let window = Window { walked: self.walked, allowed: seconds * self.fastest + SLACK, seconds, fastest: self.fastest };
        // NaN passes, as the original's compare does.
        if window.allowed.partial_cmp(&window.walked) != Some(std::cmp::Ordering::Less) {
            self.reset(now, position);
            Checked::Passed(window)
        } else {
            let to = self.start_pos;
            self.reset(now, to);
            Checked::Refused { window, to }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> [f32; 3] {
        [x, 10.0, 0.0]
    }

    #[test]
    fn steps_within_the_window_are_open() {
        let t0 = Instant::now();
        let mut check = MoveCheck::new(t0, at(0.0));
        // Any distance within the first five seconds (the 35-unit step check is the handler's).
        for i in 1..=10 {
            assert_eq!(check.update(t0 + Duration::from_millis(400 * i), at(30.0 * i as f32), 5.0), Checked::Open);
        }
        // The window closes at the first step from its end on: 330 walked, 5.0 × 5 + 50 allowed.
        let Checked::Refused { window, to } = check.update(t0 + WINDOW, at(330.0), 5.0) else { panic!() };
        assert_eq!(to, at(0.0), "back to the window's start");
        assert_eq!((window.walked, window.allowed, window.seconds), (330.0, 75.0, 5.0));
        // The new window starts at the window's start: the next step counts from there.
        assert_eq!(
            check.update(t0 + WINDOW + Duration::from_secs(5), at(20.0), 5.0),
            Checked::Passed(Window { walked: 20.0, allowed: 75.0, seconds: 5.0, fastest: 5.0 })
        );
    }

    #[test]
    fn the_allowance_is_the_fastest_speed_plus_fifty() {
        let t0 = Instant::now();
        let mut check = MoveCheck::new(t0, at(0.0));
        // 75 in 5 s at speed 5 passes (equal), 75.5 does not.
        check.update(t0 + Duration::from_secs(1), at(40.0), 5.0);
        assert!(matches!(check.update(t0 + WINDOW, at(75.0), 5.0), Checked::Passed(_)));
        let t1 = t0 + WINDOW;
        check.update(t1 + Duration::from_secs(1), at(110.0), 5.0);
        assert!(matches!(check.update(t1 + WINDOW, at(150.5), 5.0), Checked::Refused { to, .. } if to == at(75.0)));
        // The fastest speed in the window counts, the current one being lower: 7 × 5 + 50 = 85.
        let t2 = t1 + WINDOW;
        check.update(t2 + Duration::from_secs(1), at(110.0), 7.0);
        assert!(matches!(check.update(t2 + WINDOW, at(160.0), 3.0), Checked::Passed(w) if w.allowed == 85.0));
        // A longer window allows more: 10 s at 5 is 100.
        let t3 = t2 + WINDOW;
        assert!(matches!(check.update(t3 + Duration::from_secs(10), at(260.0), 5.0), Checked::Passed(w) if w.allowed == 100.0));
    }

    #[test]
    fn only_horizontal_distance_counts_and_charge_takes_off() {
        let t0 = Instant::now();
        let mut check = MoveCheck::new(t0, at(0.0));
        check.update(t0, [0.0, 500.0, 0.0], 5.0);
        check.update(t0 + Duration::from_secs(1), [30.0, 10.0, 40.0], 5.0);
        check.shorten(22.0);
        let Checked::Passed(window) = check.update(t0 + WINDOW, [30.0, 10.0, 40.0], 5.0) else { panic!() };
        assert_eq!(window.walked, 28.0);
    }

    #[test]
    fn a_reset_starts_from_the_new_position() {
        let t0 = Instant::now();
        let mut check = MoveCheck::new(t0, at(0.0));
        check.update(t0 + Duration::from_secs(1), at(30.0), 5.0);
        // A teleport far away: no distance counted from before it.
        check.reset(t0 + Duration::from_secs(2), at(5000.0));
        assert_eq!(check.update(t0 + Duration::from_secs(3), at(5010.0), 5.0), Checked::Open);
        assert!(matches!(check.update(t0 + Duration::from_secs(7), at(5020.0), 5.0), Checked::Passed(w) if w.walked == 20.0));
        assert_eq!((foot_speed(2), foot_speed(0), foot_speed(9)), (5.0, 3.0, 7.0));
    }
}
