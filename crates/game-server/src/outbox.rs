//! The packets other players' actions and the zone queue for one connection (`docs/server/audit.md` R1). A connection that stops
//! reading must not pile up memory without end: the queue is bounded by bytes, and past the limit a packet is dropped and the queue
//! marks itself overflowed, which `world_loop` takes as a lost connection.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Debug, Default)]
struct Shared {
    queued: AtomicUsize,
    overflowed: AtomicBool,
}

/// The sending side, cloned into the zone, the guilds and the friends; `send` has the shape `UnboundedSender::send` had.
#[derive(Clone)]
pub struct Outbox {
    sender: UnboundedSender<Vec<u8>>,
    shared: Arc<Shared>,
    limit: usize,
}

/// The connection's side: what `world_loop` drains.
#[derive(Debug)]
pub struct OutboxInbox {
    receiver: UnboundedReceiver<Vec<u8>>,
    shared: Arc<Shared>,
}

/// A new queue that holds up to `limit` bytes of packets.
pub fn channel(limit: usize) -> (Outbox, OutboxInbox) {
    let (sender, receiver) = unbounded_channel();
    let shared = Arc::new(Shared::default());
    (Outbox { sender, shared: shared.clone(), limit }, OutboxInbox { receiver, shared })
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbox").field("queued", &self.queued()).field("limit", &self.limit).finish()
    }
}

/// The sends this thread has put off while a `DeferScope` lives, in the order they were made.
struct Deferred {
    /// How many scopes are open on this thread (they nest; the outermost one's end delivers).
    depth: Cell<u32>,
    queue: RefCell<Vec<(Outbox, Vec<u8>)>>,
}

thread_local! {
    static DEFERRED: Deferred = const { Deferred { depth: Cell::new(0), queue: RefCell::new(Vec::new()) } };
}

/// The most entries the thread's queue keeps its room for once a scope has delivered; a bigger one (an entry into a crowd) gives the
/// extra back, so a worker thread does not keep the memory of its busiest moment.
const KEPT_ENTRIES: usize = 4096;

/// While it lives, every `Outbox::send` on this thread is queued instead of made, and the end of the outermost scope makes them, in
/// the order they were asked, through the same code a direct send runs. The zone opens one with the `maps` lock (`zone::MapsGuard`):
/// a send is an atomic add, a channel push and the wake of the receiving task, and with the lock held every other thread's step waits
/// for them (`docs/plans/load-fixes.md` section 4, design B2).
///
/// The order is kept per thread, not across threads: two operations on two threads that finish within microseconds of each other can
/// reach a third player's outbox in the other order than they held the lock. A client drops a packet about an id it does not know yet,
/// so such a swap (an appearance after a death, a hit on a just-spawned monster) shows until that unit's next update.
///
/// A send's outcome is applied at delivery, not at the call: a closed outbox drops the packet, a full one drops it and marks itself
/// overflowed, as a direct send does, and `send` inside a scope always says `Ok`. Nothing reads that result or `queued()` under the
/// lock. The scope is `!Send`: it belongs to the thread whose list it counts in, so it cannot be held across an `.await` that moves
/// the task. Dropping it delivers even while a panic unwinds, and leaves the thread with no scope open.
pub struct DeferScope {
    _thread: PhantomData<*const ()>,
}

/// Opens a deferral scope on this thread (see `DeferScope`).
pub fn defer() -> DeferScope {
    DEFERRED.with(|d| d.depth.set(d.depth.get() + 1));
    DeferScope { _thread: PhantomData }
}

impl Drop for DeferScope {
    /// Never panics: `try_with` (the list is gone only when the thread is ending) and a delivery that is `Outbox::deliver`.
    fn drop(&mut self) {
        let _ = DEFERRED.try_with(|d| {
            let depth = d.depth.get().saturating_sub(1);
            d.depth.set(depth);
            if depth > 0 {
                return;
            }
            // Taken out of the cell first, so nothing is borrowed while the packets go: delivery is outside any borrow.
            let mut sends = std::mem::take(&mut *d.queue.borrow_mut());
            for (outbox, packet) in sends.drain(..) {
                let _ = outbox.deliver(packet);
            }
            sends.shrink_to(KEPT_ENTRIES);
            *d.queue.borrow_mut() = sends;
        });
    }
}

impl Outbox {
    /// Queues a packet; inside a `DeferScope` on this thread it waits for the scope's end and says `Ok`. Past the byte limit the
    /// packet is dropped, the queue is marked overflowed and the packet comes back as an error, as it does when the connection has
    /// gone (at delivery, for a deferred one).
    pub fn send(&self, packet: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        let mut packet = Some(packet);
        // `try_with`: the list is gone only while the thread is ending, and then the send is made at once.
        let _ = DEFERRED.try_with(|d| {
            if d.depth.get() > 0
                && let Some(packet) = packet.take()
            {
                d.queue.borrow_mut().push((self.clone(), packet));
            }
        });
        match packet {
            None => Ok(()),
            Some(packet) => self.deliver(packet),
        }
    }

    /// Makes the send: the byte count, the limit and the overflow flag, the channel and the monitor's figures. A direct `send` and
    /// the end of a `DeferScope` both come here, so a deferred packet is accounted exactly as a direct one.
    fn deliver(&self, packet: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        let len = packet.len();
        let queued = self.shared.queued.fetch_add(len, Ordering::Relaxed) + len;
        if queued > self.limit {
            self.shared.queued.fetch_sub(len, Ordering::Relaxed);
            self.shared.overflowed.store(true, Ordering::Relaxed);
            return Err(SendError(packet));
        }
        match self.sender.send(packet) {
            Ok(()) => {
                crate::monitor::counters().outbox_added(len, queued);
                Ok(())
            }
            Err(error) => {
                self.shared.queued.fetch_sub(len, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    /// Bytes waiting in the queue.
    pub fn queued(&self) -> usize {
        self.shared.queued.load(Ordering::Relaxed)
    }
}

impl OutboxInbox {
    fn taken(&self, len: usize) {
        self.shared.queued.fetch_sub(len, Ordering::Relaxed);
        crate::monitor::counters().outbox_removed(len);
    }

    /// The next packet; `None` once every sender is gone and the queue is empty.
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let packet = self.receiver.recv().await?;
        self.taken(packet.len());
        Some(packet)
    }

    pub fn try_recv(&mut self) -> Result<Vec<u8>, TryRecvError> {
        let packet = self.receiver.try_recv()?;
        self.taken(packet.len());
        Ok(packet)
    }

    /// A packet was dropped because the queue was over its limit: the connection is not keeping up.
    pub fn overflowed(&self) -> bool {
        self.shared.overflowed.load(Ordering::Relaxed)
    }
}

/// What a connection that ended left in its queue goes out of the server-wide count of queued bytes (`monitor`).
impl Drop for OutboxInbox {
    fn drop(&mut self) {
        crate::monitor::counters().outbox_removed(self.shared.queued.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_past_the_byte_limit_are_dropped_and_the_queue_overflows() {
        let (outbox, mut inbox) = channel(10);
        assert!(outbox.send(vec![0; 6]).is_ok());
        assert!(!inbox.overflowed());
        assert!(outbox.send(vec![0; 6]).is_err(), "6 + 6 is over 10");
        assert!(inbox.overflowed());
        assert_eq!(outbox.queued(), 6, "the dropped packet is not counted");
        assert_eq!(inbox.try_recv().unwrap().len(), 6);
        assert_eq!(outbox.queued(), 0, "the drain subtracts what it takes");
        assert!(outbox.send(vec![0; 10]).is_ok(), "room again");
        assert!(inbox.try_recv().is_ok() && inbox.try_recv().is_err());
    }

    #[tokio::test]
    async fn recv_subtracts_and_ends_when_the_senders_are_gone() {
        let (outbox, mut inbox) = channel(100);
        outbox.send(vec![1, 2, 3]).unwrap();
        assert_eq!(inbox.recv().await.unwrap(), [1, 2, 3]);
        assert_eq!(outbox.queued(), 0);
        drop(outbox);
        assert!(inbox.recv().await.is_none());
    }

    /// Everything waiting in an inbox.
    fn taken(inbox: &mut OutboxInbox) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| inbox.try_recv().ok()).collect()
    }

    #[test]
    fn sends_inside_a_scope_wait_for_its_end_and_keep_their_order() {
        let (a, mut a_in) = channel(100);
        let (b, mut b_in) = channel(100);
        let scope = defer();
        assert!(a.send(vec![1]).is_ok() && b.send(vec![2]).is_ok() && a.send(vec![3, 3]).is_ok() && b.send(vec![4]).is_ok());
        assert!(taken(&mut a_in).is_empty() && taken(&mut b_in).is_empty(), "nothing is received inside the scope");
        assert_eq!((a.queued(), b.queued()), (0, 0), "and nothing is counted yet");
        drop(scope);
        assert_eq!(taken(&mut a_in), [vec![1], vec![3, 3]]);
        assert_eq!(taken(&mut b_in), [vec![2], vec![4]]);
        assert_eq!((a.queued(), b.queued()), (0, 0), "the drain subtracts what delivery added");
        // The scope is over: a send is made at once again.
        a.send(vec![9]).unwrap();
        assert_eq!(a.queued(), 1);
        assert_eq!(taken(&mut a_in), [vec![9]]);
    }

    #[test]
    fn a_deferred_send_past_the_limit_overflows_at_delivery() {
        let (outbox, mut inbox) = channel(10);
        let scope = defer();
        assert!(outbox.send(vec![0; 6]).is_ok());
        assert!(outbox.send(vec![0; 6]).is_ok(), "inside the scope a send says Ok whatever the outcome will be");
        assert!(!inbox.overflowed(), "the outcome is not applied yet");
        drop(scope);
        assert!(inbox.overflowed(), "6 + 6 is over 10, found when the packets are delivered");
        assert_eq!(outbox.queued(), 6, "the dropped packet is not counted");
        assert_eq!(taken(&mut inbox), [vec![0; 6]]);
    }

    #[test]
    fn a_deferred_send_to_a_closed_outbox_is_dropped_at_delivery() {
        let (outbox, inbox) = channel(100);
        let scope = defer();
        assert!(outbox.send(vec![0; 4]).is_ok());
        drop(inbox);
        drop(scope);
        assert_eq!(outbox.queued(), 0, "nothing is counted for a connection that has gone");
    }

    #[test]
    fn nested_scopes_deliver_once_at_the_outer_end() {
        let (outbox, mut inbox) = channel(100);
        let outer = defer();
        outbox.send(vec![1]).unwrap();
        let inner = defer();
        outbox.send(vec![2]).unwrap();
        drop(inner);
        assert!(taken(&mut inbox).is_empty(), "the inner end delivers nothing");
        outbox.send(vec![3]).unwrap();
        drop(outer);
        assert_eq!(taken(&mut inbox), [vec![1], vec![2], vec![3]]);
        outbox.send(vec![4]).unwrap();
        assert_eq!(taken(&mut inbox), [vec![4]], "no scope is left open");
    }

    #[test]
    fn a_panic_inside_a_scope_delivers_and_leaves_no_scope_open() {
        let (outbox, mut inbox) = channel(100);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _scope = defer();
            outbox.send(vec![1]).unwrap();
            let _inner = defer();
            outbox.send(vec![2]).unwrap();
            panic!("a panic under the lock");
        }));
        assert!(result.is_err());
        assert_eq!(taken(&mut inbox), [vec![1], vec![2]], "the unwinding delivered what was queued");
        outbox.send(vec![3]).unwrap();
        assert_eq!(taken(&mut inbox), [vec![3]], "a send outside is received at once");
    }

    #[test]
    fn a_scope_is_per_thread() {
        let (outbox, mut inbox) = channel(100);
        let scope = defer();
        let other = outbox.clone();
        std::thread::spawn(move || other.send(vec![7]).unwrap()).join().unwrap();
        assert_eq!(taken(&mut inbox), [vec![7]], "another thread's send is not held by this thread's scope");
        drop(scope);
    }

    #[test]
    fn a_queue_that_grew_gives_its_room_back() {
        let (outbox, mut inbox) = channel(usize::MAX);
        {
            let _scope = defer();
            for _ in 0..KEPT_ENTRIES * 3 {
                outbox.send(vec![0]).unwrap();
            }
            assert!(DEFERRED.with(|d| d.queue.borrow().capacity()) >= KEPT_ENTRIES * 3);
        }
        assert_eq!(taken(&mut inbox).len(), KEPT_ENTRIES * 3);
        let room = DEFERRED.with(|d| d.queue.borrow().capacity());
        assert!((1..=KEPT_ENTRIES * 2).contains(&room), "cut back from {} entries, kept {room}", KEPT_ENTRIES * 3);
    }

    #[test]
    fn a_closed_inbox_gives_the_packet_back_and_counts_nothing() {
        let (outbox, inbox) = channel(100);
        drop(inbox);
        assert!(outbox.send(vec![0; 4]).is_err());
        assert_eq!(outbox.queued(), 0);
    }
}
