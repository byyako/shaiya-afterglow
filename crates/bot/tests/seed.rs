//! `bot seed` and `bot unseed` against the test database (`TEST_DATABASE_URL`, skipped without it; never `DATABASE_URL`, which is the game
//! the maintainer plays): two characters, one per faction, are seeded under a throwaway prefix, loaded back through the game server's
//! PostgreSQL store, checked, put into the memory backend and played against each other in the in-process server with the pvp-battle
//! loop; then unseeded. A second test makes all twelve faction and class pairs (and a copy of them from templates), a third checks
//! the guards. Whatever a failing test leaves is removed.

mod support;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use bot::scenario::pvp_battle::{Battle, Config, State};
use bot::seed::{self, Defs, Options, Refused};
use bot::world::distance;
use game_server::backend::{CharacterStore, Faction, PlayCharacter};
use game_server::combat::class_stat;
use game_server::creation::starting_stats;
use game_server::inventory::{Inventory, Wearer};
use game_server::memory::MemoryBackend;
use game_server::pg::{self, PgBackend};
use game_server::server::GameData;
use game_server::skills::{self, ATTACK};
use game_server::stats;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use support::common::*;
use support::{bot_in_world, drive};

const SERVER: u8 = 1;
const PASSWORD: &str = "botpass1";

/// The test database, or `None` (and a line saying so) without `TEST_DATABASE_URL`. `DATABASE_URL` is never read here.
async fn test_pool() -> Option<PgPool> {
    dotenvy::dotenv().ok();
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set: skipping the bot seeding tests");
        return None;
    };
    Some(PgPoolOptions::new().max_connections(4).connect(&url).await.unwrap())
}

/// A prefix nothing else uses: eight characters, so an account is twelve.
fn throwaway_prefix() -> String {
    format!("z{:07x}", rand::random::<u32>() & 0x0fff_ffff)
}

/// Removes everything of a throwaway prefix, bots or not, whatever state a test left it in.
async fn scrub(pool: &PgPool, prefix: &str) {
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

/// Runs `body` as a task and cleans the prefixes up after it, whether it passed or panicked (then the panic goes on).
async fn with_cleanup<Fut: Future<Output = ()> + 'static>(pool: &PgPool, prefixes: &[&str], body: Fut) {
    let local = tokio::task::LocalSet::new();
    let result = local.run_until(async { tokio::task::spawn_local(body).await }).await;
    for prefix in prefixes {
        scrub(pool, prefix).await;
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

async fn user_of(pool: &PgPool, username: &str) -> u32 {
    sqlx::query_scalar::<_, i32>("SELECT id FROM accounts WHERE username = $1").bind(username).fetch_one(pool).await.unwrap() as u32
}

async fn count(pool: &PgPool, sql: &'static str, prefix: &str) -> i64 {
    sqlx::query_scalar(sql).bind(format!("{prefix}%")).fetch_one(pool).await.unwrap()
}

/// The accounts that are not any test's throwaway bots (the tests run side by side).
async fn total_accounts(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounts WHERE username !~ '^z[0-9a-f]{7}[0-9]{4}$'").fetch_one(pool).await.unwrap()
}

/// What a seeded bot looks like to the game server: loaded through its PostgreSQL store.
struct Loaded {
    user: u32,
    play: PlayCharacter,
    faction: Faction,
}

async fn load(pool: &PgPool, store: &PgBackend, line: &seed::BotLine) -> Loaded {
    let user = user_of(pool, &line.username).await;
    let faction = store.faction(user, SERVER).await.unwrap().expect("the account has its faction");
    let play = store.load(user, SERVER, line.character_id).await.unwrap().expect("the game server loads the character");
    Loaded { user, play, faction }
}

/// Everything the generator promises about one loaded bot (section 8.2).
async fn check_loaded(pool: &PgPool, defs: &Defs, line: &seed::BotLine, l: &Loaded) {
    let (c, p) = (&l.faction, &l.play);
    assert_eq!(c.country, line.country, "{}: the account's faction", line.username);
    assert_eq!((c.max_grow, p.family / 2), (3, line.country), "{}: the race is in the faction", line.username);
    assert_eq!(seed::race_for(line.country, p.job), Some(p.family), "{}: the first race of the faction with the class", line.username);
    assert_eq!((p.name.as_str(), p.level, p.grow, p.stat_point, p.skill_point, p.money), (line.name.as_str(), 60, 1, 0, 0, 0));
    assert_eq!(p.exp, 284_662_500, "the experience of level 60's start in the normal mode");

    // Stats: the start, one more of the class's own stat a level and five points a level on its main stat.
    let start = starting_stats(p.job, p.family);
    let added = u32::from(class_stat(p.job).is_some()) * 59 + 59 * 5;
    let (now, before): (u32, u32) = (p.stats.iter().map(|&v| u32::from(v)).sum(), start.iter().map(|&v| u32::from(v)).sum());
    assert_eq!(now - before, added, "{}: stats {:?} from {start:?}", line.username, p.stats);

    // Gear: in the slots the loader puts it in, each accepted by `can_equip`, and a weapon in slot 5 (bag 0).
    let wearer = Wearer { level: p.level, family: p.family, job: p.job, grow: p.grow, sex: p.sex };
    let inventory = Inventory::new(&p.items);
    let worn: Vec<_> = p.items.iter().filter(|i| i.bag == 0).collect();
    assert!(worn.iter().any(|i| i.slot == 5), "{}: no weapon in bag 0 slot 5", line.username);
    for item in &worn {
        let def =
            defs.items.get(item.item_type, item.type_id).unwrap_or_else(|| panic!("no definition for {}/{}", item.item_type, item.type_id));
        assert!(
            inventory.can_equip(&defs.items, def, item.slot, &wearer),
            "{}: slot {} cannot hold item {}",
            line.username,
            item.slot,
            def.item_id
        );
        assert_eq!((item.quality, item.count, item.gems), (def.quality, 1, [0; 6]));
    }
    assert_eq!(worn.len(), line.gear);
    let bag1: Vec<_> = p.items.iter().filter(|i| i.bag == 1).collect();
    assert!(
        bag1.iter().all(|i| i.item_type == 25) && bag1.iter().map(|i| u32::from(i.count)).sum::<u32>() == 40,
        "{}: potions {bag1:?}",
        line.username
    );

    // HP, MP and SP are the maxima the server computes for this character with this gear and these skills.
    let base = stats::Base { job: p.job, level: p.level, grow: p.grow, family: p.family, stats: p.stats, grade: Default::default() };
    let totals = stats::compute(&defs.status, &base, &p.items, &defs.items, &skills::passives(&p.skills, &defs.skills));
    assert_eq!(p.health, totals.max_health(), "{}: stored health is the computed maxima", line.username);

    // Skills: numbered 0, 1, 2, ... in skill id order, each usable by the class at the highest level 60 allows.
    assert_eq!(p.skills.len(), line.skills);
    assert_eq!(defs.attack_range(&p.items, &p.skills), line.range);
    assert!(!p.skills.is_empty());
    for (i, s) in p.skills.iter().enumerate() {
        assert_eq!(usize::from(s.number), i, "{}: numbers run from 0", line.username);
        let def = defs.skills.get(s.id, s.level).expect("a defined skill");
        assert!(skills::usable(def, p.level, p.job, p.grow), "{}: skill {}/{}", line.username, s.id, s.level);
        // Nothing higher can be learnt: the next level, by the server's own learning rules, is refused.
        let learner = skills::Learner { level: p.level, skill_points: u16::MAX, family: p.family, job: p.job, grow: p.grow };
        assert!(
            skills::learn(&learner, &p.skills, s.id, s.level + 1, &defs.skills).is_err(),
            "{}: skill {} is not at its highest level",
            line.username,
            s.id
        );
    }
    assert!(p.skills.windows(2).all(|w| w[0].id < w[1].id));
    // Quick slots: attack skills, by the skill's number, on bar 0, the skill's id as the row says it.
    assert!(!p.quick_slots.is_empty() && p.quick_slots.len() <= 10);
    for (i, q) in p.quick_slots.iter().enumerate() {
        let skill = p.skills.iter().find(|s| s.id == q.number).expect("a quick slot names a learnt skill");
        assert_eq!((q.bar, usize::from(q.slot), q.bag), (0, i, 100));
        assert_eq!(defs.skills.get(skill.id, skill.level).unwrap().type_effect, ATTACK);
    }

    // Where: map 18, inside its faction's rebirth box (country 1 of the map's binds is Light, 2 Fury).
    assert_eq!(p.map, 18);
    let boxes: Vec<f32> = sqlx::query_scalar("SELECT boxes FROM world.map_binds WHERE map = 18 AND country = $1")
        .bind(i32::from(line.country) + 1)
        .fetch_one(pool)
        .await
        .unwrap();
    let [x, y, z] = p.position;
    assert!(
        (boxes[0]..=boxes[3]).contains(&x) && (boxes[2]..=boxes[5]).contains(&z) && (y - boxes[1] - 0.9).abs() < 0.01,
        "{:?} in {boxes:?}",
        p.position
    );
}

#[tokio::test]
async fn two_seeded_bots_load_fight_and_are_unseeded() {
    let Some(pool) = test_pool().await else { return };
    let prefix = throwaway_prefix();
    let body = {
        let (pool, prefix) = (pool.clone(), prefix.clone());
        async move {
            let defs = Defs::load(&pool).await.unwrap();
            let seeded = seed::seed(&pool, &defs, &Options::new(&prefix, 2, PASSWORD)).await.unwrap();
            assert_eq!((seeded.created(), seeded.reset()), (2, 0));
            let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
            let loaded = [load(&pool, &store, &seeded.bots[0]).await, load(&pool, &store, &seeded.bots[1]).await];
            for (line, l) in seeded.bots.iter().zip(&loaded) {
                check_loaded(&pool, &defs, line, l).await;
            }
            // One of each faction, fighters.
            assert_eq!([loaded[0].faction.country, loaded[1].faction.country], [0, 1]);
            assert_eq!([loaded[0].play.job, loaded[1].play.job], [0, 0]);

            // The two loaded characters in the memory backend, in the in-process server with the real definitions. The test world has only
            // map 1 and the human start, so they stand there, 14 units apart, as in the part 1 test; the Fury one with 40 HP, so that the
            // kill comes within a blow or two.
            let backend = Arc::new(MemoryBackend::default());
            let ranges = [seeded.bots[0].range, seeded.bots[1].range];
            let mut ids = Vec::new();
            for (i, l) in loaded.into_iter().enumerate() {
                let mut play = l.play;
                play.map = 1;
                play.position = [580.0 + 14.0 * i as f32, 78.56, 1762.12];
                if i == 1 {
                    play.health[0] = 40;
                }
                ids.push(play.id);
                let identity = [i as u8 + 1; 16];
                backend.add_session(l.user, session(identity, Some(SERVER_ID), LOCAL));
                backend.add_character(l.user, SERVER, 0, l.faction, play);
            }
            let data = GameData {
                items: pg::load_item_defs(&pool).await.unwrap(),
                skills: pg::load_skill_defs(&pool).await.unwrap(),
                status: pg::load_status_table(&pool).await.unwrap(),
                exp: pg::load_exp(&pool).await.unwrap(),
                ..data()
            };
            let (addr, _ctx) = start_with_data(backend.clone(), config(true), data).await;
            let users = [user_of(&pool, &seeded.bots[0].username).await, user_of(&pool, &seeded.bots[1].username).await];
            let mut light = bot_in_world(addr, users[0], [1; 16]).await;
            let mut fury = bot_in_world(addr, users[1], [2; 16]).await;
            assert_eq!((light.world.id, fury.world.id), (ids[0], ids[1]));

            let config = |point, weapon_range| Config {
                point,
                radius: 0.0,
                steps_per_sec: 4.0,
                rebirth_after: Duration::from_millis(300),
                skill_every: Duration::from_secs(3600),
                weapon_range,
                ..Config::default()
            };
            let mut light_battle = Battle::new(config([630.0, 1762.0], ranges[0]));
            let mut fury_battle = Battle::new(config([594.0, 1762.0], ranges[1]));
            let fight = async {
                tokio::join!(
                    drive(&mut light, &mut light_battle, |bot, _| bot.metrics.kills >= 1),
                    drive(&mut fury, &mut fury_battle, |bot, _| bot.metrics.rebirths >= 1),
                );
            };
            tokio::time::timeout(Duration::from_secs(60), fight).await.expect("a kill and a rebirth within 60 s");

            // They saw each other as enemies (each from the other's `0x0303` answer), were never put back, one killed the other and the
            // dead one is alive again.
            assert!(light_battle.milestones.enemy_seen && fury_battle.milestones.enemy_seen);
            assert_eq!(light.world.players[&ids[1]].country, Some(1));
            assert_eq!(fury.world.players[&ids[0]].country, Some(0));
            assert_eq!((light.metrics.refusals, fury.metrics.refusals), (0, 0));
            assert_eq!((light.metrics.protocol_errors, fury.metrics.protocol_errors), (0, 0));
            assert_eq!((light.metrics.kills, light.metrics.deaths), (1, 0));
            assert_eq!((fury.metrics.kills, fury.metrics.deaths), (0, 1));
            assert!(!fury_battle.milestones.unarmed && !light_battle.milestones.unarmed, "both bots have a weapon");
            assert!(fury.world.alive && matches!(fury_battle.state, State::Walk));

            light.leave(Duration::from_secs(2)).await;
            fury.leave(Duration::from_secs(2)).await;
            wait_gone(&backend, users[0], Duration::from_secs(3)).await;
            wait_gone(&backend, users[1], Duration::from_secs(3)).await;

            // Unseeding removes everything, and only the prefix's.
            let others = total_accounts(&pool).await;
            let ids: Vec<u32> = seeded.bots.iter().map(|b| b.character_id).collect();
            assert_eq!(seed::unseed(&pool, &prefix).await.unwrap(), 2);
            for sql in ["SELECT count(*) FROM accounts WHERE username LIKE $1", "SELECT count(*) FROM characters WHERE name ILIKE $1"] {
                assert_eq!(count(&pool, sql, &prefix).await, 0, "{sql}");
            }
            for table in ["character_items", "character_skills", "character_quick_slots"] {
                let left: i64 = sqlx::query_scalar(match table {
                    "character_items" => "SELECT count(*) FROM character_items WHERE character_id = ANY($1)",
                    "character_skills" => "SELECT count(*) FROM character_skills WHERE character_id = ANY($1)",
                    _ => "SELECT count(*) FROM character_quick_slots WHERE character_id = ANY($1)",
                })
                .bind(ids.iter().map(|&id| id as i32).collect::<Vec<_>>())
                .fetch_one(&pool)
                .await
                .unwrap();
                assert_eq!(left, 0, "{table} rows of the unseeded characters");
            }
            assert_eq!(total_accounts(&pool).await, others);
            assert_eq!(seed::unseed(&pool, &prefix).await.unwrap(), 0, "nothing left to unseed");
        }
    };
    with_cleanup(&pool, &[&prefix], body).await;
}

#[tokio::test]
async fn all_twelve_pairs_are_made_and_a_template_copies_them() {
    let Some(pool) = test_pool().await else { return };
    let (prefix, copies) = (throwaway_prefix(), throwaway_prefix());
    let body = {
        let (pool, prefix, copies) = (pool.clone(), prefix.clone(), copies.clone());
        async move {
            let defs = Defs::load(&pool).await.unwrap();
            let seeded = seed::seed(&pool, &defs, &Options::new(&prefix, 12, PASSWORD)).await.unwrap();
            let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
            println!("faction class race  weapon type range gear skills  hp/mp/sp");
            let mut pairs = std::collections::BTreeSet::new();
            let mut loaded = Vec::new();
            for line in &seeded.bots {
                let l = load(&pool, &store, line).await;
                check_loaded(&pool, &defs, line, &l).await;
                println!(
                    "{:>7} {:>5} {:>4} {:>7} {:>4} {:>5} {:>4} {:>6}  {:?}",
                    line.country,
                    line.job,
                    line.family,
                    line.weapon.unwrap(),
                    line.weapon.unwrap() / 1000,
                    line.range,
                    line.gear,
                    line.skills,
                    line.health
                );
                pairs.insert((line.country, line.job));
                loaded.push(l);
            }
            assert_eq!(pairs.len(), 12);

            // `--template`: one character of each faction (the first of each), the others of the faction copy the one of their faction and
            // their class where there is one.
            let options =
                Options { templates: vec![seeded.bots[0].character_id, seeded.bots[1].character_id], ..Options::new(&copies, 4, PASSWORD) };
            let made = seed::seed(&pool, &defs, &options).await.unwrap();
            assert_eq!(made.created(), 4);
            for (copy, bot) in made.bots.iter().zip(&seeded.bots) {
                let l = load(&pool, &store, copy).await;
                let original = &loaded[(bot.number - 1) as usize].play;
                // Bots 3 and 4 are defenders, which no template has: they take the template of their faction (fighters).
                let template = &loaded[((bot.number - 1) % 2) as usize].play;
                let want = if bot.number <= 2 { original } else { template };
                assert_eq!(
                    (l.play.family, l.play.job, l.play.level, l.play.stats, l.play.health),
                    (want.family, want.job, 60, want.stats, want.health)
                );
                assert_eq!((l.play.skills.len(), l.play.quick_slots.len()), (want.skills.len(), want.quick_slots.len()));
                let key = |p: &PlayCharacter| p.items.iter().map(|i| (i.bag, i.slot, i.item_type, i.type_id, i.count)).collect::<Vec<_>>();
                assert_eq!(key(&l.play), key(want));
                assert_eq!(l.play.name, copy.name);
                assert_ne!(l.play.id, want.id);
            }
            // A template that does not exist, or whose faction no bot is of, is refused.
            let refused =
                seed::seed(&pool, &defs, &Options { templates: vec![999_999], ..Options::new(&copies, 1, PASSWORD) }).await.unwrap_err();
            assert!(refused.downcast_ref::<Refused>().is_some(), "{refused:#}");
            let light_only = Options { templates: vec![seeded.bots[0].character_id], ..Options::new(&copies, 2, PASSWORD) };
            assert!(seed::seed(&pool, &defs, &light_only).await.unwrap_err().downcast_ref::<Refused>().is_some());

            assert_eq!(seed::unseed(&pool, &copies).await.unwrap(), 4);
            assert_eq!(seed::unseed(&pool, &prefix).await.unwrap(), 12);
        }
    };
    with_cleanup(&pool, &[&prefix, &copies], body).await;
}

#[tokio::test]
async fn a_seeded_archer_fights_from_its_reach() {
    let Some(pool) = test_pool().await else { return };
    let prefix = throwaway_prefix();
    let body = {
        let (pool, prefix) = (pool.clone(), prefix.clone());
        async move {
            let defs = Defs::load(&pool).await.unwrap();
            let seeded = seed::seed(&pool, &defs, &Options::new(&prefix, 12, PASSWORD)).await.unwrap();
            // Bot 7 is the Light archer (class 3), bot 2 a Fury fighter, who only stands there and is killed.
            let (archer_line, victim_line) = (&seeded.bots[6], &seeded.bots[1]);
            assert_eq!((archer_line.country, archer_line.job), (0, 3));
            let weapon_type = archer_line.weapon.unwrap() / 1000;
            assert!([11, 13, 14].contains(&weapon_type), "the archer has a bow, not {weapon_type}");
            assert!(archer_line.range > 10, "a bow reaches far: {}", archer_line.range);
            let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
            let backend = Arc::new(MemoryBackend::default());
            let mut users = Vec::new();
            let mut ids = Vec::new();
            for (i, line) in [archer_line, victim_line].into_iter().enumerate() {
                let mut l = load(&pool, &store, line).await;
                l.play.map = 1;
                l.play.position = [580.0 + 35.0 * i as f32, 78.56, 1762.12];
                if i == 1 {
                    l.play.health[0] = 40;
                }
                ids.push(l.play.id);
                users.push(l.user);
                backend.add_session(l.user, session([i as u8 + 1; 16], Some(SERVER_ID), LOCAL));
                backend.add_character(l.user, SERVER, 0, l.faction, l.play);
            }
            let data = GameData {
                items: pg::load_item_defs(&pool).await.unwrap(),
                skills: pg::load_skill_defs(&pool).await.unwrap(),
                status: pg::load_status_table(&pool).await.unwrap(),
                exp: pg::load_exp(&pool).await.unwrap(),
                ..data()
            };
            let (addr, _ctx) = start_with_data(backend.clone(), config(true), data).await;
            let mut archer = bot_in_world(addr, users[0], [1; 16]).await;
            // The victim stands there, a connection nobody reads; the archer is played alone.
            let mut victim = bot_in_world(addr, users[1], [2; 16]).await;
            let mut battle = Battle::new(Config {
                point: [580.0, 1762.0],
                radius: 0.0,
                steps_per_sec: 4.0,
                skill_every: Duration::from_secs(3600),
                weapon_range: archer_line.range,
                ..Config::default()
            });
            let started = [580.0f32, 1762.0];
            let play = async {
                // Until the first blow lands: how far from the victim is the archer then?
                drive(&mut archer, &mut battle, |bot, _| !bot.metrics.attack_round_trip.is_empty()).await;
                let gap = distance(archer.world.position, archer.world.players[&ids[1]].position);
                drive(&mut archer, &mut battle, |bot, _| bot.metrics.kills >= 1).await;
                gap
            };
            let gap = tokio::time::timeout(Duration::from_secs(60), play).await.expect("the archer kills the victim within 60 s");
            let reach = archer_line.range as f32 + 2.3;
            println!(
                "archer {} range {} first blow at {gap:.1} (start 35.0, reach {reach})",
                archer_line.weapon.unwrap(),
                archer_line.range
            );
            assert!(gap > 10.0 && gap <= reach + 1.5, "the first blow came from {gap} away: a bow's reach is {reach}, a sword's under 4");
            assert!(archer.world.position[0] - started[0] < 35.0, "it did not walk up to the victim");
            assert_eq!((archer.metrics.kills, archer.metrics.refusals, archer.metrics.protocol_errors), (1, 0, 0));
            assert!(!battle.milestones.unarmed);
            archer.leave(Duration::from_secs(2)).await;
            victim.leave(Duration::from_secs(2)).await;
        }
    };
    with_cleanup(&pool, &[&prefix], body).await;
}

fn is_refused(error: &anyhow::Error) -> bool {
    error.downcast_ref::<Refused>().is_some()
}

#[tokio::test]
async fn the_guards_refuse_and_a_second_seed_resets() {
    let Some(pool) = test_pool().await else { return };
    let prefix = throwaway_prefix();
    let body = {
        let (pool, prefix) = (pool.clone(), prefix.clone());
        async move {
            let defs = Defs::load(&pool).await.unwrap();
            let options = |count| Options::new(&prefix, count, PASSWORD);
            let accounts = |pool: PgPool, prefix: String| async move {
                count(&pool, "SELECT count(*) FROM accounts WHERE username LIKE $1", &prefix).await
            };
            let real_players = total_accounts(&pool).await;

            // The limits.
            assert!(is_refused(&seed::seed(&pool, &defs, &options(0)).await.unwrap_err()));
            assert!(is_refused(&seed::seed(&pool, &defs, &options(1001)).await.unwrap_err()));
            assert!(is_refused(&seed::seed(&pool, &defs, &Options { level: 61, ..options(1) }).await.unwrap_err()));
            assert!(is_refused(&seed::seed(&pool, &defs, &Options { level: 0, ..options(1) }).await.unwrap_err()));
            assert!(
                is_refused(&seed::seed(&pool, &defs, &Options { map: 1, ..options(1) }).await.unwrap_err()),
                "map 1 has no rebirth box"
            );
            assert!(is_refused(&seed::seed(&pool, &defs, &Options::new("", 1, PASSWORD)).await.unwrap_err()));
            assert!(is_refused(&seed::unseed(&pool, "b_t").await.unwrap_err()));
            assert_eq!(accounts(pool.clone(), prefix.clone()).await, 0, "nothing was made");

            // An account that matches the prefix and is not a bot's refuses it: one with no character, one with a character of another
            // name, one that is staff.
            for (case, extra) in [("0003", None), ("0004", Some(("Somebody", 0))), ("0005", Some(("Z", 16)))] {
                let username = format!("{prefix}{case}");
                let status = extra.map_or(0, |e| e.1);
                let user: i32 =
                    sqlx::query_scalar("INSERT INTO accounts (username, password_hash, status) VALUES ($1, 'x', $2) RETURNING id")
                        .bind(&username)
                        .bind(status as i16)
                        .fetch_one(&pool)
                        .await
                        .unwrap();
                if let Some((name, _)) = extra {
                    let name = if name == "Z" { username.clone() } else { format!("{}{}", name, &prefix[1..]) };
                    sqlx::query(
                        "INSERT INTO characters (user_id, server_id, slot, name, family, grow, hair, face, size, job, sex, str, dex, rec, int, wis, luc,
                                hp, mp, sp, map, pos_x, pos_y, pos_z) VALUES ($1, 1, 0, $2, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0)",
                    )
                    .bind(user)
                    .bind(name)
                    .execute(&pool)
                    .await
                    .unwrap();
                }
                for result in [seed::seed(&pool, &defs, &options(2)).await.map(|_| ()), seed::unseed(&pool, &prefix).await.map(|_| ())] {
                    let error = result.expect_err("the prefix matches an account that is not a bot's");
                    assert!(is_refused(&error), "{error:#}");
                }
                assert_eq!(accounts(pool.clone(), prefix.clone()).await, 1, "case {case}: nothing was made or removed");
                scrub(&pool, &prefix).await;
            }

            // A prefix that matches no real account leaves the real ones alone; `player` (the maintainer's `player1`) is not even matched.
            assert_eq!(seed::unseed(&pool, "player").await.unwrap(), 0);
            assert_eq!(total_accounts(&pool).await, real_players);

            // A second seed puts the bots back instead of adding.
            let first = seed::seed(&pool, &defs, &options(2)).await.unwrap();
            let ids: Vec<u32> = first.bots.iter().map(|b| b.character_id).collect();
            let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
            let before = load(&pool, &store, &first.bots[0]).await.play;
            sqlx::query("DELETE FROM character_items WHERE character_id = $1 AND bag = 0 AND slot = 5")
                .bind(ids[0] as i32)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id) VALUES ($1, 3, 3, 25001, 25, 1)")
                .bind(ids[0] as i32)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("DELETE FROM character_skills WHERE character_id = $1").bind(ids[1] as i32).execute(&pool).await.unwrap();
            sqlx::query(
                "UPDATE characters SET hp = 1, level = 2, map = 1, pos_x = 1, pos_z = 1, kills = 9, deleted_at = now() WHERE id = $1",
            )
            .bind(ids[0] as i32)
            .execute(&pool)
            .await
            .unwrap();
            let again = seed::seed(&pool, &defs, &options(2)).await.unwrap();
            assert_eq!((again.created(), again.reset()), (0, 2));
            assert_eq!(again.bots.iter().map(|b| b.character_id).collect::<Vec<_>>(), ids, "the same characters");
            assert_eq!(accounts(pool.clone(), prefix.clone()).await, 2);
            assert_eq!(count(&pool, "SELECT count(*) FROM characters WHERE name ILIKE $1", &prefix).await, 2);
            let after = load(&pool, &store, &again.bots[0]).await.play;
            assert_eq!((after.level, after.health, after.kills), (60, before.health, [0; 4]));
            assert_eq!(
                (after.items.clone(), after.skills.clone(), after.quick_slots.clone()),
                (before.items.clone(), before.skills.clone(), before.quick_slots.clone())
            );
            assert_eq!(after.map, 18);
            let restored = load(&pool, &store, &again.bots[1]).await.play;
            assert_eq!(restored.skills.len(), again.bots[1].skills);
            // A bot count that grows adds only the new ones.
            let more = seed::seed(&pool, &defs, &options(3)).await.unwrap();
            assert_eq!((more.created(), more.reset()), (1, 2));

            // A bot in the game: both commands are refused until it is out. The login's session row stands for it.
            let user = user_of(&pool, &first.bots[0].username).await;
            sqlx::query(
                "INSERT INTO sessions (user_id, identity, aes_key, aes_iv, login_ip, server_id, expires_at, game_connected_at)
                 VALUES ($1, $2, $3, $4, '127.0.0.1', 1, now() + interval '2 minutes', now())",
            )
            .bind(user as i32)
            .bind(&[7u8; 16][..])
            .bind(&[1u8; 16][..])
            .bind(&[2u8; 16][..])
            .execute(&pool)
            .await
            .unwrap();
            assert!(is_refused(&seed::seed(&pool, &defs, &options(2)).await.unwrap_err()));
            assert!(is_refused(&seed::unseed(&pool, &prefix).await.unwrap_err()));
            assert_eq!(accounts(pool.clone(), prefix.clone()).await, 3);
            sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(user as i32).execute(&pool).await.unwrap();

            // The login log keeps its rows when the accounts go.
            sqlx::query("INSERT INTO login_log (account_id, username, ip, outcome) VALUES ($1, $2, '127.0.0.1', 'ok')")
                .bind(user as i32)
                .bind(&first.bots[0].username)
                .execute(&pool)
                .await
                .unwrap();
            assert_eq!(seed::unseed(&pool, &prefix).await.unwrap(), 3);
            let kept: Option<i32> = sqlx::query_scalar("SELECT account_id FROM login_log WHERE username = $1")
                .bind(&first.bots[0].username)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(kept, None, "the log row stays, no longer pointing at an account");
            assert_eq!(total_accounts(&pool).await, real_players);
        }
    };
    with_cleanup(&pool, &[&prefix], body).await;
}
