//! The script thread (`docs/plans/ext-5-lua.md` section 4.2): notification events are jobs on a bounded queue, raised with `try_send` so
//! that whoever raises one (a zone under its locks, a connection) never waits; one OS thread takes the jobs in order, runs each enabled
//! extension's handler, then the timers that are due, and sleeps until the next job or timer.

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::event::Job;
use super::{ExtScript, Limit, ScriptSet, act, api};

/// Jobs the queue holds (section 4.2).
pub const QUEUE: usize = 10_000;
/// The most the thread sleeps without a job or a timer (it looks at the set again).
const IDLE: Duration = Duration::from_secs(3600);
/// A full queue says so at most this often.
const FULL_WARN: Duration = Duration::from_secs(10);

/// The set of scripts in force: swapped whole by `start` and `reload`; shared by the zones and the thread.
#[derive(Default)]
pub struct ScriptCell {
    set: RwLock<Arc<ScriptSet>>,
    /// The stored values, shared by every extension's state.
    pub store: Arc<super::store::Store>,
}

impl ScriptCell {
    /// The set in force now: one snapshot per operation (section 10).
    pub fn get(&self) -> Arc<ScriptSet> {
        self.set.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn put(&self, set: ScriptSet) {
        *self.set.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(set);
    }
}

/// The sending side of the queue.
pub struct Jobs {
    tx: SyncSender<Job>,
    last_full: std::sync::Mutex<Option<Instant>>,
    dropped: std::sync::atomic::AtomicU64,
}

impl Jobs {
    /// Queues a job; a full queue drops it with a warning (at most one line every 10 s) and says so.
    pub fn send(&self, job: Job) -> bool {
        if self.tx.try_send(job).is_err() {
            self.dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut last = self.last_full.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            if last.is_none_or(|t| now.saturating_duration_since(t) >= FULL_WARN) {
                *last = Some(now);
                tracing::warn!(dropped = self.dropped.load(std::sync::atomic::Ordering::Relaxed), "the script queue is full or stopped: an event was dropped");
            }
            return false;
        }
        true
    }
}

/// Makes the queue and starts the thread over `cell`. The thread ends when the queue's sender is gone.
pub fn start(cell: Arc<ScriptCell>) -> Jobs {
    let (tx, rx) = sync_channel(QUEUE);
    std::thread::Builder::new()
        .name("script runner".to_owned())
        .spawn(move || supervised(&rx, &cell))
        .expect("spawning the script thread");
    Jobs { tx, last_full: std::sync::Mutex::new(None), dropped: std::sync::atomic::AtomicU64::new(0) }
}

/// The thread's loop, started again after a panic (as the other tasks, `docs/server/audit.md` R5).
fn supervised(rx: &Receiver<Job>, cell: &ScriptCell) {
    loop {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(rx, cell))) {
            Ok(()) => return,
            Err(panic) => {
                let text = panic.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default();
                tracing::error!(panic = %text, "the script thread panicked; it starts again");
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

fn run(rx: &Receiver<Job>, cell: &ScriptCell) {
    loop {
        let set = cell.get();
        let wait = next_timer(&set).map_or(IDLE, |at| at.saturating_duration_since(Instant::now()).min(IDLE));
        match rx.recv_timeout(wait) {
            Ok(job) => handle(&cell.get(), &cell.store, job),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // The jobs that came while this one ran, before the timers (they are in order).
        loop {
            match rx.try_recv() {
                Ok(job) => handle(&cell.get(), &cell.store, job),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        timers(&cell.get(), Instant::now());
    }
}

fn next_timer(set: &ScriptSet) -> Option<Instant> {
    set.extensions().iter().filter(|e| !e.is_disabled()).filter_map(|e| e.next_timer()).min()
}

/// Runs one job against the set in force.
pub fn handle(set: &ScriptSet, store: &super::store::Store, job: Job) {
    match job {
        Job::Event { event, payload } => {
            for ext in set.for_event(event) {
                // A failure is counted and logged by the extension; the next one runs.
                let _ = ext.call_built::<()>(event.name(), Limit::EVENT, |lua| act::event_args(lua, &payload));
            }
        }
        Job::Command { extension, word, p, args } => {
            let Some(ext) = set.extensions().iter().find(|e| e.name == extension) else { return };
            let label = format!("!{word}");
            let _ = ext.run(&label, Limit::EVENT, |lua| {
                let Some(f) = api::command_fn(lua, &word) else { return Ok(()) };
                f.call::<()>((act::player_handle(lua, &p)?, args))
            });
        }
        Job::OrderDone { extension, callback, outcome } => {
            let Some(ext) = set.extensions().iter().find(|e| e.name == extension) else { return };
            let _ = ext.run("order callback", Limit::EVENT, |lua| {
                let Some(f) = api::take_callback(lua, callback) else { return Ok(()) };
                match outcome {
                    super::event::Outcome::Count(n) => f.call::<()>(n),
                    super::event::Outcome::Done(ok) => f.call::<()>(ok),
                }
            });
        }
        Job::Forget { id, session } => store.end_char(id, session),
        Job::Wake => {}
    }
}

/// Runs the timers that are due at `now`, in install order.
pub fn timers(set: &ScriptSet, now: Instant) {
    for ext in set.extensions() {
        if ext.is_disabled() {
            continue;
        }
        for id in ext.due_timers(now) {
            fire(ext, id);
        }
    }
}

fn fire(ext: &ExtScript, id: u32) {
    let label = format!("timer {id}");
    let _ = ext.run(&label, Limit::EVENT, |lua| match api::timer_fn(lua, id) {
        Some(f) => f.call::<()>(()),
        None => Ok(()),
    });
    // A one-shot timer was taken out of the list when it came due; its function goes too (a repeating one keeps it).
    if !ext.has_timer(id) {
        let _ = ext.run("timer cleanup", Limit::EVENT, |lua| {
            api::drop_timer_fn(lua, id);
            Ok(())
        });
    }
}
