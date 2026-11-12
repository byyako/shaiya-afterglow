//! `bot swarm` against in-process game servers and the test database (`TEST_DATABASE_URL`, skipped without it; never `DATABASE_URL`,
//! which is the game the maintainer plays).
//!
//! 1. Twenty bots seeded in the database, loaded into the memory backend, ramped in, fought for a dozen seconds in windows and logged
//!    out; the report's numbers must add up. The game server runs on a runtime of its own so that the CPU the bots use can be told from
//!    the server's.
//! 2. The real login path: two seeded bots log in through a login server running in this process on the same database, which sends
//!    them to a game server in this process that takes its sessions from that database.
//!
//! Whatever a test creates (accounts, characters, sessions, the game server's row) is removed afterwards, also when it fails.

mod support;

use std::collections::HashMap;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use bot::bot::Bot;
use bot::enter::connect_game;
use bot::seed::{self, Defs, Options as SeedOptions};
use bot::swarm::{self, BotPlan, Enter, Options};
use game_server::backend::CharacterStore;
use game_server::memory::MemoryBackend;
use game_server::pg::{self, PgBackend};
use game_server::server::{Context, GameData, serve, start_friends, start_guilds};
use serde_json::Value;
use sqlx::PgPool;
use support::common::*;
use support::{cpu_of_threads, memory_mb, scrub, test_pool, throwaway_prefix, user_of};
use tokio::net::TcpListener;

const SERVER: u8 = 1;
const PASSWORD: &str = "swarmpass1";

/// The real definitions with the test world (map 1 only) around them, as the seeding test builds its game server's data.
async fn real_data(pool: &PgPool) -> GameData {
    GameData {
        items: pg::load_item_defs(pool).await.unwrap(),
        skills: pg::load_skill_defs(pool).await.unwrap(),
        status: pg::load_status_table(pool).await.unwrap(),
        exp: pg::load_exp(pool).await.unwrap(),
        ..data()
    }
}

/// Runs `body` as a task and cleans up after it whether it passed or panicked (then the panic goes on).
async fn with_cleanup<F: Future<Output = ()> + 'static, C: Future<Output = ()>>(body: F, cleanup: C) {
    // `seed::seed` holds a thread-local generator across awaits, so the body runs on a local set (as the seeding tests do).
    let local = tokio::task::LocalSet::new();
    let result = local.run_until(async { tokio::task::spawn_local(body).await }).await;
    cleanup.await;
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

fn runtime(threads: usize, name: &str) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(threads).thread_name(name).enable_all().build().unwrap()
}

fn num(v: &Value) -> u64 {
    v.as_u64().unwrap_or_else(|| panic!("{v} is not a count"))
}

/// The sums of a report's counters over all of its windows: what the bots counted in all.
fn totals(report: &Value, field: &str) -> u64 {
    report["windows"].as_array().unwrap().iter().map(|w| num(&w[field])).sum()
}

#[test]
fn twenty_bots_ramp_fight_and_log_out_and_the_report_adds_up() {
    let bots_rt = runtime(2, "bots20");
    let server_rt = runtime(2, "srv20");
    let server_handle = server_rt.handle().clone();
    bots_rt.block_on(async move {
        let Some(pool) = test_pool().await else { return };
        let prefix = throwaway_prefix();
        let body = {
            let (pool, prefix) = (pool.clone(), prefix.clone());
            async move {
                // Twenty; `SWARM_TEST_BOTS` makes it more, to see how many this machine runs in one process.
                let bots: u32 = std::env::var("SWARM_TEST_BOTS").ok().and_then(|n| n.parse().ok()).unwrap_or(20).max(20);
                let defs = Defs::load(&pool).await.unwrap();
                let seeded = seed::seed(&pool, &defs, &SeedOptions::new(&prefix, bots, PASSWORD)).await.unwrap();
                assert_eq!(seeded.created(), bots as usize);

                // The battle is at (400, 400) of the test world's map 1; the bots are in the database on map 18, where the rows say they
                // stand, and in the memory backend on map 1 in two rows facing each other, 40 units apart, with little HP so that the
                // fights end in a blow or two.
                let mut options = Options::new("unused:30800", bots, PASSWORD);
                options.prefix = prefix.clone();
                options.at = Some((18, 400.0, 400.0));
                (options.ramp, options.radius, options.steps_per_sec) = (20.0, 30.0, 4.0);
                (options.skill_every, options.rebirth_after) = (Duration::from_secs(3), Duration::from_millis(300));
                (options.window, options.logout_wait) = (Duration::from_secs(2), Duration::from_secs(5));
                options.duration = Duration::from_secs(14 + u64::from(bots) / 20);
                let setup = swarm::prepare(&pool, &defs, &options).await.unwrap();
                assert_eq!((setup.plans.len(), setup.map, setup.point), (bots as usize, 18, [400.0, 400.0]));
                // The plans carry what the database holds: the reach `bot seed` reported for each bot (a bow's for the archers, a sword's
                // for the rest) and skills the bot can use.
                let mut far = 0;
                for (plan, line) in setup.plans.iter().zip(&seeded.bots) {
                    assert_eq!(plan.config.weapon_range, line.range, "{}", plan.username);
                    far += usize::from(line.range > 10);
                    assert!(!plan.config.skills.is_empty(), "{}: no skills to use", plan.username);
                    assert_eq!((plan.username.as_str(), plan.character_id), (seed::bot_names(&prefix, plan.number).0.as_str(), line.character_id));
                    assert!(!plan.config.idle);
                }
                assert!(far >= 2, "the bow-users (archers, perhaps rangers) of both factions have a long reach: {far}");
                // The default battle point is the midpoint of the rebirth boxes of map 18: about (636, 394).
                let middle = swarm::default_point(&pool, 18).await.unwrap();
                assert!((middle[0] - 636.0).abs() < 5.0 && (middle[1] - 394.0).abs() < 5.0, "{middle:?}");

                let backend = Arc::new(MemoryBackend::default());
                let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
                let mut who: HashMap<u32, (u32, [u8; 16])> = HashMap::new();
                for (i, plan) in setup.plans.iter().enumerate() {
                    let user = user_of(&pool, &plan.username).await;
                    let faction = store.faction(user, SERVER).await.unwrap().unwrap();
                    let mut play = store.load(user, SERVER, plan.character_id).await.unwrap().unwrap();
                    let row = (i / 2) as f32;
                    play.map = 1;
                    play.position = [if faction.country == 0 { 380.0 } else { 420.0 }, 78.56, 380.0 + 3.0 * row];
                    play.health[0] = 60;
                    let identity = [i as u8 + 1; 16];
                    backend.add_session(user, session(identity, Some(SERVER_ID), LOCAL));
                    backend.add_character(user, SERVER, 0, faction, play);
                    who.insert(plan.number, (user, identity));
                }
                let data = real_data(&pool).await;
                let config = bot_config();
                let started = server_handle.spawn(start_with_data(backend.clone(), config, data)).await.unwrap();
                let addr = started.0;
                // The stats line on the server's runtime, as `main` starts it for `stats_every`; the lines show with --nocapture.
                let _ = tracing_subscriber::fmt().with_env_filter("stats=info").with_test_writer().try_init();
                server_handle.spawn(game_server::server::stats_line(started.1.clone()));

                let users: Vec<u32> = who.values().map(|(user, _)| *user).collect();
                let who = Arc::new(who);
                let enter: Enter = Arc::new(move |plan: BotPlan| {
                    let (user, identity) = who[&plan.number];
                    Box::pin(async move {
                        let (link, screen) = connect_game(&addr.to_string(), user, &identity, &LOGIN_KEYS).await?;
                        Bot::enter_from(link, &screen).await
                    })
                });

                let (rss_before, _) = memory_mb();
                let (bots_cpu, server_cpu, wall) = (cpu_of_threads("bots20"), cpu_of_threads("srv20"), Instant::now());
                let args = options.args_json(setup.map, setup.point);
                let report = swarm::run(setup.plans, enter, &options, args, &OnceLock::new(), std::future::pending()).await;
                let (bots_cpu, server_cpu, wall) =
                    (cpu_of_threads("bots20") - bots_cpu, cpu_of_threads("srv20") - server_cpu, wall.elapsed().as_secs_f64());
                let (rss_after, peak) = memory_mb();
                println!("\n{report}\n");
                println!(
                    "measured: {bots} bots over {wall:.1} s: the bots' threads used {:.2} s of CPU ({:.0} % of one core), the server's {:.2} s ({:.0} %); \
                     resident memory {rss_before:.0} MB before, {rss_after:.0} MB after, peak {peak:.0} MB",
                    bots_cpu.as_secs_f64(),
                    100.0 * bots_cpu.as_secs_f64() / wall,
                    server_cpu.as_secs_f64(),
                    100.0 * server_cpu.as_secs_f64() / wall
                );

                // The server's own figures for the run: the same ones the line carried, read once more (over what came after the last line).
                let ctx = started.1.clone();
                let figures = server_handle.spawn(async move { game_server::server::stats_figures(&ctx) }).await.unwrap();
                println!("stats at the end: {figures}");
                assert_eq!(figures.workers, 2, "the server's runtime has two workers");
                assert!(figures.in_pps > 0.0 || figures.secs < 0.5, "the bots' packets were counted: {figures}");
                assert_eq!(figures.outbox_closed, 0, "no connection was closed for its outbox");
                assert!(figures.rss_mb > 0 || !std::path::Path::new("/proc/self/statm").exists());

                // The report as the file keeps it.
                let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{prefix}.json"));
                std::fs::write(&path, serde_json::to_string_pretty(&report.json()).unwrap()).unwrap();
                let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                std::fs::remove_file(&path).unwrap();
                assert_eq!(v, report.json());

                // The run: twenty bots, 2 s windows, 14 s.
                assert_eq!((num(&v["schema"]), num(&v["run"]["bots"])), (1, u64::from(bots)));
                assert_eq!(v["run"]["args"]["at"]["map"], 18);
                assert!((v["run"]["duration_s"].as_f64().unwrap() - options.duration.as_secs_f64()).abs() < 0.5);
                // The ramp: every bot got in, all within a few seconds of the first (twenty a second), none failed.
                assert_eq!((num(&v["ramp"]["logins_ok"]), num(&v["ramp"]["logins_failed"])), (u64::from(bots), 0));
                let last_login = v["ramp"]["last_login_s"].as_f64().unwrap();
                let ramp_time = f64::from(bots) / 20.0;
                assert!((ramp_time - 0.2..ramp_time + 4.0).contains(&last_login), "the ramp of 20 at 20 a second ended at {last_login} s");
                assert_eq!(v["notes"].as_array().unwrap().len(), 0, "{:?}", v["notes"]);
                // Windows: the ramp's apart from the steady state, which starts after the last login.
                let windows = v["windows"].as_array().unwrap();
                assert!(windows.len() >= 5, "{} windows", windows.len());
                assert!(windows[0]["ramp"].as_bool().unwrap());
                for w in windows {
                    assert_eq!(w["ramp"].as_bool().unwrap(), w["start_s"].as_f64().unwrap() < last_login, "window {}", w["index"]);
                }
                let steady = &v["summary"];
                assert!(steady["steady"].as_bool().unwrap(), "no steady window");
                assert!(steady["windows"].as_array().unwrap().len() >= 3);
                assert!(steady["from_s"].as_f64().unwrap() >= last_login);
                // Everyone is in the world in every steady window, and nobody left.
                assert_eq!(steady["bots_in_world"], serde_json::json!({"min": f64::from(bots), "avg": f64::from(bots), "max": f64::from(bots)}));
                assert_eq!(steady["disconnects"], serde_json::json!({"closed": 0, "timeout": 0, "error": 0}));
                // No refusals, no protocol errors, anywhere in the run.
                assert_eq!((totals(&v, "refusals"), totals(&v, "protocol_errors")), (0, 0));
                assert_eq!((num(&steady["refusals"]), num(&steady["protocol_errors"])), (0, 0));
                // Kills equal deaths across the swarm (every death is a bot's: the test world's monsters are far away), and fights happened.
                let (kills, deaths) = (totals(&v, "kills"), totals(&v, "deaths"));
                assert_eq!(kills, deaths, "kills {kills} deaths {deaths}");
                assert!(kills >= 2, "{kills} kills in 14 s");
                assert!(totals(&v, "rebirths") <= deaths);
                // The summary is the steady windows added up: the same figures from the window list.
                let in_steady = |field: &str| -> u64 {
                    windows.iter().filter(|w| steady["windows"].as_array().unwrap().contains(&w["index"])).map(|w| num(&w[field])).sum()
                };
                for field in ["steps", "kills", "deaths", "rebirths"] {
                    assert_eq!(num(&steady[field]), in_steady(field), "{field}");
                }
                // Every packet counted by opcode adds up to the totals, and the fight's own were seen.
                let w = &windows[windows.len() / 2];
                let by_opcode: u64 = w["opcodes"].as_object().unwrap().values().map(|o| num(&o["rx_packets"])).sum();
                assert_eq!(by_opcode, num(&w["packets"]["rx"]["packets"]));
                assert!(steady["opcodes"]["0x0501"]["tx_packets"].as_u64().unwrap() > 0 && steady["opcodes"]["0x0502"].is_object());
                assert!(num(&steady["attack_rtt_ms"]["n"]) > 0 && steady["attack_rtt_ms"]["p50"].is_number());
                // The acknowledgement (0x0502 to the server's 0x0212): measured, and small on loopback, with no attack interval in it.
                assert!(num(&steady["attack_ack_ms"]["n"]) > 0, "the attack acknowledgement was measured");
                assert!(steady["attack_ack_ms"]["p95"].as_f64().unwrap() < 250.0, "{}", steady["attack_ack_ms"]);
                assert!(num(&steady["step_echo_ms"]["n"]) > 0, "the step echo was measured");
                assert!(num(&steady["packets"]["rx"]["packets"]) > num(&steady["packets"]["tx"]["packets"]));
                // Logged out cleanly, all twenty, and the server has let go of every one.
                assert_eq!((num(&v["logout"]["bots"]), num(&v["logout"]["clean"])), (u64::from(bots), u64::from(bots)));
                for user in users {
                    wait_gone(&backend, user, Duration::from_secs(5)).await;
                }
            }
        };
        with_cleanup(body, scrub(&pool, &prefix)).await;
    });
    server_rt.shutdown_background();
}

/// A config for a game server with many bots: nothing limits the connections of one address.
fn bot_config() -> game_server::config::Config {
    game_server::config::Config {
        max_per_ip: 1000,
        connects_per_minute: 100_000,
        max_connections: 1000,
        stats_every: Duration::from_secs(4),
        ..config(true)
    }
}

/// A free address in 127/8 that no other test uses, so that the game server's row in the login server's table is its own.
fn loopback_address() -> Ipv4Addr {
    Ipv4Addr::new(127, rand::random_range(16..=250), rand::random_range(0..=255), rand::random_range(1..=254))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_bots_log_in_through_a_login_server_to_a_game_server_on_the_same_database() {
    let Some(pool) = test_pool().await else { return };
    let prefix = throwaway_prefix();
    // The game server's id in the login server's list: its own, out of the range of the real ones.
    let server_id: u8 = rand::random_range(100..=250);
    let address = loopback_address();
    let body = {
        let (pool, prefix) = (pool.clone(), prefix.clone());
        async move {
            let defs = Defs::load(&pool).await.unwrap();
            let mut seed_options = SeedOptions::new(&prefix, 2, PASSWORD);
            seed_options.server_id = server_id;
            let seeded = seed::seed(&pool, &defs, &seed_options).await.unwrap();
            // The test world has map 1 only: the two bots stand there at the human start, 14 units apart, the Fury one with 40 HP so that
            // the fight is over in a blow or two.
            for (i, line) in seeded.bots.iter().enumerate() {
                sqlx::query("UPDATE characters SET map = 1, pos_x = $2, pos_y = 78.56, pos_z = 1762.12, hp = $3 WHERE id = $1")
                    .bind(line.character_id as i32)
                    .bind(580.0f32 + 14.0 * i as f32)
                    .bind(if i == 1 { 40i16 } else { line.health[0] as i16 })
                    .execute(&pool)
                    .await
                    .unwrap();
            }

            // The login server, over the same database; its list has the game server at its own loopback address.
            let login_config = login_server::config::Config {
                bind: "127.0.0.1:0".parse().unwrap(),
                database_url: String::new(),
                heartbeat_timeout: Duration::from_secs(60),
                key_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(60),
                check_versions: false,
                limits: login_server::limits::LimitConfig::default(),
                key_pool_size: 2,
                max_connections: 50,
            };
            let keys = tokio::task::spawn_blocking(|| login_server::keys::KeyPool::generate(2)).await.unwrap();
            let login_store = login_server::pg::PgBackend::new(pool.clone(), login_config.heartbeat_timeout);
            let login_ctx = Arc::new(login_server::server::Context::new(login_config, keys, login_store));
            let login_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let login_addr = login_listener.local_addr().unwrap();
            let login_task = tokio::spawn(login_server::server::serve(login_listener, login_ctx));
            login_server::pg::add_server(&pool, server_id, "Swarm test", address, 100, None).await.unwrap();
            login_server::pg::heartbeat(&pool, server_id, 0).await.unwrap();

            // The game server, taking its sessions and characters from the same database, listening at the address the list gives.
            let config = game_server::config::Config { server_id, kill_count: false, ..config(true) };
            let mut ctx = Context::new(config, PgBackend::new(pool.clone(), Duration::from_secs(7200)), real_data(&pool).await);
            ctx.set_game_log(game_server::game_log::GameLog::memory(true).0);
            let ctx = Arc::new(ctx);
            start_guilds(&ctx).await.unwrap();
            start_friends(&ctx);
            let game_listener = TcpListener::bind(SocketAddr::from((address, 0))).await.unwrap();
            let game_port = game_listener.local_addr().unwrap().port();
            let game_task = tokio::spawn(serve(game_listener, ctx.clone()));

            let mut options = Options::new(&login_addr.to_string(), 2, PASSWORD);
            options.prefix = prefix.clone();
            options.server = server_id;
            options.game_port = Some(game_port);
            options.at = Some((1, 587.0, 1762.0));
            (options.ramp, options.radius, options.steps_per_sec) = (10.0, 10.0, 4.0);
            (options.skill_every, options.rebirth_after) = (Duration::from_secs(3), Duration::from_millis(300));
            (options.window, options.duration, options.logout_wait) =
                (Duration::from_secs(2), Duration::from_secs(8), Duration::from_secs(3));
            let setup = swarm::prepare(&pool, &defs, &options).await.unwrap();
            assert_eq!((setup.map, setup.point), (1, [587.0, 1762.0]));
            let note = Arc::new(OnceLock::new());
            let enter = swarm::login_entry(&options, note.clone());
            let args = options.args_json(setup.map, setup.point);
            let report = swarm::run(setup.plans, enter, &options, args, &note, std::future::pending()).await;
            println!("\n{report}\n");
            let v = report.json();

            // Both logged in through the login server, to the game server the list named.
            assert_eq!((num(&v["ramp"]["logins_ok"]), num(&v["ramp"]["logins_failed"])), (2, 0), "{:?}", v["notes"]);
            let server = &v["run"]["server"];
            assert_eq!(server["game_server_id"], server_id);
            assert_eq!(server["game_address"], format!("{address}:{game_port}"));
            assert!(server["servers"].as_array().unwrap().iter().any(|s| s["id"] == server_id && s["name"] == "Swarm test"));
            // They were in the world, fought without a refusal or a protocol error, and logged out cleanly.
            assert_eq!(v["summary"]["bots_in_world"]["max"], 2.0);
            assert_eq!((totals(&v, "refusals"), totals(&v, "protocol_errors")), (0, 0));
            assert_eq!(totals(&v, "kills"), totals(&v, "deaths"));
            assert!(totals(&v, "kills") >= 1, "the Light bot killed the Fury one");
            assert_eq!((num(&v["logout"]["bots"]), num(&v["logout"]["clean"])), (2, 2));
            // The game server let both go: no session of the two is marked as in the game any more.
            let mut in_game = 1;
            for _ in 0..100 {
                in_game = sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM sessions WHERE user_id IN (SELECT id FROM accounts WHERE username LIKE $1) AND game_connected_at IS NOT NULL",
                )
                .bind(format!("{prefix}%"))
                .fetch_one(&pool)
                .await
                .unwrap();
                if in_game == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert_eq!(in_game, 0, "sessions still in the game");
            login_task.abort();
            game_task.abort();
        }
    };
    with_cleanup(body, async {
        let _ = login_server::pg::remove_server(&pool, server_id).await;
        scrub(&pool, &prefix).await;
    })
    .await;
}
