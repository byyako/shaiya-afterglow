//! Locks that survive a panic (`docs/server/audit.md` R5): a panic inside a lock poisons it, and a poisoned `std` lock would make
//! every later `lock().unwrap()` panic as well, taking the whole server down with one bug. These take the guard anyway, clear the
//! flag so the log gets one line per panic, and go on. The data may be half-changed; the player whose task panicked is cleaned up
//! and saved as on a lost connection, and the rest of the server runs on.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// `Mutex::lock` that recovers from poisoning.
pub trait Relock<T> {
    fn guard(&self) -> MutexGuard<'_, T>;
}

impl<T> Relock<T> for Mutex<T> {
    fn guard(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned: PoisonError<MutexGuard<'_, T>>| {
            tracing::error!(lock = std::any::type_name::<T>(), "a lock was poisoned by a panic; carrying on with its data");
            self.clear_poison();
            poisoned.into_inner()
        })
    }
}

/// `RwLock::read` and `write` that recover from poisoning.
pub trait RelockRw<T> {
    fn read_guard(&self) -> RwLockReadGuard<'_, T>;
    fn write_guard(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RelockRw<T> for RwLock<T> {
    fn read_guard(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|poisoned| {
            tracing::error!(lock = std::any::type_name::<T>(), "a lock was poisoned by a panic; carrying on with its data");
            self.clear_poison();
            poisoned.into_inner()
        })
    }

    fn write_guard(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|poisoned| {
            tracing::error!(lock = std::any::type_name::<T>(), "a lock was poisoned by a panic; carrying on with its data");
            self.clear_poison();
            poisoned.into_inner()
        })
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
}
