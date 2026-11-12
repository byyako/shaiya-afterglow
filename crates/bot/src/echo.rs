//! The step echo (section 6 of `docs/plans/test-bots.md`): how long after a bot's step another bot sees it. One step in ten is stamped on
//! a board every bot can read; a bot that sees another's step compares what it saw with the stamp. The board is a row of atomics, so no
//! bot waits on a lock in its packet path.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// One tick of a stamp: 100 microseconds, so a 32-bit time lasts five days.
const TICK: Duration = Duration::from_micros(100);
/// A step is stamped every this many steps of a bot.
pub const EVERY: u64 = 10;
/// A sighting this long after the stamp is not the same step (the stamp is overwritten ten steps on, seconds later).
const LONGEST: Duration = Duration::from_secs(30);

pub struct EchoBoard {
    base: Instant,
    /// Per bot: the position's hash in the high half and the time of the step in ticks plus one in the low half; 0 when none.
    slots: Vec<AtomicU64>,
    /// A character id to its slot.
    index: HashMap<u32, usize>,
}

/// A step's place as 32 bits: both coordinates' bits, mixed.
fn hash(x: f32, z: f32) -> u32 {
    x.to_bits() ^ z.to_bits().rotate_left(13)
}

impl EchoBoard {
    /// A board for the characters `ids`; slot `i` is that of `ids[i]`.
    pub fn new(ids: &[u32], base: Instant) -> Arc<Self> {
        Arc::new(Self {
            base,
            slots: ids.iter().map(|_| AtomicU64::new(0)).collect(),
            index: ids.iter().enumerate().map(|(slot, &id)| (id, slot)).collect(),
        })
    }

    fn ticks(&self, at: Instant) -> u64 {
        (at.saturating_duration_since(self.base).as_micros() / TICK.as_micros()) as u64
    }

    /// Bot `slot` sent a step to `(x, z)` just now.
    pub fn stamp(&self, slot: usize, x: f32, z: f32, at: Instant) {
        if let Some(cell) = self.slots.get(slot) {
            let time = (self.ticks(at) + 1) & 0xffff_ffff;
            cell.store(u64::from(hash(x, z)) << 32 | time, Ordering::Relaxed);
        }
    }

    /// Another bot's step to `(x, z)` was seen at `at`: the delay since its stamp, when it is the stamped step.
    pub fn seen(&self, mover: u32, x: f32, z: f32, at: Instant) -> Option<Duration> {
        let stamp = self.slots[*self.index.get(&mover)?].load(Ordering::Relaxed);
        if stamp == 0 || (stamp >> 32) as u32 != hash(x, z) {
            return None;
        }
        let stamped = (stamp & 0xffff_ffff).wrapping_sub(1);
        let now = self.ticks(at) & 0xffff_ffff;
        let delay = TICK * u32::try_from(now.wrapping_sub(stamped) & 0xffff_ffff).ok()?;
        (delay < LONGEST).then_some(delay)
    }
}

/// A bot's handle on the board.
pub struct Echo {
    pub board: Arc<EchoBoard>,
    pub slot: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamped_step_is_timed_by_the_one_who_sees_it_and_no_other() {
        let base = Instant::now();
        let board = EchoBoard::new(&[10, 20], base);
        let t = base + Duration::from_secs(3);
        board.stamp(1, 5.5, -7.25, t);
        // Seen 42 ms later: the same position of the right mover.
        let d = board.seen(20, 5.5, -7.25, t + Duration::from_millis(42)).unwrap();
        assert!(d >= Duration::from_millis(41) && d <= Duration::from_millis(43), "{d:?}");
        // Another position, another mover, an unknown mover and an unstamped one are not it.
        assert_eq!(board.seen(20, 5.5, -7.0, t), None);
        assert_eq!(board.seen(10, 5.5, -7.25, t), None);
        assert_eq!(board.seen(99, 5.5, -7.25, t), None);
        // A stamp from long before is not this step.
        assert_eq!(board.seen(20, 5.5, -7.25, t + Duration::from_secs(60)), None);
    }
}
