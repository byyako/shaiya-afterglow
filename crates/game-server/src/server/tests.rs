use super::tasks::{accept_backoff, listener_is_gone};
use super::*;

#[tokio::test]
async fn a_connection_that_goes_wakes_only_its_accounts_waiters() {
    use std::sync::atomic::AtomicUsize;
    use std::task::{Poll, Wake, Waker};
    struct Wakes(AtomicUsize);
    impl Wake for Wakes {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let users = OnlineUsers::default();
    let (a, _) = Online::claim(&users, 1, Duration::ZERO).await.expect("account 1");
    let (b, _) = Online::claim(&users, 2, Duration::ZERO).await.expect("account 2");
    assert!(Online::claim(&users, 2, Duration::ZERO).await.is_none(), "taken");

    // A waiter for account 1, with a waker that counts.
    let wakes = Arc::new(Wakes(AtomicUsize::new(0)));
    let waker = Waker::from(wakes.clone());
    let mut task = std::task::Context::from_waker(&waker);
    let mut waiter = std::pin::pin!(Online::claim(&users, 1, Duration::from_secs(5)));
    assert!(waiter.as_mut().poll(&mut task).is_pending());
    // Account 2's connection goes: account 1's waiter is not woken.
    drop(b);
    assert_eq!(wakes.0.load(Ordering::Relaxed), 0);
    // Its own account's goes: it is, and takes the slot.
    drop(a);
    assert_eq!(wakes.0.load(Ordering::Relaxed), 1);
    assert!(matches!(waiter.as_mut().poll(&mut task), Poll::Ready(Some(_))));
}

#[test]
fn accept_errors_back_off_from_100_ms_to_a_second() {
    let waits: Vec<_> = (1..=7).map(|n| accept_backoff(n).as_millis()).collect();
    assert_eq!(waits, [100, 200, 400, 800, 1000, 1000, 1000]);
    assert_eq!(accept_backoff(u32::MAX), Duration::from_secs(1));
}

#[test]
fn only_a_listener_that_is_gone_ends_serve() {
    let os = |code| std::io::Error::from_raw_os_error(code);
    assert!(listener_is_gone(&os(9)) && listener_is_gone(&os(88)) && listener_is_gone(&os(22)));
    // EMFILE, ENFILE, ENOMEM, ENOBUFS, ECONNABORTED, EPROTO.
    for code in [24, 23, 12, 105, 103, 71] {
        assert!(!listener_is_gone(&os(code)), "{code}");
    }
    assert!(!listener_is_gone(&std::io::Error::other("x")));
}

#[test]
fn death_costs_five_percent_of_the_level_above_level_ten() {
    // Normal mode: 100 per level, so level 11 runs from 1000 to 1100 (grow 1 is the table's mode 2).
    let table = ExpTable::from_rows((1..=12).map(|level| (level, 2, 100)));
    assert_eq!(death_exp_loss(1050, 11, 1, &table, 0, 5), 5);
    assert_eq!(death_exp_loss(1003, 11, 1, &table, 0, 5), 3, "never below the level's start");
    assert_eq!(death_exp_loss(1000, 11, 1, &table, 0, 5), 0);
    assert_eq!(death_exp_loss(950, 10, 1, &table, 0, 5), 0, "up to level 10 nothing is lost");
    assert_eq!(death_exp_loss(1050, 11, 1, &table, 50, 5), 3, "the faction grade spares half (5 - 2)");
    assert_eq!(death_exp_loss(1050, 11, 1, &table, 100, 5), 0);
    // The item flags' percents (`docs/original/item-flags.md` section 3): a charm of level 1 or 2 loses 1 %, no-loss or level 3 none.
    assert_eq!(death_exp_loss(1050, 11, 1, &table, 0, 1), 1);
    assert_eq!(death_exp_loss(1050, 11, 1, &table, 0, 0), 0);
}

#[test]
fn a_party_drop_handed_to_a_leaving_member_goes_on_the_ground() {
    use crate::zone::Dropped;
    let sword = Loot::Item { item_type: 5, type_id: 1, count: 1, quality: 10, kept: None };
    let at = [10.0, 0.0, 20.0];
    // An item where it dropped, as the zone had it.
    let event = ZoneEvent::Loot { loot: sword, position: [1.0, 0.0, 2.0], owner: 9, party: 4, dropped: Dropped::MOB };
    assert_eq!(left_on_the_ground(event, at, 7), Some((sword, [1.0, 0.0, 2.0], 9, 4, Dropped::MOB)));
    // Gold at the member's feet, its own, a monster's still raised when picked up.
    assert_eq!(left_on_the_ground(ZoneEvent::Gold { amount: 30, from_mob: true }, at, 7), Some((Loot::Gold(30), at, 7, 0, Dropped::MOB)));
    assert_eq!(left_on_the_ground(ZoneEvent::KillGrade, at, 7), None);
}

#[test]
fn a_rules_factor_of_one_changes_nothing_and_the_others_truncate() {
    // The item flags' factor is applied first, as award_exp does, then the rules'.
    let item_then_rules = |gain: u32, item: f64, rules: f64| {
        let raised = (f64::from(gain) * item) as u32;
        ruled_exp(raised, raised, rules)
    };
    for gain in [0u32, 1, 7, 144, 99_999, u32::MAX] {
        assert_eq!(ruled_exp(gain, gain, 1.0), (gain, gain), "a factor of exactly 1: today's numbers");
        assert_eq!(ruled_exp(gain, 0, 1.0), (gain, 0));
    }
    assert_eq!(ruled_exp(144, 144, 2.0), (288, 288));
    assert_eq!(ruled_exp(144, 0, 0.5), (72, 0));
    assert_eq!(ruled_exp(7, 7, 0.5), (3, 3), "truncated");
    assert_eq!(ruled_exp(144, 144, 0.0), (0, 0), "a factor of 0 wins");
    assert_eq!(ruled_exp(u32::MAX, u32::MAX, 100.0), (u32::MAX, u32::MAX), "saturates");
    // With the item factor: 100 x 1.5 = 150, then x 0.5 = 75; 101 x 1.5 = 151 (151.5 cut), x 0.5 = 75 (75.5 cut).
    assert_eq!(item_then_rules(100, 1.5, 0.5), (75, 75));
    assert_eq!(item_then_rules(101, 1.5, 0.5), (75, 75));
    assert_eq!(item_then_rules(100, 1.5, 1.0), (150, 150), "no rule: the item factor alone");
}

#[test]
fn script_commands_are_parsed_from_chat_lines() {
    use super::script::{may_use, parse_command};
    let cmd = |t: &[u8]| parse_command(t);
    assert_eq!(cmd(b"!bounty Bob 1000\0"), Some(("bounty".into(), "Bob 1000".into())));
    assert_eq!(cmd(b"!streak"), Some(("streak".into(), String::new())));
    assert_eq!(cmd(b"!Streak\0"), Some(("streak".into(), String::new())), "any case");
    assert_eq!(cmd(b"!hi   spaced  out  \0"), Some(("hi".into(), "spaced  out".into())));
    // The 8 bytes the client puts in front with a chat colour.
    assert_eq!(cmd(b"<<012c05!hi there\0"), Some(("hi".into(), "there".into())));
    assert_eq!(cmd(b"<<012c05!hi"), Some(("hi".into(), String::new())));
    // Not commands: the whisper prefix (a space), no word, a word with other characters, a marker that is not one, text before it.
    for text in [
        &b"! name text"[..],
        b"!",
        b"!\0",
        b"!bad-word x",
        b"!hi,there",
        b"<<12c05!hi",
        b"<<012x05!hi",
        b"<<012c0",
        b"hello !hi",
        b" !hi",
        b"",
        b"!!hi",
    ] {
        assert_eq!(cmd(text), None, "{:?}", String::from_utf8_lossy(text));
    }
    // The text is cut at its first zero.
    assert_eq!(cmd(b"!a\0!b"), Some(("a".into(), String::new())));
    // Staff levels: 0 is everyone, n staff of levels 1 to n.
    assert!(may_use(0, 0) && may_use(0, 5));
    assert!(!may_use(3, 0), "a player cannot use a staff command");
    assert!(may_use(3, 1) && may_use(3, 3) && !may_use(3, 4) && !may_use(1, 2));
}

/// A connection over a loopback socket and the far side's stream. The shutdown sender is returned so the connection does not
/// see a stop.
async fn loopback_connection() -> (Connection, tokio::net::TcpStream, watch::Sender<bool>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("address");
    let (server, far) = tokio::join!(listener.accept(), tokio::net::TcpStream::connect(addr));
    let (stream, peer) = server.expect("accept");
    let (stop, shutdown) = watch::channel(false);
    let conn = Connection {
        stream,
        decoder: FrameDecoder::new(),
        recv: None,
        send: None,
        xor: None,
        shutdown,
        closed: false,
        kick: None,
        kicked: false,
        kick_notice: Duration::ZERO,
        hold_until: None,
        last_read: Instant::now(),
        write_timeout: Duration::from_secs(5),
        flood: PacketFlood::new(1000, Instant::now()),
        batch: Vec::new(),
        peer,
    };
    (conn, far.expect("connect"), stop)
}

/// Reads `count` frames from the far side.
async fn read_frames(far: &mut tokio::net::TcpStream, count: usize) -> Vec<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut decoder = FrameDecoder::new();
    let mut frames = Vec::new();
    let mut buffer = [0u8; 4096];
    while frames.len() < count {
        while let Some(frame) = decoder.next_frame().expect("a valid frame") {
            frames.push(frame);
        }
        if frames.len() == count {
            break;
        }
        let n = timeout(Duration::from_secs(2), far.read(&mut buffer)).await.expect("bytes within 2 s").expect("read");
        assert!(n > 0, "the connection ended after {} frames", frames.len());
        decoder.push(&buffer[..n]);
    }
    frames
}

/// Packets of different lengths (the payload's first byte says which one).
fn queued_packets() -> Vec<Vec<u8>> {
    [3usize, 40, 0, 700, 12].iter().enumerate().map(|(i, len)| shaiya_proto::encode(0x0100 + i as u16, &vec![i as u8 + 1; *len])).collect()
}

#[tokio::test]
async fn queued_packets_go_out_in_one_write_in_queue_order() {
    let (mut conn, mut far, _stop) = loopback_connection().await;
    let table = Arc::new(XorTable::new(&XOR_SEED));
    conn.xor = Some(table.clone());
    let (outbox, mut inbox) = crate::outbox::channel(1 << 20);
    let originals = queued_packets();
    for packet in &originals {
        outbox.send(packet.clone()).expect("queued");
    }
    let first = inbox.recv().await.expect("the first packet");
    conn.send_queued(first, &mut inbox).await.expect("sent");
    assert_eq!(outbox.queued(), 0, "one call took all five");
    let frames = read_frames(&mut far, originals.len()).await;
    for (mut frame, original) in frames.into_iter().zip(&originals) {
        assert_ne!(frame, *original, "sealed on the wire");
        table.apply(&mut frame[2..]);
        assert_eq!(&frame, original);
    }
}

#[tokio::test]
async fn a_batch_under_aes_keeps_the_stream_in_order() {
    let (mut conn, mut far, _stop) = loopback_connection().await;
    let (key, iv) = ([7u8; 16], [9u8; 16]);
    conn.send = Some(CtrStream::new(&key, &iv));
    let (outbox, mut inbox) = crate::outbox::channel(1 << 20);
    let originals = queued_packets();
    // A packet sent directly first: the stream position must carry over into the batch.
    let lone = shaiya_proto::encode(0x0200, &[5; 9]);
    conn.send(lone.clone()).await.expect("sent");
    for packet in &originals {
        outbox.send(packet.clone()).expect("queued");
    }
    let first = inbox.recv().await.expect("the first packet");
    conn.send_queued(first, &mut inbox).await.expect("sent");
    let frames = read_frames(&mut far, originals.len() + 1).await;
    let mut stream = CtrStream::new(&key, &iv);
    for (mut frame, original) in frames.into_iter().zip(std::iter::once(&lone).chain(&originals)) {
        stream.apply(&mut frame[2..]);
        assert_eq!(&frame, original);
    }
}

#[tokio::test]
async fn a_batch_stops_at_batch_bytes_and_leaves_the_rest_queued() {
    use tokio::io::AsyncReadExt;
    let (mut conn, mut far, _stop) = loopback_connection().await;
    let (outbox, mut inbox) = crate::outbox::channel(1 << 20);
    // Not real frames (a frame is at most 2048 bytes): the far side reads raw bytes.
    let big: Vec<Vec<u8>> = (1..=3u8).map(|i| vec![i; 10 * 1024]).collect();
    for packet in &big {
        outbox.send(packet.clone()).expect("queued");
    }
    let first = inbox.recv().await.expect("the first packet");
    conn.send_queued(first, &mut inbox).await.expect("sent");
    // 10 KB is under the cap, so the second was taken; 20 KB is over it, so the third was not.
    assert_eq!(outbox.queued(), 10 * 1024);
    let mut got = vec![0u8; 20 * 1024];
    timeout(Duration::from_secs(2), far.read_exact(&mut got)).await.expect("20 KB within 2 s").expect("read");
    assert_eq!(&got[..10 * 1024], &big[0][..]);
    assert_eq!(&got[10 * 1024..], &big[1][..]);
    let mut more = [0u8; 1];
    assert!(timeout(Duration::from_millis(100), far.read(&mut more)).await.is_err(), "nothing of the third packet");
    assert_eq!(inbox.try_recv().expect("the third is still queued"), big[2]);
    assert_eq!(outbox.queued(), 0);
}

#[tokio::test]
async fn a_lone_packet_is_written_as_it_is() {
    let (mut conn, mut far, _stop) = loopback_connection().await;
    let table = Arc::new(XorTable::new(&XOR_SEED));
    conn.xor = Some(table.clone());
    let (outbox, mut inbox) = crate::outbox::channel(1 << 20);
    let original = shaiya_proto::encode(0x0301, &[4; 30]);
    outbox.send(original.clone()).expect("queued");
    let first = inbox.recv().await.expect("the first packet");
    conn.send_queued(first, &mut inbox).await.expect("sent");
    assert_eq!(conn.batch.capacity(), 0, "no batch buffer for one packet");
    let mut frame = read_frames(&mut far, 1).await.remove(0);
    table.apply(&mut frame[2..]);
    assert_eq!(frame, original);
}

#[tokio::test]
async fn a_closed_connection_takes_nothing_more_from_the_outbox() {
    use tokio::io::AsyncReadExt;
    let (mut conn, mut far, _stop) = loopback_connection().await;
    conn.closed = true;
    let (outbox, mut inbox) = crate::outbox::channel(1 << 20);
    let originals = queued_packets();
    for packet in &originals {
        outbox.send(packet.clone()).expect("queued");
    }
    let first = inbox.recv().await.expect("the first packet");
    let after_first: usize = originals[1..].iter().map(Vec::len).sum();
    conn.send_queued(first, &mut inbox).await.expect("nothing to do is not an error");
    assert_eq!(outbox.queued(), after_first, "what was behind the first is still queued");
    let mut byte = [0u8; 1];
    assert!(timeout(Duration::from_millis(100), far.read(&mut byte)).await.is_err(), "nothing was written");
}
