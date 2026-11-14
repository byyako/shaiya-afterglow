//! Runs the game server's SQL against a real PostgreSQL, inside a throwaway schema that is dropped afterwards.
//! Skipped (and says so) when DATABASE_URL is not set.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use game_server::backend::{
    AdminStore, CharacterStore, Claim, CreateOutcome, FriendStore, FriendWrite, GuildCreated, GuildStore, GuildWrite, Handover, Heartbeat,
    ItemStore, Kept, MarketItem, MarketOp, MarketStore, MarketWrite, Marks, NewAuction, NewCharacter, NewGuild, NewItem, NewMoneyRow,
    QuestEnd, QuestStore, QuickSlot, RankingRow, SessionGate, Skill, SkillStore,
};
use game_server::combat::Progress;
use game_server::creation::place_base_items;
use game_server::inventory::SlotChange;
use game_server::pg::{self, PgBackend};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

struct TestDb {
    admin: PgPool,
    pool: PgPool,
    schema: String,
}

impl TestDb {
    async fn create() -> Option<Self> {
        dotenvy::dotenv().ok();
        let Ok(url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
            eprintln!("DATABASE_URL not set: skipping the PostgreSQL tests");
            return None;
        };
        let schema = format!("test_{:08x}", rand::random::<u32>());
        let admin = PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}"))).execute(&admin).await.unwrap();
        let path = schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .after_connect(move |conn, _| {
                let path = path.clone();
                Box::pin(async move {
                    sqlx::query(AssertSqlSafe(format!("SET search_path TO {path}"))).execute(conn).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        sqlx::migrate!("../../db/migrations").run(&pool).await.unwrap();
        Some(Self { admin, pool, schema })
    }

    async fn drop(self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {} CASCADE", self.schema))).execute(&self.admin).await.unwrap();
    }

    async fn account(&self, name: &str) -> u32 {
        let id: i32 = sqlx::query_scalar("INSERT INTO accounts (username, password_hash) VALUES ($1, 'x') RETURNING id")
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .unwrap();
        id as u32
    }

    /// The row the login server writes after login and server selection.
    async fn session(&self, user: u32, identity: [u8; 16], server: i16, ip: &str) {
        sqlx::query(
            "INSERT INTO sessions (user_id, identity, aes_key, aes_iv, login_ip, server_id, expires_at)
             VALUES ($1, $2, $3, $4, $5::inet, $6, now() + interval '2 minutes')
             ON CONFLICT (user_id) DO UPDATE SET identity = EXCLUDED.identity, game_connected_at = NULL,
                 server_id = EXCLUDED.server_id, login_ip = EXCLUDED.login_ip, expires_at = EXCLUDED.expires_at",
        )
        .bind(user as i32)
        .bind(&identity[..])
        .bind(&[1u8; 16][..])
        .bind(&[2u8; 16][..])
        .bind(ip)
        .bind(server)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

const HOME: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 20));

fn claim(user_id: u32, identity: [u8; 16], ip: Option<IpAddr>) -> Claim {
    Claim { user_id, identity, server_id: 1, ip }
}

fn hero(user_id: u32, slot: u8, name: &str) -> NewCharacter {
    NewCharacter {
        user_id,
        server_id: 1,
        slot,
        name: name.into(),
        family: 0,
        grow: 1,
        hair: 1,
        face: 2,
        size: 3,
        job: 0,
        sex: 0,
        level: 1,
        stat_point: 0,
        skill_point: 5,
        stats: [14, 12, 9, 8, 7, 15],
        health: [250, 85, 195],
        map: 1,
        dir: 0,
        position: [577.98, 78.56, 1762.12],
        money: 0,
        items: vec![
            NewItem { bag: 0, slot: 5, item_id: 5001, item_type: 5, type_id: 1, quality: 1200, count: 1, kept: None },
            NewItem { bag: 1, slot: 0, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 10, kept: None },
        ],
    }
}

#[tokio::test]
async fn sessions_are_claimed_once_and_released() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let user = db.account("alice").await;
    for (id, address) in [(1, "10.0.0.1"), (2, "10.0.0.2")] {
        sqlx::query("INSERT INTO game_servers (id, name, address) VALUES ($1, 'Test', $2::inet)")
            .bind(id as i16)
            .bind(address)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    db.session(user, [7; 16], 1, "192.168.0.20").await;

    assert!(backend.claim(&claim(user, [8; 16], None)).await.unwrap().is_none(), "wrong identity");
    assert!(backend.claim(&Claim { server_id: 2, ..claim(user, [7; 16], None) }).await.unwrap().is_none(), "other server");
    let elsewhere = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    assert!(backend.claim(&claim(user, [7; 16], Some(elsewhere))).await.unwrap().is_none(), "other address");

    // The read-only check follows the same conditions and takes nothing.
    assert!(!backend.claimable(&claim(user, [8; 16], None)).await.unwrap(), "wrong identity");
    assert!(!backend.claimable(&Claim { server_id: 2, ..claim(user, [7; 16], None) }).await.unwrap(), "other server");
    assert!(!backend.claimable(&claim(user, [7; 16], Some(elsewhere))).await.unwrap(), "other address");
    assert!(backend.claimable(&claim(user, [7; 16], Some(HOME))).await.unwrap());
    assert!(backend.claimable(&claim(user, [7; 16], Some(HOME))).await.unwrap(), "asking does not use it up");
    let claimed = backend.claim(&claim(user, [7; 16], Some(HOME))).await.unwrap().expect("claimed");
    assert_eq!((claimed.keys.key, claimed.keys.iv, claimed.privilege), ([1; 16], [2; 16], 11));
    assert!(backend.claim(&claim(user, [7; 16], Some(HOME))).await.unwrap().is_none(), "only once");
    assert!(!backend.claimable(&claim(user, [7; 16], Some(HOME))).await.unwrap(), "used");

    // The heartbeat needs the server registered, and keeps in-game sessions alive.
    assert!(backend.heartbeat(9, 1, true, &[user]).await.is_err(), "unregistered server");
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '5 seconds'").execute(&db.pool).await.unwrap();
    backend.heartbeat(1, 1, true, &[user]).await.unwrap();
    let (players, left): (i32, f64) = sqlx::query_as(
        "SELECT players, (SELECT extract(epoch FROM expires_at - now())::float8 FROM sessions) FROM game_servers WHERE id = 1",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(players, 1);
    assert!(left > 100.0, "session renewed ({left}s left)");

    // Release only removes the session it claimed: after a newer login it leaves the new one alone.
    db.session(user, [9; 16], 1, "192.168.0.20").await;
    backend.release(user, [7; 16]).await.unwrap();
    assert!(backend.claim(&claim(user, [9; 16], None)).await.unwrap().is_some(), "newer session untouched");
    backend.release(user, [9; 16]).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions").fetch_one(&db.pool).await.unwrap();
    assert_eq!(count, 0);

    // An expired session cannot be claimed.
    db.session(user, [3; 16], 1, "192.168.0.20").await;
    sqlx::query("UPDATE sessions SET expires_at = now() - interval '1 second'").execute(&db.pool).await.unwrap();
    assert!(backend.claim(&claim(user, [3; 16], None)).await.unwrap().is_none(), "expired");
    db.drop().await;
}

#[tokio::test]
async fn the_heartbeat_renews_only_the_sessions_this_server_holds() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    for (id, address) in [(1, "10.0.0.1"), (2, "10.0.0.2")] {
        sqlx::query("INSERT INTO game_servers (id, name, address) VALUES ($1, 'Test', $2::inet)")
            .bind(id as i16)
            .bind(address)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    // A: claimed on server 1 and held. B: claimed on server 1, its end lost (not held). C: claimed on server 2 (named too: the server id still has to match). D: not claimed yet on server 1, named.
    let [a, b, c, d] = [db.account("a").await, db.account("b").await, db.account("c").await, db.account("d").await];
    for (user, server, identity) in [(a, 1, [1; 16]), (b, 1, [2; 16]), (c, 2, [3; 16]), (d, 1, [4; 16])] {
        db.session(user, identity, server, "192.168.0.20").await;
    }
    for (user, server, identity) in [(a, 1, [1; 16]), (b, 1, [2; 16]), (c, 2, [3; 16])] {
        let claim = Claim { user_id: user, identity, server_id: server, ip: None };
        backend.claim(&claim).await.unwrap().expect("claimed");
    }
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '5 seconds'").execute(&db.pool).await.unwrap();

    backend.heartbeat(1, 1, true, &[a, c, d]).await.unwrap();
    let left = |user: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, f64>("SELECT extract(epoch FROM expires_at - now())::float8 FROM sessions WHERE user_id = $1")
                .bind(user as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    assert!(left(a).await > 100.0, "the held session is renewed");
    assert!(left(b).await < 10.0, "a session nobody here holds is left to run out");
    assert!(left(c).await < 10.0, "another server's session is not renewed");
    assert!(left(d).await < 10.0, "a session not claimed yet is not renewed");

    // An empty list renews nothing, and the server's own row is still updated.
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '5 seconds'").execute(&db.pool).await.unwrap();
    backend.heartbeat(1, 3, true, &[]).await.unwrap();
    assert!(left(a).await < 10.0, "nothing named, nothing renewed");
    let players: i32 = sqlx::query_scalar("SELECT players FROM game_servers WHERE id = 1").fetch_one(&db.pool).await.unwrap();
    assert_eq!(players, 3);
    db.drop().await;
}

#[tokio::test]
async fn a_held_session_is_renewed_only_when_it_runs_low() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    sqlx::query("INSERT INTO game_servers (id, name, address) VALUES (1, 'Test', '10.0.0.1'::inet)").execute(&db.pool).await.unwrap();
    let [fresh, low] = [db.account("fresh").await, db.account("low").await];
    for (user, identity) in [(fresh, [1; 16]), (low, [2; 16])] {
        db.session(user, identity, 1, "192.168.0.20").await;
        backend.claim(&claim(user, identity, None)).await.unwrap().expect("claimed");
    }
    // The 110 s row is set to a fixed instant, so "unchanged" means the same timestamp, not just about the same.
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '110 seconds' WHERE user_id = $1")
        .bind(fresh as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET expires_at = now() + interval '80 seconds' WHERE user_id = $1")
        .bind(low as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    let expiry = |user: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT expires_at::text FROM sessions WHERE user_id = $1")
                .bind(user as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let left = |user: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, f64>("SELECT extract(epoch FROM expires_at - now())::float8 FROM sessions WHERE user_id = $1")
                .bind(user as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let before = expiry(fresh).await;

    backend.heartbeat(1, 2, true, &[fresh, low]).await.unwrap();

    assert_eq!(expiry(fresh).await, before, "a session with 110 s left is not written");
    let low_left = left(low).await;
    assert!((115.0..=120.5).contains(&low_left), "a session with 80 s left is renewed to about 120 s, got {low_left}");
    db.drop().await;
}

#[tokio::test]
async fn characters_follow_the_original_rules() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    let bob = db.account("bob").await;

    assert_eq!(backend.faction(alice, 1).await.unwrap(), None);
    assert!(backend.choose_faction(alice, 1, 0).await.unwrap());
    assert!(backend.choose_faction(alice, 1, 1).await.unwrap(), "may change while there are no characters");
    assert!(backend.choose_faction(alice, 1, 0).await.unwrap());

    let CreateOutcome::Created(id) = backend.create(&hero(alice, 2, "Hero")).await.unwrap() else { panic!("not created") };
    assert!(!backend.choose_faction(alice, 1, 1).await.unwrap(), "no change with a character");
    assert_eq!(backend.faction(alice, 1).await.unwrap().unwrap().country, 0);

    // Names are unique across servers ignoring case; slots per account and server.
    assert_eq!(backend.create(&hero(bob, 0, "HERO")).await.unwrap(), CreateOutcome::NameTaken);
    assert_eq!(backend.create(&NewCharacter { server_id: 2, ..hero(bob, 0, "hero") }).await.unwrap(), CreateOutcome::NameTaken);
    assert_eq!(backend.create(&hero(alice, 2, "Other")).await.unwrap(), CreateOutcome::SlotTaken);
    assert!(matches!(backend.create(&NewCharacter { server_id: 2, ..hero(alice, 2, "Other") }).await.unwrap(), CreateOutcome::Created(_)));

    let list = backend.characters(alice, 1).await.unwrap();
    assert_eq!(list.len(), 1);
    let c = &list[0];
    assert_eq!((c.id, c.slot, c.level, c.name.as_slice()), (id, 2, 1, &b"Hero"[..]));
    assert_eq!((c.stats, c.health, c.map), ([14, 12, 9, 8, 7, 15], [250, 85, 195], 1));
    assert!(!c.delete_pending);
    // The equipment shows in the list: the sword in slot 5; the potions in bag 1 do not.
    assert_eq!((c.equipment_type, c.equipment_type_id), ([0, 0, 0, 0, 0, 5, 0, 0], [0, 0, 0, 0, 0, 1, 0, 0]));

    // Loading for the world: the character with its items, only for its owner.
    let play = backend.load(alice, 1, id).await.unwrap().expect("loaded");
    assert_eq!((play.id, play.name.as_str(), play.level, play.grow, play.map), (id, "Hero", 1, 1, 1));
    assert_eq!(play.position, [577.98, 78.56, 1762.12]);
    let spots: Vec<_> = play.items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count, i.quality)).collect();
    assert_eq!(spots, [(0, 5, 5, 1, 1200), (1, 0, 25, 10, 0)]);
    assert!(backend.load(bob, 1, id).await.unwrap().is_none(), "not bob's");
    backend.save_position(id, 1, [600.5, 80.0, 1700.25], 270, [10, 20, 30]).await.unwrap();
    let moved = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!((moved.map, moved.position, moved.dir), (1, [600.5, 80.0, 1700.25], 270));
    // A position that is not finite is not written; the health is (`docs/original/move-check.md` section 10).
    backend.save_position(id, 2, [f32::NAN, 80.0, f32::INFINITY], 90, [11, 21, 31]).await.unwrap();
    let kept = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!((kept.map, kept.position, kept.dir, kept.health), (1, [600.5, 80.0, 1700.25], 270, [11, 21, 31]));
    assert!(backend.load(alice, 2, id).await.unwrap().is_none(), "other server");

    // A cloak's gems are its look in the list.
    sqlx::query("INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, gems) VALUES ($1, 0, 7, 24001, 24, 1, '\\x010203040506')")
        .bind(id as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    let c = &backend.characters(alice, 1).await.unwrap()[0];
    assert_eq!((c.equipment_type[7], c.cloak), (24, [1, 2, 3, 4, 5, 6]));

    // Deletion is a request; only the owner can make it, and it can be undone.
    assert!(!backend.request_delete(bob, 1, id).await.unwrap(), "not bob's character");
    assert!(backend.request_delete(alice, 1, id).await.unwrap());
    assert!(!backend.request_delete(alice, 1, id).await.unwrap(), "already pending");
    assert!(backend.characters(alice, 1).await.unwrap()[0].delete_pending);
    assert!(backend.load(alice, 1, id).await.unwrap().is_none(), "pending deletion");
    assert!(backend.restore(alice, 1, id).await.unwrap());
    assert!(!backend.restore(alice, 1, id).await.unwrap(), "nothing to restore");

    // Two hours after the request the character is gone, and the faction with it.
    assert!(backend.request_delete(alice, 1, id).await.unwrap());
    sqlx::query("UPDATE characters SET delete_requested_at = now() - interval '2 hours 1 second' WHERE id = $1")
        .bind(id as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(!backend.restore(alice, 1, id).await.unwrap(), "too late to restore");
    assert!(backend.characters(alice, 1).await.unwrap().is_empty());
    assert_eq!(backend.faction(alice, 1).await.unwrap(), None, "faction cleared with the last character");

    // The name of a deleted character is free unless it was above level 10 and deleted less than 7 days ago.
    assert!(matches!(backend.create(&hero(bob, 0, "Hero")).await.unwrap(), CreateOutcome::Created(_)));
    let CreateOutcome::Created(veteran) = backend.create(&hero(bob, 1, "Veteran")).await.unwrap() else { panic!() };
    sqlx::query("UPDATE characters SET level = 11, deleted_at = now() WHERE id = $1").bind(veteran as i32).execute(&db.pool).await.unwrap();
    assert_eq!(backend.create(&hero(alice, 0, "veteran")).await.unwrap(), CreateOutcome::NameTaken);
    sqlx::query("UPDATE characters SET deleted_at = now() - interval '8 days' WHERE id = $1")
        .bind(veteran as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(backend.create(&hero(alice, 0, "Veteran")).await.unwrap(), CreateOutcome::Created(_)));
    db.drop().await;
}

#[tokio::test]
async fn class_table_loads_from_the_game_definitions() {
    let Some(db) = TestDb::create().await else { return };
    // `defs` is a separate schema loaded by tools/defs; skip if this database does not have it.
    let present: bool = sqlx::query_scalar("SELECT to_regclass('defs.status_defs') IS NOT NULL").fetch_one(&db.pool).await.unwrap();
    if present {
        let base = pg::load_class_base(&db.pool).await.unwrap();
        assert_eq!((base[0].hp, base[0].sp, base[0].mp), (205, 125, 50), "fighter, level 1");
        assert_eq!((base[4].hp, base[4].sp, base[4].mp), (150, 60, 170), "mage, level 1");

        // Every race and class combination gets its weapon equipped and one more item in bag 1.
        let placed = place_base_items(pg::load_base_items(&db.pool).await.unwrap());
        assert_eq!(placed.len(), 12);
        for ((family, job), items) in &placed {
            let spots: Vec<_> = items.iter().map(|i| (i.bag, i.slot)).collect();
            assert_eq!(spots, [(0, 5), (1, 0)], "race {family} class {job}");
        }
        let fighter = &placed[&(0, 0)];
        assert_eq!((fighter[0].item_id, fighter[0].quality, fighter[1].item_id, fighter[1].count), (5001, 1200, 25001, 10));

        let exp = pg::load_exp(&db.pool).await.unwrap();
        assert_eq!(exp.bounds(1, 1), (0, 100), "normal mode, level 1");
        assert_eq!(exp.bounds(0, 2), (70, 200), "easy mode, level 2: 70 + 130");
    } else {
        eprintln!("defs.status_defs not loaded: skipping");
    }
    db.drop().await;
}

#[tokio::test]
async fn trades_change_the_money_and_one_slot_together() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Trader")).await.unwrap() else { panic!("not created") };
    sqlx::query("UPDATE characters SET money = 1000 WHERE id = $1").bind(id as i32).execute(&db.pool).await.unwrap();
    let sword = NewItem { bag: 1, slot: 1, item_id: 5001, item_type: 5, type_id: 1, quality: 1200, count: 1, kept: None };

    assert!(backend.trade(id, -500, &[SlotChange::Insert(sword)]).await.unwrap().is_some());
    assert!(backend.trade(id, -100, &[SlotChange::SetCount { bag: 1, slot: 0, count: 20 }]).await.unwrap().is_some());
    // Refused, and nothing changes: too expensive, a taken slot, an empty slot, past the money's maximum.
    assert!(!backend.trade(id, -401, &[SlotChange::Insert(NewItem { slot: 2, ..sword })]).await.unwrap().is_some());
    assert!(!backend.trade(id, -1, &[SlotChange::Insert(sword)]).await.unwrap().is_some());
    assert!(!backend.trade(id, 5, &[SlotChange::Remove { bag: 1, slot: 9 }]).await.unwrap().is_some());
    assert!(!backend.trade(id, i64::from(u32::MAX), &[SlotChange::Remove { bag: 1, slot: 1 }]).await.unwrap().is_some());
    assert!(backend.trade(id, 100, &[SlotChange::Remove { bag: 1, slot: 1 }]).await.unwrap().is_some());
    // Money alone, as a gatekeeper charges it.
    assert!(backend.trade(id, -50, &[]).await.unwrap().is_some());
    assert!(!backend.trade(id, -1_000_000, &[]).await.unwrap().is_some());
    // Swapping the equipped sword (bag 0 slot 5) with the potions is one statement on the deferrable slot key (migration 0007);
    // then the sword moves to an empty slot. A move onto a taken slot is refused.
    assert!(backend.trade(id, 0, &[SlotChange::Swap { a: (0, 5), b: (1, 0) }]).await.unwrap().is_some());
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (1, 0), to: (2, 3) }]).await.unwrap().is_some());
    assert!(!backend.trade(id, 0, &[SlotChange::Move { from: (2, 3), to: (0, 5) }]).await.unwrap().is_some());

    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(play.money, 450);
    let slots: Vec<_> = play.items.iter().map(|i| (i.bag, i.slot, i.count)).collect();
    assert_eq!(slots, [(0, 5, 20), (2, 3, 1)]);
    db.drop().await;
}

#[tokio::test]
async fn gems_craft_names_and_make_types_are_stored() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Linker")).await.unwrap() else { panic!("not created") };
    let quest = Kept { make_type: b'Q', ..Kept::default() };
    let reward = NewItem { bag: 2, slot: 0, item_id: 5001, item_type: 5, type_id: 1, quality: 1200, count: 1, kept: Some(quest) };
    assert!(backend.trade(id, 0, &[SlotChange::Insert(reward)]).await.unwrap().is_some());
    let name = Kept::pad_craft_name(b"01020304050607080953");
    let changes =
        [SlotChange::SetGems { bag: 2, slot: 0, gems: [7, 0, 9, 0, 0, 0] }, SlotChange::SetCraftName { bag: 2, slot: 0, craft_name: name }];
    assert!(backend.trade(id, 0, &changes).await.unwrap().is_some());
    // An empty place is refused, with nothing changed.
    assert!(!backend.trade(id, 0, &[SlotChange::SetGems { bag: 2, slot: 1, gems: [1; 6] }]).await.unwrap().is_some());
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    let item = play.items.iter().find(|i| (i.bag, i.slot) == (2, 0)).unwrap();
    assert_eq!((item.gems, item.craft_name.as_slice()), ([7, 0, 9, 0, 0, 0], &b"01020304050607080953"[..]));
    assert_eq!(backend.make_types(id, &[(2, 0), (2, 1), (0, 5)]).await.unwrap(), [b'Q', 0, 0]);
    db.drop().await;
}

#[tokio::test]
async fn a_dropped_item_keeps_its_row_through_the_ground() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Dropper")).await.unwrap() else { panic!("not created") };
    let sword = NewItem { bag: 1, slot: 1, item_id: 5001, item_type: 5, type_id: 1, quality: 1100, count: 1, kept: None };
    assert!(backend.trade(id, 0, &[SlotChange::Insert(sword)]).await.unwrap().is_some());
    sqlx::query(
        "UPDATE character_items SET gems = '\\x010200000000', craft_name = '0102', make_type = 'T', made_at = '2020-01-02 03:04:05.5+00'
                  WHERE character_id = $1 AND bag = 1 AND slot = 1",
    )
    .bind(id as i32)
    .execute(&db.pool)
    .await
    .unwrap();
    let row = |slot: i16| {
        sqlx::query_as::<_, (i64, String, String, Vec<u8>, String)>(
            "SELECT uid, made_at::text, make_type, gems, craft_name FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = $2",
        )
        .bind(id as i32)
        .bind(slot)
        .fetch_one(&db.pool)
    };
    let before = row(1).await.unwrap();

    // Whole: the row leaves with everything a new row would not have; a slot not holding `had` changes nothing.
    assert_eq!(backend.drop_item(id, 1, 1, 2, 1).await.unwrap(), None);
    let (kept, dropped_row) = backend.drop_item(id, 1, 1, 1, 1).await.unwrap().unwrap();
    assert_eq!((dropped_row.uid, kept.origin.map(|o| o.0)), (before.0, Some(before.0)), "the dropped row is named");
    assert_eq!((kept.gems, kept.craft_name(), kept.make_type), ([1, 2, 0, 0, 0, 0], &b"0102"[..], b'T'));
    assert!(row(1).await.is_err(), "the row is gone");
    // Picked up again elsewhere: the same uid and making time.
    assert!(backend.trade(id, 0, &[SlotChange::Insert(NewItem { slot: 4, kept: Some(kept), ..sword })]).await.unwrap().is_some());
    assert_eq!(row(4).await.unwrap(), before);
    // Wear or a repair sets the quality; an empty slot refuses it.
    assert!(backend.trade(id, 0, &[SlotChange::SetQuality { bag: 1, slot: 4, quality: 500 }]).await.unwrap().is_some());
    assert!(!backend.trade(id, 0, &[SlotChange::SetQuality { bag: 1, slot: 9, quality: 500 }]).await.unwrap().is_some());
    assert_eq!(backend.load(alice, 1, id).await.unwrap().unwrap().items.iter().find(|i| (i.bag, i.slot) == (1, 4)).unwrap().quality, 500);

    // Part of the starting potions (bag 1 slot 0, 10 of them): the count goes down; the part is a new item of make type 'U'.
    let (split, kept_row) = backend.drop_item(id, 1, 0, 10, 3).await.unwrap().unwrap();
    assert_eq!(split, Kept::SPLIT);
    let potion_uid: i64 = sqlx::query_scalar("SELECT uid FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = 0")
        .bind(id as i32)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(kept_row.uid, potion_uid, "the row that keeps the rest");
    let potion = NewItem { bag: 1, slot: 5, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 3, kept: Some(Kept::SPLIT) };
    assert!(backend.trade(id, 0, &[SlotChange::Insert(potion)]).await.unwrap().is_some());
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    let counts: Vec<_> = play.items.iter().filter(|i| i.bag == 1).map(|i| (i.slot, i.count)).collect();
    assert_eq!(counts, [(0, 7), (4, 1), (5, 3)]);
    assert_eq!(row(5).await.unwrap().2, "U");
    db.drop().await;
}

#[tokio::test]
async fn save_points_and_body_changes_are_stored() {
    use game_server::backend::{Body, SavePoint};
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Traveller")).await.unwrap() else { panic!("not created") };
    // A slot saved twice keeps the last.
    backend.save_point(id, SavePoint { slot: 2, map: 1, position: [1.0, 2.0, 3.0] }).await.unwrap();
    backend.save_point(id, SavePoint { slot: 2, map: 5, position: [4.0, 5.0, 6.0] }).await.unwrap();
    backend.save_point(id, SavePoint { slot: 0, map: 1, position: [7.0, 8.0, 9.0] }).await.unwrap();
    // The body with one of the scroll (here the 10 starting potions in bag 1 slot 0); a wrong count changes nothing.
    let body = Body { hair: 9, face: 8, size: 7, sex: 1 };
    assert!(!backend.change_body(id, body, (1, 0), 3).await.unwrap());
    assert!(backend.change_body(id, body, (1, 0), 10).await.unwrap());
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(
        play.save_points,
        [SavePoint { slot: 0, map: 1, position: [7.0, 8.0, 9.0] }, SavePoint { slot: 2, map: 5, position: [4.0, 5.0, 6.0] }]
    );
    assert_eq!((play.hair, play.face, play.size, play.sex), (9, 8, 7, 1));
    assert_eq!(play.items.iter().find(|i| (i.bag, i.slot) == (1, 0)).unwrap().count, 9);
    db.drop().await;
}

#[tokio::test]
async fn the_warehouse_belongs_to_the_account_on_its_server() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(alice, 2, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Banker")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(second) = backend.create(&hero(alice, 1, "Second")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(elsewhere) = backend.create(&NewCharacter { server_id: 2, ..hero(alice, 0, "Elsewhere") }).await.unwrap()
    else {
        panic!("not created")
    };
    sqlx::query("UPDATE characters SET money = 100 WHERE id = $1").bind(id as i32).execute(&db.pool).await.unwrap();
    let uid = |table: &'static str, owner: &'static str, key: i32, slot: i16| {
        let pool = db.pool.clone();
        async move {
            let bag = if table == "bank_items" { "" } else { "AND bag = 1" };
            sqlx::query_scalar::<_, i64>(AssertSqlSafe(format!("SELECT uid FROM {table} WHERE {owner} = $1 {bag} AND slot = $2")))
                .bind(key)
                .bind(slot)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let potion_uid = uid("character_items", "character_id", id as i32, 0).await.unwrap();

    // In, from the inventory and from the equipment's sword moved out first; the item keeps its uid.
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (1, 0), to: (100, 3) }]).await.unwrap().is_some());
    assert_eq!(uid("bank_items", "user_id", alice as i32, 3).await, Some(potion_uid));
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (0, 5), to: (1, 1) }]).await.unwrap().is_some());
    // Refused, nothing changed: into a taken warehouse slot, from an empty one, out into a taken bag slot, a new item made there.
    assert!(!backend.trade(id, 0, &[SlotChange::Move { from: (1, 1), to: (100, 3) }]).await.unwrap().is_some());
    assert!(!backend.trade(id, 0, &[SlotChange::Move { from: (100, 9), to: (1, 5) }]).await.unwrap().is_some());
    assert!(!backend.trade(id, 0, &[SlotChange::Move { from: (100, 3), to: (1, 1) }]).await.unwrap().is_some());
    let made = NewItem { bag: 100, slot: 7, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 1, kept: None };
    assert!(!backend.trade(id, 0, &[SlotChange::Insert(made)]).await.unwrap().is_some());
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (1, 1), to: (100, 130) }]).await.unwrap().is_some());
    // Inside: a swap in one statement, a move, a stack split as a combine leaves it.
    assert!(backend.trade(id, 0, &[SlotChange::Swap { a: (100, 3), b: (100, 130) }]).await.unwrap().is_some());
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (100, 130), to: (100, 4) }]).await.unwrap().is_some());
    assert!(backend.trade(id, 0, &[SlotChange::SetCount { bag: 100, slot: 4, count: 6 }]).await.unwrap().is_some());

    // The second character of the account sees the same warehouse; a fee it cannot pay changes nothing. The first takes the
    // potions out, the fee paid with the move.
    let play = backend.load(alice, 1, second).await.unwrap().unwrap();
    let bank: Vec<_> = play.bank.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    assert_eq!(bank, [(100, 3, 5, 1), (100, 4, 25, 6)]);
    assert!(!backend.trade(second, -1, &[SlotChange::Move { from: (100, 4), to: (1, 5) }]).await.unwrap().is_some());
    assert!(backend.trade(id, -3, &[SlotChange::Move { from: (100, 4), to: (1, 5) }]).await.unwrap().is_some());
    assert_eq!(uid("character_items", "character_id", id as i32, 5).await, Some(potion_uid));
    assert!(backend.trade(id, 0, &[SlotChange::Remove { bag: 100, slot: 3 }]).await.unwrap().is_some());

    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(play.money, 97);
    assert!(play.bank.is_empty());
    let slots: Vec<_> = play.items.iter().map(|i| (i.bag, i.slot, i.count)).collect();
    assert_eq!(slots, [(1, 5, 6)]);
    // The account's warehouse on another server is its own.
    assert!(backend.trade(elsewhere, 0, &[SlotChange::Move { from: (1, 0), to: (100, 0) }]).await.unwrap().is_some());
    assert!(backend.load(alice, 1, second).await.unwrap().unwrap().bank.is_empty());
    assert_eq!(backend.load(alice, 2, elsewhere).await.unwrap().unwrap().bank.len(), 1);
    db.drop().await;
}

#[tokio::test]
async fn exchanges_move_money_and_items_between_two_characters_together() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    let mut ids = Vec::new();
    for (user, name) in [(alice, "Giver"), (bob, "Taker")] {
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push(id);
    }
    let (giver, taker) = (ids[0], ids[1]);
    sqlx::query("UPDATE characters SET money = 1000 WHERE id = $1").bind(giver as i32).execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE character_items SET gems = '\\x010203040506', craft_name = 'abc' WHERE character_id = $1 AND bag = 1")
        .bind(giver as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    let uid = |id: u32, bag: i16, slot: i16| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT uid FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3")
                .bind(id as i32)
                .bind(bag)
                .bind(slot)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let sword_uid = uid(giver, 0, 5).await.unwrap();
    // The sword is equipped; move it to the inventory first so that it can be handed over whole.
    assert!(backend.trade(giver, 0, &[SlotChange::Move { from: (0, 5), to: (1, 1) }]).await.unwrap().is_some());
    let part = Handover { giver, from: (1, 0), had: 10, count: 4, receiver: taker, to: (1, 1) };
    let whole = Handover { giver, from: (1, 1), had: 1, count: 1, receiver: taker, to: (1, 2) };

    // Refused, nothing changed: the giver's slot does not hold what it should, the receiver's slot is taken, the money is short.
    assert!(!backend.exchange([(giver, -300), (taker, 300)], &[Handover { had: 9, ..part }]).await.unwrap().is_some());
    assert!(!backend.exchange([(giver, -300), (taker, 300)], &[part, Handover { to: (1, 0), ..whole }]).await.unwrap().is_some());
    assert!(!backend.exchange([(giver, -1001), (taker, 1001)], &[part]).await.unwrap().is_some());
    let play = backend.load(alice, 1, giver).await.unwrap().unwrap();
    assert_eq!((play.money, play.items.len()), (1000, 2));

    assert!(backend.exchange([(giver, -300), (taker, 300)], &[part, whole]).await.unwrap().is_some());
    let giver_play = backend.load(alice, 1, giver).await.unwrap().unwrap();
    let taker_play = backend.load(bob, 1, taker).await.unwrap().unwrap();
    assert_eq!(giver_play.money, 700);
    assert_eq!(taker_play.money, 300);
    let slots = |items: &[game_server::backend::ItemUnit]| items.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect::<Vec<_>>();
    assert_eq!(slots(&giver_play.items), [(1, 0, 25, 6)]);
    assert_eq!(slots(&taker_play.items), [(0, 5, 5, 1), (1, 0, 25, 10), (1, 1, 25, 4), (1, 2, 5, 1)]);
    // The copy keeps the gems and the craft name and is marked as traded; the whole sword keeps its uid.
    let copy = &taker_play.items[2];
    assert_eq!((copy.gems, copy.craft_name.as_slice()), ([1, 2, 3, 4, 5, 6], &b"abc"[..]));
    let make_type: String = sqlx::query_scalar("SELECT make_type FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = 1")
        .bind(taker as i32)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(make_type, "T");
    assert_eq!(uid(taker, 1, 2).await, Some(sword_uid));
    db.drop().await;
}

#[tokio::test]
async fn guilds_are_created_changed_and_disbanded() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let mut ids = Vec::new();
    for name in ["Master", "Member", "Asker"] {
        let user = db.account(&name.to_lowercase()).await;
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push((user, id));
    }
    let [(master_user, master), (_, member), (asker_user, asker)] = ids[..] else { unreachable!() };
    sqlx::query("UPDATE characters SET money = 1500 WHERE id = $1").bind(master as i32).execute(&db.pool).await.unwrap();
    let new = |name: &str, cost| NewGuild {
        server_id: 1,
        name: name.into(),
        country: 0,
        remark: "Hi".into(),
        members: vec![(master, 1), (member, 9)],
        payer: master,
        cost,
    };
    assert_eq!(backend.create_guild(&new("Knights", 2000)).await.unwrap(), GuildCreated::NoMoney);
    let GuildCreated::Created(guild) = backend.create_guild(&new("Knights", 1000)).await.unwrap() else { panic!("not created") };
    assert_eq!(backend.create_guild(&new("KNIGHTS", 0)).await.unwrap(), GuildCreated::NameTaken, "names compare without case");
    let master_play = backend.load(master_user, 1, master).await.unwrap().unwrap();
    assert_eq!(master_play.money, 500);
    assert!(master_play.guild_joined_secs.is_some_and(|secs| secs < 60));
    assert_eq!(backend.load(asker_user, 1, asker).await.unwrap().unwrap().guild_joined_secs, None);

    backend.write_guild(&GuildWrite::Add { guild, character: asker, rank: 0 }).await.unwrap();
    backend.write_guild(&GuildWrite::Remark { guild, remark: "Welcome".into() }).await.unwrap();
    let loaded = backend.load_guilds(1).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!((loaded[0].id, loaded[0].name.as_str(), loaded[0].remark.as_str()), (guild, "Knights", "Welcome"));
    let ranks: Vec<(u32, u8)> = loaded[0].members.iter().map(|m| (m.id, m.rank)).collect();
    assert_eq!(ranks, [(master, 1), (member, 9), (asker, 0)]);
    assert_eq!(backend.load(asker_user, 1, asker).await.unwrap().unwrap().guild_joined_secs, None, "a request is not a join");

    backend.write_guild(&GuildWrite::Accept { character: asker }).await.unwrap();
    backend.write_guild(&GuildWrite::Rank { character: member, rank: 2 }).await.unwrap();
    backend.write_guild(&GuildWrite::Remove { character: asker }).await.unwrap();
    let loaded = backend.load_guilds(1).await.unwrap();
    assert_eq!(loaded[0].members.iter().map(|m| (m.id, m.rank)).collect::<Vec<_>>(), [(master, 1), (member, 2)]);
    assert!(backend.load(asker_user, 1, asker).await.unwrap().unwrap().guild_joined_secs.is_some(), "accepted: joined");

    // The house (docs/original/guild-house.md section 11): a new guild's, then bought, Etin, an upgrade.
    assert_eq!(loaded[0].house, game_server::guild_house::House::default());
    backend.write_guild(&GuildWrite::HouseBought { guild, keep_etin: 2000 }).await.unwrap();
    backend.write_guild(&GuildWrite::Etin { guild, etin: 5000, returns: 3 }).await.unwrap();
    backend.write_guild(&GuildWrite::Upgrade { guild, slot: 0, key: 2, level: 1, etin: 4000, keep_etin: 2200 }).await.unwrap();
    backend.write_guild(&GuildWrite::Upgrade { guild, slot: 0, key: 2, level: 2, etin: 2500, keep_etin: 2500 }).await.unwrap();
    let house = backend.load_guilds(1).await.unwrap().remove(0).house;
    assert_eq!((house.bought, house.etin, house.etin_returns, house.keep_etin, house.ranking), (true, 2500, 3, 2500, 31));
    assert_eq!(house.slots[0], (2, 2));

    // The ranking update (docs/original/guild-ranking.md section 8): points, Etin, ranking, byUseHouse; the return count back to 0.
    assert_eq!(backend.load_ranking_due().await.unwrap(), None, "nothing waits at first");
    backend
        .write_guild(&GuildWrite::Ranking { rows: vec![RankingRow { guild, points: 150, etin: 500, ranking: 3, use_house: 1 }] })
        .await
        .unwrap();
    let house = backend.load_guilds(1).await.unwrap().remove(0).house;
    assert_eq!((house.points, house.etin, house.ranking, house.use_house, house.etin_returns), (150, 500, 3, 1, 0));
    assert_eq!(house.slots[0], (2, 2), "the NPCs stay");
    // Deviation 4: the due time is kept across a restart, and cleared once the update ran.
    backend.write_guild(&GuildWrite::RankingDue(Some(1_800_000_000))).await.unwrap();
    assert_eq!(backend.load_ranking_due().await.unwrap(), Some(1_800_000_000));
    backend.write_guild(&GuildWrite::RankingDue(None)).await.unwrap();
    assert_eq!(backend.load_ranking_due().await.unwrap(), None);

    backend.write_guild(&GuildWrite::Disband { guild }).await.unwrap();
    assert!(backend.load_guilds(1).await.unwrap().is_empty());
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_members").fetch_one(&db.pool).await.unwrap();
    assert_eq!(left, 0);
    // The name is free again.
    sqlx::query("UPDATE characters SET money = 1000 WHERE id = $1").bind(master as i32).execute(&db.pool).await.unwrap();
    assert!(matches!(backend.create_guild(&new("Knights", 1000)).await.unwrap(), GuildCreated::Created(id) if id != guild));
    db.drop().await;
}

#[tokio::test]
async fn the_guild_bank_moves_items_with_the_money_and_loads_with_the_guilds() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Banker")).await.unwrap() else { panic!("not created") };
    sqlx::query("UPDATE characters SET money = 100 WHERE id = $1").bind(id as i32).execute(&db.pool).await.unwrap();
    let new =
        NewGuild { server_id: 1, name: "Vault".into(), country: 0, remark: String::new(), members: vec![(id, 1)], payer: id, cost: 0 };
    let GuildCreated::Created(guild) = backend.create_guild(&new).await.unwrap() else { panic!("not created") };
    let bag_uid = |bag: i16, slot: i16| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT uid FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3")
                .bind(id as i32)
                .bind(bag)
                .bind(slot)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let bank_uid = |slot: i16| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT uid FROM guild_bank_items WHERE guild_id = $1 AND slot = $2")
                .bind(guild as i32)
                .bind(slot)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let potion_uid = bag_uid(1, 0).await.unwrap();
    let sword_uid = bag_uid(0, 5).await.unwrap();
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (0, 5), to: (1, 1) }]).await.unwrap().is_some());

    // In: an item keeps its uid; a taken slot, an empty source and a new item made in the bank change nothing.
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (1, 0), to: (255, 3) }]).await.unwrap().is_some());
    assert_eq!(bank_uid(3).await, Some(potion_uid));
    assert_eq!(bag_uid(1, 0).await, None);
    assert!(!backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (1, 1), to: (255, 3) }]).await.unwrap().is_some());
    assert!(!backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (1, 9), to: (255, 4) }]).await.unwrap().is_some());
    let made = NewItem { bag: 255, slot: 7, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 1, kept: None };
    assert!(!backend.guild_bank(id, guild, 0, &[SlotChange::Insert(made)]).await.unwrap().is_some());
    assert_eq!(bag_uid(1, 1).await, Some(sword_uid), "a refused move changed nothing");

    // Out with a fee: the money goes down with the move, and a fee it cannot pay changes nothing.
    assert!(!backend.guild_bank(id, guild, -101, &[SlotChange::Move { from: (255, 3), to: (1, 5) }]).await.unwrap().is_some());
    assert_eq!(bank_uid(3).await, Some(potion_uid));
    assert!(backend.guild_bank(id, guild, -3, &[SlotChange::Move { from: (255, 3), to: (1, 5) }]).await.unwrap().is_some());
    assert_eq!(bag_uid(1, 5).await, Some(potion_uid));
    assert_eq!(bank_uid(3).await, None);
    assert!(
        !backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (255, 3), to: (1, 6) }]).await.unwrap().is_some(),
        "an empty bank slot"
    );
    let money: i64 = sqlx::query_scalar("SELECT money FROM characters WHERE id = $1").bind(id as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(money, 97);

    // A bag place and a bank place exchange their items, each keeping its uid; an empty bank slot refuses and rolls back.
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (1, 5), to: (255, 3) }]).await.unwrap().is_some());
    assert!(!backend.guild_bank(id, guild, 0, &[SlotChange::Swap { a: (1, 1), b: (255, 9) }]).await.unwrap().is_some());
    assert_eq!(bag_uid(1, 1).await, Some(sword_uid));
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Swap { a: (1, 1), b: (255, 3) }]).await.unwrap().is_some());
    assert_eq!((bag_uid(1, 1).await, bank_uid(3).await), (Some(potion_uid), Some(sword_uid)));
    assert!(backend.guild_bank(id, guild, -1, &[SlotChange::Swap { a: (255, 3), b: (1, 1) }]).await.unwrap().is_some(), "either order");
    assert_eq!((bag_uid(1, 1).await, bank_uid(3).await), (Some(sword_uid), Some(potion_uid)));

    // Inside the bank: a move, a swap in one statement, and a combine as it is saved (a count and a removal).
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (1, 1), to: (255, 7) }]).await.unwrap().is_some());
    assert!(
        !backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (255, 7), to: (255, 3) }]).await.unwrap().is_some(),
        "a taken target"
    );
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Swap { a: (255, 3), b: (255, 7) }]).await.unwrap().is_some());
    assert_eq!((bank_uid(3).await, bank_uid(7).await), (Some(sword_uid), Some(potion_uid)));
    assert!(!backend.guild_bank(id, guild, 0, &[SlotChange::Swap { a: (255, 3), b: (255, 9) }]).await.unwrap().is_some());
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (255, 3), to: (255, 4) }]).await.unwrap().is_some());
    assert!(
        backend
            .guild_bank(id, guild, 0, &[SlotChange::SetCount { bag: 255, slot: 7, count: 6 }, SlotChange::Remove { bag: 255, slot: 4 }])
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(bank_uid(4).await, None);

    // The bank loads with the guild, bag 255, by slot; a lowered count is saved; a disbanded guild's rows stay.
    assert!(backend.guild_bank(id, guild, 0, &[SlotChange::Move { from: (255, 7), to: (255, 2) }]).await.unwrap().is_some());
    backend.write_guild(&GuildWrite::BankCount { guild, slot: 2, count: 5 }).await.unwrap();
    let loaded = backend.load_guilds(1).await.unwrap();
    let bank: Vec<_> = loaded[0].bank.iter().map(|i| (i.bag, i.slot, i.item_type, i.count)).collect();
    assert_eq!(bank, [(255, 2, 25, 5)]);
    backend.write_guild(&GuildWrite::Disband { guild }).await.unwrap();
    assert!(backend.load_guilds(1).await.unwrap().is_empty());
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_bank_items").fetch_one(&db.pool).await.unwrap();
    assert_eq!(kept, 1, "the disbanded guild's items are kept");
    db.drop().await;
}

#[tokio::test]
async fn an_etin_return_and_the_house_purchase_are_each_one_transaction_for_a_member_and_the_master() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let mut ids = Vec::new();
    for name in ["Master", "Member", "Asker", "Alone"] {
        let user = db.account(&name.to_lowercase()).await;
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push(id);
    }
    let [master, member, asker, alone] = ids[..] else { unreachable!() };
    let new = NewGuild {
        server_id: 1,
        name: "Knights".into(),
        country: 0,
        remark: String::new(),
        members: vec![(master, 1), (member, 9)],
        payer: master,
        cost: 0,
    };
    let GuildCreated::Created(guild) = backend.create_guild(&new).await.unwrap() else { panic!("not created") };
    backend.write_guild(&GuildWrite::Add { guild, character: asker, rank: 0 }).await.unwrap();
    let etin =
        |slot| SlotChange::Insert(NewItem { bag: 1, slot, item_id: 100_001, item_type: 100, type_id: 1, quality: 0, count: 5, kept: None });
    for who in [member, asker, alone] {
        assert!(backend.trade(who, 0, &[etin(3)]).await.unwrap().is_some());
    }
    let house = || async { backend.load_guilds(1).await.unwrap().remove(0).house };
    let item_at = |who: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i32>("SELECT count(*)::int FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = 3")
                .bind(who as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };

    // A member: the item goes and the guild's Etin and return count grow together.
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_some());
    assert_eq!((item_at(member).await, house().await.etin, house().await.etin_returns), (0, 50, 1));
    // A slot that is not as the change expects, a join request, a stranger: nothing changes.
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert!(backend.guild_return_etin(asker, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert!(backend.guild_return_etin(alone, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert_eq!((item_at(asker).await, item_at(alone).await, house().await.etin, house().await.etin_returns), (1, 1, 50, 1));
    // The Etin stops at 32 bits.
    sqlx::query("UPDATE guilds SET etin = 4294967290 WHERE id = $1").bind(guild as i32).execute(&db.pool).await.unwrap();
    assert!(backend.trade(member, 0, &[etin(3)]).await.unwrap().is_some());
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_some());
    assert_eq!(house().await.etin, u32::MAX);
    sqlx::query("UPDATE guilds SET etin = 50 WHERE id = $1").bind(guild as i32).execute(&db.pool).await.unwrap();

    // The house: only the master, only with the money, once; a failure takes no money.
    let money = |who: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT money FROM characters WHERE id = $1").bind(who as i32).fetch_one(&pool).await.unwrap()
        }
    };
    for who in [master, member] {
        sqlx::query("UPDATE characters SET money = 60000000 WHERE id = $1").bind(who as i32).execute(&db.pool).await.unwrap();
    }
    assert!(!backend.buy_guild_house(member, guild, 50_000_000, 2000).await.unwrap(), "not the master");
    assert!(!backend.buy_guild_house(master, guild + 99, 50_000_000, 2000).await.unwrap(), "not that guild's master");
    sqlx::query("UPDATE characters SET money = 49999999 WHERE id = $1").bind(master as i32).execute(&db.pool).await.unwrap();
    assert!(!backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap(), "too little money");
    assert_eq!((money(master).await, money(member).await, house().await.bought), (49_999_999, 60_000_000, false));
    sqlx::query("UPDATE characters SET money = 60000000 WHERE id = $1").bind(master as i32).execute(&db.pool).await.unwrap();
    assert!(backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap());
    let bought = house().await;
    assert_eq!((money(master).await, bought.bought, bought.keep_etin, bought.etin), (10_000_000, true, 2000, 50));
    assert!(!backend.buy_guild_house(master, guild, 50_000_000, 2000).await.unwrap(), "bought already");
    assert_eq!(money(master).await, 10_000_000, "and nothing was paid again");

    // A disbanded guild takes neither.
    backend.write_guild(&GuildWrite::Disband { guild }).await.unwrap();
    assert!(backend.trade(member, 0, &[etin(3)]).await.unwrap().is_some());
    assert!(backend.guild_return_etin(member, guild, 50, &[SlotChange::Remove { bag: 1, slot: 3 }]).await.unwrap().is_none());
    assert_eq!(item_at(member).await, 1);
    db.drop().await;
}

#[tokio::test]
async fn gifts_between_the_same_two_accounts_at_once_do_not_deadlock() {
    use game_server::backend::{GiftOrder, GiftOutcome, PointStore};
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(bob, 1, 1).await.unwrap();
    let CreateOutcome::Created(a) = backend.create(&hero(alice, 0, "Alice")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(b) = backend.create(&NewCharacter { family: 2, ..hero(bob, 0, "Bobby") }).await.unwrap() else {
        panic!("not created")
    };
    sqlx::query("UPDATE accounts SET points = 1000").execute(&db.pool).await.unwrap();
    let gift = |from: u32, character: u32, to: &str| GiftOrder {
        user_id: from,
        server_id: 1,
        character_id: character,
        target: to.into(),
        code: "Stone".into(),
        price: 1,
        rows: vec![(100048, 1)],
    };
    // Each round sends a gift each way at the same moment (audit R25): both must go through.
    for _ in 0..25 {
        let (order_a, order_b) = (gift(alice, a, "Bobby"), gift(bob, b, "Alice"));
        let (to_bob, to_alice) = tokio::join!(backend.gift(&order_a), backend.gift(&order_b));
        assert!(matches!(to_bob.expect("no deadlock"), GiftOutcome::Sent { .. }));
        assert!(matches!(to_alice.expect("no deadlock"), GiftOutcome::Sent { .. }));
    }
    assert_eq!((backend.points(alice).await.unwrap(), backend.points(bob).await.unwrap()), (975, 975));
    db.drop().await;
}

#[tokio::test]
async fn friends_and_blocks_are_stored_once_and_load_without_the_deleted() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let mut ids = Vec::new();
    for name in ["Ann", "Bob", "Cid"] {
        let user = db.account(&name.to_lowercase()).await;
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push((user, id));
    }
    let [(ann_user, ann), (bob_user, bob), (cid_user, cid)] = ids[..] else { unreachable!() };
    for write in [
        FriendWrite::Friends(vec![(ann, bob), (bob, ann)]),
        FriendWrite::Friends(vec![(ann, bob), (ann, cid)]),
        FriendWrite::FriendMemo { character: ann, friend: bob, memo: "Bob's alt".into() },
        FriendWrite::Block { character: bob, blocked: cid },
        FriendWrite::Block { character: bob, blocked: cid },
        FriendWrite::BlockMemo { character: bob, blocked: cid, memo: "rude".into() },
    ] {
        backend.write_friends(&write).await.unwrap();
    }
    let ann_saved = backend.load(ann_user, 1, ann).await.unwrap().unwrap();
    let listed = |c: &[game_server::backend::StoredFriend]| c.iter().map(|f| (f.id, f.name.clone(), f.memo.clone())).collect::<Vec<_>>();
    assert_eq!(listed(&ann_saved.friends), [(bob, "Bob".into(), "Bob's alt".into()), (cid, "Cid".into(), String::new())], "each pair once");
    let bob_saved = backend.load(bob_user, 1, bob).await.unwrap().unwrap();
    assert_eq!(listed(&bob_saved.friends), [(ann, "Ann".into(), String::new())]);
    assert_eq!(listed(&bob_saved.blocks), [(cid, "Cid".into(), "rude".into())]);

    // Cid deleted: left out, but kept for a restore.
    assert!(backend.request_delete(cid_user, 1, cid).await.unwrap());
    assert_eq!(backend.load(ann_user, 1, ann).await.unwrap().unwrap().friends.len(), 1);
    assert!(backend.load(bob_user, 1, bob).await.unwrap().unwrap().blocks.is_empty());
    assert!(backend.restore(cid_user, 1, cid).await.unwrap());
    assert_eq!(backend.load(ann_user, 1, ann).await.unwrap().unwrap().friends.len(), 2);

    backend.write_friends(&FriendWrite::RemoveFriend { character: ann, friend: bob }).await.unwrap();
    backend.write_friends(&FriendWrite::RemoveBlock { character: bob, blocked: cid }).await.unwrap();
    assert_eq!(backend.load(ann_user, 1, ann).await.unwrap().unwrap().friends.iter().map(|f| f.id).collect::<Vec<_>>(), [cid]);
    let bob_saved = backend.load(bob_user, 1, bob).await.unwrap().unwrap();
    assert_eq!((bob_saved.friends.len(), bob_saved.blocks.len()), (1, 0), "removing is one-sided");
    db.drop().await;
}

#[tokio::test]
async fn progress_is_saved() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Climber")).await.unwrap() else { panic!("not created") };
    let progress = Progress { level: 2, exp: 144, stat_point: 5, skill_point: 8, stats: [15, 12, 9, 8, 7, 15] };
    backend.save_progress(id, &progress, [260, 90, 200]).await.unwrap();
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!((play.level, play.exp, play.stat_point, play.skill_point, play.stats), (2, 144, 5, 8, [15, 12, 9, 8, 7, 15]));
    assert_eq!(play.health, [260, 90, 200]);
    // Honor (Hg), as an Increase Fame item leaves it; above i16::MAX it comes back as stored.
    backend.save_honor(id, 40000).await.unwrap();
    assert_eq!(backend.load(alice, 1, id).await.unwrap().unwrap().honor, 40000);
    db.drop().await;
}

#[tokio::test]
async fn pvp_rewards_and_the_stat_reset_are_saved_with_their_steps() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Climber")).await.unwrap() else { panic!("not created") };
    let play = || async { backend.load(alice, 1, id).await.unwrap().unwrap() };
    let money = play().await.money;

    // The death reward: the gold and the step together, once per step (audit R12).
    assert!(backend.claim_death_reward(id, 500, 0, 1).await.unwrap());
    let after = play().await;
    assert_eq!((after.money, after.kill_steps), (money + 500, [0, 1]));
    assert!(!backend.claim_death_reward(id, 500, 0, 1).await.unwrap(), "the same step again: nothing");
    assert_eq!(play().await.money, money + 500);
    sqlx::query("UPDATE characters SET money = $2 WHERE id = $1")
        .bind(id as i32)
        .bind(i64::from(u32::MAX - 10))
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(!backend.claim_death_reward(id, 11, 1, 2).await.unwrap(), "past 32 bits: nothing, the step is not taken");
    let after = play().await;
    assert_eq!((after.money, after.kill_steps), (u32::MAX - 10, [0, 1]));
    assert!(backend.claim_death_reward(id, 10, 1, 2).await.unwrap());
    assert_eq!((play().await.money, play().await.kill_steps), (u32::MAX, [0, 2]));

    // The kill reward: the points (the new total) and the step.
    let points = play().await.stat_point;
    assert!(backend.claim_kill_reward(id, points + 3, 0, 1).await.unwrap());
    let after = play().await;
    assert_eq!((after.stat_point, after.kill_steps), (points + 3, [1, 2]));
    assert!(!backend.claim_kill_reward(id, points + 6, 0, 1).await.unwrap());
    assert_eq!(play().await.stat_point, points + 3);

    // The stat reset: points, base stats and the first reward step back to 0 in one write.
    let progress = Progress { level: 2, exp: 144, stat_point: 9, skill_point: 8, stats: [10, 10, 10, 10, 10, 10] };
    backend.save_stat_reset(id, &progress, [260, 90, 200], [0, 2]).await.unwrap();
    let after = play().await;
    assert_eq!((after.stat_point, after.stats, after.kill_steps, after.health), (9, [10; 6], [0, 2], [260, 90, 200]));
    db.drop().await;
}

#[tokio::test]
async fn skills_and_quick_slots_are_saved() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Learner")).await.unwrap() else { panic!("not created") };
    backend.learn(id, 3, Skill { id: 10, level: 1, number: 0, delay: 0 }).await.unwrap();
    backend.learn(id, 2, Skill { id: 7, level: 1, number: 1, delay: 0 }).await.unwrap();
    // A higher level replaces the row under its number.
    backend.learn(id, 0, Skill { id: 10, level: 2, number: 0, delay: 0 }).await.unwrap();
    let first = [QuickSlot { bar: 0, slot: 0, bag: 100, number: 0 }, QuickSlot { bar: 0, slot: 1, bag: 1, number: 3 }];
    backend.save_quick_slots(id, &first).await.unwrap();
    let second = [QuickSlot { bar: 2, slot: 4, bag: 100, number: 1 }];
    backend.save_quick_slots(id, &second).await.unwrap();
    // The cooldowns left, by number.
    backend
        .save_cooldowns(id, &[Skill { id: 10, level: 2, number: 0, delay: 0 }, Skill { id: 7, level: 1, number: 1, delay: 25 }])
        .await
        .unwrap();
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(play.skill_point, 0);
    assert_eq!(play.skills, [Skill { id: 10, level: 2, number: 0, delay: 0 }, Skill { id: 7, level: 1, number: 1, delay: 25 }]);
    assert_eq!(play.quick_slots, second, "replaced as a whole");
    assert!(!play.free_skill_reset_used);
    // A skill reset: every skill gone, the points set; the free one is remembered and stays used.
    backend.reset_skills(id, 32, true).await.unwrap();
    backend.reset_skills(id, 32, false).await.unwrap();
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!((play.skill_point, play.skills.len(), play.free_skill_reset_used), (32, 0, true));
    assert_eq!(play.quick_slots, second, "the quick slots stay");
    db.drop().await;
}

#[tokio::test]
async fn quests_are_started_counted_ended_and_timed() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Quester")).await.unwrap() else { panic!("not created") };
    let letter = NewItem { bag: 1, slot: 1, item_id: 27030, item_type: 27, type_id: 30, quality: 0, count: 5, kept: None };

    // Started with its item, together; a taken slot refuses the whole start.
    assert!(backend.start_quest(id, 41, Some(3600), &[SlotChange::Insert(letter)]).await.unwrap().is_some());
    assert!(!backend.start_quest(id, 42, None, &[SlotChange::Insert(letter)]).await.unwrap().is_some());
    assert!(backend.start_quest(id, 7, None, &[]).await.unwrap().is_some());
    backend.quest_counts(id, 7, [3, 1, 0]).await.unwrap();
    backend.save_quest_times(id, &[(41, 1234)]).await.unwrap();
    let quests = |play: &game_server::backend::PlayCharacter| {
        play.quests.iter().map(|q| (q.quest_id, q.finished, q.success, q.counts, q.seconds_left)).collect::<Vec<_>>()
    };
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(quests(&play), [(7, false, false, [3, 1, 0], None), (41, false, false, [0; 3], Some(1234))]);
    assert!(play.items.iter().any(|i| (i.bag, i.slot, i.count) == (1, 1, 5)));

    // Ended with the items taken, the money and a reward, together; ending it again is refused, as is an end whose item is gone.
    let reward = NewItem { bag: 1, slot: 2, item_id: 25001, item_type: 25, type_id: 1, quality: 0, count: 1, kept: None };
    let end = QuestEnd {
        quest_id: 41,
        success: true,
        keep: true,
        money: 500,
        changes: vec![SlotChange::Remove { bag: 1, slot: 1 }, SlotChange::Insert(reward)],
    };
    assert!(
        !backend.end_quest(id, &QuestEnd { changes: vec![SlotChange::Remove { bag: 1, slot: 9 }], ..end.clone() }).await.unwrap().is_some()
    );
    assert!(backend.end_quest(id, &end).await.unwrap().is_some());
    assert!(!backend.end_quest(id, &QuestEnd { changes: Vec::new(), ..end.clone() }).await.unwrap().is_some());
    // A repeatable quest leaves no row; a failure stays as finished.
    assert!(
        backend
            .end_quest(id, &QuestEnd { quest_id: 7, success: false, keep: false, money: 0, changes: Vec::new() })
            .await
            .unwrap()
            .is_some()
    );
    assert!(backend.start_quest(id, 9, None, &[]).await.unwrap().is_some());
    assert!(
        backend
            .end_quest(id, &QuestEnd { quest_id: 9, success: false, keep: true, money: 0, changes: Vec::new() })
            .await
            .unwrap()
            .is_some()
    );
    let play = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!(quests(&play), [(9, true, false, [0; 3], None), (41, true, true, [0; 3], None)]);
    assert_eq!(play.money, 500);
    let slots: Vec<_> = play.items.iter().map(|i| (i.bag, i.slot, i.item_type)).collect();
    assert_eq!(slots, [(0, 5, 5), (1, 0, 25), (1, 2, 25)]);
    db.drop().await;
}

#[tokio::test]
async fn enchant_tables_load() {
    // The rows as the loader turns them into a table (docs/plans/enchant-table.md): none, 21, anything else refused.
    assert_eq!(pg::enchant_table(&[]).unwrap(), None, "no rows: the caller keeps the built-in tables");
    let rows: Vec<(i16, i32, i32, i16, i16)> = (0..21).map(|s| (s, 900_000 - i32::from(s), 990_000, 7 * s, 5 * s)).collect();
    let table = pg::enchant_table(&rows).unwrap().expect("21 rows");
    assert_eq!((table.rate(0, false), table.add(20, false), table.add(20, true)), (900_000, 140, 100));
    assert!(pg::enchant_table(&rows[..20]).is_err(), "20 rows");
    let mut negative = rows.clone();
    negative[3].4 = -1;
    assert!(pg::enchant_table(&negative).is_err(), "a negative value");
    let Some(db) = TestDb::create().await else { return };
    // `world` is imported separately (tools/world/import.py); migration 0035 adds the table to a `world` built before it.
    let present: bool = sqlx::query_scalar("SELECT to_regclass('world.item_enchant') IS NOT NULL").fetch_one(&db.pool).await.unwrap();
    if present {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM world.item_enchant").fetch_one(&db.pool).await.unwrap();
        let loaded = pg::load_item_enchant(&db.pool).await.unwrap();
        assert_eq!(loaded.is_some(), count == 21, "{count} rows");
    }
    db.drop().await;
}

#[tokio::test]
async fn world_and_item_definitions_load() {
    let Some(db) = TestDb::create().await else { return };
    // `world` and `defs` are imported separately (tools/world, tools/defs); skip if this database does not have them.
    let present: bool = sqlx::query_scalar("SELECT to_regclass('world.maps') IS NOT NULL AND to_regclass('defs.items') IS NOT NULL")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    if present {
        let world = pg::load_world(&db.pool, false).await.unwrap();
        assert_eq!(world.map_count(), 68);
        assert_eq!(world.npcs().values().map(Vec::len).sum::<usize>(), 1724);
        // GuildHouse.ini (docs/original/guild-house.md section 2): the file's general goods costs, the built-in potion merchant's,
        // Fury's level-2 weapons merchant, and the merchants' shop types on map 51.
        let house = world.guild_house();
        assert_eq!((house.buy_money, house.keep_etin), (50_000_000, 2000));
        assert_eq!(house.npcs[&0x0301].upgrade, [150, 200, 400]);
        assert_eq!(house.npcs[&0x0401].keep, [50, 200, 500]);
        assert_eq!(house.levels[&(1, 1, 2)], 209);
        assert_eq!(house.mix_bonus, [0, 0, 1, 1, 2, 2]);
        assert_eq!([195, 199, 203, 204].map(|id| world.shop_type(id)), [1, 2, 4, 3]);
        // The NPCs with a walk (docs/original/guards.md section 1): 229 guards, 49 type 9, 10 type 7, 4 type 10, a merchant, a
        // blacksmith; the guards' moving numbers.
        let walking = world.npcs().values().flatten().filter(|n| world.npc_walk(n.object_id).is_some()).count();
        assert_eq!(walking, 294);
        let guard = |id| world.npc_def(8, id);
        assert_eq!((guard(1).move_dist, guard(1).move_speed), (16, 8000));
        assert_eq!((guard(102).move_dist, guard(102).move_speed), (0, 0));
        assert!(world.shop(1).is_some_and(|goods| !goods.is_empty()), "merchant 1 sells something");
        // Gatekeeper 1's first destination: map 19, y as in the file plus the 0.9 the original adds; its third is empty.
        let keeper = world.gatekeeper(1).expect("gatekeeper 1");
        let first = keeper.gates[0].expect("a first destination");
        assert_eq!((first.map, first.cost), (19, 8600));
        let stored: f32 =
            sqlx::query_scalar("SELECT pos_y FROM world.npc_gates WHERE npc_type_id = 1 AND seq = 0").fetch_one(&db.pool).await.unwrap();
        assert_eq!(first.position[1], stored + 0.9);
        assert!(world.gatekeeper(2).expect("gatekeeper 2").gates[2].is_none());
        // Portal 5 of map 1, the one the US client used in the first live test: Light only, to map 1 near the human start.
        let portal = world.portal(1, 5).expect("map 1 portal 5");
        assert_eq!((portal.country, portal.to_map, portal.to[0].round()), (1, 1, 542.0));
        assert_eq!((world.create_type(64), world.create_type(51), world.create_type(1)), (3, 6, 0));
        // The eight Lua scripts load as shipped (docs/original/instances.md section 16).
        let scripts = pg::load_mob_lua(&db.pool).await.unwrap();
        assert_eq!(scripts.0.keys().copied().collect::<std::collections::BTreeSet<_>>(), (111..=118).collect());
        for (ai, source) in &scripts.0 {
            let lua = game_server::mob_lua::load(source, &format!("{ai}.lua"), 1).unwrap_or_else(|e| panic!("{ai}.lua: {e}"));
            assert!(lua.globals().get::<mlua::Function>("WhileCombat").is_ok(), "{ai}.lua has its hooks");
        }
        // Party instance maps (docs/original/instances.md section 1): Cailum Sacra always open for 1 to 7, map 64 an evening raid.
        let (sacra, raid) = (world.instance_rules(65), world.instance_rules(64));
        assert_eq!((sacra.open_times.len(), sacra.min_users, sacra.max_users, sacra.expire.as_secs()), (0, 1, 7, 330));
        assert_eq!((raid.open_times.len(), raid.open_times[0], raid.min_users, raid.max_users), (6, [1, 21, 1], 18, 30));
        // Monsters: one respawn pass spawns one per area entry that can spawn and finds a free spot.
        let mut mobs = pg::load_mob_defs(&db.pool).await.unwrap();
        let zones = game_server::zone::Zones::new(&world, &mobs);
        let mut seed = 12345u32;
        let mut rand = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) & 0x7fff
        };
        let spawned = zones.respawn(std::time::Instant::now(), &world, &mobs, &mut rand);
        eprintln!("first respawn pass: {spawned} monsters, {} on map 1", zones.mob_count(1));
        assert!(spawned > 5000, "{spawned}");
        let skills = pg::load_skill_defs(&db.pool).await.unwrap();
        assert_eq!(skills.len(), 3 * 326 + 355, "levels 1 to 3, and the monsters' 100");
        // 1,873 monster entries name a skill; a hiding (kind 4) and a kind-7 row are cleared (docs/original/mob-skills.md section 1).
        assert_eq!(mobs.attach_skills(&skills), 1871);
        // Skill 4 (bags): a passive of kind 2 giving 3, 4 and 5 bags.
        let bags: Vec<_> = (1..=3).map(|level| skills.get(4, level).map(|d| (d.type_effect, d.kind, d.bags))).collect();
        assert_eq!(bags, [Some((0, 2, 3)), Some((0, 2, 4)), Some((0, 2, 5))]);
        // Skill 29 level 1 (a magic attack on one monster): type attack 3 stored as magic (2), 29 damage by the formula, 20 MP, a
        // second's cast, 24 more reach, for weapons of real types 1, 7 to 10 among others.
        let ball = skills.get(29, 1).expect("skill 29");
        assert_eq!((ball.attack_kind, ball.type_effect, ball.target_type, ball.kind), (2, 1, 3, 21));
        assert_eq!((ball.damage_type, ball.damage, ball.need_sp, ball.need_mp), (1, [29, 0, 0], 0, 20));
        assert_eq!((ball.ready_time, ball.reset_time, ball.attack_range), (4, 1, 24));
        assert!(ball.need_weapons[0] && ball.need_weapons[6..10].iter().all(|&w| w));
        // Monster 40: SP and MP, a resisted state and a resisted skill.
        let m = mobs.get(40).expect("monster 40");
        assert_eq!((m.sp, m.mp, m.resist_states[0], m.resist_skills[0]), (1160, 2366, true, 87));
        // Quests (docs/original/quest.md section 1): 1,720, quest 1 kills five of monster 2008 for a guard captain, quest 41 a
        // "bring more for more" quest.
        let quests = pg::load_quest_defs(&db.pool).await.unwrap();
        assert_eq!(quests.len(), 1720);
        let first = quests.get(1).unwrap();
        assert_eq!(
            (first.start_type, first.start_npc, first.end_type, first.end_npc, first.mob_kills[0]),
            (1, (8, 83), 2, (8, 87), (2008, 5))
        );
        assert_eq!((first.results[0].exp, first.results[0].money, first.exp(0)), (50, 330, 100));
        let more = quests.get(41).unwrap();
        assert_eq!((more.end_items[0], more.time_limit, more.results.map(|r| r.need_item)), ((27, 30, 5), 60, [(1, 5), (1, 10), (1, 15)]));
        let items = pg::load_item_defs(&db.pool).await.unwrap();
        let potion = items.get(25, 1).expect("item 25001");
        assert_eq!(potion.item_id, 25001);
        assert!(potion.max_count > 1);
        // The Kill Boost Stones' rows (the extension `kill-boost` of shaiya-afterglow-extensions, installed with `data-editor ext install`:
        // `docs/server/deviations.md` section 1): the kill-boost stone 100/212 gives skill 200/1, a buff with no ability lasting an hour and
        // kept on death (`docs/original/operator-console.md` decision 3).
        let stone = items.get(100, 212).expect("item 100212");
        if game_server::item_use::effect(stone) == (game_server::item_use::Effect::Skill { id: 200, level: 1 }) {
            let boost = skills.get(200, 1).expect("skill 200/1");
            assert_eq!(
                (boost.type_effect, boost.kind, boost.keep_time, boost.keep_on_death),
                (game_server::skills::BUFF, game_server::buffs::ABILITIES, 3600, true)
            );
            assert_eq!(boost.abilities, [(0, 0); 3]);
        } else {
            eprintln!(
                "the kill-boost extension is not installed (data-editor ext install <shaiya-afterglow-extensions>/kill-boost): skipping the kill-boost rows"
            );
        }
        assert_eq!(skills.get(200, 100).map(|curse| curse.kind), Some(62), "the monsters' Weakening Curse is left as it was");
    } else {
        eprintln!("world or defs not loaded: skipping");
    }
    db.drop().await;
}

#[tokio::test]
async fn faction_kill_counts_are_kept_per_server() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    assert_eq!(backend.load_kill_counts(1).await.unwrap(), [0, 0], "never saved");
    backend.save_kill_counts(1, [150, 4_000_000_000]).await.unwrap();
    backend.save_kill_counts(2, [7, 0]).await.unwrap();
    backend.save_kill_counts(1, [151, 4_000_000_000]).await.unwrap();
    assert_eq!(backend.load_kill_counts(1).await.unwrap(), [151, 4_000_000_000]);
    assert_eq!(backend.load_kill_counts(2).await.unwrap(), [7, 0]);
    db.drop().await;
}

#[tokio::test]
async fn the_monster_id_counter_is_kept_per_server() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    for (id, address) in [(1i16, "10.0.0.1"), (2, "10.0.0.2")] {
        sqlx::query("INSERT INTO game_servers (id, name, address) VALUES ($1, 'Test', $2::inet)")
            .bind(id)
            .bind(address)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    assert_eq!(backend.load_mob_id(1).await.unwrap(), 1, "never saved, as a missing MobID.ini");
    assert_eq!(backend.load_mob_id(9).await.unwrap(), 1, "no such server");
    backend.save_mob_id(1, 4_000_000_000).await.unwrap();
    backend.save_mob_id(2, 77).await.unwrap();
    assert_eq!(backend.load_mob_id(1).await.unwrap(), 4_000_000_000);
    assert_eq!(backend.load_mob_id(2).await.unwrap(), 77);
    backend.save_mob_id(1, u32::MAX).await.unwrap();
    assert_eq!(backend.load_mob_id(1).await.unwrap(), u32::MAX);
    assert!(backend.save_mob_id(9, 5).await.is_err(), "unregistered server");
    db.drop().await;
}

#[tokio::test]
async fn every_monster_ai_parses() {
    let Some(db) = TestDb::create().await else { return };
    let present: bool = sqlx::query_scalar("SELECT to_regclass('world.mob_ai') IS NOT NULL").fetch_one(&db.pool).await.unwrap();
    if present {
        let (library, failed) = pg::load_mob_ai(&db.pool).await.unwrap();
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(library.machines.len(), 27, "mob_ai.ini's 10 to 36");
        let croco = &library.machines[&11];
        assert_eq!((croco.name.as_str(), croco.states.len(), croco.states[croco.start].name.as_str()), ("Croco1", 4, "init"));
        // Every function call the AIs make is one the dispatcher knows.
        for (ai, m) in &library.machines {
            for s in &m.states {
                let conditions =
                    s.on_enter.iter().chain(&s.on_update).chain(&s.on_exit).chain(s.transitions.iter().flat_map(|t| &t.conditions));
                for c in conditions {
                    for t in &c.tokens {
                        if let game_server::mob_ai::Token::Function { func, .. } = t {
                            assert!(func.is_some(), "AI {ai} state {}", s.name);
                        }
                    }
                }
            }
        }
        assert_eq!(library.lines.get(&5).map(Vec::as_slice), Some(&b"Die, fool!"[..]), "the English of deviation 1");
    } else {
        eprintln!("world.mob_ai not loaded: skipping");
    }
    db.drop().await;
}

#[tokio::test]
async fn operator_commands_read_and_write_accounts_guilds_quests_and_events() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("Alice").await;
    let staff = db.account("staff").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(staff, 1, 0).await.unwrap();
    let CreateOutcome::Created(hero_id) = backend.create(&hero(alice, 0, "Hero")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(boss) = backend.create(&hero(staff, 0, "Boss")).await.unwrap() else { panic!("not created") };

    // Staff levels: the status word, as the login server reads it.
    backend.set_account_status(alice, 32).await.unwrap();
    let status: i16 = sqlx::query_scalar("SELECT status FROM accounts WHERE id = $1").bind(alice as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(status, 32);
    assert_eq!(backend.account_id("aLiCe").await.unwrap(), Some((alice, 32)), "ignoring case");
    assert_eq!(backend.account_id("nobody").await.unwrap(), None);
    assert_eq!(backend.character_account("hERO").await.unwrap(), Some((alice, 32)));

    // Marks round trip through load.
    backend.save_marks(hero_id, Marks { grow: 2, vg: 300, cg: 255, og: 7, ig: 9 }).await.unwrap();
    let play = backend.load(alice, 1, hero_id).await.unwrap().unwrap();
    assert_eq!((play.grow, play.vg, play.cg, play.og, play.ig), (2, 300, 255, 7, 9));

    // Guild points and standing.
    sqlx::query("UPDATE characters SET money = 1000 WHERE id = $1").bind(boss as i32).execute(&db.pool).await.unwrap();
    let GuildCreated::Created(guild) = backend
        .create_guild(&NewGuild {
            server_id: 1,
            name: "Builders".into(),
            country: 0,
            remark: String::new(),
            members: vec![(boss, 1)],
            payer: boss,
            cost: 1000,
        })
        .await
        .unwrap()
    else {
        panic!("not created")
    };
    backend.write_guild(&GuildWrite::Points { guild, points: 100 }).await.unwrap();
    backend.write_guild(&GuildWrite::Standing { guild, ranking: 5, use_house: 1 }).await.unwrap();
    let house = backend.load_guilds(1).await.unwrap().remove(0).house;
    assert_eq!((house.points, house.ranking, house.use_house), (100, 5, 1));

    // Account bans: with an end and without, then lifted; a staff account is not a block.
    backend.block_account(alice, Some(5)).await.unwrap();
    let (status, ends_in_minutes): (i16, Option<f64>) =
        sqlx::query_as("SELECT status, (extract(epoch FROM blocked_until - now()) / 60)::float8 FROM accounts WHERE id = $1")
            .bind(alice as i32)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(status, 2);
    assert!(ends_in_minutes.is_some_and(|m| (4.0..=5.0).contains(&m)), "{ends_in_minutes:?}");
    assert!(backend.lift_block(alice).await.unwrap());
    assert!(!backend.lift_block(alice).await.unwrap(), "no longer blocked");
    backend.block_account(alice, None).await.unwrap();
    let ends: Option<f64> = sqlx::query_scalar("SELECT extract(epoch FROM blocked_until)::float8 FROM accounts WHERE id = $1")
        .bind(alice as i32)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(ends, None, "no end");
    assert!(backend.lift_block(alice).await.unwrap());
    let status: i16 = sqlx::query_scalar("SELECT status FROM accounts WHERE id = $1").bind(alice as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(status, 0);
    backend.set_account_status(alice, 16).await.unwrap();
    assert!(!backend.lift_block(alice).await.unwrap(), "a staff account (status 16) is left alone");
    assert_eq!(backend.character_account("Hero").await.unwrap(), Some((alice, 16)));

    // A deleted character is not found.
    sqlx::query("UPDATE characters SET deleted_at = now() WHERE id = $1").bind(hero_id as i32).execute(&db.pool).await.unwrap();
    assert_eq!(backend.character_account("Hero").await.unwrap(), None);
    assert_eq!(backend.character_id("Hero").await.unwrap(), None);
    assert_eq!(backend.character_id("BOSS").await.unwrap(), Some(boss));

    // Closed quests.
    assert!(backend.closed_quests(1).await.unwrap().is_empty());
    backend.set_quest_closed(1, 12, true).await.unwrap();
    backend.set_quest_closed(1, 7, true).await.unwrap();
    backend.set_quest_closed(1, 7, true).await.unwrap();
    backend.set_quest_closed(2, 99, true).await.unwrap();
    assert_eq!(backend.closed_quests(1).await.unwrap(), [7, 12]);
    backend.set_quest_closed(1, 7, false).await.unwrap();
    assert_eq!(backend.closed_quests(1).await.unwrap(), [12]);

    // Event tags: once, whatever the case; removed once.
    assert!(backend.add_event(boss, "Winners").await.unwrap());
    assert!(!backend.add_event(boss, "Winners").await.unwrap());
    assert!(!backend.add_event(boss, "WINNERS").await.unwrap());
    assert!(backend.remove_event(boss, "winners").await.unwrap());
    assert!(!backend.remove_event(boss, "winners").await.unwrap());
    db.drop().await;
}

#[tokio::test]
async fn user_questions_are_stored_taken_answered_and_loaded_open() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let mut ids = Vec::new();
    for name in ["Asker", "Staff", "Other"] {
        let user = db.account(&name.to_lowercase()).await;
        backend.choose_faction(user, 1, 0).await.unwrap();
        let CreateOutcome::Created(id) = backend.create(&hero(user, 0, name)).await.unwrap() else { panic!("not created") };
        ids.push(id);
    }
    let [asker, gm, other] = ids[..] else { unreachable!() };
    assert!(backend.open_questions(1).await.unwrap().is_empty());

    // Add, take, release, take again, answer.
    let text = b"where is the blacksmith?\0";
    let first = backend.add_question(1, asker, "Asker", text).await.unwrap();
    let second = backend.add_question(1, other, "Other", b"\xb0\xa1 hi\0").await.unwrap();
    let elsewhere = backend.add_question(2, other, "Other", b"another server\0").await.unwrap();
    assert!(first < second && second < elsewhere);
    backend.take_question(first, gm).await.unwrap();
    let taken: Option<i32> =
        sqlx::query_scalar("SELECT taken_by FROM user_questions WHERE id = $1").bind(first as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(taken, Some(gm as i32));
    backend.release_question(first).await.unwrap();
    let taken: Option<i32> =
        sqlx::query_scalar("SELECT taken_by FROM user_questions WHERE id = $1").bind(first as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(taken, None);
    backend.take_question(first, gm).await.unwrap();
    backend.answer_question(first, gm, 4, b"at the forge\0", Some("Asker")).await.unwrap();
    let row: (Option<i32>, i16, Vec<u8>, String, bool) =
        sqlx::query_as("SELECT answered_by, answer_kind, answer, answer_to, answered_at IS NOT NULL FROM user_questions WHERE id = $1")
            .bind(first as i32)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(row, (Some(gm as i32), 4, b"at the forge\0".to_vec(), "Asker".to_owned(), true));

    // Only the unanswered ones of this server load, oldest first, with the hold cleared.
    let third = backend.add_question(1, asker, "Asker", b"later\0").await.unwrap();
    backend.take_question(second, gm).await.unwrap();
    let open = backend.open_questions(1).await.unwrap();
    assert_eq!(open.iter().map(|q| q.id).collect::<Vec<_>>(), [second, third]);
    assert_eq!((open[0].character_id, open[0].name.as_str(), open[0].text.as_slice()), (other, "Other", &b"\xb0\xa1 hi\0"[..]));
    assert!(open[0].asked_unix > 1_700_000_000);
    let taken: Option<i32> =
        sqlx::query_scalar("SELECT taken_by FROM user_questions WHERE id = $1").bind(second as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(taken, None, "taken_by is cleared at load");
    let again: Option<i32> =
        sqlx::query_scalar("SELECT taken_by FROM user_questions WHERE id = $1").bind(elsewhere as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(again, None);

    // An answer is final.
    backend.answer_question(first, other, 1, b"again\0", None).await.unwrap();
    let kind: i16 =
        sqlx::query_scalar("SELECT answer_kind FROM user_questions WHERE id = $1").bind(first as i32).fetch_one(&db.pool).await.unwrap();
    assert_eq!(kind, 4);

    // A deleted character's questions go with it.
    sqlx::query("DELETE FROM characters WHERE id = $1").bind(other as i32).execute(&db.pool).await.unwrap();
    assert_eq!(backend.open_questions(1).await.unwrap().iter().map(|q| q.id).collect::<Vec<_>>(), [third]);
    db.drop().await;
}

#[tokio::test]
async fn the_market_moves_items_and_gold_between_bags_auctions_and_rows() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(bob, 1, 0).await.unwrap();
    let CreateOutcome::Created(seller) = backend.create(&hero(alice, 0, "Seller")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(buyer) = backend.create(&hero(bob, 0, "Buyer")).await.unwrap() else { panic!("not created") };
    sqlx::query("UPDATE characters SET money = 1000 WHERE id = $1").bind(seller as i32).execute(&db.pool).await.unwrap();
    sqlx::query("UPDATE characters SET money = 10000 WHERE id = $1").bind(buyer as i32).execute(&db.pool).await.unwrap();
    assert!(backend.trade(seller, 0, &[SlotChange::Move { from: (0, 5), to: (1, 1) }]).await.unwrap().is_some());
    let uid = |table: &'static str, key: &'static str, id: i64| {
        let pool = db.pool.clone();
        async move {
            let sql = format!("SELECT uid FROM {table} WHERE {key} = $1");
            sqlx::query_scalar::<_, i64>(AssertSqlSafe(sql)).bind(id).fetch_optional(&pool).await.unwrap()
        }
    };
    let bag_uid = |character: u32, bag: i16, slot: i16| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT uid FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3")
                .bind(character as i32)
                .bind(bag)
                .bind(slot)
                .fetch_optional(&pool)
                .await
                .unwrap()
        }
    };
    let money = |character: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT money FROM characters WHERE id = $1")
                .bind(character as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let (potion_uid, sword_uid) = (bag_uid(seller, 1, 0).await.unwrap(), bag_uid(seller, 1, 1).await.unwrap());
    let potion = MarketItem { item_type: 25, type_id: 1, count: 3, ..Default::default() };
    let sword = MarketItem { item_type: 5, type_id: 1, quality: 1200, count: 1, ..Default::default() };
    let auction = |item: &MarketItem| {
        MarketOp::NewAuction(NewAuction {
            server_id: 1,
            seller,
            seller_name: "Seller".into(),
            ends_at: 1_800_000_000_000_000,
            min: 1000,
            direct: 5000,
            period: 1,
            deposit: 120,
            item: item.clone(),
        })
    };

    // Part of a stack: a new row (a new uid) for the auction, the stack lowered, the fee paid. A wrong count changes nothing.
    let wrong = MarketWrite { character: seller, money: -120, take: Some((1, 0, 9, 3)), ops: vec![auction(&potion)], ..Default::default() };
    assert_eq!(backend.market(&wrong).await.unwrap(), None);
    assert_eq!(money(seller).await, 1000);
    let register = MarketWrite { take: Some((1, 0, 10, 3)), ..wrong };
    let written = backend.market(&register).await.unwrap().unwrap();
    let [first] = written.auction_ids[..] else { panic!("{written:?}") };
    let first_uid = uid("market_auctions", "id", first.into()).await.unwrap();
    assert_ne!(first_uid, potion_uid);
    assert_eq!(bag_uid(seller, 1, 0).await, Some(potion_uid));
    let count: i16 =
        sqlx::query_scalar("SELECT count FROM character_items WHERE uid = $1").bind(potion_uid).fetch_one(&db.pool).await.unwrap();
    assert_eq!((count, money(seller).await), (7, 880));

    // The whole item: its row moves, uid kept.
    let whole = MarketWrite { character: seller, money: -120, take: Some((1, 1, 1, 1)), ops: vec![auction(&sword)], ..Default::default() };
    let [second] = backend.market(&whole).await.unwrap().unwrap().auction_ids[..] else { panic!() };
    assert_eq!(uid("market_auctions", "id", second.into()).await, Some(sword_uid));
    assert_eq!(bag_uid(seller, 1, 1).await, None);

    // A bid, then the end with it: the item to the bidder, the gold row to the seller, in one write.
    let bid = |id: u32| MarketOp::SetBid { id, bidder: buyer, name: "Buyer".into(), price: 1050 };
    assert!(
        backend
            .market(&MarketWrite { character: buyer, money: -1050, ops: vec![bid(first)], ..Default::default() })
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        backend.market(&MarketWrite { character: buyer, money: -1050, ops: vec![bid(999)], ..Default::default() }).await.unwrap(),
        None
    );
    assert_eq!(money(buyer).await, 8950);
    let (auctions, items, rows) = backend.load_market(1).await.unwrap();
    assert_eq!(
        auctions.iter().map(|a| (a.id, a.price, a.bidder.clone())).collect::<Vec<_>>(),
        [(first, 1050, Some((buyer, "Buyer".into()))), (second, 1000, None)]
    );
    assert_eq!((auctions[1].item.quality, auctions[0].item.count, items.len(), rows.len()), (1200, 3, 0, 0));
    let deadline = 1_900_000_000_000_000;
    let gold = NewMoneyRow {
        market_id: first,
        character: seller,
        result: 0,
        money: 1050,
        deposit: 120,
        return_money: 1149,
        deadline,
        item: potion.clone(),
    };
    let end = MarketWrite {
        ops: vec![MarketOp::EndAuction { id: first, character: buyer, result: 0, deadline }, MarketOp::NewMoneyRow(gold)],
        ..Default::default()
    };
    let [money_id] = backend.market(&end).await.unwrap().unwrap().money_ids[..] else { panic!() };
    assert_eq!(uid("market_result_items", "market_id", first.into()).await, Some(first_uid));
    assert_eq!(uid("market_auctions", "id", first.into()).await, None);
    let (auctions, items, rows) = backend.load_market(1).await.unwrap();
    assert_eq!(
        (auctions.len(), items[0].character, items[0].deadline, rows[0].id, rows[0].return_money),
        (1, buyer, deadline, money_id, 1149)
    );
    assert!(backend.load_market(2).await.unwrap().0.is_empty(), "another server's auctions are not loaded");

    // Collecting: the item row into a free place (uid kept; a taken place or another's row refused), the gold with the row gone.
    let collect = |character: u32, slot: u8| MarketWrite { character, place: Some((first, 1, slot)), ..Default::default() };
    assert_eq!(backend.market(&collect(buyer, 0)).await.unwrap(), None, "a taken place");
    assert_eq!(backend.market(&collect(seller, 5)).await.unwrap(), None, "not the seller's row");
    assert!(backend.market(&collect(buyer, 3)).await.unwrap().is_some());
    assert_eq!(bag_uid(buyer, 1, 3).await, Some(first_uid));
    let get = MarketWrite { character: seller, money: 1149, ops: vec![MarketOp::DeleteMoneyRow(money_id)], ..Default::default() };
    assert!(backend.market(&get).await.unwrap().is_some());
    assert_eq!(backend.market(&get).await.unwrap(), None, "collected once");
    assert_eq!(money(seller).await, 760 + 1149);

    // A cancel gives the seller its item back, uid kept; the interest list loads with the character in order.
    let cancel =
        MarketWrite { ops: vec![MarketOp::EndAuction { id: second, character: seller, result: 1, deadline }], ..Default::default() };
    assert!(backend.market(&cancel).await.unwrap().is_some());
    assert!(backend.market(&MarketWrite { character: seller, place: Some((second, 2, 0)), ..Default::default() }).await.unwrap().is_some());
    assert_eq!(bag_uid(seller, 2, 0).await, Some(sword_uid));
    let (auctions, items, rows) = backend.load_market(1).await.unwrap();
    assert!(auctions.is_empty() && items.is_empty() && rows.is_empty());
    backend.save_interest(buyer, &[7, 3, 9]).await.unwrap();
    backend.save_interest(buyer, &[3, 9]).await.unwrap();
    assert_eq!(backend.load(bob, 1, buyer).await.unwrap().unwrap().interest, [3, 9]);
    db.drop().await;
}

#[tokio::test]
async fn renames_are_allowed_by_the_stone_and_made_with_the_old_name_kept() {
    // docs/original/small-items.md section 9.4.
    use game_server::backend::RenameOutcome;
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(bob, 2, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Ranger")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(other) = backend.create(&NewCharacter { server_id: 2, ..hero(bob, 0, "Priest") }).await.unwrap() else {
        panic!("not created")
    };
    // An auction of hers, on which she is the top bidder as well (as the rows allow), follows the new name.
    sqlx::query(
        "INSERT INTO market_auctions (server_id, seller_id, seller_name, ends_at, min_money, direct_money, period, deposit, bidder_id,
                                      bidder_name, price, item_id, type, type_id)
         VALUES (1, $1, 'Ranger', now() + interval '1 day', 1, 2, 1, 0, $1, 'Ranger', 1, 25001, 25, 1)",
    )
    .bind(id as i32)
    .execute(&db.pool)
    .await
    .unwrap();

    // No rename without the stone. The stone (here one of the 10 potions in bag 1 slot 0) and the count go together.
    assert_eq!(backend.rename(alice, 1, id, "Ranger2").await.unwrap(), RenameOutcome::NoRename);
    assert!(!backend.characters(alice, 1).await.unwrap()[0].rename_allowed);
    assert!(!backend.add_rename(id, &SlotChange::SetCount { bag: 1, slot: 1, count: 1 }).await.unwrap(), "an empty slot");
    assert!(backend.add_rename(id, &SlotChange::SetCount { bag: 1, slot: 0, count: 9 }).await.unwrap());
    assert!(backend.characters(alice, 1).await.unwrap()[0].rename_allowed);
    let count = |id: u32| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_scalar::<_, i16>("SELECT rename_count FROM characters WHERE id = $1")
                .bind(id as i32)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(count(id).await, 1);

    // Taken in any case, also on another server; not another account's character; then renamed.
    assert_eq!(backend.rename(alice, 1, id, "PRIEST").await.unwrap(), RenameOutcome::Taken);
    assert_eq!(backend.rename(bob, 2, id, "Ranger2").await.unwrap(), RenameOutcome::NotFound);
    assert_eq!(backend.rename(alice, 2, id, "Ranger2").await.unwrap(), RenameOutcome::NotFound, "another server");
    assert_eq!(backend.rename(alice, 1, id, "Ranger2").await.unwrap(), RenameOutcome::Renamed { old: "Ranger".into() });
    let (name, old, left): (String, Option<String>, i16) =
        sqlx::query_as("SELECT name, old_name, rename_count FROM characters WHERE id = $1")
            .bind(id as i32)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!((name.as_str(), old.as_deref(), left), ("Ranger2", Some("Ranger"), 0));
    let log: Vec<(i16, String, String)> =
        sqlx::query_as("SELECT server_id, old_name, new_name FROM character_renames WHERE character_id = $1")
            .bind(id as i32)
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(log, [(1, "Ranger".into(), "Ranger2".into())]);
    let names: (String, String) = sqlx::query_as("SELECT seller_name, bidder_name FROM market_auctions").fetch_one(&db.pool).await.unwrap();
    assert_eq!(names, ("Ranger2".into(), "Ranger2".into()));
    assert_eq!(backend.rename(alice, 1, id, "Ranger3").await.unwrap(), RenameOutcome::NoRename, "used up");
    // The old name is free again.
    assert!(matches!(backend.create(&hero(alice, 1, "Ranger")).await.unwrap(), CreateOutcome::Created(_)));

    // At 255 the stone is refused, with the slot untouched.
    sqlx::query("UPDATE characters SET rename_count = 255 WHERE id = $1").bind(other as i32).execute(&db.pool).await.unwrap();
    assert!(!backend.add_rename(other, &SlotChange::SetCount { bag: 1, slot: 0, count: 9 }).await.unwrap());
    let potions: i16 = sqlx::query_scalar("SELECT count FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = 0")
        .bind(other as i32)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!((count(other).await, potions), (255, 10));
    db.drop().await;
}

#[tokio::test]
async fn the_item_mall_takes_points_and_gives_items_together() {
    use game_server::backend::{Bought, GiftNotice, GiftOrder, GiftOutcome, PointStore, Premium, Purchase, Storage, StoredPointItem};
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    backend.choose_faction(alice, 1, 0).await.unwrap();
    backend.choose_faction(bob, 1, 1).await.unwrap();
    let CreateOutcome::Created(buyer) = backend.create(&hero(alice, 0, "Buyer")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(friend) = backend.create(&NewCharacter { family: 2, ..hero(bob, 0, "Friend") }).await.unwrap() else {
        panic!("not created")
    };
    sqlx::query("UPDATE accounts SET points = 500, premium = 2, premium_until = now() + interval '1 day' WHERE id = $1")
        .bind(alice as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_servers (id, name, address) VALUES (1, 'Test', '10.0.0.1'::inet)").execute(&db.pool).await.unwrap();
    db.session(alice, [3; 16], 1, "192.168.0.20").await;
    let claimed = backend.claim(&claim(alice, [3; 16], None)).await.unwrap().unwrap();
    assert_eq!(claimed.premium.kind, 2);
    assert!(claimed.premium.until.is_some_and(|until| until > 0));

    // A purchase: points, items and the log row together; short points change nothing.
    let stone = |bag, slot| NewItem {
        bag,
        slot,
        item_id: 100048,
        item_type: 100,
        type_id: 48,
        quality: 0,
        count: 1,
        kept: Some(Kept { make_type: b'X', ..Kept::default() }),
    };
    let purchase =
        Purchase { user_id: alice, character_id: buyer, code: "Stone".into(), price: 300, items: vec![stone(1, 1), stone(1, 2)] };
    let Bought::Done(200, bought_rows) = backend.buy(&purchase).await.unwrap() else { panic!("bought") };
    let uids: Vec<i64> =
        sqlx::query_scalar("SELECT uid FROM character_items WHERE character_id = $1 AND bag = 1 AND slot IN (1, 2) ORDER BY slot")
            .bind(buyer as i32)
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(bought_rows.iter().map(|r| r.uid).collect::<Vec<_>>(), uids, "the purchase names the rows it made");
    assert_eq!(backend.buy(&purchase).await.unwrap(), Bought::NoPoints);
    let taken = Purchase { price: 10, items: vec![stone(1, 0)], ..purchase.clone() };
    assert_eq!(backend.buy(&taken).await.unwrap(), Bought::Refused, "bag 1 slot 0 holds the potions");
    assert_eq!(backend.points(alice).await.unwrap(), 200, "the refused purchase was rolled back");
    let make: String = sqlx::query_scalar("SELECT make_type FROM character_items WHERE character_id = $1 AND bag = 1 AND slot = 2")
        .bind(buyer as i32)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(make, "X");

    // A gift to the other account (any case), then refusals.
    let order = GiftOrder {
        user_id: alice,
        server_id: 1,
        character_id: buyer,
        target: "friend".into(),
        code: "Stone".into(),
        price: 50,
        rows: vec![(100048, 1), (25001, 50)],
    };
    let rows = vec![StoredPointItem { slot: 0, item_id: 100048, count: 1 }, StoredPointItem { slot: 1, item_id: 25001, count: 50 }];
    assert_eq!(backend.gift(&order).await.unwrap(), GiftOutcome::Sent { points_left: 150, target_user: bob, rows: rows.clone() });
    assert_eq!(backend.gift(&GiftOrder { target: "Buyer".into(), ..order.clone() }).await.unwrap(), GiftOutcome::OwnAccount);
    assert_eq!(backend.gift(&GiftOrder { target: "Nobody".into(), ..order.clone() }).await.unwrap(), GiftOutcome::NoCharacter);
    assert_eq!(backend.gift(&GiftOrder { price: 1000, ..order.clone() }).await.unwrap(), GiftOutcome::NoPoints);
    assert_eq!(backend.gift(&GiftOrder { rows: vec![(100048, 1); 239], ..order.clone() }).await.unwrap(), GiftOutcome::StorageFull);
    assert_eq!(backend.points(alice).await.unwrap(), 150);

    // Loading: the points, premium, the buyer's log newest first, the recipient's storage.
    let me = backend.load(alice, 1, buyer).await.unwrap().unwrap();
    assert_eq!((me.mall.points, me.mall.premium.kind), (150, 2));
    assert_eq!(me.mall.log.iter().map(|l| (l.code.as_str(), l.points)).collect::<Vec<_>>(), [("Stone", 50), ("Stone", 300)]);
    let them = backend.load(bob, 1, friend).await.unwrap().unwrap();
    assert_eq!((them.mall.stored.clone(), them.mall.premium), (rows.clone(), Premium::default()));

    // Taking a row: the row out and the item in together; a wrong row or a taken place changes nothing.
    let potions = NewItem {
        bag: 1,
        slot: 3,
        item_id: 25001,
        item_type: 25,
        type_id: 1,
        quality: 0,
        count: 50,
        kept: Some(Kept { make_type: b'X', ..Kept::default() }),
    };
    assert!(!backend.take_stored(friend, Storage::Mall, StoredPointItem { count: 49, ..rows[1] }, &potions).await.unwrap().is_some());
    assert!(!backend.take_stored(friend, Storage::Mall, rows[1], &NewItem { slot: 0, ..potions }).await.unwrap().is_some());
    assert!(backend.take_stored(friend, Storage::Mall, rows[1], &potions).await.unwrap().is_some());
    assert!(
        !backend.take_stored(friend, Storage::Mall, rows[1], &NewItem { slot: 4, ..potions }).await.unwrap().is_some(),
        "already taken"
    );
    assert_eq!(backend.load(bob, 1, friend).await.unwrap().unwrap().mall.stored, rows[..1]);
    sqlx::query("INSERT INTO billing_items (user_id, slot, item_id, count) VALUES ($1, 7, 100048, 1)")
        .bind(bob as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(
        backend
            .take_stored(friend, Storage::Billing, StoredPointItem { slot: 7, item_id: 100048, count: 1 }, &stone(1, 5))
            .await
            .unwrap()
            .is_some()
    );

    // Notices: kept per account and server, read once, oldest first.
    for sender in ["A", "B"] {
        backend.add_gift_notice(bob, 1, &GiftNotice { sender: sender.into(), code: "Stone".into() }).await.unwrap();
    }
    backend.add_gift_notice(bob, 2, &GiftNotice { sender: "C".into(), code: "Stone".into() }).await.unwrap();
    let senders: Vec<_> = backend.take_gift_notices(bob, 1).await.unwrap().into_iter().map(|n| n.sender).collect();
    assert_eq!(senders, ["A", "B"]);
    assert!(backend.take_gift_notices(bob, 1).await.unwrap().is_empty());

    // The shipped product list (`defs` is imported separately; skipped without it): 84 products, every item defined within its stack.
    let present: bool = sqlx::query_scalar("SELECT to_regclass('defs.product_list') IS NOT NULL").fetch_one(&db.pool).await.unwrap();
    if present {
        let products = pg::load_products(&db.pool).await.unwrap();
        let items = pg::load_item_defs(&db.pool).await.unwrap();
        assert_eq!((products.len(), products.problems(&items)), (84, Vec::<String>::new()));
    }
    db.drop().await;
}

#[tokio::test]
async fn a_character_loads_on_a_pool_of_one_connection() {
    use game_server::backend::{Premium, SavePoint, SavedQuest, StoredPointItem};
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    for user in [alice, bob] {
        backend.choose_faction(user, 1, 0).await.unwrap();
    }
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Loader")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(friend) = backend.create(&hero(bob, 0, "Friend")).await.unwrap() else { panic!("not created") };
    // Items in the bags and one moved to the warehouse, skills, quick slots, a quest, a save point, a friend, the mall's rows.
    assert!(backend.trade(id, 0, &[SlotChange::Move { from: (1, 0), to: (100, 3) }]).await.unwrap().is_some());
    backend.learn(id, 3, Skill { id: 10, level: 1, number: 0, delay: 0 }).await.unwrap();
    backend.save_quick_slots(id, &[QuickSlot { bar: 0, slot: 1, bag: 1, number: 3 }]).await.unwrap();
    assert!(backend.start_quest(id, 7, Some(600), &[]).await.unwrap().is_some());
    backend.save_point(id, SavePoint { slot: 1, map: 1, position: [1.0, 2.0, 3.0] }).await.unwrap();
    backend.write_friends(&FriendWrite::Friends(vec![(id, friend)])).await.unwrap();
    sqlx::query("UPDATE accounts SET points = 40, premium = 1, premium_until = now() + interval '1 day' WHERE id = $1")
        .bind(alice as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO point_items (user_id, server_id, slot, item_id, count) VALUES ($1, 1, 0, 100048, 2)")
        .bind(alice as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO billing_items (user_id, slot, item_id, count) VALUES ($1, 4, 25001, 3)")
        .bind(alice as i32)
        .execute(&db.pool)
        .await
        .unwrap();
    let usual = backend.load(alice, 1, id).await.unwrap().unwrap();
    assert_eq!((usual.bank.len(), usual.skills.len(), usual.quick_slots.len(), usual.quests.len()), (1, 1, 1, 1), "the rows are there");
    assert_eq!((usual.save_points.len(), usual.friends.len(), usual.account_name.as_str()), (1, 1, "alice"));
    assert_eq!((usual.mall.points, usual.mall.premium.kind), (40, 1));
    assert_eq!(usual.mall.stored, [StoredPointItem { slot: 0, item_id: 100048, count: 2 }]);
    assert_eq!(usual.mall.billing, [StoredPointItem { slot: 4, item_id: 25001, count: 3 }]);
    assert_ne!(usual.mall.premium, Premium::default());
    assert_eq!(usual.quests, [SavedQuest { quest_id: 7, finished: false, success: false, counts: [0, 0, 0], seconds_left: Some(600) }]);

    // The same load on a pool that has one connection: it needs no second connection, whatever the number of its statements.
    let path = db.schema.clone();
    let one = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(move |conn, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query(AssertSqlSafe(format!("SET search_path TO {path}"))).execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*db.pool.connect_options()).clone())
        .await
        .unwrap();
    let single = PgBackend::new(one.clone(), Duration::from_secs(7200));
    assert_eq!(single.load(alice, 1, id).await.unwrap().unwrap(), usual, "the same character");
    // The connection went back to the pool: the next load, and a character that is not there, find it free.
    assert_eq!(single.load(alice, 1, id).await.unwrap().unwrap(), usual);
    assert!(single.load(alice, 1, friend).await.unwrap().is_none(), "another account's character");
    // (A connection given back reaches the idle list a moment later, so only the size is exact here.)
    assert_eq!(single.pool_gauges().map(|(size, _)| size), Some(1));
    one.close().await;
    db.drop().await;
}

#[tokio::test]
async fn a_loaded_character_has_its_bank_its_bags_and_its_account_fields() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let (alice, bob) = (db.account("alice").await, db.account("bob").await);
    for user in [alice, bob] {
        backend.choose_faction(user, 1, 0).await.unwrap();
    }
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Banker")).await.unwrap() else { panic!("not created") };
    let CreateOutcome::Created(other) = backend.create(&hero(bob, 0, "Other")).await.unwrap() else { panic!("not created") };
    // The rows are written in scrambled order, so the order the load returns is the query's: bags then slots, the bank by slot. Bob's
    // bank row and bob's character's item must not turn up on alice's character.
    sqlx::query("DELETE FROM character_items WHERE character_id = ANY($1)")
        .bind(vec![id as i32, other as i32])
        .execute(&db.pool)
        .await
        .unwrap();
    for (character, bag, slot, item_id, kind, type_id, count, craft) in [
        (id, 5, 23, 1005, 10, 5, 1, "Last"),
        (id, 0, 5, 1001, 1, 1, 1, ""),
        (id, 2, 4, 1002, 2, 2, 3, "Smith"),
        (id, 0, 1, 1003, 3, 3, 1, ""),
        (other, 0, 0, 1004, 4, 4, 9, ""),
    ] {
        sqlx::query(
            "INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, quality, gems, count, craft_name)
             VALUES ($1, $2, $3, $4, $5, $6, 700, '\\x010203040506', $7, $8)",
        )
        .bind(character as i32)
        .bind(bag as i16)
        .bind(slot as i16)
        .bind(item_id)
        .bind(kind as i16)
        .bind(type_id as i16)
        .bind(count as i16)
        .bind(craft)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    for (user, server, slot, item_id, kind, type_id, count, craft) in [
        (alice, 1, 9, 2001, 20, 1, 2, "Vault"),
        (alice, 1, 2, 2002, 21, 2, 1, ""),
        (alice, 0, 0, 2003, 22, 3, 1, ""), // alice's warehouse on another server
        (bob, 1, 1, 2004, 23, 4, 1, ""),
    ] {
        sqlx::query(
            "INSERT INTO bank_items (user_id, server_id, slot, item_id, type, type_id, quality, gems, count, craft_name)
             VALUES ($1, $2, $3, $4, $5, $6, 0, '\\x000000000000', $7, $8)",
        )
        .bind(user as i32)
        .bind(server as i16)
        .bind(slot as i16)
        .bind(item_id)
        .bind(kind as i16)
        .bind(type_id as i16)
        .bind(count as i16)
        .bind(craft)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query("UPDATE accounts SET points = 7, premium = 2, premium_until = to_timestamp(2000000000) WHERE id = $1")
        .bind(alice as i32)
        .execute(&db.pool)
        .await
        .unwrap();

    let loaded = backend.load(alice, 1, id).await.unwrap().unwrap();
    let rows = |items: &[game_server::backend::ItemUnit]| {
        items.iter().map(|i| (i.bag, i.slot, i.item_type, i.type_id, i.quality, i.gems, i.count, i.craft_name.clone())).collect::<Vec<_>>()
    };
    assert_eq!(
        rows(&loaded.items),
        [
            (0, 1, 3, 3, 700, [1, 2, 3, 4, 5, 6], 1, b"".to_vec()),
            (0, 5, 1, 1, 700, [1, 2, 3, 4, 5, 6], 1, b"".to_vec()),
            (2, 4, 2, 2, 700, [1, 2, 3, 4, 5, 6], 3, b"Smith".to_vec()),
            (5, 23, 10, 5, 700, [1, 2, 3, 4, 5, 6], 1, b"Last".to_vec()),
        ],
        "the bags only, by bag and slot"
    );
    assert_eq!(
        rows(&loaded.bank),
        [(100, 2, 21, 2, 0, [0; 6], 1, b"".to_vec()), (100, 9, 20, 1, 0, [0; 6], 2, b"Vault".to_vec())],
        "the warehouse only, by slot, this account on this server"
    );
    assert_eq!((loaded.account_id, loaded.account_name.as_str()), (alice, "alice"));
    assert_eq!((loaded.mall.points, loaded.mall.premium), (7, game_server::backend::Premium { kind: 2, until: Some(2_000_000_000) }));
    assert_eq!((loaded.name.as_str(), loaded.id), ("Banker", id));
    // A character with an empty warehouse and no premium end.
    let bobs = backend.load(bob, 1, other).await.unwrap().unwrap();
    assert_eq!((bobs.items.len(), bobs.bank.len(), bobs.account_name.as_str()), (1, 1, "bob"));
    assert_eq!((bobs.mall.points, bobs.mall.premium), (0, game_server::backend::Premium { kind: 0, until: None }));
    // Another account's character, and a deleted one, are not found.
    assert!(backend.load(bob, 1, id).await.unwrap().is_none());
    db.drop().await;
}

/// `characters.user_id` references `accounts`, so a character without its account cannot be written; with the constraint's triggers
/// off (superuser only) the row can be made, and the load answers it with an error as it did when the account's name was a separate
/// `SELECT` (`fetch_one`), not with "no such character".
#[tokio::test]
async fn a_character_without_an_account_row_fails_to_load() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Orphan")).await.unwrap() else { panic!("not created") };
    assert!(sqlx::query("DELETE FROM accounts WHERE id = $1").bind(alice as i32).execute(&db.pool).await.is_err(), "the foreign key holds");
    let mut conn = db.pool.acquire().await.unwrap();
    if sqlx::query("SET session_replication_role = replica").execute(&mut *conn).await.is_err() {
        eprintln!("not a superuser: the orphan row cannot be made, skipping");
        drop(conn);
        db.drop().await;
        return;
    }
    sqlx::query("DELETE FROM accounts WHERE id = $1").bind(alice as i32).execute(&mut *conn).await.unwrap();
    sqlx::query("RESET session_replication_role").execute(&mut *conn).await.unwrap();
    drop(conn);
    let loaded = backend.load(alice, 1, id).await;
    assert!(loaded.is_err(), "an error, not Ok(None): {loaded:?}");
    db.drop().await;
}

#[tokio::test]
async fn the_game_log_writes_action_and_chat_rows() {
    use game_server::game_log::{self, Entry, Header};
    let Some(db) = TestDb::create().await else { return };
    let header = Header {
        user_id: 3,
        user_name: "alice".into(),
        character_id: 9,
        character_name: "O'Hara".into(),
        level: 20,
        exp: 4_000_000_000,
        map: 5,
        position: [1.5, 2.5, 3.5],
    };
    let entries = vec![
        Entry::Action(game_log::skill_learned(header.clone(), 21, 2, 4, "Slash")),
        Entry::Action(game_log::keep_top_grade(1)),
        Entry::Chat(game_log::chat_line(&header, 2, Some("Bob's"), "it's a whisper")),
    ];
    pg::write_game_log(&db.pool, 1, &entries).await.unwrap();
    pg::write_game_log(&db.pool, 1, &[]).await.unwrap();

    type Row = (i16, i16, String, i32, i64, i16, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT server_id, action, character_name, user_id, exp, map, value1, value2, value3, text1, text2 FROM game_log ORDER BY id",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(rows[0], (1, 141, "O'Hara".into(), 3, 4_000_000_000, 5, Some(21), Some(2), Some(4), Some("Slash".into()), None));
    assert_eq!((rows[1].1, rows[1].2.as_str(), rows[1].3, rows[1].6, rows[1].7), (170, "", 0, Some(1), None));
    let chat: (i16, i16, Option<String>, String, i16) =
        sqlx::query_as("SELECT server_id, chat_type, target_name, text, map FROM chat_log").fetch_one(&db.pool).await.unwrap();
    assert_eq!(chat, (1, 2, Some("Bob's".into()), "it's a whisper".into(), 5));
    let at: bool =
        sqlx::query_scalar("SELECT bool_and(at BETWEEN now() - interval '1 minute' AND now() + interval '1 minute') FROM game_log")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(at, "the time is the moment the record was made");

    // The retention pass: rows older than the days go.
    sqlx::query("UPDATE game_log SET at = now() - interval '10 days' WHERE action = 141").execute(&db.pool).await.unwrap();
    assert_eq!(pg::purge_game_log(&db.pool, 7).await.unwrap(), 1);
    assert_eq!(pg::purge_game_log(&db.pool, 7).await.unwrap(), 0);
    db.drop().await;
}

#[tokio::test]
async fn item_writes_return_the_rows_they_touch() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Marker")).await.unwrap() else { panic!("not created") };
    let uid_at = |bag: i16, slot: i16| {
        let (pool, id) = (db.pool.clone(), id as i32);
        async move {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT uid, (extract(epoch FROM made_at) * 1000000)::int8 FROM character_items WHERE character_id = $1 AND bag = $2 AND slot = $3",
            )
            .bind(id)
            .bind(bag)
            .bind(slot)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let stored = |(uid, made_at): (i64, i64)| Some(game_server::backend::Stored { uid, made_at });
    let sword = NewItem { bag: 1, slot: 1, item_id: 5001, item_type: 5, type_id: 1, quality: 1200, count: 1, kept: None };

    // An insert returns the row it made.
    let rows = backend.trade(id, 0, &[SlotChange::Insert(sword)]).await.unwrap().unwrap();
    assert_eq!(rows[0].row, stored(uid_at(1, 1).await));
    // A change of the stack of potions names the same row; a move keeps it, to the warehouse too; a swap names both.
    let potions = uid_at(1, 0).await;
    let rows = backend.trade(id, 0, &[SlotChange::SetCount { bag: 1, slot: 0, count: 4 }]).await.unwrap().unwrap();
    assert_eq!(rows[0].row, stored(potions));
    let rows = backend.trade(id, 0, &[SlotChange::Move { from: (1, 0), to: (100, 3) }]).await.unwrap().unwrap();
    assert_eq!(rows[0].row, stored(potions), "a move to the warehouse keeps the row");
    let sword_row = uid_at(1, 1).await;
    let rows = backend
        .trade(id, 0, &[SlotChange::Move { from: (100, 3), to: (1, 0) }, SlotChange::Swap { a: (1, 0), b: (1, 1) }])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rows[0].row, stored(potions));
    assert_eq!((rows[1].row, rows[1].swapped), (stored(sword_row), stored(potions)), "the row now at a, the row now at b");
    // A delete returns the row it removed; a refused change returns nothing.
    let rows = backend.trade(id, 0, &[SlotChange::Remove { bag: 1, slot: 0 }]).await.unwrap().unwrap();
    assert_eq!(rows[0].row, stored(sword_row));
    assert!(backend.trade(id, 0, &[SlotChange::Remove { bag: 1, slot: 0 }]).await.unwrap().is_none());
    db.drop().await;
}

/// The scripts' stored values (`docs/plans/ext-5-lua.md` section 7.6) against the real `content` tables, which the throwaway schema of
/// the other tests cannot hide: the rows it makes carry a random name and are removed at the end.
#[tokio::test]
async fn script_values_are_written_coalesced_deleted_and_cascade() {
    use game_server::script::store::Store;
    dotenvy::dotenv().ok();
    let Ok(url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("DATABASE_URL not set: skipping the PostgreSQL tests");
        return;
    };
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await.unwrap();
    sqlx::migrate!("../../db/migrations").run(&pool).await.unwrap();
    let backend = PgBackend::new(pool.clone(), Duration::from_secs(7200));
    let tag = format!("t{:08x}", rand::random::<u32>());
    let (ext_a, ext_b) = (format!("{tag}-a"), format!("{tag}-b"));
    for ext in [&ext_a, &ext_b] {
        sqlx::query("INSERT INTO content.extensions (name, version, title, description, manifest) VALUES ($1, '1.0.0', 't', 'd', '{}')")
            .bind(ext)
            .execute(&pool)
            .await
            .unwrap();
    }
    let user: i32 = sqlx::query_scalar("INSERT INTO accounts (username, password_hash) VALUES ($1, 'x') RETURNING id")
        .bind(&tag)
        .fetch_one(&pool)
        .await
        .unwrap();
    let CreateOutcome::Created(character) = backend.create(&hero(user as u32, 0, &tag)).await.unwrap() else { panic!("not created") };
    let count = |ext: String, table: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(AssertSqlSafe(format!("SELECT count(*) FROM content.{table} WHERE extension = '{ext}'")))
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };

    // 100 sets of one key leave one write, and one row with the last value.
    let store = Store::default();
    store.begin_char(character, Vec::new());
    for n in 0..100 {
        store.set(&ext_a, character, "streak", Some(serde_json::json!(n))).unwrap();
    }
    store.set(&ext_a, character, "name", Some(serde_json::json!("Walker"))).unwrap();
    store.set(&ext_b, character, "best", Some(serde_json::json!(true))).unwrap();
    store.set_globals(&ext_a, Vec::new());
    store.set_global(&ext_a, "bounty.1", Some(serde_json::json!(5000))).unwrap();
    let batch = store.take_pending();
    assert_eq!(batch.len(), 4);
    backend.write_script_values(&batch).await.unwrap();
    assert_eq!(count(ext_a.clone(), "script_values").await, 2);
    let mut rows = backend.script_values(character).await.unwrap();
    rows.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    assert_eq!(
        rows,
        [
            (ext_a.clone(), "name".to_owned(), serde_json::json!("Walker")),
            (ext_a.clone(), "streak".to_owned(), serde_json::json!(99)),
            (ext_b.clone(), "best".to_owned(), serde_json::json!(true)),
        ]
    );
    assert_eq!(count(ext_a.clone(), "script_globals").await, 1);

    // A value deleted, a global changed; a write for a character that is gone is skipped, not an error that blocks the batch.
    store.set(&ext_a, character, "name", None).unwrap();
    store.set_global(&ext_a, "bounty.1", Some(serde_json::json!(7500))).unwrap();
    let mut batch = store.take_pending();
    batch.push(game_server::script::store::ScriptWrite {
        extension: ext_a.clone(),
        character: Some(2_000_000_000),
        key: "x".into(),
        value: Some(serde_json::json!(1)),
    });
    backend.write_script_values(&batch).await.unwrap();
    assert_eq!(count(ext_a.clone(), "script_values").await, 1);
    let global: serde_json::Value =
        sqlx::query_scalar("SELECT value FROM content.script_globals WHERE extension = $1").bind(&ext_a).fetch_one(&pool).await.unwrap();
    assert_eq!(global, serde_json::json!(7500));

    // The extension's removal takes its values; a character's deletion takes the rest.
    sqlx::query("DELETE FROM content.extensions WHERE name = $1").bind(&ext_a).execute(&pool).await.unwrap();
    assert_eq!((count(ext_a.clone(), "script_values").await, count(ext_a.clone(), "script_globals").await), (0, 0));
    assert_eq!(count(ext_b.clone(), "script_values").await, 1);
    // The shared `content` schema is created by whichever test binary migrates first, and the ones that migrate inside a throwaway
    // schema leave `script_values` without its key to `characters` once that schema is dropped (the table is guarded by IF NOT EXISTS);
    // a fresh database, as in CI, is in that case. The cascade is asserted where the key is there.
    let keyed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'content.script_values'::regclass AND contype = 'f' \
         AND confrelid = 'characters'::regclass)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM characters WHERE id = $1").bind(character as i32).execute(&pool).await.unwrap();
    if keyed {
        assert_eq!(count(ext_b.clone(), "script_values").await, 0);
    } else {
        eprintln!(
            "content.script_values has no key to characters here (the schema was made by a throwaway migration): cascade not checked"
        );
    }

    sqlx::query("DELETE FROM content.extensions WHERE name = $1").bind(&ext_b).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM accounts WHERE id = $1").bind(user).execute(&pool).await.unwrap();
}

#[tokio::test]
async fn a_characters_pet_is_stored_replaced_cleared_and_goes_with_the_character() {
    // docs/plans/ext-4-pets.md section 8.
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(7200));
    let alice = db.account("alice").await;
    backend.choose_faction(alice, 1, 0).await.unwrap();
    let CreateOutcome::Created(id) = backend.create(&hero(alice, 0, "Ranger")).await.unwrap() else { panic!("not created") };
    assert_eq!(backend.pet(id).await.unwrap(), None, "none out");
    backend.set_pet(id, Some("P00123.6731A2F0.00")).await.unwrap();
    assert_eq!(backend.pet(id).await.unwrap().as_deref(), Some("P00123.6731A2F0.00"));
    // One row a character: another egg replaces it.
    backend.set_pet(id, Some("P00124.6731A2F0.01")).await.unwrap();
    assert_eq!(backend.pet(id).await.unwrap().as_deref(), Some("P00124.6731A2F0.01"));
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM character_pets").fetch_one(&db.pool).await.unwrap();
    assert_eq!(rows, 1);
    backend.set_pet(id, None).await.unwrap();
    assert_eq!(backend.pet(id).await.unwrap(), None);
    backend.set_pet(id, None).await.unwrap();
    // The row goes with its character.
    backend.set_pet(id, Some("P00123.6731A2F0.00")).await.unwrap();
    sqlx::query("DELETE FROM characters WHERE id = $1").bind(id as i32).execute(&db.pool).await.unwrap();
    assert_eq!(backend.pet(id).await.unwrap(), None, "ON DELETE CASCADE");
    db.drop().await;
}
