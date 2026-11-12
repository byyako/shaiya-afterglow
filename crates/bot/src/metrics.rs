//! One bot's counters and round trips (section 6 of `docs/plans/test-bots.md`). The swarm's 10-second windows and `report.json` build
//! on these in part 3: nothing here knows of other bots.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::{Duration, Instant};

use game_server::combat as server_combat;

use crate::link::LinkError;

/// Packets and bytes of one opcode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub packets: u64,
    pub bytes: u64,
}

/// Round trips in microseconds.
#[derive(Clone, Debug, Default)]
pub struct Samples(Vec<u64>);

impl Samples {
    pub fn record(&mut self, took: Duration) {
        self.0.push(took.as_micros() as u64);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The `p`th percentile (0 to 100, nearest rank), `None` without samples.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.0.is_empty() {
            return None;
        }
        let mut sorted = self.0.clone();
        sorted.sort_unstable();
        let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
        Some(Duration::from_micros(sorted[rank.min(sorted.len()) - 1]))
    }

    pub fn max(&self) -> Option<Duration> {
        self.0.iter().max().map(|&m| Duration::from_micros(m))
    }

    /// The samples, in microseconds.
    pub fn micros(&self) -> &[u64] {
        &self.0
    }
}

/// Buckets to an octave of microseconds: a percentile read from it is within 4.4 % of the true one.
const PER_OCTAVE: f64 = 16.0;

/// Round trips of many bots kept in a few hundred counters instead of a vector of samples: the swarm's windows add the step echo of
/// every observer (up to 300 times as many samples as steps), which would not fit in memory for a long run. Counts the samples in
/// logarithmic buckets of microseconds; the maximum is kept exactly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Histogram {
    counts: Vec<u32>,
    len: u64,
    max_us: u64,
}

fn bucket_of(us: u64) -> usize {
    if us <= 1 { 0 } else { ((us as f64).log2() * PER_OCTAVE).floor() as usize + 1 }
}

/// The upper edge of a bucket in microseconds.
fn bucket_top(bucket: usize) -> f64 {
    if bucket == 0 { 1.0 } else { (bucket as f64 / PER_OCTAVE).exp2() }
}

impl Histogram {
    pub fn record_micros(&mut self, us: u64) {
        let bucket = bucket_of(us);
        if self.counts.len() <= bucket {
            self.counts.resize(bucket + 1, 0);
        }
        self.counts[bucket] += 1;
        self.len += 1;
        self.max_us = self.max_us.max(us);
    }

    pub fn record(&mut self, took: Duration) {
        self.record_micros(took.as_micros() as u64);
    }

    pub fn add_samples(&mut self, samples: &Samples) {
        for &us in samples.micros() {
            self.record_micros(us);
        }
    }

    pub fn merge(&mut self, other: &Self) {
        if self.counts.len() < other.counts.len() {
            self.counts.resize(other.counts.len(), 0);
        }
        for (mine, theirs) in self.counts.iter_mut().zip(&other.counts) {
            *mine += theirs;
        }
        self.len += other.len;
        self.max_us = self.max_us.max(other.max_us);
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn max(&self) -> Option<Duration> {
        (self.len > 0).then(|| Duration::from_micros(self.max_us))
    }

    /// The `p`th percentile (0 to 100, nearest rank) as the upper edge of its bucket, never above the maximum; `None` without samples.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.len == 0 {
            return None;
        }
        let rank = (((p / 100.0) * self.len as f64).ceil().max(1.0) as u64).min(self.len);
        let mut seen = 0;
        for (bucket, &n) in self.counts.iter().enumerate() {
            seen += u64::from(n);
            if seen >= rank {
                return Some(Duration::from_micros((bucket_top(bucket).round() as u64).min(self.max_us)));
            }
        }
        self.max()
    }
}

#[derive(Debug, Default)]
pub struct Metrics {
    pub received: BTreeMap<u16, Count>,
    pub sent: BTreeMap<u16, Count>,
    /// From a `0x0502` sent to the first result for that target: the delay between the click and the first blow.
    pub attack_round_trip: Samples,
    /// From a `0x0511` sent to its outcome.
    pub skill_round_trip: Samples,
    /// How long after a step of another bot (one in ten is stamped, `crate::echo`) this bot saw it: the delay with which the others see
    /// a player move.
    pub step_echo: Samples,
    /// The bot's own `0x0502` results by code (0 hit, 1 critical, 3 miss, 5 out of reach, 7 no weapon, 10 cannot attack).
    pub results: BTreeMap<u8, u64>,
    pub steps: u64,
    /// `0x020B` on the bot's own id: moves the server refused.
    pub refusals: u64,
    pub protocol_errors: u64,
    pub kills: u64,
    pub deaths: u64,
    pub rebirths: u64,
    pub logins: u64,
    pub login_time: Duration,
    /// Why the connection ended, when it did.
    pub disconnect: Option<LinkError>,
    pending_attack: Option<(u32, Instant)>,
    pending_skill: Option<Instant>,
    warned: HashSet<u16>,
}

impl Metrics {
    pub fn packet_received(&mut self, opcode: u16, len: usize) {
        let c = self.received.entry(opcode).or_default();
        c.packets += 1;
        c.bytes += len as u64;
    }

    pub fn packet_sent(&mut self, opcode: u16, len: usize) {
        let c = self.sent.entry(opcode).or_default();
        c.packets += 1;
        c.bytes += len as u64;
    }

    /// A packet of the wrong size for its opcode. True the first time for that opcode, which is when it is logged.
    pub fn protocol_error(&mut self, opcode: u16) -> bool {
        self.protocol_errors += 1;
        self.warned.insert(opcode)
    }

    /// A `0x0502` went out; the first result for `target` answers it.
    pub fn attack_sent(&mut self, target: u32, now: Instant) {
        self.pending_attack = Some((target, now));
    }

    /// A `0x0502` result for the bot's own attack (attacker the bot).
    pub fn attack_result(&mut self, result: u8, target: u32, now: Instant) {
        *self.results.entry(result).or_default() += 1;
        if let Some((aimed, since)) = self.pending_attack
            && (aimed == target || target == 0)
        {
            self.pending_attack = None;
            // Only a blow (hit, critical, miss) is the first blow; a refusal ends the wait without a sample.
            if matches!(result, server_combat::RESULT_HIT | server_combat::RESULT_CRITICAL | server_combat::RESULT_MISS) {
                self.attack_round_trip.record(now - since);
            }
        }
    }

    pub fn skill_sent(&mut self, now: Instant) {
        self.pending_skill = Some(now);
    }

    pub fn skill_outcome(&mut self, now: Instant) {
        if let Some(since) = self.pending_skill.take() {
            self.skill_round_trip.record(now - since);
        }
    }

    /// Hands over what was counted since the last call, for the swarm's window, and starts counting again. What is still pending (an
    /// attack waiting for its first blow), the opcodes already warned of and the reason the connection ended stay.
    pub fn take_window(&mut self) -> Metrics {
        let (pending_attack, pending_skill) = (self.pending_attack, self.pending_skill);
        let (warned, disconnect) = (std::mem::take(&mut self.warned), self.disconnect.clone());
        let taken = std::mem::take(self);
        (self.pending_attack, self.pending_skill, self.warned, self.disconnect) = (pending_attack, pending_skill, warned, disconnect);
        taken
    }

    /// The figures of one screen.
    pub fn summary(&self) -> Summary {
        let total = |m: &BTreeMap<u16, Count>| {
            m.values().fold(Count::default(), |a, c| Count { packets: a.packets + c.packets, bytes: a.bytes + c.bytes })
        };
        Summary {
            received: total(&self.received),
            sent: total(&self.sent),
            attack_p50: self.attack_round_trip.percentile(50.0),
            attack_p95: self.attack_round_trip.percentile(95.0),
            attack_p99: self.attack_round_trip.percentile(99.0),
            attack_max: self.attack_round_trip.max(),
            skill_p50: self.skill_round_trip.percentile(50.0),
            refusals: self.refusals,
            protocol_errors: self.protocol_errors,
            kills: self.kills,
            deaths: self.deaths,
            rebirths: self.rebirths,
            steps: self.steps,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub received: Count,
    pub sent: Count,
    pub attack_p50: Option<Duration>,
    pub attack_p95: Option<Duration>,
    pub attack_p99: Option<Duration>,
    pub attack_max: Option<Duration>,
    pub skill_p50: Option<Duration>,
    pub refusals: u64,
    pub protocol_errors: u64,
    pub kills: u64,
    pub deaths: u64,
    pub rebirths: u64,
    pub steps: u64,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = |d: Option<Duration>| d.map_or_else(|| "-".to_owned(), |d| format!("{:.1} ms", d.as_secs_f64() * 1000.0));
        writeln!(
            f,
            "received {} packets, {} bytes; sent {} packets, {} bytes",
            self.received.packets, self.received.bytes, self.sent.packets, self.sent.bytes
        )?;
        writeln!(
            f,
            "attack round trip p50 {} p95 {} p99 {} max {}; skill p50 {}",
            ms(self.attack_p50),
            ms(self.attack_p95),
            ms(self.attack_p99),
            ms(self.attack_max),
            ms(self.skill_p50)
        )?;
        write!(
            f,
            "steps {}, refusals {}, protocol errors {}, kills {}, deaths {}, rebirths {}",
            self.steps, self.refusals, self.protocol_errors, self.kills, self.deaths, self.rebirths
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_nearest_rank() {
        let mut s = Samples::default();
        assert_eq!(s.percentile(50.0), None);
        for ms in 1..=100 {
            s.record(Duration::from_millis(ms));
        }
        assert_eq!(s.percentile(50.0), Some(Duration::from_millis(50)));
        assert_eq!(s.percentile(95.0), Some(Duration::from_millis(95)));
        assert_eq!(s.percentile(99.0), Some(Duration::from_millis(99)));
        assert_eq!(s.max(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn the_round_trip_runs_from_the_attack_to_its_first_blow() {
        let mut m = Metrics::default();
        let t = Instant::now();
        m.attack_sent(7, t);
        m.attack_result(0, 9, t + Duration::from_millis(5)); // another target's blow: not the answer
        assert!(m.attack_round_trip.is_empty());
        m.attack_result(0, 7, t + Duration::from_millis(40));
        m.attack_result(0, 7, t + Duration::from_millis(2000)); // the second blow is not a round trip
        assert_eq!(m.attack_round_trip.len(), 1);
        assert_eq!(m.attack_round_trip.max(), Some(Duration::from_millis(40)));
        assert_eq!(m.results[&0], 3);
        // A refusal ends the wait without a sample.
        m.attack_sent(8, t);
        m.attack_result(server_combat::RESULT_OUT_OF_RANGE, 0, t + Duration::from_millis(3));
        assert_eq!(m.attack_round_trip.len(), 1);
    }

    #[test]
    fn a_protocol_error_is_logged_once_per_opcode() {
        let mut m = Metrics::default();
        assert!(m.protocol_error(0x0201));
        assert!(!m.protocol_error(0x0201));
        assert!(m.protocol_error(0x0501));
        assert_eq!(m.protocol_errors, 3);
    }
}

#[cfg(test)]
mod histogram_tests {
    use super::*;

    #[test]
    fn a_histogram_reads_percentiles_within_a_few_percent_and_keeps_the_maximum() {
        let mut h = Histogram::default();
        assert_eq!(h.percentile(50.0), None);
        for ms in 1..=1000 {
            h.record(Duration::from_millis(ms));
        }
        assert_eq!(h.len(), 1000);
        assert_eq!(h.max(), Some(Duration::from_millis(1000)));
        for (p, want) in [(50.0, 500.0), (95.0, 950.0), (99.0, 990.0)] {
            let got = h.percentile(p).unwrap().as_secs_f64() * 1000.0;
            assert!(got >= want && got <= want * 1.05, "p{p}: {got} for {want}");
        }
        // Never above the maximum, and two histograms merge as one.
        assert_eq!(h.percentile(100.0), Some(Duration::from_millis(1000)));
        let mut other = Histogram::default();
        other.record(Duration::from_secs(5));
        h.merge(&other);
        assert_eq!((h.len(), h.max()), (1001, Some(Duration::from_secs(5))));
    }

    #[test]
    fn taking_a_window_keeps_what_is_still_pending() {
        let mut m = Metrics::default();
        let t = Instant::now();
        m.steps = 4;
        m.packet_sent(0x0502, 10);
        m.attack_sent(7, t);
        let window = m.take_window();
        assert_eq!((window.steps, window.sent[&0x0502].packets), (4, 1));
        assert_eq!((m.steps, m.sent.len()), (0, 0));
        // The blow comes in the next window and is still timed from the attack.
        m.attack_result(0, 7, t + Duration::from_millis(30));
        assert_eq!(m.attack_round_trip.max(), Some(Duration::from_millis(30)));
    }
}
