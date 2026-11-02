//! Fuzz harness for the game server's packet dispatch (`docs/server/audit.md` section 5.3): random and hostile packets over loopback
//! TCP against `MemoryBackend`, the way `tests/game_flow.rs` drives it.
//!
//! A panic inside a connection task is caught by tokio and would pass unseen, so a hook counts them. Every property checks that the
//! count did not change, that a probe client can still enter the world and be answered (no zone lock is held or poisoned) and that the
//! fuzzed accounts are released. Each property has an `#[ignore]` twin that takes its case count from `PROPTEST_CASES`:
//!
//! ```text
//! PROPTEST_CASES=5000 ~/.cargo/bin/cargo test --release -p game-server --test fuzz_dispatch -- --include-ignored
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::panic;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Once, OnceLock};
use std::time::{Duration, Instant};

use game_server::memory::{MemoryBackend, StoredSession};
use game_server::server::Context;
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::FileFailurePersistence;
use shaiya_crypto::CtrStream;
use shaiya_proto::game as g;
use shaiya_proto::world as w;
use shaiya_proto::{encode, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::time::timeout;

mod common;
use common::*;

// The client opcodes of the table in `docs/server/audit.md` section 5.4, with the layout of what each one carries.

/// One field of a payload. The sizes are those of the decoders.
#[derive(Clone, Copy)]
enum F {
    /// Any byte.
    B,
    /// A bag, a slot, a count.
    G,
    S,
    C,
    /// An object, character or ground id.
    I,
    /// Money or a price.
    M,
    /// A coordinate.
    X,
    /// A `u16`.
    W,
    /// A name field of 21 bytes, a guild name field of 25.
    N,
    K,
    /// Raw bytes of this size.
    R(usize),
    /// `u8 len` and text.
    T,
}

use F::*;

const PLAYER: &[(u16, &[F])] = &[
    // login, game, character screen
    (0xA301, &[I, R(16)]),
    (0x0102, &[B, B, B, B, B, B, B, B, N]),
    (0x0103, &[I]),
    (0x0104, &[I]),
    (0x010F, &[I]),
    (0x0109, &[B]),
    (0x010E, &[I, N]),
    (0x0301, &[I]),
    (0x0302, &[I]),
    (0x0303, &[I]),
    (0x0304, &[I]),
    (0x0305, &[I]),
    (0x0307, &[I]),
    (0x0003, &[]),
    // world, items, skills, combat
    (0x0501, &[B, W, X, X, X]),
    (0x0E04, &[I]),
    (0x0506, &[B]),
    (0x0204, &[G, S, G, S]),
    (0x020A, &[B]),
    (0x0227, &[B]),
    (0x0208, &[W, W, W, W, W, W]),
    (0x020C, &[I, B]),
    (0x0209, &[W, B]),
    (0x0511, &[B, I]),
    (0x0517, &[B, I]),
    (0x010B, &[B, R(20)]),
    (0x0502, &[I]),
    (0x0503, &[I]),
    (0x0553, &[B]),
    (0x0205, &[I]),
    (0x0206, &[G, S, C]),
    (0x050A, &[G, S]),
    (0x0557, &[G, S, I]),
    (0x0559, &[G, S, B, I]),
    (0x0224, &[B]),
    (0x0554, &[G, S]),
    (0x0220, &[R(12)]),
    (0x0226, &[B, B, R(4)]),
    (0x021D, &[I]),
    (0x021E, &[B]),
    (0x0107, &[]),
    (0x010D, &[]),
    // chat
    (0x1101, &[T]),
    (0x1103, &[T]),
    (0x1104, &[T]),
    (0x1105, &[T]),
    (0x1107, &[T]),
    (0x1108, &[T]),
    (0x1111, &[T]),
    (0x1112, &[T]),
    (0x1102, &[N, T]),
    (0x1201, &[W, R(16)]),
    // party and union
    (0x0B0E, &[B, B, I]),
    (0x0B12, &[I, I]),
    (0x0B02, &[I]),
    (0x0B06, &[I]),
    (0x0B07, &[I]),
    (0x0B0F, &[I]),
    (0x0B13, &[I]),
    (0x0B14, &[I]),
    (0x0B17, &[I]),
    (0x0B1A, &[I]),
    (0x0B03, &[B]),
    (0x0B15, &[B]),
    (0x0B09, &[N]),
    (0x0B11, &[N]),
    (0x0B18, &[N]),
    (0x0B10, &[B, B]),
    // trade, duel, guild war, personal shop
    (0x0A01, &[I]),
    (0x2401, &[I]),
    (0x2501, &[I]),
    (0x2309, &[I]),
    (0x0A02, &[B]),
    (0x0A05, &[B]),
    (0x0A07, &[B]),
    (0x2402, &[B]),
    (0x2409, &[B]),
    (0x240B, &[B]),
    (0x2502, &[B]),
    (0x2306, &[B]),
    (0x0A06, &[G, S, C, S]),
    (0x240A, &[G, S, C, S]),
    (0x0A08, &[M]),
    (0x240C, &[M]),
    (0x2303, &[B, R(40)]),
    (0x2305, &[G, S, S, M]),
    (0x2307, &[S, M]),
    (0x230D, &[S, C]),
    // guild
    (0x0D25, &[W, B]),
    (0x0D21, &[K, R(65)]),
    (0x0D20, &[R(65)]),
    (0x0D04, &[B]),
    (0x0D06, &[B]),
    (0x0D22, &[B]),
    (0x0D05, &[I]),
    (0x0D07, &[I]),
    (0x0D0A, &[I]),
    (0x0D34, &[I]),
    (0x0D35, &[I]),
    (0x0D08, &[B, I]),
    (0x0D0C, &[B, I]),
    // friends, quests, shops, gems, market, mall, mini games
    (0x2202, &[N]),
    (0x2209, &[N]),
    (0x2203, &[B]),
    (0x2205, &[I]),
    (0x220A, &[I]),
    (0x2206, &[I, T]),
    (0x220B, &[I, T]),
    (0x0902, &[I, W]),
    (0x0903, &[I, W]),
    (0x0904, &[I, W]),
    (0x0907, &[W, B]),
    (0x0908, &[W]),
    (0x0702, &[I, B, C]),
    (0x0703, &[G, S, C]),
    (0x0801, &[G, S, G, S, G, S]),
    (0x0802, &[G, S, B, B, G, S]),
    (0x0803, &[G, S, R(6)]),
    (0x0804, &[G, S]),
    (0x0808, &[G, S]),
    (0x0805, &[G, S, G, S, C, G, S]),
    (0x0806, &[G, S, G, S]),
    (0x0807, &[G, G, G, S, S, S]),
    (0x1C01, &[R(37)]),
    (0x1C02, &[B, B]),
    (0x1C03, &[B]),
    (0x1C08, &[G, S, C, B, M, M]),
    (0x1C09, &[I]),
    (0x1C0A, &[I]),
    (0x1C0C, &[I]),
    (0x1C0D, &[I]),
    (0x1C15, &[I]),
    (0x1C16, &[I]),
    (0x1C0B, &[I, M]),
    (0x2602, &[N]),
    (0x2603, &[N, N]),
    (0x2607, &[B]),
    (0xB102, &[B]),
    (0x3101, &[B]),
    (0x3107, &[B]),
];

const STAFF: &[(u16, &[F])] = &[
    (0xF101, &[T]),
    (0xF102, &[N, T]),
    (0xF103, &[T]),
    (0xF104, &[T]),
    (0xF105, &[T]),
    (0xF10A, &[T]),
    (0xF202, &[B, I, W, R(8)]),
    (0xF203, &[I, N, W, R(8)]),
    (0xF601, &[N, B]),
    (0xF602, &[N]),
    (0xF701, &[B, I, N]),
    (0xF702, &[B, B, B]),
    (0xF703, &[N, B, B]),
    (0xF704, &[W, B]),
    (0xF705, &[W, B]),
    (0xF709, &[N]),
    (0xF70A, &[W, N]),
    (0xF70B, &[N]),
    (0xF70C, &[B, W, B]),
    (0xF70D, &[B, W, B]),
    (0xF70E, &[I]),
    (0xF801, &[N]),
    (0xF802, &[N]),
    (0xF803, &[N]),
    (0xF804, &[N]),
    (0xF805, &[W, N]),
    (0xF806, &[N]),
    (0xF809, &[W]),
    (0xF80A, &[W]),
    (0xF80B, &[N, W]),
    (0xF80C, &[N, W]),
    (0xF80D, &[N]),
    (0xF80E, &[N]),
    (0xF80F, &[N, N]),
    (0xF810, &[N, N]),
    (0xF901, &[N]),
    (0xF904, &[N]),
    (0xF905, &[N, X, X, W]),
    (0xF906, &[T]),
    (0xF907, &[W, T]),
    (0xF908, &[N, W, T]),
    (0xF909, &[W, T]),
    (0xF90A, &[B, W, T]),
    (0xF90B, &[W, T]),
    (0xF90C, &[N, X, X, W]),
    (0xFA07, &[N, T]),
    (0xFA08, &[N]),
    (0xFA09, &[N]),
    (0xFA0A, &[X, X, W]),
    (0xFA0B, &[N]),
    (0xFA0D, &[W, N]),
    (0xFA0E, &[N]),
    (0xFA0F, &[W, N]),
    (0xFA10, &[N]),
    (0xFA11, &[W]),
    (0xFA12, &[X, X, W, K]),
    (0xFA13, &[K, N, B]),
    (0xFA14, &[K, B]),
    (0xFA15, &[K, I]),
    (0xFA18, &[B]),
    (0xFA19, &[N, I]),
    (0xFA1A, &[N]),
];

/// The opcodes the two-player property leans on: trade, duel, shop, party, drop and pick-up, item moves and uses.
const EXCHANGE: &[u16] = &[
    0x0A01, 0x0A02, 0x0A05, 0x0A06, 0x0A07, 0x0A08, 0x2401, 0x2402, 0x2409, 0x240A, 0x240B, 0x240C, 0x2303, 0x2305, 0x2306, 0x2307, 0x2309,
    0x230D, 0x0B02, 0x0B03, 0x0B06, 0x0B07, 0x0B09, 0x0B0F, 0x0206, 0x0205, 0x0204, 0x050A, 0x0557, 0x0559, 0x0702, 0x0703, 0x0554, 0x0805,
    0x0801, 0x1C08, 0x2501, 0x2502,
];

fn layout(op: u16) -> Option<&'static [F]> {
    PLAYER.iter().chain(STAFF).find(|(o, _)| *o == op).map(|(_, f)| *f)
}

fn size(f: &[F]) -> usize {
    f.iter()
        .map(|f| match f {
            B | G | S | C => 1,
            W => 2,
            I | M | X => 4,
            N => 21,
            K => 25,
            R(n) => *n,
            T => 17,
        })
        .sum()
}

/// One packet as the strategy makes it. What the ids and names are is known only when it is sent.
#[derive(Clone, Debug)]
struct Raw {
    op: u16,
    /// 0 to 99: which kind of payload.
    kind: u8,
    /// The choices of the field-aware payload, taken in turn.
    nums: Vec<u32>,
    /// Random bytes for the other kinds.
    bytes: Vec<u8>,
}

fn opcode(staff: bool, exchange: bool) -> BoxedStrategy<u16> {
    let known: Vec<u16> = PLAYER.iter().chain(if staff { STAFF } else { &[] }).map(|(o, _)| *o).collect();
    let admin: Vec<u16> = STAFF.iter().map(|(o, _)| *o).collect();
    let listed = if exchange {
        prop_oneof![8 => select(EXCHANGE.to_vec()), 2 => select(known.clone())].boxed()
    } else if staff {
        prop_oneof![5 => select(admin), 3 => select(known.clone())].boxed()
    } else {
        select(known.clone()).boxed()
    };
    // The unused numbers of those families: a known opcode with another low byte.
    let unused = (select(known), any::<u8>()).prop_map(|(op, low)| (op & 0xFF00) | u16::from(low));
    prop_oneof![7 => listed, 1 => unused, 2 => any::<u16>()].boxed()
}

fn raw(staff: bool, exchange: bool) -> impl Strategy<Value = Raw> {
    let bytes = prop_oneof![17 => vec(any::<u8>(), 0..=64), 3 => vec(any::<u8>(), 0..=2044)];
    // The exchange property aims at the hot paths, so most of its payloads are field-aware.
    let kind =
        if exchange { prop_oneof![25 => 0u8..40, 50 => 40u8..55, 15 => 55u8..85, 10 => 85u8..100].boxed() } else { (0u8..100).boxed() };
    (opcode(staff, exchange), kind, vec(any::<u32>(), 0..24), bytes).prop_map(|(op, kind, nums, bytes)| Raw { op, kind, nums, bytes })
}

/// What the ids of a packet can be.
struct Ids {
    own: u32,
    other: u32,
    own_name: Vec<u8>,
    other_name: Vec<u8>,
    ground: Vec<u32>,
}

/// Takes the next choice of a packet.
struct Picks<'a>(std::slice::Iter<'a, u32>);

impl Picks<'_> {
    fn next(&mut self) -> u32 {
        self.0.next().copied().unwrap_or(0)
    }
}

fn pick<T: Copy>(picks: &mut Picks, pool: &[T]) -> T {
    pool[picks.next() as usize % pool.len()]
}

fn field(out: &mut Vec<u8>, f: F, picks: &mut Picks, ids: &Ids, bytes: &mut impl Iterator<Item = u8>) {
    const BAGS: [u8; 10] = [0, 1, 2, 5, 6, 100, 255, 1, 1, 1];
    const SLOTS: [u8; 14] = [0, 1, 2, 3, 4, 23, 24, 239, 240, 255, 0, 1, 2, 3];
    const COUNTS: [u8; 5] = [0, 1, 2, 254, 255];
    const MONEY: [u32; 6] = [0, 1, 300, 1000, 100_000, u32::MAX];
    const FLOATS: [f32; 9] = [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e38, 1200.0, 580.0, 1765.0];
    match f {
        B => out.push(pick(picks, &[0u8, 1, 2, 3, 4, 5, 255])),
        G => out.push(pick(picks, &BAGS)),
        S => out.push(pick(picks, &SLOTS)),
        C => out.push(pick(picks, &COUNTS)),
        I => {
            let id = match picks.next() % 12 {
                0..=2 => ids.own,
                3..=5 => ids.other,
                6 => 0,
                7 => 1,
                8 => u32::MAX,
                9 | 10 if !ids.ground.is_empty() => ids.ground[picks.next() as usize % ids.ground.len()],
                _ => picks.next() % 40,
            };
            out.extend_from_slice(&id.to_le_bytes());
        }
        M => out.extend_from_slice(&pick(picks, &MONEY).to_le_bytes()),
        X => out.extend_from_slice(&pick(picks, &FLOATS).to_le_bytes()),
        W => out.extend_from_slice(&(picks.next() as u16).to_le_bytes()),
        N | K => {
            let len = if matches!(f, N) { 21 } else { 25 };
            let name: Vec<u8> = match picks.next() % 6 {
                0 => ids.own_name.clone(),
                1 | 2 => ids.other_name.clone(),
                3 => b"Walker".to_vec(),
                4 => Vec::new(),
                _ => bytes.take(len).collect(),
            };
            let mut field = vec![0u8; len];
            let n = name.len().min(len);
            field[..n].copy_from_slice(&name[..n]);
            out.extend_from_slice(&field);
        }
        R(n) => out.extend(bytes.take(n)),
        T => {
            let len = pick(picks, &[2u8, 5, 12, 40, 0, 1, 129, 255]);
            out.push(len);
            let text: &[u8] = match picks.next() % 4 {
                0 => b"hello hello hello hello hello hello hello hello hello hello",
                1 => b"<<005c01hello<<003c02abc<<002c03xy hello hello hello hello",
                2 => b"<<999c01x<<0c1 <<<<<< hello hello hello hello hello hello",
                _ => b"",
            };
            if text.is_empty() {
                out.extend(bytes.take(usize::from(len)));
            } else {
                out.extend(text.iter().copied().cycle().take(usize::from(len)));
            }
        }
    }
}

/// The payload of a packet: 40% random bytes of the length the decoder takes, 15% field-aware, 30% random bytes of 0 to 64, 15%
/// random bytes of 0 to 2,044.
fn payload(raw: &Raw, ids: &Ids) -> Vec<u8> {
    let layout = layout(raw.op);
    let mut bytes = raw.bytes.iter().copied().chain(std::iter::repeat(0x5a));
    match (raw.kind, layout) {
        (0..40, Some(f)) => bytes.take(size(f)).collect(),
        (0..40, None) => bytes.take(raw.nums.first().copied().unwrap_or(4) as usize % 17).collect(),
        (40..55, Some(f)) => {
            let mut out = Vec::new();
            let mut picks = Picks(raw.nums.iter());
            for &f in f {
                field(&mut out, f, &mut picks, ids, &mut bytes);
            }
            out
        }
        (40..85, _) => raw.bytes.iter().copied().take(64).collect(),
        _ => raw.bytes.clone(),
    }
}

fn packet(raw: &Raw, ids: &Ids) -> Vec<u8> {
    let mut payload = payload(raw, ids);
    payload.truncate(2044);
    encode(raw.op, &payload)
}

// The test server and its clients.

static PANICS: AtomicUsize = AtomicUsize::new(0);

/// Counts the panics of the server's tasks and prints them. Tasks run on the runtime's worker threads; a test's own thread is where its
/// assertions fail, and that already fails that test, so counting it would make one failure fail the others as well.
fn watch_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if std::thread::current().name().is_some_and(|name| name.starts_with("tokio-")) {
                PANICS.fetch_add(1, Ordering::SeqCst);
                eprintln!("PANIC in a server task: {info}");
            }
            default(info);
        }));
    });
}

static NEXT_USER: AtomicU32 = AtomicU32::new(10_000);

struct Fixture {
    runtime: Runtime,
    addr: SocketAddr,
    ctx: Arc<Context<Arc<MemoryBackend>>>,
    backend: Arc<MemoryBackend>,
}

impl Fixture {
    fn new() -> Self {
        watch_panics();
        // `FUZZ_LOG=game_server=debug` shows what the server logs, for tracing a failure.
        if let Ok(filter) = std::env::var("FUZZ_LOG") {
            let _ = tracing_subscriber::fmt().with_env_filter(filter).with_test_writer().try_init();
        }
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let backend = Arc::new(MemoryBackend::default());
        let mut config = config(true);
        // Random GM chat must not be able to quit the server; short timeouts so that the dead are released fast.
        config.console_level = 0;
        config.max_connections = 500;
        config.max_per_ip = 100_000;
        config.connects_per_minute = 1_000_000;
        config.handshake_timeout = Duration::from_secs(2);
        config.idle_timeout = Duration::from_secs(5);
        config.linger = Duration::from_millis(30);
        config.kick_notice = Duration::from_millis(30);
        config.logout_delay = Duration::from_millis(30);
        config.town_delay = Duration::from_millis(30);
        config.handshake_wait = Duration::from_millis(300);
        let (addr, ctx) = runtime.block_on(start_with_data(backend.clone(), config, data_with(true)));
        Self { runtime, addr, ctx, backend }
    }

    /// A fresh account that has logged in and picked this server: its id and identity.
    fn account(&self, privilege: u8) -> (u32, [u8; 16]) {
        let user = NEXT_USER.fetch_add(1, Ordering::SeqCst);
        let mut identity = [0xa5; 16];
        identity[..4].copy_from_slice(&user.to_le_bytes());
        self.backend.add_session(user, StoredSession { privilege, ..session(identity, Some(SERVER_ID), LOCAL) });
        (user, identity)
    }
}

/// From the handshake to a created character: its id.
async fn create_character(client: &mut Client, user: u32, identity: [u8; 16], name: &[u8]) -> u32 {
    client.enter(user, identity).await;
    client.write(encode(g::OP_CHAR_COUNTRY, &[0])).await;
    client.write(create(0, 0, 0, name)).await;
    assert_eq!(client.read().await.unwrap(), g::char_create_response(0), "character created");
    let list = client.read_list().await;
    let id = u32::from_le_bytes(list[4][1..5].try_into().unwrap());
    assert_ne!(id, 0);
    id
}

/// Selects the character and enters the world.
async fn into_world(client: &mut Client, id: u32) {
    client.write(g::char_id_request(g::OP_CHAR_SELECT, id)).await;
    while frame::opcode(&client.read().await.expect("burst")) != w::OP_CHAR_QUICK {}
    client.write(encode(w::OP_USER_ENTER, &[])).await;
    // `read_zone_entry` expects what a player gets; a staff account is sent more, so this reads up to the player's HP, MP and SP.
    while frame::opcode(&client.read().await.expect("zone entry")) != shaiya_proto::combat::OP_RECOVER_SET {}
}

fn name_of(user: u32) -> Vec<u8> {
    format!("F{user}").into_bytes()
}

/// A second client on a fresh account enters the world and is answered to a step: no zone lock is held or poisoned.
async fn probe(fx: &Fixture) {
    let (user, identity) = fx.account(11);
    timeout(Duration::from_secs(10), async {
        let mut client = Client::connect(fx.addr).await;
        let id = create_character(&mut client, user, identity, &name_of(user)).await;
        into_world(&mut client, id).await;
        client.write(step(580.0, 1765.0)).await;
        loop {
            let packet = client.read().await.expect("the probe is not cut off");
            if frame::opcode(&packet) == w::OP_MOVE && frame::payload(&packet)[..4] == id.to_le_bytes() {
                break;
            }
        }
    })
    .await
    .expect("the probe entered the world and was answered");
    wait_gone(&fx.backend, user, Duration::from_secs(10)).await;
}

/// The writing side of a fuzzed connection. What the server sends is read and thrown away; the reader says when it is closed.
struct Link {
    writer: OwnedWriteHalf,
    cipher: Option<CtrStream>,
    closed: watch::Receiver<bool>,
    reader: tokio::task::JoinHandle<()>,
}

impl Link {
    fn new(client: Client) -> Self {
        let Client { stream, send, .. } = client;
        let (mut read, writer) = stream.into_split();
        let (tx, closed) = watch::channel(false);
        let reader = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            while matches!(read.read(&mut buffer).await, Ok(n) if n > 0) {}
            let _ = tx.send(true);
        });
        Self { writer, cipher: send, closed, reader }
    }

    /// Whether the server has closed the connection.
    fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    /// Sends bytes (encrypted after the length field, as a packet is); false when the connection is gone.
    async fn send(&mut self, mut bytes: Vec<u8>) -> bool {
        if self.is_closed() {
            return false;
        }
        if let Some(cipher) = &mut self.cipher
            && bytes.len() > 2
        {
            cipher.apply(&mut bytes[2..]);
        }
        self.writer.write_all(&bytes).await.is_ok()
    }

    /// Waits a moment for the server to close the connection, then lets go of it.
    async fn finish(mut self, wait: Duration) {
        let _ = timeout(wait, self.closed.wait_for(|closed| *closed)).await;
        self.reader.abort();
    }
}

/// The cases that failed go to `proptest-regressions/fuzz_dispatch.txt` (the default place is found from `lib.rs`, which a test file
/// has no sibling of).
fn config_cases(default: u32) -> ProptestConfig {
    ProptestConfig {
        cases: default,
        max_shrink_iters: 200,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct("proptest-regressions/fuzz_dispatch.txt"))),
        ..ProptestConfig::default()
    }
}

/// The cases of a deep run: `PROPTEST_CASES`, or 5,000.
fn deep_cases() -> u32 {
    std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(5000)
}

/// How long the server has after the last packet to answer it and close, before the check goes on.
const SETTLE: Duration = Duration::from_millis(40);

/// The same property twice: with a fixed number of cases, and as an `#[ignore]` twin whose count is `PROPTEST_CASES`.
macro_rules! property {
    ($short:ident, $long:ident, $cases:expr, $case:ident in $strategy:expr => $body:expr) => {
        proptest! {
            #![proptest_config(config_cases($cases))]
            #[test]
            fn $short($case in $strategy) {
                static FIXTURE: OnceLock<Fixture> = OnceLock::new();
                let fx = FIXTURE.get_or_init(Fixture::new);
                $body(fx, &$case)?;
            }
        }
        proptest! {
            #![proptest_config(ProptestConfig { cases: deep_cases(), ..config_cases(0) })]
            #[test]
            #[ignore = "deep run: PROPTEST_CASES=5000 cargo test --release -p game-server --test fuzz_dispatch -- --include-ignored"]
            fn $long($case in $strategy) {
                static FIXTURE: OnceLock<Fixture> = OnceLock::new();
                let fx = FIXTURE.get_or_init(Fixture::new);
                $body(fx, &$case)?;
            }
        }
    };
}

fn no_panic(before: usize) -> Result<(), TestCaseError> {
    prop_assert_eq!(PANICS.load(Ordering::SeqCst), before, "a panic was counted (printed above)");
    Ok(())
}

// 1 and 4: one player in the world, ordinary or staff.

/// With `FUZZ_TRACE` set, the opcodes of each case are printed before it runs, so a failure that is the server's state after an earlier
/// case (the shrinker then reduces it to any packet) can be traced back to the case that changed it.
fn trace(what: &str, packets: impl IntoIterator<Item = u16>) {
    if std::env::var_os("FUZZ_TRACE").is_some() {
        eprintln!("TRACE {what}: {}", packets.into_iter().map(|op| format!("{op:04X}")).collect::<Vec<_>>().join(" "));
    }
}

fn one_player(fx: &Fixture, privilege: u8, packets: &[Raw]) -> Result<(), TestCaseError> {
    trace(if privilege == 1 { "staff" } else { "player" }, packets.iter().map(|raw| raw.op));
    let before = PANICS.load(Ordering::SeqCst);
    let user = fx.runtime.block_on(async {
        let (user, identity) = fx.account(privilege);
        let mut client = Client::connect(fx.addr).await;
        let id = create_character(&mut client, user, identity, &name_of(user)).await;
        into_world(&mut client, id).await;
        let ids = Ids { own: id, other: id.wrapping_add(1), own_name: name_of(user), other_name: b"Nobody".to_vec(), ground: Vec::new() };
        let mut link = Link::new(client);
        for raw in packets {
            let ground: Vec<u32> = fx.ctx.zones.ground_items(1).iter().map(|i| i.id).collect();
            if !link.send(packet(raw, &Ids { ground, ..clone_ids(&ids) })).await {
                break;
            }
        }
        link.finish(SETTLE).await;
        probe(fx).await;
        // The fuzzed account is gone: the connection ended, everything was saved and the session released.
        wait_gone(&fx.backend, user, Duration::from_secs(10)).await;
        user
    });
    let _ = user;
    no_panic(before)
}

fn clone_ids(ids: &Ids) -> Ids {
    Ids { own: ids.own, other: ids.other, own_name: ids.own_name.clone(), other_name: ids.other_name.clone(), ground: ids.ground.clone() }
}

property!(one_player_survives, one_player_survives_long, 64,
    packets in vec(raw(false, false), 1..=40) => |fx: &Fixture, packets: &Vec<Raw>| one_player(fx, 11, packets));

property!(staff_dispatch_survives, staff_dispatch_survives_long, 48,
    packets in vec(raw(true, false), 1..=40) => |fx: &Fixture, packets: &Vec<Raw>| one_player(fx, 1, packets));

// 3: before the world, random packets before the handshake and on the character screen.

#[derive(Clone, Debug)]
struct Early {
    /// Sent in plain text on a connection that has not shaken hands.
    before: Vec<Raw>,
    /// Bytes that are not even packets.
    garbage: Vec<Vec<u8>>,
    /// Sent on the character screen.
    screen: Vec<Raw>,
}

fn early() -> impl Strategy<Value = Early> {
    (vec(raw(false, false), 0..=14), vec(vec(any::<u8>(), 0..=300), 0..=3), vec(raw(false, false), 0..=30))
        .prop_map(|(before, garbage, screen)| Early { before, garbage, screen })
}

fn before_the_world(fx: &Fixture, case: &Early) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    fx.runtime.block_on(async {
        let ids = Ids { own: 0, other: 1, own_name: b"Nobody".to_vec(), other_name: b"Nobody".to_vec(), ground: Vec::new() };

        // A connection that never shakes hands.
        {
            let mut link = Link::new(Client::connect(fx.addr).await);
            for raw in &case.before {
                if !link.send(packet(raw, &ids)).await {
                    break;
                }
            }
            for bytes in &case.garbage {
                if !link.send(bytes.clone()).await {
                    break;
                }
            }
            link.finish(SETTLE).await;
        }

        // A connection on the character screen.
        let (user, identity) = fx.account(11);
        let mut client = Client::connect(fx.addr).await;
        let id = create_character(&mut client, user, identity, &name_of(user)).await;
        let ids = Ids { own: id, other: id.wrapping_add(1), own_name: name_of(user), ..ids };
        let mut link = Link::new(client);
        for raw in &case.screen {
            if !link.send(packet(raw, &ids)).await {
                break;
            }
        }
        link.finish(SETTLE).await;

        probe(fx).await;
        wait_gone(&fx.backend, user, Duration::from_secs(10)).await;
    });
    no_panic(before)
}

property!(before_the_world_survives, before_the_world_survives_long, 48,
    case in early() => before_the_world);

// 2: two players, and value is conserved.

#[derive(Clone, Debug)]
struct Pair {
    /// Who sends it (false: the first player) and the packet.
    packets: Vec<(bool, Raw)>,
}

fn pair() -> impl Strategy<Value = Pair> {
    vec((any::<bool>(), raw(false, true)), 1..=60).prop_map(|packets| Pair { packets })
}

/// What a player starts with: the gold, and the count of each (type, type id) in bags and warehouse.
#[derive(Debug, Default)]
struct Wealth {
    gold: u64,
    items: HashMap<(u8, u8), u64>,
}

impl Wealth {
    fn add(&mut self, backend: &MemoryBackend, id: u32, user: u32) {
        let (money, items) = backend.stored(id).unwrap();
        self.gold += u64::from(money);
        for item in items.iter().chain(&backend.bank(user, SERVER_ID)) {
            *self.items.entry((item.item_type, item.type_id)).or_default() += u64::from(item.count);
        }
    }

    fn add_ground(&mut self, ctx: &Context<Arc<MemoryBackend>>, maps: &[u16]) {
        use game_server::drops::Loot;
        for item in maps.iter().flat_map(|&map| ctx.zones.ground_items(map)) {
            match item.loot {
                Loot::Gold(amount) => self.gold += u64::from(amount),
                Loot::Item { item_type, type_id, count, .. } => *self.items.entry((item_type, type_id)).or_default() += u64::from(count),
            }
        }
    }
}

fn two_players(fx: &Fixture, case: &Pair) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    let (start, end) = fx.runtime.block_on(async {
        // Nothing of an earlier case is left on the ground to be picked up.
        fx.ctx.zones.expire_items(Instant::now() + Duration::from_secs(3600));

        let place = |bag, slot, item_type: u8, type_id: u8, count| w::ItemUnit {
            bag,
            slot,
            item_type,
            type_id,
            quality: 1200,
            count,
            ..Default::default()
        };
        let mut players = Vec::new();
        for _ in 0..2 {
            let (user, identity) = fx.account(11);
            let mut client = Client::connect(fx.addr).await;
            let id = create_character(&mut client, user, identity, &name_of(user)).await;
            // Gold, stackable and unstackable items in the bags, and a warehouse, on a spot away from the merchants and the monsters.
            fx.backend.set_money(id, 5_000);
            fx.backend.set_position(id, 1, [1200.0, 80.0, 1200.0]);
            fx.backend.give_items(
                id,
                vec![
                    place(1, 1, 24, 1, 1),
                    place(1, 2, 100, 48, 1),
                    place(1, 3, 30, 1, 5),
                    place(1, 4, 44, 147, 10),
                    place(1, 5, 42, 1, 1),
                ],
            );
            fx.backend.set_bank(user, SERVER_ID, vec![place(100, 0, 25, 1, 50), place(100, 1, 24, 1, 1)]);
            players.push((client, id, user));
        }
        let mut start = Wealth::default();
        for (_, id, user) in &players {
            start.add(&fx.backend, *id, *user);
        }
        for (client, id, _) in &mut players {
            into_world(client, *id).await;
        }
        let names = [name_of(players[0].2), name_of(players[1].2)];
        let ids = [players[0].1, players[1].1];
        let users = [players[0].2, players[1].2];
        let mut links: Vec<Link> = players.into_iter().map(|(client, ..)| Link::new(client)).collect();

        for (second, raw) in &case.packets {
            let (me, you) = if *second { (1, 0) } else { (0, 1) };
            let ground: Vec<u32> = fx.ctx.zones.ground_items(1).iter().map(|i| i.id).collect();
            let ids = Ids { own: ids[me], other: ids[you], own_name: names[me].clone(), other_name: names[you].clone(), ground };
            // A closed connection is a normal outcome; the other player goes on.
            links[me].send(packet(raw, &ids)).await;
            tokio::task::yield_now().await;
        }
        for link in links {
            link.finish(SETTLE / 2).await;
        }
        probe(fx).await;
        for user in users {
            wait_gone(&fx.backend, user, Duration::from_secs(10)).await;
        }

        // Once both are gone everything is saved: what is left of the gold and of each item is at most what was there.
        let mut end = Wealth::default();
        for (id, user) in ids.iter().zip(users) {
            end.add(&fx.backend, *id, user);
        }
        end.add_ground(&fx.ctx, &[1, 2, 3, 35, 42]);
        (start, end)
    });
    no_panic(before)?;
    for (kind, count) in &end.items {
        let had = start.items.get(kind).copied().unwrap_or(0);
        prop_assert!(*count <= had, "item {kind:?} grew: {had} to {count}");
    }
    // Selling works anywhere, as in the original (`docs/original/shop.md` section 3), so gold may grow by what the lost items sell for.
    let sold: u64 = start
        .items
        .iter()
        .map(|(&(item_type, type_id), &had)| {
            let lost = had.saturating_sub(end.items.get(&(item_type, type_id)).copied().unwrap_or(0));
            lost * u64::from(fx.ctx.data.items.get(item_type, type_id).map_or(0, |def| def.sell))
        })
        .sum();
    prop_assert!(end.gold <= start.gold + sold, "gold grew: {} to {} with {sold} for sale", start.gold, end.gold);
    Ok(())
}

property!(two_players_conserve_value, two_players_conserve_value_long, 32,
    case in pair() => two_players);
