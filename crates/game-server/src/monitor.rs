//! The server's own measurements, for the `stats` line and the console's `stats` (`docs/plans/test-bots.md` section 7): traffic,
//! outbox sizes, the monster pass, the time spent waiting for locks, the save queue. Everything here is an atomic, always on and
//! cheap; nothing is logged unless `stats_every` asks for the line. The counters are process-wide statics because the places that
//! bump them (`sync::Relock::guard`, an `Outbox` cloned into the zones) have no `Context` to hand.
//!
//! The lock table is the delicate part: `Relock::guard` is taken at hundreds of places, so a lock that is free costs one `try_lock`
//! and nothing here, and only a lock that has to be waited for is timed and recorded, in a fixed table of slots claimed with a
//! compare-exchange by a hash of the lock's type name. No lock, no allocation and no hash map on that path.

use std::sync::atomic::{
    AtomicI64, AtomicU64,
    Ordering::Relaxed,
    Ordering::{AcqRel, Acquire, Release},
};
use std::time::{Duration, Instant};

/// Slots for the lock types waited on. The server has about fifty; a full table puts the rest in the `other` slot.
pub const LOCK_SLOTS: usize = 128;
/// Bytes of a short type name a slot keeps.
const NAME_BYTES: usize = 64;
const NAME_WORDS: usize = NAME_BYTES / 8;
/// The name the `other` slot goes by.
pub const OTHER: &str = "other";

/// What one lock type has been waited on, since the start.
struct Slot {
    /// A hash of the type's full name; 0 while the slot is free.
    key: AtomicU64,
    ready: AtomicU64,
    name: [AtomicU64; NAME_WORDS],
    wait_ns: AtomicU64,
    count: AtomicU64,
    longest_ns: AtomicU64,
}

impl Slot {
    const fn new() -> Self {
        Self {
            key: AtomicU64::new(0),
            ready: AtomicU64::new(0),
            name: [const { AtomicU64::new(0) }; NAME_WORDS],
            wait_ns: AtomicU64::new(0),
            count: AtomicU64::new(0),
            longest_ns: AtomicU64::new(0),
        }
    }

    fn set_name(&self, full: &str) {
        let mut bytes = [0u8; NAME_BYTES];
        let len = shorten(full, &mut bytes);
        // Segments dropped on the way leave what was written over them behind the end.
        bytes[len..].fill(0);
        let (words, _) = bytes.as_chunks::<8>();
        for (word, chunk) in self.name.iter().zip(words) {
            word.store(u64::from_le_bytes(*chunk), Relaxed);
        }
        self.ready.store(1, Release);
    }

    /// The short name, empty until it is set.
    fn name(&self) -> String {
        if self.ready.load(Acquire) == 0 {
            return String::new();
        }
        let mut bytes = Vec::with_capacity(NAME_BYTES);
        for word in &self.name {
            bytes.extend_from_slice(&word.load(Relaxed).to_le_bytes());
        }
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    fn record(&self, wait: Duration) {
        let ns = u64::try_from(wait.as_nanos()).unwrap_or(u64::MAX);
        self.wait_ns.fetch_add(ns, Relaxed);
        self.count.fetch_add(1, Relaxed);
        self.longest_ns.fetch_max(ns, Relaxed);
    }
}

/// FNV-1a, never 0.
fn hash(name: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in name.as_bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h | 1
}

/// `name` with the module paths taken out (`std::collections::hash::map::HashMap<u16, game_server::zone::Placed>` becomes
/// `HashMap<u16, Placed>`), written into `out` (cut at its end); returns the length.
fn shorten(name: &str, out: &mut [u8]) -> usize {
    let bytes = name.as_bytes();
    let (mut len, mut ident, mut i) = (0, 0, 0);
    while i < bytes.len() {
        if bytes[i] == b':' && bytes.get(i + 1) == Some(&b':') {
            // The identifier just written was a path segment: drop it.
            len = ident;
            i += 2;
            continue;
        }
        if len < out.len() {
            out[len] = bytes[i];
            len += 1;
        }
        if !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            ident = len;
        }
        i += 1;
    }
    len
}

/// The short form of a type name, as the stats line shows it.
pub fn short_name(name: &str) -> String {
    let mut out = [0u8; 256];
    let len = shorten(name, &mut out);
    String::from_utf8_lossy(&out[..len]).into_owned()
}

/// What a lock type was waited on, for one reading.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LockStat {
    pub name: String,
    pub wait_ns: u64,
    pub count: u64,
    pub longest_ns: u64,
}

/// The lock waits, per type. Recording takes no lock and allocates nothing.
pub struct LockTable {
    slots: [Slot; LOCK_SLOTS],
    other: Slot,
}

impl Default for LockTable {
    fn default() -> Self {
        Self::new()
    }
}

impl LockTable {
    pub const fn new() -> Self {
        Self { slots: [const { Slot::new() }; LOCK_SLOTS], other: Slot::new() }
    }

    fn slot_for(&self, name: &str) -> &Slot {
        let key = hash(name);
        let start = key as usize % LOCK_SLOTS;
        for i in 0..LOCK_SLOTS {
            let slot = &self.slots[(start + i) % LOCK_SLOTS];
            match slot.key.load(Acquire) {
                k if k == key => return slot,
                0 => match slot.key.compare_exchange(0, key, AcqRel, Acquire) {
                    Ok(_) => {
                        slot.set_name(name);
                        return slot;
                    }
                    Err(now) if now == key => return slot,
                    Err(_) => {}
                },
                _ => {}
            }
        }
        &self.other
    }

    /// A wait of `wait` for a lock of the type called `name` (`std::any::type_name`).
    pub fn record(&self, name: &'static str, wait: Duration) {
        self.slot_for(name).record(wait);
    }

    /// Every type that has been waited on, with the `other` slot last when it holds anything. `reset_longest` starts the longest
    /// waits again.
    pub fn read(&self, reset_longest: bool) -> Vec<LockStat> {
        let take = |slot: &Slot, name: String| LockStat {
            name,
            wait_ns: slot.wait_ns.load(Relaxed),
            count: slot.count.load(Relaxed),
            longest_ns: if reset_longest { slot.longest_ns.swap(0, Relaxed) } else { slot.longest_ns.load(Relaxed) },
        };
        let mut all: Vec<LockStat> = self.slots.iter().filter(|s| s.key.load(Acquire) != 0).map(|s| take(s, s.name())).collect();
        if self.other.count.load(Relaxed) > 0 {
            all.push(take(&self.other, OTHER.to_owned()));
        }
        all
    }
}

/// The counters of the line, all since the start except the gauges.
pub struct Counters {
    in_packets: AtomicU64,
    in_bytes: AtomicU64,
    out_packets: AtomicU64,
    out_bytes: AtomicU64,
    /// Writes to client sockets that went through (a batch of packets is one).
    out_writes: AtomicU64,
    /// Bytes queued in every outbox now, and the most any one outbox held since the last reading.
    outbox_queued: AtomicI64,
    outbox_peak: AtomicU64,
    outbox_closed: AtomicU64,
    mob_passes: AtomicU64,
    mob_pass_ns: AtomicU64,
    mob_pass_max_ns: AtomicU64,
    mob_period_ms: AtomicU64,
    saves_queued: AtomicI64,
}

impl Default for Counters {
    fn default() -> Self {
        Self::new()
    }
}

impl Counters {
    pub const fn new() -> Self {
        Self {
            in_packets: AtomicU64::new(0),
            in_bytes: AtomicU64::new(0),
            out_packets: AtomicU64::new(0),
            out_bytes: AtomicU64::new(0),
            out_writes: AtomicU64::new(0),
            outbox_queued: AtomicI64::new(0),
            outbox_peak: AtomicU64::new(0),
            outbox_closed: AtomicU64::new(0),
            mob_passes: AtomicU64::new(0),
            mob_pass_ns: AtomicU64::new(0),
            mob_pass_max_ns: AtomicU64::new(0),
            mob_period_ms: AtomicU64::new(0),
            saves_queued: AtomicI64::new(0),
        }
    }

    /// A frame of `len` bytes read from a client.
    pub fn frame_in(&self, len: usize) {
        self.in_packets.fetch_add(1, Relaxed);
        self.in_bytes.fetch_add(len as u64, Relaxed);
    }

    /// A packet of `len` bytes written to a client's socket.
    pub fn frame_out(&self, len: usize) {
        self.out_packets.fetch_add(1, Relaxed);
        self.out_bytes.fetch_add(len as u64, Relaxed);
    }

    /// One write to a client's socket went through, of one packet or a batch of them (`out_pps` counts the packets).
    pub fn write_done(&self) {
        self.out_writes.fetch_add(1, Relaxed);
    }

    /// A packet of `len` bytes joined an outbox that now holds `queued` bytes.
    pub fn outbox_added(&self, len: usize, queued: usize) {
        self.outbox_queued.fetch_add(len as i64, Relaxed);
        self.outbox_peak.fetch_max(queued as u64, Relaxed);
    }

    /// `len` bytes left an outbox (drained, or dropped with a connection that ended).
    pub fn outbox_removed(&self, len: usize) {
        self.outbox_queued.fetch_sub(len as i64, Relaxed);
    }

    /// A connection ended because its outbox passed `outbox_limit`.
    pub fn outbox_limit_close(&self) {
        self.outbox_closed.fetch_add(1, Relaxed);
    }

    /// One pass (wake-up) of the monster updater took `took`.
    pub fn mob_pass(&self, took: Duration) {
        let ns = u64::try_from(took.as_nanos()).unwrap_or(u64::MAX);
        self.mob_passes.fetch_add(1, Relaxed);
        self.mob_pass_ns.fetch_add(ns, Relaxed);
        self.mob_pass_max_ns.fetch_max(ns, Relaxed);
    }

    /// How often the monster updater wakes.
    pub fn set_mob_period(&self, period: Duration) {
        self.mob_period_ms.store(period.as_millis() as u64, Relaxed);
    }

    /// A save joined the save queue.
    pub fn save_queued(&self) {
        self.saves_queued.fetch_add(1, Relaxed);
    }

    /// A save was written (or given up).
    pub fn save_done(&self) {
        self.saves_queued.fetch_sub(1, Relaxed);
    }

    /// Saves waiting for the writer now.
    pub fn saves_waiting(&self) -> u64 {
        self.saves_queued.load(Relaxed).max(0) as u64
    }

    /// Bytes waiting in every outbox now.
    pub fn outbox_bytes(&self) -> u64 {
        self.outbox_queued.load(Relaxed).max(0) as u64
    }
}

static COUNTERS: Counters = Counters::new();
static LOCKS: LockTable = LockTable::new();

/// The server's counters.
pub fn counters() -> &'static Counters {
    &COUNTERS
}

/// The server's lock waits.
pub fn locks() -> &'static LockTable {
    &LOCKS
}

/// Times a `Relock` that was not free: `start` is taken just before the blocking lock.
pub fn lock_waited(name: &'static str, start: Instant) {
    LOCKS.record(name, start.elapsed());
}

/// Records a pass of the monster updater when it goes out of scope, whichever way the pass ends.
pub struct PassTimer(Instant);

impl PassTimer {
    pub fn start() -> Self {
        Self(Instant::now())
    }
}

impl Drop for PassTimer {
    fn drop(&mut self) {
        COUNTERS.mob_pass(self.0.elapsed());
    }
}

/// What the caller knows that the counters do not.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gauges {
    pub players: u32,
    pub connections: usize,
    pub game_log_dropped: u64,
    pub rss_mb: u64,
    pub workers: usize,
}

/// One lock type's waits over an interval, as the line shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct LockWait {
    pub name: String,
    pub wait_ms: f64,
    pub count: u64,
    pub longest_ms: f64,
}

/// The figures of one line (section 7's table), over the interval since the last reading.
#[derive(Clone, Debug, PartialEq)]
pub struct Figures {
    pub secs: f64,
    pub players: u32,
    pub connections: usize,
    pub in_pps: f64,
    pub in_bps: f64,
    pub out_pps: f64,
    pub out_bps: f64,
    pub outbox_max: u64,
    pub outbox_sum: u64,
    pub outbox_closed: u64,
    pub mob_pass_max_ms: f64,
    pub mob_pass_avg_ms: f64,
    pub mob_period_ms: u64,
    pub lock_wait_ms: f64,
    pub lock_waits: u64,
    /// The three types waited on longest in the interval (total wait), longest first.
    pub lock_worst: Vec<LockWait>,
    pub saves_queued: u64,
    pub game_log_dropped: u64,
    pub rss_mb: u64,
    pub workers: usize,
    /// Writes a second to client sockets: `out_pps` over this is the packets per write.
    pub out_wps: f64,
}

/// A lock type's short name as the operator knows it, where there is one.
fn friendly(name: &str) -> &str {
    match name {
        "HashMap<u16, HashMap<u32, Placed>>" => "maps",
        "HashMap<u16, MobMap>" => "mobs",
        other => other,
    }
}

impl std::fmt::Display for Figures {
    /// `key=value` pairs on one line: the `stats` log line and the console's answer.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "secs={:.1} players={} connections={} in_pps={:.0} in_bps={:.0} out_pps={:.0} out_bps={:.0} outbox_max={} outbox_sum={} \
             outbox_closed={} mob_pass_max_ms={:.2} mob_pass_avg_ms={:.2} mob_period_ms={} lock_wait_ms={:.1} lock_waits={} lock_worst=\"",
            self.secs,
            self.players,
            self.connections,
            self.in_pps,
            self.in_bps,
            self.out_pps,
            self.out_bps,
            self.outbox_max,
            self.outbox_sum,
            self.outbox_closed,
            self.mob_pass_max_ms,
            self.mob_pass_avg_ms,
            self.mob_period_ms,
            self.lock_wait_ms,
            self.lock_waits,
        )?;
        if self.lock_worst.is_empty() {
            write!(f, "-")?;
        }
        for (i, lock) in self.lock_worst.iter().enumerate() {
            let sep = if i == 0 { "" } else { "; " };
            write!(f, "{sep}{} {:.1}ms/{} max {:.1}ms", friendly(&lock.name), lock.wait_ms, lock.count, lock.longest_ms)?;
        }
        write!(
            f,
            "\" saves_queued={} game_log_dropped={} rss_mb={} workers={} out_wps={:.0}",
            self.saves_queued, self.game_log_dropped, self.rss_mb, self.workers, self.out_wps
        )
    }
}

/// Turns the counters into the figures of an interval: remembers what they were at the last reading.
pub struct Sampler {
    at: Instant,
    in_packets: u64,
    in_bytes: u64,
    out_packets: u64,
    out_bytes: u64,
    out_writes: u64,
    outbox_closed: u64,
    passes: u64,
    pass_ns: u64,
    /// By lock name: the total wait and count at the last reading.
    locks: Vec<(String, u64, u64)>,
}

impl Sampler {
    /// Starts at `now` with the counters' present values as the base.
    pub fn new(now: Instant, counters: &Counters, table: &LockTable) -> Self {
        let mut sampler = Self {
            at: now,
            in_packets: 0,
            in_bytes: 0,
            out_packets: 0,
            out_bytes: 0,
            out_writes: 0,
            outbox_closed: 0,
            passes: 0,
            pass_ns: 0,
            locks: Vec::new(),
        };
        sampler.sample(now, counters, table, Gauges::default());
        sampler
    }

    /// The figures since the last reading (or the start); the maxima start again.
    pub fn sample(&mut self, now: Instant, counters: &Counters, table: &LockTable, gauges: Gauges) -> Figures {
        let secs = now.saturating_duration_since(self.at).as_secs_f64();
        let per = |delta: u64| if secs > 0.0 { delta as f64 / secs } else { 0.0 };
        let (in_packets, in_bytes) = (counters.in_packets.load(Relaxed), counters.in_bytes.load(Relaxed));
        let (out_packets, out_bytes) = (counters.out_packets.load(Relaxed), counters.out_bytes.load(Relaxed));
        let out_writes = counters.out_writes.load(Relaxed);
        let outbox_closed = counters.outbox_closed.load(Relaxed);
        let (passes, pass_ns) = (counters.mob_passes.load(Relaxed), counters.mob_pass_ns.load(Relaxed));
        let pass_max_ns = counters.mob_pass_max_ns.swap(0, Relaxed);
        let new_passes = passes - self.passes;
        let mut waits = Vec::new();
        let mut now_locks = Vec::new();
        for stat in table.read(true) {
            let (was_ns, was_count) = self.locks.iter().find(|(n, _, _)| *n == stat.name).map_or((0, 0), |(_, ns, count)| (*ns, *count));
            let (ns, count) = (stat.wait_ns - was_ns, stat.count - was_count);
            if count > 0 {
                waits.push(LockWait { name: stat.name.clone(), wait_ms: ns as f64 / 1e6, count, longest_ms: stat.longest_ns as f64 / 1e6 });
            }
            now_locks.push((stat.name, stat.wait_ns, stat.count));
        }
        let lock_wait_ms = waits.iter().map(|w| w.wait_ms).sum();
        let lock_waits = waits.iter().map(|w| w.count).sum();
        waits.sort_by(|a, b| b.wait_ms.total_cmp(&a.wait_ms).then_with(|| a.name.cmp(&b.name)));
        waits.truncate(3);
        let figures = Figures {
            secs,
            players: gauges.players,
            connections: gauges.connections,
            in_pps: per(in_packets - self.in_packets),
            in_bps: per(in_bytes - self.in_bytes),
            out_pps: per(out_packets - self.out_packets),
            out_bps: per(out_bytes - self.out_bytes),
            outbox_max: counters.outbox_peak.swap(0, Relaxed),
            outbox_sum: counters.outbox_bytes(),
            outbox_closed: outbox_closed - self.outbox_closed,
            mob_pass_max_ms: pass_max_ns as f64 / 1e6,
            mob_pass_avg_ms: if new_passes > 0 { (pass_ns - self.pass_ns) as f64 / new_passes as f64 / 1e6 } else { 0.0 },
            mob_period_ms: counters.mob_period_ms.load(Relaxed),
            lock_wait_ms,
            lock_waits,
            lock_worst: waits,
            saves_queued: counters.saves_waiting(),
            game_log_dropped: gauges.game_log_dropped,
            rss_mb: gauges.rss_mb,
            workers: gauges.workers,
            out_wps: per(out_writes - self.out_writes),
        };
        (self.at, self.in_packets, self.in_bytes, self.out_packets, self.out_bytes) = (now, in_packets, in_bytes, out_packets, out_bytes);
        self.out_writes = out_writes;
        (self.outbox_closed, self.passes, self.pass_ns, self.locks) = (outbox_closed, passes, pass_ns, now_locks);
        figures
    }
}

/// The process's resident memory in MB, from `/proc/self/statm` (pages of 4 KB); 0 where there is none.
pub fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/statm").ok().and_then(|text| rss_mb_of(&text)).unwrap_or(0)
}

/// The resident set of a `statm` line (second field, in 4 KB pages) in MB.
pub fn rss_mb_of(statm: &str) -> Option<u64> {
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096 / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_add_up_and_the_maxima_start_again() {
        let c = Counters::new();
        let table = LockTable::new();
        let t0 = Instant::now();
        let mut sampler = Sampler::new(t0, &c, &table);
        for _ in 0..10 {
            c.frame_in(20);
        }
        c.frame_out(1000);
        c.frame_out(500);
        c.outbox_added(300, 300);
        c.outbox_added(700, 1000);
        c.outbox_removed(300);
        c.outbox_limit_close();
        c.mob_pass(Duration::from_millis(4));
        c.mob_pass(Duration::from_millis(2));
        c.set_mob_period(Duration::from_millis(500));
        c.save_queued();
        c.save_queued();
        c.save_done();
        let f = sampler.sample(t0 + Duration::from_secs(2), &c, &table, Gauges { players: 3, connections: 4, ..Gauges::default() });
        assert_eq!((f.secs, f.players, f.connections), (2.0, 3, 4));
        assert_eq!((f.in_pps, f.in_bps, f.out_pps, f.out_bps), (5.0, 100.0, 1.0, 750.0));
        assert_eq!((f.outbox_max, f.outbox_sum, f.outbox_closed), (1000, 700, 1));
        assert_eq!((f.mob_pass_max_ms, f.mob_pass_avg_ms, f.mob_period_ms), (4.0, 3.0, 500));
        assert_eq!(f.saves_queued, 1);
        // The next interval counts from here: deltas and the cleared maxima.
        c.frame_in(100);
        let g = sampler.sample(t0 + Duration::from_secs(3), &c, &table, Gauges::default());
        assert_eq!((g.in_pps, g.in_bps, g.out_pps, g.outbox_closed), (1.0, 100.0, 0.0, 0));
        assert_eq!((g.outbox_max, g.mob_pass_max_ms, g.mob_pass_avg_ms), (0, 0.0, 0.0));
        assert_eq!(g.outbox_sum, 700, "the sum is a gauge, not an interval");
    }

    #[test]
    fn a_zero_length_interval_gives_zero_rates() {
        let c = Counters::new();
        let table = LockTable::new();
        let t0 = Instant::now();
        let mut sampler = Sampler::new(t0, &c, &table);
        c.frame_in(5);
        let f = sampler.sample(t0, &c, &table, Gauges::default());
        assert_eq!((f.in_pps, f.in_bps), (0.0, 0.0));
    }

    #[test]
    fn a_free_lock_records_nothing_and_a_wait_is_kept_under_its_type() {
        let table = LockTable::new();
        assert!(table.read(false).is_empty(), "nothing waited on, nothing recorded");
        table.record("a::b::Alpha<c::Beta>", Duration::from_millis(5));
        table.record("a::b::Alpha<c::Beta>", Duration::from_millis(9));
        table.record("x::Gamma", Duration::from_millis(1));
        let all = table.read(false);
        assert_eq!(all.len(), 2);
        let alpha = all.iter().find(|s| s.name == "Alpha<Beta>").expect("the short name");
        assert_eq!((alpha.count, alpha.wait_ns, alpha.longest_ns), (2, 14_000_000, 9_000_000));
        let reset = table.read(true);
        assert_eq!(reset.iter().find(|s| s.name == "Alpha<Beta>").unwrap().longest_ns, 9_000_000);
        assert_eq!(table.read(false).iter().find(|s| s.name == "Alpha<Beta>").unwrap().longest_ns, 0, "the longest starts again");
        assert_eq!(table.read(false).iter().find(|s| s.name == "Alpha<Beta>").unwrap().count, 2, "the totals do not");
    }

    #[test]
    fn a_full_table_falls_back_to_the_other_slot() {
        let table = LockTable::new();
        let names: Vec<&'static str> = (0..LOCK_SLOTS + 5).map(|i| &*Box::leak(format!("Type{i}").into_boxed_str())).collect();
        for name in &names {
            table.record(name, Duration::from_micros(10));
        }
        let all = table.read(false);
        assert_eq!(all.len(), LOCK_SLOTS + 1, "every slot and the other one");
        let other = all.last().unwrap();
        assert_eq!((other.name.as_str(), other.count), (OTHER, 5));
        // A type that has a slot still records there once the table is full.
        table.record(names[0], Duration::from_micros(10));
        assert_eq!(table.read(false).iter().find(|s| s.name == "Type0").unwrap().count, 2);
    }

    #[test]
    fn the_worst_three_locks_are_ranked_by_their_wait_in_the_interval() {
        let c = Counters::new();
        let table = LockTable::new();
        let t0 = Instant::now();
        let mut sampler = Sampler::new(t0, &c, &table);
        table.record("A", Duration::from_millis(50));
        let first = sampler.sample(t0 + Duration::from_secs(1), &c, &table, Gauges::default());
        assert_eq!((first.lock_waits, first.lock_worst.len()), (1, 1));
        for (name, ms) in [("A", 1), ("B", 7), ("C", 5), ("D", 3), ("E", 9)] {
            table.record(name, Duration::from_millis(ms));
        }
        let second = sampler.sample(t0 + Duration::from_secs(2), &c, &table, Gauges::default());
        let ranked: Vec<_> = second.lock_worst.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(ranked, ["E", "B", "C"], "A's old 50 ms is not this interval's");
        assert_eq!(second.lock_waits, 5);
        assert!((second.lock_wait_ms - 25.0).abs() < 1e-9);
    }

    #[test]
    fn writes_are_counted_apart_from_packets() {
        let c = Counters::new();
        let table = LockTable::new();
        let t0 = Instant::now();
        let mut sampler = Sampler::new(t0, &c, &table);
        // Ten packets went out in two writes: a batch of nine and a packet alone.
        for _ in 0..10 {
            c.frame_out(100);
        }
        c.write_done();
        c.write_done();
        let first = sampler.sample(t0 + Duration::from_secs(2), &c, &table, Gauges::default());
        assert_eq!((first.out_pps, first.out_wps), (5.0, 1.0));
        // The next interval counts only what came after.
        c.frame_out(100);
        c.write_done();
        let second = sampler.sample(t0 + Duration::from_secs(3), &c, &table, Gauges::default());
        assert_eq!((second.out_pps, second.out_wps), (1.0, 1.0));
        let idle = sampler.sample(t0 + Duration::from_secs(4), &c, &table, Gauges::default());
        assert_eq!((idle.out_pps, idle.out_wps), (0.0, 0.0));
    }

    #[test]
    fn type_names_lose_their_module_paths() {
        assert_eq!(
            short_name("std::collections::hash::map::HashMap<u16, std::collections::hash::map::HashMap<u32, game_server::zone::Placed>>"),
            "HashMap<u16, HashMap<u32, Placed>>"
        );
        assert_eq!(short_name("game_server::party::Parties"), "Parties");
        assert_eq!(short_name("()"), "()");
        assert_eq!(short_name("core::option::Option<tokio::sync::mpsc::UnboundedSender<Save>>"), "Option<UnboundedSender<Save>>");
    }

    #[test]
    fn the_line_reads_from_fixed_figures() {
        let figures = Figures {
            secs: 10.0,
            players: 301,
            connections: 303,
            in_pps: 1234.4,
            in_bps: 56789.0,
            out_pps: 90210.6,
            out_bps: 4_200_000.0,
            outbox_max: 65536,
            outbox_sum: 1_000_000,
            outbox_closed: 2,
            mob_pass_max_ms: 12.345,
            mob_pass_avg_ms: 1.5,
            mob_period_ms: 500,
            lock_wait_ms: 321.04,
            lock_waits: 4410,
            lock_worst: vec![
                LockWait { name: "HashMap<u16, HashMap<u32, Placed>>".into(), wait_ms: 300.0, count: 4000, longest_ms: 8.3 },
                LockWait { name: "Parties".into(), wait_ms: 21.04, count: 410, longest_ms: 0.5 },
            ],
            saves_queued: 0,
            game_log_dropped: 7,
            rss_mb: 412,
            workers: 4,
            out_wps: 30070.4,
        };
        assert_eq!(
            figures.to_string(),
            "secs=10.0 players=301 connections=303 in_pps=1234 in_bps=56789 out_pps=90211 out_bps=4200000 outbox_max=65536 \
             outbox_sum=1000000 outbox_closed=2 mob_pass_max_ms=12.35 mob_pass_avg_ms=1.50 mob_period_ms=500 lock_wait_ms=321.0 \
             lock_waits=4410 lock_worst=\"maps 300.0ms/4000 max 8.3ms; Parties 21.0ms/410 max 0.5ms\" saves_queued=0 \
             game_log_dropped=7 rss_mb=412 workers=4 out_wps=30070"
        );
        let quiet = Figures { lock_worst: Vec::new(), ..figures };
        assert!(quiet.to_string().contains("lock_worst=\"-\" "));
    }

    #[test]
    fn resident_memory_is_read_from_statm() {
        assert_eq!(rss_mb_of("123456 51200 900 10 0 4000 0\n"), Some(200));
        assert_eq!(rss_mb_of("garbage"), None);
        assert!(rss_mb() > 0 || !std::path::Path::new("/proc/self/statm").exists());
    }
}
