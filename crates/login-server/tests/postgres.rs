//! Runs the SQL layer against a real PostgreSQL, inside a throwaway schema that is dropped afterwards.
//! Skipped (and says so) when DATABASE_URL is not set.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use login_server::accounts::{AccountStore, AuthResult};
use login_server::backend::{NewSession, ServerDirectory, SessionStore};
use login_server::pg::{self, PgBackend};
use shaiya_crypto::SessionKeys;
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
}

const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 20));

#[tokio::test]
async fn game_servers_follow_their_heartbeat() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(15));

    pg::add_server(&db.pool, 1, "Alpha", Ipv4Addr::new(10, 0, 0, 1), 800, Some(0x10042600)).await.unwrap();
    pg::add_server(&db.pool, 2, "Beta", Ipv4Addr::new(10, 0, 0, 2), 1000, None).await.unwrap();

    // One IP per game server is enforced by the database.
    assert!(pg::add_server(&db.pool, 3, "Gamma", Ipv4Addr::new(10, 0, 0, 1), 1000, None).await.is_err(), "duplicate address");
    assert!(pg::add_server(&db.pool, 1, "Again", Ipv4Addr::new(10, 0, 0, 9), 1000, None).await.is_err(), "duplicate id");

    // No heartbeat yet: everything is closed.
    let servers = backend.servers().await.unwrap();
    assert_eq!(servers.len(), 2);
    assert!(servers.iter().all(|s| !s.open), "closed until a heartbeat arrives");
    assert_eq!(servers[0].version, Some(0x10042600));
    assert_eq!(servers[1].version, None);

    pg::heartbeat(&db.pool, 1, 42).await.unwrap();
    let alpha = backend.server(1).await.unwrap().unwrap();
    assert!(alpha.open);
    assert_eq!((alpha.players, alpha.capacity, alpha.address), (42, 800, Ipv4Addr::new(10, 0, 0, 1)));
    assert!(!backend.server(2).await.unwrap().unwrap().open, "no heartbeat for Beta");
    assert!(backend.server(99).await.unwrap().is_none());

    // The operator switch closes a server whatever it reports.
    pg::set_enabled(&db.pool, 1, false).await.unwrap();
    assert!(!backend.server(1).await.unwrap().unwrap().open);
    pg::set_enabled(&db.pool, 1, true).await.unwrap();
    assert!(backend.server(1).await.unwrap().unwrap().open);

    // A stale heartbeat closes it again.
    sqlx::query("UPDATE game_servers SET last_seen = now() - interval '1 minute' WHERE id = 1").execute(&db.pool).await.unwrap();
    assert!(!backend.server(1).await.unwrap().unwrap().open, "heartbeat older than the timeout");

    assert!(pg::heartbeat(&db.pool, 77, 0).await.is_err(), "unknown server");
    let listed = pg::list_servers(&db.pool).await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed[1].seen_ago.is_none());

    pg::remove_server(&db.pool, 2).await.unwrap();
    assert_eq!(backend.servers().await.unwrap().len(), 1);
    db.drop().await;
}

#[tokio::test]
async fn sessions_are_created_replaced_bound_and_purged() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(15));
    let user = backend.accounts().create_account("alice", "secret", 0).await.unwrap();
    pg::add_server(&db.pool, 1, "Alpha", Ipv4Addr::new(10, 0, 0, 1), 800, None).await.unwrap();

    let keys = SessionKeys { key: [1; 16], iv: [2; 16] };
    backend.create(NewSession { user_id: user, identity: [7; 16], keys, ip: IP }).await.unwrap();

    let row = |sql: &'static str| sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>, String, Option<i16>)>(sql);
    let sql = "SELECT identity, aes_key, aes_iv, host(login_ip), server_id FROM sessions";
    let (identity, key, iv, ip, server) = row(sql).fetch_one(&db.pool).await.unwrap();
    assert_eq!((identity, key, iv, ip.as_str(), server), (vec![7; 16], vec![1; 16], vec![2; 16], "192.168.0.20", None));

    // A newer login replaces the session; the older identity can no longer claim a server.
    backend.create(NewSession { user_id: user, identity: [8; 16], keys, ip: IP }).await.unwrap();
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sessions").fetch_one(&db.pool).await.unwrap(), 1);
    assert!(backend.bind_server(user, [7; 16], 1).await.is_err(), "stale identity");
    backend.bind_server(user, [8; 16], 1).await.unwrap();
    let (.., server) = row(sql).fetch_one(&db.pool).await.unwrap();
    assert_eq!(server, Some(1));

    // Binding shortens the window to the select timeout (about two minutes).
    let seconds: f64 =
        sqlx::query_scalar("SELECT extract(epoch FROM expires_at - now())::float8 FROM sessions").fetch_one(&db.pool).await.unwrap();
    assert!((60.0..=121.0).contains(&seconds), "expires in {seconds}s");

    // Cleanup removes only sessions that expired more than a minute ago.
    assert_eq!(backend.purge_expired_sessions().await.unwrap(), 0);
    sqlx::query("UPDATE sessions SET expires_at = now() - interval '2 minutes'").execute(&db.pool).await.unwrap();
    assert_eq!(backend.purge_expired_sessions().await.unwrap(), 1);

    // Deleting a server clears the choice but keeps the session; deleting the account removes it.
    backend.create(NewSession { user_id: user, identity: [9; 16], keys, ip: IP }).await.unwrap();
    backend.bind_server(user, [9; 16], 1).await.unwrap();
    pg::remove_server(&db.pool, 1).await.unwrap();
    let (.., server) = row(sql).fetch_one(&db.pool).await.unwrap();
    assert_eq!(server, None);
    sqlx::query("DELETE FROM accounts WHERE id = $1").bind(user as i32).execute(&db.pool).await.unwrap();
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sessions").fetch_one(&db.pool).await.unwrap(), 0);
    db.drop().await;
}

#[tokio::test]
async fn accounts_follow_the_status_rules_in_sql() {
    let Some(db) = TestDb::create().await else { return };
    let backend = PgBackend::new(db.pool.clone(), Duration::from_secs(15));
    let accounts = backend.accounts();
    let player = accounts.create_account("player", "pass01", 0).await.unwrap();
    let boss = accounts.create_account("Boss", "pass02", 16).await.unwrap();
    accounts.create_account("banned", "pass03", 2).await.unwrap();
    assert!(accounts.create_account("PLAYER", "other1", 0).await.is_err(), "names are unique ignoring case");
    assert!(accounts.create_account("shorty", "abcde", 0).await.is_err(), "a new password has 6 to 16 characters");

    assert_eq!(backend.authenticate("player", "pass01", IP).await.unwrap(), AuthResult::Accepted { user_id: player, privilege: 11 });
    assert_eq!(backend.authenticate("boss", "pass02", IP).await.unwrap(), AuthResult::Accepted { user_id: boss, privilege: 1 });
    assert_eq!(backend.authenticate("player", "nope", IP).await.unwrap(), AuthResult::WrongPassword);
    assert_eq!(backend.authenticate("nobody", "x", IP).await.unwrap(), AuthResult::UnknownAccount);
    assert!(matches!(backend.authenticate("banned", "pass03", IP).await.unwrap(), AuthResult::Refused(_)));
    assert_eq!(backend.authenticate("banned", "wrong", IP).await.unwrap(), AuthResult::WrongPassword);

    // A timed block lifts itself once its end date has passed (status 2 to 6 only).
    sqlx::query("UPDATE accounts SET blocked_until = now() - interval '1 second' WHERE username = 'banned'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(backend.authenticate("banned", "pass03", IP).await.unwrap(), AuthResult::Accepted { privilege: 11, .. }));
    assert_eq!(sqlx::query_scalar::<_, i16>("SELECT status FROM accounts WHERE username = 'banned'").fetch_one(&db.pool).await.unwrap(), 0);

    sqlx::query("UPDATE accounts SET deleted = true WHERE username = 'player'").execute(&db.pool).await.unwrap();
    assert_eq!(backend.authenticate("player", "pass01", IP).await.unwrap(), AuthResult::UnknownAccount);

    let outcomes: Vec<String> = sqlx::query_scalar("SELECT outcome FROM login_log ORDER BY id").fetch_all(&db.pool).await.unwrap();
    assert_eq!(outcomes[..2], ["ok", "ok"]);
    assert!(outcomes.contains(&"bad_password".to_owned()) && outcomes.contains(&"unknown".to_owned()));
    db.drop().await;
}
