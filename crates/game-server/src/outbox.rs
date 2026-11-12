//! The packets other players' actions and the zone queue for one connection (`docs/server/audit.md` R1). A connection that stops
//! reading must not pile up memory without end: the queue is bounded by bytes, and past the limit a packet is dropped and the queue
//! marks itself overflowed, which `world_loop` takes as a lost connection.

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

impl Outbox {
    /// Queues a packet. Past the byte limit the packet is dropped, the queue is marked overflowed and the packet comes back as an
    /// error, as it does when the connection has gone.
    pub fn send(&self, packet: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
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

    #[test]
    fn a_closed_inbox_gives_the_packet_back_and_counts_nothing() {
        let (outbox, inbox) = channel(100);
        drop(inbox);
        assert!(outbox.send(vec![0; 4]).is_err());
        assert_eq!(outbox.queued(), 0);
    }
}
