//! Fuzz harness for the login server (`docs/server/audit.md` section 5.5): random packets before the key, after the key and after a
//! login, over loopback TCP against `MemoryBackend`. A panic inside a connection task is caught by tokio and would pass unseen, so a hook
//! counts them. Every property checks that the count did not change and that a real client still logs in afterwards. Each property has an
//! `#[ignore]` twin that takes its case count from `PROPTEST_CASES`:
//!
//! ```text
//! PROPTEST_CASES=5000 ~/.cargo/bin/cargo test --release -p login-server --test fuzz_login -- --include-ignored
//! ```

use std::net::SocketAddr;
use std::panic;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once, OnceLock};
use std::time::Duration;

use login_server::keys::KeyPool;
use login_server::limits::LimitConfig;
use login_server::server::{Context, serve};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::FileFailurePersistence;
use shaiya_proto::login::{self, select_result};
use shaiya_proto::{encode, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::time::timeout;

mod common;
use common::*;

/// One packet as the strategy makes it.
#[derive(Clone, Debug)]
struct Raw {
    op: u16,
    /// 0 to 99: which kind of payload.
    kind: u8,
    /// The choices of the field-aware payload.
    nums: Vec<u32>,
    bytes: Vec<u8>,
}

const OPS: [u16; 5] = [login::OP_HANDSHAKE, login::OP_LOGIN, login::OP_SELECT_SERVER, 0x0003, login::OP_SERVER_LIST];

/// The names and passwords of the test accounts (`common::backend`), unknown ones, and the extremes.
const USERS: [&str; 9] = ["alice", "GMBob", "boss", "banned", "leaving", "longpw", "nobody", "", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"];
const PASSWORDS: [&str; 9] =
    ["secret", "gmpass", "bosspass", "banpass", "leavepass", "abcdefghijklmnop", "wrong", "", "bbbbbbbbbbbbbbbbbbb"];

fn raw(ops: BoxedStrategy<u16>) -> impl Strategy<Value = Raw> {
    let bytes = prop_oneof![17 => vec(any::<u8>(), 0..=64), 3 => vec(any::<u8>(), 0..=2044)];
    (ops, 0u8..100, vec(any::<u32>(), 0..8), bytes).prop_map(|(op, kind, nums, bytes)| Raw { op, kind, nums, bytes })
}

/// The login server's opcodes (70%), the unused numbers of the same families (10%), any number (20%).
fn any_op() -> BoxedStrategy<u16> {
    let unused = (select(OPS.to_vec()), any::<u8>()).prop_map(|(op, low)| (op & 0xFF00) | u16::from(low));
    prop_oneof![7 => select(OPS.to_vec()), 1 => unused, 2 => any::<u16>()].boxed()
}

/// After a login the packets that matter are the server selection and the repeated login.
fn after_login_op() -> BoxedStrategy<u16> {
    prop_oneof![6 => Just(login::OP_SELECT_SERVER), 2 => Just(login::OP_LOGIN), 2 => any_op()].boxed()
}

fn field(raw: &Raw, at: usize) -> u32 {
    raw.nums.get(at).copied().unwrap_or(0)
}

fn credential(text: &str, len: usize) -> Vec<u8> {
    let mut field = vec![0u8; len];
    field[..text.len().min(len)].copy_from_slice(&text.as_bytes()[..text.len().min(len)]);
    field
}

/// The payload of a packet: 40% random bytes of the length the decoder takes, 15% field-aware, 30% random bytes of 0 to 64, 15% random
/// bytes of 0 to 2,044.
fn payload(raw: &Raw) -> Vec<u8> {
    let fill = |n: usize| raw.bytes.iter().copied().chain(std::iter::repeat(0x5a)).take(n).collect::<Vec<u8>>();
    match (raw.kind, raw.op) {
        (0..40, login::OP_HANDSHAKE) => fill(129),
        (0..40, login::OP_LOGIN) => fill(48 + field(raw, 0) as usize % 4),
        (0..40, login::OP_SELECT_SERVER) => fill(5),
        (0..40, _) => fill(field(raw, 0) as usize % 8),
        (40..55, login::OP_HANDSHAKE) => {
            // A ciphertext that is the modulus's top, zero, or one, with the length byte right and sometimes wrong.
            let mut p = vec![if field(raw, 1).is_multiple_of(4) { 0x7f } else { 0x80 }];
            p.extend(match field(raw, 0) % 3 {
                0 => vec![0xff; 128],
                1 => vec![0; 128],
                _ => fill(128),
            });
            p
        }
        (40..55, login::OP_LOGIN) => {
            let user = USERS[field(raw, 0) as usize % USERS.len()];
            let pass = PASSWORDS[field(raw, 1) as usize % PASSWORDS.len()];
            let mut p = credential(user, 32);
            p.extend(credential(pass, 16 + field(raw, 2) as usize % 4));
            p
        }
        (40..55, login::OP_SELECT_SERVER) => {
            let id = [0u8, 1, 1, 1, 2, 255][field(raw, 0) as usize % 6];
            let version = [0u32, 108, 108, 1, u32::MAX][field(raw, 1) as usize % 5];
            let mut p = vec![id];
            p.extend(version.to_le_bytes());
            p
        }
        (40..85, _) => fill(raw.bytes.len().min(64)),
        _ => raw.bytes.clone(),
    }
}

fn packet(raw: &Raw) -> Vec<u8> {
    let mut payload = payload(raw);
    payload.truncate(2044);
    encode(raw.op, &payload)
}

static PANICS: AtomicUsize = AtomicUsize::new(0);

/// Counts the panics of the server's tasks and prints them. Tasks run on the runtime's worker threads; a test's own thread is where its
/// assertions fail, and that already fails that test.
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

/// One pool of keys for the file: generation is slow.
fn keys() -> KeyPool {
    static KEYS: OnceLock<KeyPool> = OnceLock::new();
    KEYS.get_or_init(|| KeyPool::generate(2)).clone()
}

/// A server with limits raised so that 127.0.0.1 does not lock itself out, except in the property that tests the lockout.
struct Fixture {
    runtime: Runtime,
    addr: SocketAddr,
}

impl Fixture {
    fn new() -> Self {
        watch_panics();
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let mut config = test_config(None);
        config.max_connections = 1000;
        config.limits = LimitConfig {
            max_connections: 100_000,
            connects_per_minute: 10_000_000,
            failure_limit: u32::MAX,
            pair_failure_limit: u32::MAX,
            account_failure_limit: u32::MAX,
            ..LimitConfig::default()
        };
        let addr = runtime.block_on(serve_with_keys(config, backend(None), keys()));
        Self { runtime, addr }
    }
}

/// The fuzzed connection's side: sends bytes, ignoring a closed connection, and reads what comes back for a moment.
struct Link(Client);

impl Link {
    async fn send(&mut self, mut bytes: Vec<u8>, encrypted: bool) -> bool {
        if encrypted && bytes.len() > 2 {
            self.0.send.as_mut().unwrap().apply(&mut bytes[2..]);
        }
        self.0.stream.write_all(&bytes).await.is_ok()
    }

    /// Reads and throws away until the server closes the connection or `wait` is up: a connection is answered or closed, or waiting.
    async fn drain(&mut self, wait: Duration) {
        let mut buffer = [0u8; 8192];
        let _ = timeout(wait, async { while matches!(self.0.stream.read(&mut buffer).await, Ok(n) if n > 0) {} }).await;
    }
}

const SETTLE: Duration = Duration::from_millis(30);

/// A real client still logs in, picks the server and is answered.
async fn real_client(addr: SocketAddr) {
    timeout(Duration::from_secs(10), async {
        let mut c = Client::connect(addr).await;
        c.handshake().await;
        let reply = c.login("alice", "secret").await;
        assert_eq!(frame::payload(&reply)[0], 0, "a real client logs in");
        let list = c.read().await.expect("the server list follows");
        assert_eq!(frame::opcode(&list), login::OP_SERVER_LIST);
        c.send_encrypted(login::select_server_request(1, 108)).await;
        let pick = c.read().await.expect("the server selection is answered");
        assert_eq!(frame::payload(&pick)[0], select_result::SUCCESS);
    })
    .await
    .expect("the real client was answered in time");
}

fn no_panic(before: usize) -> Result<(), TestCaseError> {
    prop_assert_eq!(PANICS.load(Ordering::SeqCst), before, "a panic was counted (printed above)");
    Ok(())
}

/// The cases that failed go to `proptest-regressions/fuzz_login.txt` (the default place is found from `lib.rs`, which a test file has no
/// sibling of).
fn config_cases(default: u32) -> ProptestConfig {
    ProptestConfig {
        cases: default,
        max_shrink_iters: 200,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct("proptest-regressions/fuzz_login.txt"))),
        ..ProptestConfig::default()
    }
}

/// The cases of a deep run: `PROPTEST_CASES`, or 5,000.
fn deep_cases() -> u32 {
    std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(5000)
}

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
            #[ignore = "deep run: PROPTEST_CASES=5000 cargo test --release -p login-server --test fuzz_login -- --include-ignored"]
            fn $long($case in $strategy) {
                static FIXTURE: OnceLock<Fixture> = OnceLock::new();
                let fx = FIXTURE.get_or_init(Fixture::new);
                $body(fx, &$case)?;
            }
        }
    };
}

#[derive(Clone, Debug)]
struct Case {
    packets: Vec<Raw>,
    /// Bytes that are not even packets, after the packets.
    garbage: Vec<u8>,
}

fn case(ops: BoxedStrategy<u16>) -> impl Strategy<Value = Case> {
    (vec(raw(ops), 0..=12), vec(any::<u8>(), 0..=200)).prop_map(|(packets, garbage)| Case { packets, garbage })
}

// Before the key: the server has sent its key and waits for the answer; what comes instead is random.

fn before_the_key(fx: &Fixture, case: &Case) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    fx.runtime.block_on(async {
        let mut link = Link(Client::connect(fx.addr).await);
        for raw in &case.packets {
            if !link.send(packet(raw), false).await {
                break;
            }
        }
        link.send(case.garbage.clone(), false).await;
        link.drain(SETTLE).await;
        drop(link);
        real_client(fx.addr).await;
    });
    no_panic(before)
}

property!(before_the_key_survives, before_the_key_survives_long, 64, case in case(any_op()) => before_the_key);

// After the key: the connection is encrypted and has not logged in.

fn after_the_key(fx: &Fixture, case: &Case) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    fx.runtime.block_on(async {
        let mut client = Client::connect(fx.addr).await;
        client.handshake().await;
        let mut link = Link(client);
        for raw in &case.packets {
            if !link.send(packet(raw), true).await {
                break;
            }
        }
        link.send(case.garbage.clone(), true).await;
        link.drain(SETTLE).await;
        drop(link);
        real_client(fx.addr).await;
    });
    no_panic(before)
}

property!(after_the_key_survives, after_the_key_survives_long, 64, case in case(any_op()) => after_the_key);

// After a login: the server list is out, and the selection, a second login or noise comes.

fn after_a_login(fx: &Fixture, case: &Case) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    fx.runtime.block_on(async {
        let mut client = Client::connect(fx.addr).await;
        client.handshake().await;
        let reply = client.login("alice", "secret").await;
        assert_eq!(frame::payload(&reply)[0], 0);
        client.read().await.expect("the server list");
        let mut link = Link(client);
        for raw in &case.packets {
            if !link.send(packet(raw), true).await {
                break;
            }
        }
        link.send(case.garbage.clone(), true).await;
        link.drain(SETTLE).await;
        drop(link);
        real_client(fx.addr).await;
    });
    no_panic(before)
}

property!(after_a_login_survives, after_a_login_survives_long, 64, case in case(after_login_op()) => after_a_login);

// The lockout: a server of its own for each case, with a failure limit of 6.

const FAILURE_LIMIT: u32 = 6;

/// The account and password pairs that log in.
const VALID: [(&str, &str); 4] = [("alice", "secret"), ("gmbob", "gmpass"), ("boss", "bosspass"), ("longpw", "abcdefghijklmnop")];

fn attempts() -> impl Strategy<Value = Vec<(usize, usize)>> {
    // Mostly the right pairs and the right names with wrong passwords, so that both sides of the limit are reached.
    vec((0..USERS.len(), 0..PASSWORDS.len()), 1..=14)
}

fn lockout(fx: &Fixture, attempts: &Vec<(usize, usize)>) -> Result<(), TestCaseError> {
    let before = PANICS.load(Ordering::SeqCst);
    let outcome = fx.runtime.block_on(async {
        let mut config = test_config(None);
        config.limits = LimitConfig { failure_limit: FAILURE_LIMIT, connects_per_minute: 100_000, ..LimitConfig::default() };
        let ctx = Arc::new(Context::new(config, keys(), backend(None)));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(serve(listener, ctx));

        let mut failures = 0;
        let mut refused = false;
        let mut verdict: Result<(), String> = Ok(());
        for &(user, pass) in attempts {
            let (user, pass) = (USERS[user], PASSWORDS[pass]);
            let Some(mut client) = Client::try_connect(addr).await else {
                // No key: the address is locked out. That takes the failures, and it stays.
                if failures < FAILURE_LIMIT {
                    verdict = Err(format!("locked out after {failures} failures, the limit is {FAILURE_LIMIT}"));
                }
                refused = true;
                continue;
            };
            if refused {
                verdict = Err("a locked-out address was let in again".into());
            }
            client.handshake().await;
            let reply = client.login(user, pass).await;
            match frame::payload(&reply)[0] {
                0 if !VALID.iter().any(|(u, p)| u.eq_ignore_ascii_case(user) && *p == pass) => {
                    verdict = Err(format!("{user:?} with {pass:?} logged in"));
                }
                3 => failures += 1,
                _ => {}
            }
        }
        server.abort();
        verdict
    });
    no_panic(before)?;
    prop_assert!(outcome.is_ok(), "{}", outcome.unwrap_err());
    Ok(())
}

property!(lockout_holds, lockout_holds_long, 24, attempts in attempts() => lockout);
