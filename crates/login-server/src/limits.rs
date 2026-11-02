//! Limits against connection floods and password guessing: per address, per account, and per account and address together
//! (`docs/server/audit.md` R7).

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LimitConfig {
    /// Simultaneous connections from one address.
    pub max_connections: u32,
    /// New connections from one address in a minute (`docs/server/audit.md` R10); the rest are closed unread.
    pub connects_per_minute: u32,
    /// Failed logins from one address within `window` before it is locked out.
    pub failure_limit: u32,
    pub window: Duration,
    /// How long a locked-out address is refused.
    pub lockout: Duration,
    /// Wrong passwords for one account from one address within `pair_window` before that address is answered "wrong
    /// password" for that account, without hashing, for `pair_lockout`.
    pub pair_failure_limit: u32,
    pub pair_window: Duration,
    pub pair_lockout: Duration,
    /// Wrong passwords for one account from all addresses together within `account_window` before every address is
    /// answered "wrong password" for it until the window clears.
    pub account_failure_limit: u32,
    pub account_window: Duration,
    /// How many accounts (and account and address pairs) are tracked at most; the oldest are dropped past it.
    pub tracked_accounts: usize,
}

impl Default for LimitConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            connects_per_minute: 30,
            failure_limit: 20,
            window: Duration::from_secs(600),
            lockout: Duration::from_secs(600),
            pair_failure_limit: 5,
            pair_window: Duration::from_secs(900),
            pair_lockout: Duration::from_secs(900),
            account_failure_limit: 30,
            account_window: Duration::from_secs(3600),
            tracked_accounts: 100_000,
        }
    }
}

#[derive(Default)]
struct Entry {
    connections: u32,
    /// When the address's recent connections were let in.
    recent: VecDeque<Instant>,
    failures: VecDeque<Instant>,
    locked_until: Option<Instant>,
}

/// Wrong passwords for one account from one address.
struct PairEntry {
    failures: VecDeque<Instant>,
    blocked_until: Option<Instant>,
    last: Instant,
}

/// Wrong passwords for one account from all addresses.
struct AccountEntry {
    failures: VecDeque<Instant>,
    last: Instant,
}

#[derive(Default)]
struct Guesses {
    /// Keyed by the lowercased account name, unknown names included, so the answer does not reveal which names exist.
    pairs: HashMap<(String, IpAddr), PairEntry>,
    accounts: HashMap<String, AccountEntry>,
}

pub struct IpLimits {
    config: LimitConfig,
    entries: Mutex<HashMap<IpAddr, Entry>>,
    guesses: Mutex<Guesses>,
}

/// Holds one connection slot for an address; the slot is released when dropped.
pub struct ConnectionGuard {
    limits: Arc<IpLimits>,
    ip: IpAddr,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    LockedOut,
    TooManyConnections,
    TooFast,
}

/// The connection rate is counted over this long.
const RATE_WINDOW: Duration = Duration::from_secs(60);

impl IpLimits {
    pub fn new(config: LimitConfig) -> Arc<Self> {
        Arc::new(Self { config, entries: Mutex::new(HashMap::new()), guesses: Mutex::new(Guesses::default()) })
    }

    pub fn try_connect(self: &Arc<Self>, ip: IpAddr) -> Result<ConnectionGuard, Refusal> {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(ip).or_default();
        if entry.locked_until.is_some_and(|until| until > now) {
            return Err(Refusal::LockedOut);
        }
        if entry.connections >= self.config.max_connections {
            return Err(Refusal::TooManyConnections);
        }
        // A refused attempt does not count, so an address that keeps trying is let in a minute after its last accepted connection.
        while entry.recent.front().is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW) {
            entry.recent.pop_front();
        }
        if entry.recent.len() >= self.config.connects_per_minute as usize {
            return Err(Refusal::TooFast);
        }
        entry.recent.push_back(now);
        entry.connections += 1;
        Ok(ConnectionGuard { limits: self.clone(), ip })
    }

    /// Records a failed login. Returns true if the address is now locked out.
    pub fn record_failure(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(ip).or_default();
        while entry.failures.front().is_some_and(|t| now.duration_since(*t) > self.config.window) {
            entry.failures.pop_front();
        }
        entry.failures.push_back(now);
        if entry.failures.len() as u32 >= self.config.failure_limit {
            entry.locked_until = Some(now + self.config.lockout);
            entry.failures.clear();
            return true;
        }
        false
    }

    /// Whether `account` (the lowercased name) is answered "wrong password" from `ip` without being checked.
    pub fn login_blocked(&self, account: &str, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut guesses = self.guesses.lock().unwrap();
        if guesses.pairs.get(&(account.to_owned(), ip)).is_some_and(|pair| pair.blocked_until.is_some_and(|until| until > now)) {
            return true;
        }
        match guesses.accounts.get_mut(account) {
            Some(entry) => {
                prune(&mut entry.failures, now, self.config.account_window);
                entry.failures.len() as u32 >= self.config.account_failure_limit
            }
            None => false,
        }
    }

    /// Records a wrong password for `account` (the lowercased name) from `ip`. A blocked account's attempts are not
    /// recorded, so nobody can keep a block going by trying.
    pub fn record_account_failure(&self, account: &str, ip: IpAddr) {
        let now = Instant::now();
        let mut guesses = self.guesses.lock().unwrap();
        let pair = guesses.pairs.entry((account.to_owned(), ip)).or_insert_with(|| PairEntry {
            failures: VecDeque::new(),
            blocked_until: None,
            last: now,
        });
        prune(&mut pair.failures, now, self.config.pair_window);
        pair.failures.push_back(now);
        pair.last = now;
        if pair.failures.len() as u32 >= self.config.pair_failure_limit {
            pair.blocked_until = Some(now + self.config.pair_lockout);
            pair.failures.clear();
        }
        let entry = guesses.accounts.entry(account.to_owned()).or_insert_with(|| AccountEntry { failures: VecDeque::new(), last: now });
        prune(&mut entry.failures, now, self.config.account_window);
        entry.failures.push_back(now);
        entry.last = now;
        let cap = self.config.tracked_accounts;
        let (pair_window, account_window) = (self.config.pair_window, self.config.account_window);
        if guesses.pairs.len() > cap {
            trim(
                &mut guesses.pairs,
                cap,
                |p| p.last,
                |p| p.blocked_until.is_some_and(|until| until > now) || now.duration_since(p.last) <= pair_window,
            );
        }
        if guesses.accounts.len() > cap {
            trim(&mut guesses.accounts, cap, |a| a.last, |a| now.duration_since(a.last) <= account_window);
        }
    }

    /// A good login clears the counts of its own account only: that account's total, and its count from this address.
    pub fn record_success(&self, account: &str, ip: IpAddr) {
        let mut guesses = self.guesses.lock().unwrap();
        guesses.pairs.remove(&(account.to_owned(), ip));
        guesses.accounts.remove(account);
    }

    fn release(&self, ip: IpAddr) {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get_mut(&ip) {
            entry.connections = entry.connections.saturating_sub(1);
            while entry.recent.front().is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW) {
                entry.recent.pop_front();
            }
            let idle = entry.connections == 0
                && entry.recent.is_empty()
                && entry.locked_until.is_none_or(|until| until <= now)
                && entry.failures.back().is_none_or(|t| now.duration_since(*t) > self.config.window);
            if idle {
                entries.remove(&ip);
            }
        }
    }
}

/// Drops the failures older than `window` from the front.
fn prune(failures: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while failures.front().is_some_and(|t| now.duration_since(*t) > window) {
        failures.pop_front();
    }
}

/// Brings a map of over `cap` entries back under it: the entries that no longer count go first, then the oldest tenth of the
/// rest (so a flood of new names costs one pass in many, not one a request).
fn trim<K: Clone + Eq + std::hash::Hash, V>(map: &mut HashMap<K, V>, cap: usize, last: impl Fn(&V) -> Instant, live: impl Fn(&V) -> bool) {
    map.retain(|_, v| live(v));
    if map.len() <= cap {
        return;
    }
    let mut ages: Vec<(Instant, K)> = map.iter().map(|(k, v)| (last(v), k.clone())).collect();
    let drop = (map.len() - cap).max(cap / 10).min(ages.len());
    ages.select_nth_unstable_by_key(drop - 1, |(t, _)| *t);
    for (_, key) in &ages[..drop] {
        map.remove(key);
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.limits.release(self.ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, last])
    }

    fn limits(max: u32, failures: u32, window_ms: u64, lockout_ms: u64) -> Arc<IpLimits> {
        IpLimits::new(LimitConfig {
            max_connections: max,
            connects_per_minute: 1000,
            failure_limit: failures,
            window: Duration::from_millis(window_ms),
            lockout: Duration::from_millis(lockout_ms),
            ..LimitConfig::default()
        })
    }

    #[test]
    fn connections_are_capped_per_address_and_released_on_drop() {
        let l = limits(2, 100, 1000, 1000);
        let a = l.try_connect(ip(1)).unwrap();
        let _b = l.try_connect(ip(1)).unwrap();
        assert_eq!(l.try_connect(ip(1)).err(), Some(Refusal::TooManyConnections));
        assert!(l.try_connect(ip(2)).is_ok(), "other addresses are unaffected");
        drop(a);
        assert!(l.try_connect(ip(1)).is_ok());
    }

    #[test]
    fn enough_failures_lock_the_address_out_then_it_recovers() {
        let l = limits(10, 3, 5000, 80);
        assert!(!l.record_failure(ip(1)));
        assert!(!l.record_failure(ip(1)));
        assert!(l.record_failure(ip(1)), "third failure locks");
        assert_eq!(l.try_connect(ip(1)).err(), Some(Refusal::LockedOut));
        assert!(l.try_connect(ip(2)).is_ok());
        std::thread::sleep(Duration::from_millis(120));
        assert!(l.try_connect(ip(1)).is_ok(), "lockout expires");
    }

    #[test]
    fn failures_outside_the_window_do_not_count() {
        let l = limits(10, 3, 40, 1000);
        assert!(!l.record_failure(ip(1)));
        assert!(!l.record_failure(ip(1)));
        std::thread::sleep(Duration::from_millis(80));
        assert!(!l.record_failure(ip(1)), "old failures expired");
    }

    #[test]
    fn a_good_login_does_not_clear_the_addresss_failures() {
        let l = limits(10, 3, 5000, 1000);
        l.record_failure(ip(1));
        l.record_failure(ip(1));
        l.record_success("alice", ip(1));
        assert!(l.record_failure(ip(1)), "the address count stands: a good login of an account of one's own is no reset");
    }

    fn guard(pair: u32, account: u32) -> Arc<IpLimits> {
        IpLimits::new(LimitConfig { pair_failure_limit: pair, account_failure_limit: account, ..LimitConfig::default() })
    }

    #[test]
    fn five_wrong_passwords_block_that_address_for_that_account_only() {
        let l = guard(5, 30);
        for _ in 0..4 {
            l.record_account_failure("alice", ip(1));
        }
        assert!(!l.login_blocked("alice", ip(1)));
        l.record_account_failure("alice", ip(1));
        assert!(l.login_blocked("alice", ip(1)));
        assert!(!l.login_blocked("alice", ip(2)), "another address is not blocked");
        assert!(!l.login_blocked("bob", ip(1)), "another account is not blocked");
    }

    #[test]
    fn a_pair_block_lapses() {
        let l = IpLimits::new(LimitConfig { pair_failure_limit: 2, pair_lockout: Duration::from_millis(60), ..LimitConfig::default() });
        l.record_account_failure("alice", ip(1));
        l.record_account_failure("alice", ip(1));
        assert!(l.login_blocked("alice", ip(1)));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!l.login_blocked("alice", ip(1)));
    }

    #[test]
    fn pair_failures_outside_the_window_do_not_count() {
        let l = IpLimits::new(LimitConfig { pair_failure_limit: 3, pair_window: Duration::from_millis(40), ..LimitConfig::default() });
        l.record_account_failure("alice", ip(1));
        l.record_account_failure("alice", ip(1));
        std::thread::sleep(Duration::from_millis(80));
        l.record_account_failure("alice", ip(1));
        assert!(!l.login_blocked("alice", ip(1)));
    }

    #[test]
    fn many_addresses_together_block_the_account_everywhere() {
        let l = guard(5, 6);
        for i in 0..6 {
            l.record_account_failure("alice", ip(i));
        }
        assert!(l.login_blocked("alice", ip(100)), "an address that never failed is answered the same way");
        assert!(!l.login_blocked("bob", ip(100)));
    }

    #[test]
    fn the_account_block_clears_when_its_hour_does() {
        let l =
            IpLimits::new(LimitConfig { account_failure_limit: 2, account_window: Duration::from_millis(60), ..LimitConfig::default() });
        l.record_account_failure("alice", ip(1));
        l.record_account_failure("alice", ip(2));
        assert!(l.login_blocked("alice", ip(3)));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!l.login_blocked("alice", ip(3)));
    }

    #[test]
    fn a_good_login_clears_only_its_own_account() {
        let l = guard(3, 30);
        for _ in 0..2 {
            l.record_account_failure("alice", ip(1));
            l.record_account_failure("bob", ip(1));
        }
        l.record_success("alice", ip(1));
        l.record_account_failure("alice", ip(1));
        l.record_account_failure("bob", ip(1));
        assert!(!l.login_blocked("alice", ip(1)), "alice's count started over");
        assert!(l.login_blocked("bob", ip(1)), "bob's did not");
    }

    #[test]
    fn the_tracked_names_are_bounded_and_the_oldest_go_first() {
        let l = IpLimits::new(LimitConfig { tracked_accounts: 50, ..LimitConfig::default() });
        for i in 0..500 {
            l.record_account_failure(&format!("name{i}"), ip(1));
        }
        let guesses = l.guesses.lock().unwrap();
        assert!(guesses.accounts.len() <= 50 && guesses.pairs.len() <= 50);
        assert!(guesses.accounts.contains_key("name499"), "the newest is kept");
        assert!(!guesses.accounts.contains_key("name0"), "the oldest is dropped");
    }

    #[test]
    fn new_connections_are_limited_per_minute_even_when_each_one_closes() {
        let l = IpLimits::new(LimitConfig { connects_per_minute: 3, ..LimitConfig::default() });
        for _ in 0..3 {
            drop(l.try_connect(ip(1)).unwrap());
        }
        assert_eq!(l.try_connect(ip(1)).err(), Some(Refusal::TooFast));
        assert!(l.try_connect(ip(2)).is_ok(), "another address");
        l.entries.lock().unwrap().get_mut(&ip(1)).unwrap().recent.iter_mut().for_each(|t| *t -= Duration::from_secs(61));
        assert!(l.try_connect(ip(1)).is_ok(), "the minute is over");
    }

    #[test]
    fn idle_entries_are_forgotten_when_their_minute_is_over() {
        let l = limits(10, 5, 20, 20);
        drop(l.try_connect(ip(1)).unwrap());
        assert_eq!(l.entries.lock().unwrap().len(), 1, "its minute is not over");
        l.entries.lock().unwrap().get_mut(&ip(1)).unwrap().recent.iter_mut().for_each(|t| *t -= Duration::from_secs(61));
        drop(l.try_connect(ip(2)).unwrap());
        l.release(ip(1));
        assert!(!l.entries.lock().unwrap().contains_key(&ip(1)));
    }
}
