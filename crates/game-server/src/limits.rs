//! Per-address limits on game connections (`docs/server/audit.md` R2): how many an address may hold at once and how many new ones
//! it may open in a minute. The login server's `IpLimits` is the pattern; this one has no login failures to count.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::sync::Relock;

/// The rate is counted over this long.
const RATE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Entry {
    connections: usize,
    /// When the address's recent connections were let in.
    recent: VecDeque<Instant>,
}

pub struct IpLimits {
    max_per_ip: usize,
    per_minute: u32,
    entries: Mutex<HashMap<IpAddr, Entry>>,
}

/// Holds one connection slot for an address; the slot is released when dropped.
pub struct ConnectionGuard {
    limits: Arc<IpLimits>,
    ip: IpAddr,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    TooManyConnections,
    TooFast,
}

impl IpLimits {
    pub fn new(max_per_ip: usize, per_minute: u32) -> Arc<Self> {
        Arc::new(Self { max_per_ip, per_minute, entries: Mutex::new(HashMap::new()) })
    }

    /// Lets a new connection from `ip` in, or says why not. A refused attempt does not count towards the rate, so an address that
    /// keeps trying is let in again a minute after its last accepted connection.
    pub fn try_connect(self: &Arc<Self>, ip: IpAddr) -> Result<ConnectionGuard, Refusal> {
        let now = Instant::now();
        let mut entries = self.entries.guard();
        let entry = entries.entry(ip).or_default();
        let result = if entry.connections >= self.max_per_ip {
            Err(Refusal::TooManyConnections)
        } else {
            prune(entry, now);
            if entry.recent.len() >= self.per_minute as usize {
                Err(Refusal::TooFast)
            } else {
                entry.connections += 1;
                entry.recent.push_back(now);
                Ok(ConnectionGuard { limits: self.clone(), ip })
            }
        };
        if result.is_err() && entry.connections == 0 && entry.recent.is_empty() {
            entries.remove(&ip);
        }
        result
    }

    /// Addresses the limits remember.
    pub fn tracked(&self) -> usize {
        self.entries.guard().len()
    }

    fn release(&self, ip: IpAddr) {
        let now = Instant::now();
        let mut entries = self.entries.guard();
        if let Some(entry) = entries.get_mut(&ip) {
            entry.connections = entry.connections.saturating_sub(1);
            prune(entry, now);
            if entry.connections == 0 && entry.recent.is_empty() {
                entries.remove(&ip);
            }
        }
    }

    /// Forgets the addresses whose connections are gone and whose minute is over; run now and then, since an address that opened
    /// connections and never closed one cleanly is only pruned when it comes back.
    pub fn sweep(&self) {
        let now = Instant::now();
        self.entries.guard().retain(|_, entry| {
            prune(entry, now);
            entry.connections > 0 || !entry.recent.is_empty()
        });
    }
}

fn prune(entry: &mut Entry, now: Instant) {
    while entry.recent.front().is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW) {
        entry.recent.pop_front();
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.limits.release(self.ip);
    }
}

/// A token bucket: `per_sec` tokens a second up to `burst`, one token a packet or line (`docs/server/audit.md` R4). A rate of 0 is
/// no limit.
#[derive(Clone, Debug)]
pub struct Bucket {
    tokens: f64,
    per_sec: f64,
    burst: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(per_sec: u32, burst: u32, now: Instant) -> Self {
        Self { tokens: f64::from(burst), per_sec: f64::from(per_sec), burst: f64::from(burst), last: now }
    }

    /// Whether the bucket has refilled to at least half of its burst.
    pub fn recovered(&self) -> bool {
        self.tokens >= self.burst / 2.0
    }

    /// Takes a token if there is one.
    pub fn take(&mut self, now: Instant) -> bool {
        if self.per_sec == 0.0 {
            return true;
        }
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = self.last.max(now);
        self.tokens = (self.tokens + elapsed * self.per_sec).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// How long a connection may stay over its packet rate before it is ended.
pub const FLOOD_GRACE: Duration = Duration::from_secs(5);

/// The packets a connection sends: a bucket of `per_sec` with a burst of twice that, and the time since the packets last found it
/// empty without a break.
#[derive(Clone, Debug)]
pub struct PacketFlood {
    bucket: Bucket,
    over_since: Option<Instant>,
}

impl PacketFlood {
    pub fn new(per_sec: u32, now: Instant) -> Self {
        Self { bucket: Bucket::new(per_sec, per_sec.saturating_mul(2), now), over_since: None }
    }

    /// A packet came in. True when the connection has been over its rate for `FLOOD_GRACE` in a row and is to be ended. A stretch over
    /// the rate starts with a packet that finds the bucket empty and ends when the bucket has refilled to half its burst, so a flood
    /// that gets a token now and then stays over it.
    pub fn packet(&mut self, now: Instant) -> bool {
        if !self.bucket.take(now) {
            self.over_since.get_or_insert(now);
        } else if self.bucket.recovered() {
            self.over_since = None;
        }
        self.over_since.is_some_and(|since| now.saturating_duration_since(since) >= FLOOD_GRACE)
    }
}

/// Lets a log line through at most once in `interval`, for lines a client can make as often as it likes (R4, R18).
#[derive(Clone, Debug)]
pub struct Every {
    interval: Duration,
    last: Option<Instant>,
}

impl Every {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None }
    }

    pub fn due(&mut self, now: Instant) -> bool {
        if self.last.is_some_and(|last| now.saturating_duration_since(last) < self.interval) {
            return false;
        }
        self.last = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, last])
    }

    #[test]
    fn connections_are_capped_per_address_and_released_on_drop() {
        let l = IpLimits::new(2, 1000);
        let a = l.try_connect(ip(1)).unwrap();
        let _b = l.try_connect(ip(1)).unwrap();
        assert_eq!(l.try_connect(ip(1)).err(), Some(Refusal::TooManyConnections));
        assert!(l.try_connect(ip(2)).is_ok(), "other addresses are unaffected");
        drop(a);
        assert!(l.try_connect(ip(1)).is_ok());
    }

    #[test]
    fn new_connections_are_limited_per_minute_even_when_each_one_closes() {
        let l = IpLimits::new(100, 3);
        for _ in 0..3 {
            drop(l.try_connect(ip(1)).unwrap());
        }
        assert_eq!(l.try_connect(ip(1)).err(), Some(Refusal::TooFast));
        assert!(l.try_connect(ip(2)).is_ok(), "another address");
        // Old entries leave the window.
        l.entries.guard().get_mut(&ip(1)).unwrap().recent.iter_mut().for_each(|t| *t -= Duration::from_secs(61));
        assert!(l.try_connect(ip(1)).is_ok());
    }

    #[test]
    fn idle_addresses_are_forgotten() {
        let l = IpLimits::new(2, 100);
        let guard = l.try_connect(ip(1)).unwrap();
        assert_eq!(l.tracked(), 1);
        drop(guard);
        assert_eq!(l.tracked(), 1, "its minute is not over");
        l.entries.guard().get_mut(&ip(1)).unwrap().recent.iter_mut().for_each(|t| *t -= Duration::from_secs(61));
        l.sweep();
        assert_eq!(l.tracked(), 0);
    }

    #[test]
    fn a_bucket_gives_its_burst_then_its_rate() {
        let start = Instant::now();
        let mut bucket = Bucket::new(3, 5, start);
        assert_eq!((0..8).filter(|_| bucket.take(start)).count(), 5, "the burst, then nothing");
        assert!(!bucket.take(start + Duration::from_millis(300)), "0.9 of a token");
        assert!(bucket.take(start + Duration::from_millis(340)), "a token after a third of a second");
        assert!(!bucket.take(start + Duration::from_millis(340)));
        // A long rest refills to the burst and no further.
        let later = start + Duration::from_secs(60);
        assert_eq!((0..8).filter(|_| bucket.take(later)).count(), 5);
        assert!((0..100).all(|_| Bucket::new(0, 0, start).take(start)), "a rate of 0 is no limit");
    }

    #[test]
    fn a_connection_is_ended_after_five_seconds_over_its_packet_rate() {
        let start = Instant::now();
        let mut flood = PacketFlood::new(100, start);
        // 300 packets in the first second: the burst of 200 and the second's 100.
        for i in 0..300 {
            assert!(!flood.packet(start + Duration::from_micros(i * 3000)));
        }
        // Then 1000 a second: over from the start, ended once five seconds have passed.
        let mut ended = None;
        for i in 0..10_000u64 {
            let now = start + Duration::from_secs(1) + Duration::from_micros(i * 1000);
            if flood.packet(now) {
                ended = Some(now - start - Duration::from_secs(1));
                break;
            }
        }
        // The stretch began with the first packet that found the bucket empty, just before the first second was over.
        let ended = ended.expect("ended") + Duration::from_secs(1);
        assert!(ended >= Duration::from_secs(5) && ended < Duration::from_secs(7), "{ended:?}");
    }

    #[test]
    fn a_pause_ends_the_stretch_over_the_rate() {
        let start = Instant::now();
        let mut flood = PacketFlood::new(100, start);
        // Four seconds over the rate, a quiet two, four more: never five in a row.
        for second in [0u64, 6] {
            for i in 0..4000u64 {
                assert!(
                    !flood.packet(start + Duration::from_secs(second) + Duration::from_micros(i * 1000)),
                    "second {second}, packet {i}"
                );
            }
        }
    }

    #[test]
    fn a_log_gate_lets_one_line_through_per_interval() {
        let start = Instant::now();
        let mut gate = Every::new(Duration::from_secs(10));
        assert!(gate.due(start));
        assert!(!gate.due(start + Duration::from_secs(9)));
        assert!(gate.due(start + Duration::from_secs(10)));
        assert!(!gate.due(start + Duration::from_secs(11)));
    }
}
