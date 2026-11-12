//! What the bots' tests against the in-process game server share: the game server's own test helpers (`tests/common`) and the two
//! helpers that put a bot in the world and play it.
#![allow(dead_code)]

#[path = "../../../game-server/tests/common/mod.rs"]
pub mod common;

use std::time::Duration;

use bot::bot::Bot;
use bot::enter::connect_game;
use bot::scenario::pvp_battle::{Battle, run};
use common::LOGIN_KEYS;

/// A bot of the account `user`, entered with its first character.
pub async fn bot_in_world(addr: std::net::SocketAddr, user: u32, identity: [u8; 16]) -> Bot {
    let (link, screen) = connect_game(&addr.to_string(), user, &identity, &LOGIN_KEYS).await.unwrap();
    Bot::enter_from(link, &screen).await.unwrap()
}

/// Plays `bot` in slices until `done` says so.
pub async fn drive(bot: &mut Bot, battle: &mut Battle, done: impl Fn(&Bot, &Battle) -> bool) {
    while !done(bot, battle) {
        run(bot, battle, tokio::time::sleep(Duration::from_millis(40))).await.expect("the connection held");
    }
}

// What the swarm's tests share: the test database, throwaway prefixes and a measure of the CPU and memory the bots use.

use std::time::Duration as Dur;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// The test database, or `None` (and a line saying so) without `TEST_DATABASE_URL`. `DATABASE_URL` is never read here.
pub async fn test_pool() -> Option<PgPool> {
    dotenvy::dotenv().ok();
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set: skipping the swarm tests");
        return None;
    };
    Some(PgPoolOptions::new().max_connections(4).connect(&url).await.unwrap())
}

/// A prefix nothing else uses: eight characters, so an account is twelve.
pub fn throwaway_prefix() -> String {
    format!("z{:07x}", rand::random::<u32>() & 0x0fff_ffff)
}

/// Removes everything of a throwaway prefix, whatever state a test left it in.
pub async fn scrub(pool: &PgPool, prefix: &str) {
    let like = format!("{prefix}%");
    for sql in [
        "DELETE FROM sessions WHERE user_id IN (SELECT id FROM accounts WHERE username LIKE $1)",
        "DELETE FROM characters WHERE user_id IN (SELECT id FROM accounts WHERE username LIKE $1)",
        "UPDATE login_log SET account_id = NULL WHERE account_id IN (SELECT id FROM accounts WHERE username LIKE $1)",
        "DELETE FROM login_log WHERE username LIKE $1",
        "DELETE FROM accounts WHERE username LIKE $1",
    ] {
        if let Err(error) = sqlx::query(sql).bind(&like).execute(pool).await {
            eprintln!("cleaning {prefix}: {error}");
        }
    }
}

pub async fn user_of(pool: &PgPool, username: &str) -> u32 {
    sqlx::query_scalar::<_, i32>("SELECT id FROM accounts WHERE username = $1").bind(username).fetch_one(pool).await.unwrap() as u32
}

/// CPU time (user and system) used so far by this process's threads whose name starts with `prefix` (a runtime built with
/// `thread_name(prefix)`), from `/proc`, in clock ticks of 10 ms.
pub fn cpu_of_threads(prefix: &str) -> Dur {
    let mut ticks = 0u64;
    for task in std::fs::read_dir("/proc/self/task").into_iter().flatten().flatten() {
        let Ok(stat) = std::fs::read_to_string(task.path().join("stat")) else { continue };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else { continue };
        if !stat[open + 1..close].starts_with(prefix) {
            continue;
        }
        let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        ticks +=
            rest.get(11).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0) + rest.get(12).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
    }
    Dur::from_millis(ticks * 10)
}

/// The process's resident memory now and at its peak, in MB (`VmRSS` and `VmHWM`).
pub fn memory_mb() -> (f64, f64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |key: &str| {
        status
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    (kb("VmRSS:") / 1024.0, kb("VmHWM:") / 1024.0)
}
