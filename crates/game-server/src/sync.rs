//! Locks that survive a panic (`docs/server/audit.md` R5): a panic inside a lock poisons it, and a poisoned `std` lock would make
//! every later `lock().unwrap()` panic as well, taking the whole server down with one bug. These take the guard anyway, clear the
//! flag so the log gets one line per panic, and go on. The data may be half-changed; the player whose task panicked is cleaned up
//! and saved as on a lost connection, and the rest of the server runs on.

use std::any::type_name;
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::Instant;

use crate::monitor::lock_waited;

/// What a poisoned lock gets: one log line, the flag cleared, the data as it was left.
fn recovered<G>(lock: &'static str, poisoned: PoisonError<G>, clear: impl FnOnce()) -> G {
    tracing::error!(lock, "a lock was poisoned by a panic; carrying on with its data");
    clear();
    poisoned.into_inner()
}

/// `Mutex::lock` that recovers from poisoning. A lock that is free costs one `try_lock`; one that has to be waited for is timed
/// into `monitor::locks()` under its type (`docs/plans/test-bots.md` section 7).
pub trait Relock<T> {
    fn guard(&self) -> MutexGuard<'_, T>;
}

impl<T> Relock<T> for Mutex<T> {
    fn guard(&self) -> MutexGuard<'_, T> {
        match self.try_lock() {
            Ok(held) => held,
            Err(TryLockError::Poisoned(poisoned)) => recovered(type_name::<T>(), poisoned, || self.clear_poison()),
            Err(TryLockError::WouldBlock) => {
                let start = Instant::now();
                let locked = self.lock();
                lock_waited(type_name::<T>(), start);
                locked.unwrap_or_else(|poisoned| recovered(type_name::<T>(), poisoned, || self.clear_poison()))
            }
        }
    }
}

/// `RwLock::read` and `write` that recover from poisoning, and time a wait as `Relock` does.
pub trait RelockRw<T> {
    fn read_guard(&self) -> RwLockReadGuard<'_, T>;
    fn write_guard(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RelockRw<T> for RwLock<T> {
    fn read_guard(&self) -> RwLockReadGuard<'_, T> {
        match self.try_read() {
            Ok(held) => held,
            Err(TryLockError::Poisoned(poisoned)) => recovered(type_name::<T>(), poisoned, || self.clear_poison()),
            Err(TryLockError::WouldBlock) => {
                let start = Instant::now();
                let locked = self.read();
                lock_waited(type_name::<T>(), start);
                locked.unwrap_or_else(|poisoned| recovered(type_name::<T>(), poisoned, || self.clear_poison()))
            }
        }
    }

    fn write_guard(&self) -> RwLockWriteGuard<'_, T> {
        match self.try_write() {
            Ok(held) => held,
            Err(TryLockError::Poisoned(poisoned)) => recovered(type_name::<T>(), poisoned, || self.clear_poison()),
            Err(TryLockError::WouldBlock) => {
                let start = Instant::now();
                let locked = self.write();
                lock_waited(type_name::<T>(), start);
                locked.unwrap_or_else(|poisoned| recovered(type_name::<T>(), poisoned, || self.clear_poison()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_mutex_poisoned_by_a_panic_still_locks() {
        let lock = Arc::new(Mutex::new(5u32));
        let other = lock.clone();
        let _ = std::thread::spawn(move || {
            let mut held = other.guard();
            *held = 6;
            panic!("inside the lock");
        })
        .join();
        assert!(lock.lock().is_err(), "the panic did poison it");
        assert_eq!(*lock.guard(), 6, "the guard recovers the data");
        assert!(lock.lock().is_ok(), "and the flag is cleared");
    }

    #[test]
    fn an_rwlock_poisoned_by_a_panic_still_reads_and_writes() {
        let lock = Arc::new(RwLock::new(String::from("a")));
        let other = lock.clone();
        let _ = std::thread::spawn(move || {
            let _held = other.write_guard();
            panic!("inside the lock");
        })
        .join();
        assert_eq!(*lock.read_guard(), "a");
        lock.write_guard().push('b');
        assert_eq!(*lock.read_guard(), "ab");
    }

    /// Types only these tests lock, so that their rows in the process-wide table are theirs alone.
    struct Probe(u32);
    struct Probe2(u32);
    struct Probe3(u32);
    struct Probe4(u32);

    fn waited_on(short_name: &str) -> crate::monitor::LockStat {
        crate::monitor::locks().read(false).into_iter().find(|s| s.name == short_name).unwrap_or_default()
    }

    #[test]
    fn a_free_lock_is_not_timed() {
        let lock = Mutex::new(Probe(1));
        for _ in 0..1000 {
            lock.guard().0 += 1;
        }
        assert_eq!(lock.guard().0, 1001);
        assert_eq!(waited_on("Probe").count, 0, "no contention, no record");
    }

    #[test]
    fn a_contended_wait_is_recorded_under_its_type() {
        let lock = Arc::new(Mutex::new(Probe2(0)));
        let held = lock.guard();
        let other = lock.clone();
        let waiter = std::thread::spawn(move || {
            other.guard().0 += 1;
        });
        std::thread::sleep(std::time::Duration::from_millis(60));
        drop(held);
        waiter.join().unwrap();
        assert_eq!(lock.guard().0, 1);
        let stat = waited_on("Probe2");
        assert_eq!(stat.count, 1);
        assert!(stat.wait_ns >= 40_000_000 && stat.longest_ns == stat.wait_ns, "waited about 60 ms: {stat:?}");
    }

    #[test]
    fn a_lock_poisoned_while_a_thread_waits_still_recovers_and_is_timed() {
        let lock = Arc::new(Mutex::new(Probe3(5)));
        let holder = {
            let lock = lock.clone();
            std::thread::spawn(move || {
                let _held = lock.guard();
                std::thread::sleep(std::time::Duration::from_millis(80));
                panic!("inside the lock");
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(30));
        // This thread waits for the holder, which panics: the wait ends on a poisoned lock and the guard takes it anyway.
        assert_eq!(lock.guard().0, 5);
        let _ = holder.join();
        assert_eq!(waited_on("Probe3").count, 1, "the wait was timed");
        assert!(lock.lock().is_ok(), "and the flag is cleared");
    }

    #[test]
    fn an_rwlock_wait_is_recorded_too() {
        let lock = Arc::new(RwLock::new(Probe4(0)));
        let held = lock.write_guard();
        let other = lock.clone();
        let reader = std::thread::spawn(move || other.read_guard().0);
        std::thread::sleep(std::time::Duration::from_millis(40));
        drop(held);
        assert_eq!(reader.join().unwrap(), 0);
        assert_eq!(waited_on("Probe4").count, 1);
    }

    /// `Mutex::lock` as `guard` was before the lock timing, for the comparison below.
    fn old_guard<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
        lock.lock().unwrap_or_else(|poisoned| {
            lock.clear_poison();
            poisoned.into_inner()
        })
    }

    /// Run by hand: `cargo test -p game-server --lib sync::tests::bench -- --ignored --nocapture`. What an uncontended `guard` costs now
    /// against what it cost before the timing (`docs/plans/test-bots.md` section 7).
    #[test]
    #[ignore = "a timing loop, run by hand"]
    fn bench_an_uncontended_guard() {
        const N: u32 = 20_000_000;
        let lock = Mutex::new(0u64);
        let time = |f: &dyn Fn(&Mutex<u64>)| {
            let mut best = std::time::Duration::MAX;
            for _ in 0..5 {
                let start = Instant::now();
                for _ in 0..N {
                    f(std::hint::black_box(&lock));
                }
                best = best.min(start.elapsed());
            }
            best.as_nanos() as f64 / f64::from(N)
        };
        let (before, after) = (time(&|l| *old_guard(l) += 1), time(&|l| *l.guard() += 1));
        println!("uncontended lock + unlock: before {before:.2} ns, after {after:.2} ns");
    }
}
