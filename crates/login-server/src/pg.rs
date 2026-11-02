//! PostgreSQL implementation of the login server's backend, plus the operator functions behind the `servers` tool.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sqlx::PgPool;

use crate::accounts::{AccountStore, AuthResult, PgAccountStore};
use crate::backend::{NewSession, SESSION_LOGIN_SECS, SESSION_SELECT_SECS, ServerDirectory, ServerInfo, SessionStore};

pub struct PgBackend {
    pool: PgPool,
    accounts: PgAccountStore,
    /// A game server that has not heartbeated within this time counts as closed.
    heartbeat_timeout: Duration,
}

impl PgBackend {
    pub fn new(pool: PgPool, heartbeat_timeout: Duration) -> Self {
        Self { accounts: PgAccountStore::new(pool.clone()), pool, heartbeat_timeout }
    }

    pub fn accounts(&self) -> &PgAccountStore {
        &self.accounts
    }

    /// Removes sessions that expired more than a minute ago.
    pub async fn purge_expired_sessions(&self) -> Result<u64> {
        let done = sqlx::query("DELETE FROM sessions WHERE expires_at < now() - interval '1 minute'").execute(&self.pool).await?;
        Ok(done.rows_affected())
    }
}

type ServerRow = (i16, String, String, i32, Option<i64>, i32, bool);

/// `$1` is the heartbeat timeout in seconds. A macro keeps the text a compile-time literal, which sqlx requires.
macro_rules! server_query {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, host(address), capacity, version, players,
                    (enabled AND last_seen IS NOT NULL AND last_seen > now() - make_interval(secs => $1)) AS open
               FROM game_servers ",
            $tail
        )
    };
}

fn server_from_row(row: ServerRow) -> Result<ServerInfo> {
    let (id, name, address, capacity, version, players, open) = row;
    Ok(ServerInfo {
        id: u8::try_from(id)?,
        name,
        address: address.parse::<Ipv4Addr>().context("game server address is not IPv4")?,
        capacity: u16::try_from(capacity)?,
        players: u16::try_from(players)?,
        version: version.map(u32::try_from).transpose()?,
        open,
    })
}

impl ServerDirectory for PgBackend {
    async fn servers(&self) -> Result<Vec<ServerInfo>> {
        let rows: Vec<ServerRow> =
            sqlx::query_as(server_query!("ORDER BY id")).bind(self.heartbeat_timeout.as_secs_f64()).fetch_all(&self.pool).await?;
        rows.into_iter().map(server_from_row).collect()
    }

    async fn server(&self, id: u8) -> Result<Option<ServerInfo>> {
        let row: Option<ServerRow> = sqlx::query_as(server_query!("WHERE id = $2"))
            .bind(self.heartbeat_timeout.as_secs_f64())
            .bind(i16::from(id))
            .fetch_optional(&self.pool)
            .await?;
        row.map(server_from_row).transpose()
    }
}

impl SessionStore for PgBackend {
    async fn create(&self, session: NewSession) -> Result<()> {
        // The game servers end an older connection of the account on the notification (the original session server's 0x0802,
        // docs/original/client-extras.md section 3.2); it goes out when this commits.
        sqlx::query(
            "WITH s AS (
                 INSERT INTO sessions (user_id, identity, aes_key, aes_iv, login_ip, server_id, created_at, expires_at)
                 VALUES ($1, $2, $3, $4, $5::inet, NULL, now(), now() + make_interval(secs => $6))
                 ON CONFLICT (user_id) DO UPDATE SET
                     identity = EXCLUDED.identity, aes_key = EXCLUDED.aes_key, aes_iv = EXCLUDED.aes_iv,
                     login_ip = EXCLUDED.login_ip, server_id = NULL, game_connected_at = NULL, created_at = now(),
                     expires_at = EXCLUDED.expires_at
                 RETURNING user_id, created_at)
             SELECT pg_notify('account_login', user_id || ' ' || (extract(epoch FROM created_at) * 1000000)::bigint) FROM s",
        )
        .bind(i32::try_from(session.user_id)?)
        .bind(&session.identity[..])
        .bind(&session.keys.key[..])
        .bind(&session.keys.iv[..])
        .bind(session.ip.to_string())
        .bind(SESSION_LOGIN_SECS)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn bind_server(&self, user_id: u32, identity: [u8; 16], server_id: u8) -> Result<()> {
        let done = sqlx::query(
            "UPDATE sessions SET server_id = $3, expires_at = now() + make_interval(secs => $4)
              WHERE user_id = $1 AND identity = $2",
        )
        .bind(i32::try_from(user_id)?)
        .bind(&identity[..])
        .bind(i16::from(server_id))
        .bind(SESSION_SELECT_SECS)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            bail!("no session to bind (replaced by a newer login?)");
        }
        Ok(())
    }
}

impl AccountStore for PgBackend {
    fn authenticate(&self, username: &str, password: &str, ip: IpAddr) -> impl Future<Output = Result<AuthResult>> + Send {
        self.accounts.authenticate(username, password, ip)
    }
}

// ---- operator functions (used by the `servers` tool) ----

pub struct ServerRecord {
    pub id: u8,
    pub name: String,
    pub address: Ipv4Addr,
    pub capacity: u16,
    pub version: Option<u32>,
    pub enabled: bool,
    pub players: u16,
    /// Seconds since the last heartbeat, if there ever was one.
    pub seen_ago: Option<f64>,
}

pub async fn add_server(pool: &PgPool, id: u8, name: &str, address: Ipv4Addr, capacity: u16, version: Option<u32>) -> Result<()> {
    if name.is_empty() || name.len() > 31 {
        bail!("name must be 1 to 31 characters");
    }
    sqlx::query("INSERT INTO game_servers (id, name, address, capacity, version) VALUES ($1, $2, $3::inet, $4, $5)")
        .bind(i16::from(id))
        .bind(name)
        .bind(address.to_string())
        .bind(i32::from(capacity))
        .bind(version.map(i64::from))
        .execute(pool)
        .await
        .context("could not add the server (id or address already used?)")?;
    Ok(())
}

pub async fn remove_server(pool: &PgPool, id: u8) -> Result<()> {
    let done = sqlx::query("DELETE FROM game_servers WHERE id = $1").bind(i16::from(id)).execute(pool).await?;
    if done.rows_affected() == 0 {
        bail!("no game server with id {id}");
    }
    Ok(())
}

pub async fn set_enabled(pool: &PgPool, id: u8, enabled: bool) -> Result<()> {
    let done = sqlx::query("UPDATE game_servers SET enabled = $2 WHERE id = $1").bind(i16::from(id)).bind(enabled).execute(pool).await?;
    if done.rows_affected() == 0 {
        bail!("no game server with id {id}");
    }
    Ok(())
}

pub async fn heartbeat(pool: &PgPool, id: u8, players: u16) -> Result<()> {
    let done = sqlx::query("UPDATE game_servers SET players = $2, last_seen = now() WHERE id = $1")
        .bind(i16::from(id))
        .bind(i32::from(players))
        .execute(pool)
        .await?;
    if done.rows_affected() == 0 {
        bail!("no game server with id {id}");
    }
    Ok(())
}

/// The columns `list_servers` reads: id, name, address, capacity, version, enabled, players, seconds since the last heartbeat.
type ServerCols = (i16, String, String, i32, Option<i64>, bool, i32, Option<f64>);

pub async fn list_servers(pool: &PgPool) -> Result<Vec<ServerRecord>> {
    let rows: Vec<ServerCols> = sqlx::query_as(
        "SELECT id, name, host(address), capacity, version, enabled, players,
                extract(epoch FROM now() - last_seen)::float8
           FROM game_servers ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(id, name, address, capacity, version, enabled, players, seen_ago)| {
            Ok(ServerRecord {
                id: u8::try_from(id)?,
                name,
                address: address.parse()?,
                capacity: u16::try_from(capacity)?,
                version: version.map(u32::try_from).transpose()?,
                enabled,
                players: u16::try_from(players)?,
                seen_ago,
            })
        })
        .collect()
}

/// An account's id by name (any case), not deleted.
pub async fn account_by_name(pool: &PgPool, username: &str) -> Result<u32> {
    let id: Option<i32> = sqlx::query_scalar("SELECT id FROM accounts WHERE lower(username) = lower($1) AND NOT deleted")
        .bind(username)
        .fetch_optional(pool)
        .await?;
    match id {
        Some(id) => Ok(u32::try_from(id)?),
        None => bail!("no account named {username}"),
    }
}

/// An account's item mall state (`docs/original/item-mall.md`): points, premium type and its end (the database's local time),
/// billing storage rows.
pub struct MallRecord {
    pub points: u32,
    pub premium: u8,
    pub premium_until: Option<String>,
    pub billing_items: u32,
}

pub async fn mall_record(pool: &PgPool, user_id: u32) -> Result<MallRecord> {
    let (points, premium, until, billing): (i32, i16, Option<String>, i64) = sqlx::query_as(
        "SELECT points, premium, to_char(premium_until, 'YYYY-MM-DD HH24:MI TZ'), (SELECT count(*) FROM billing_items WHERE user_id = accounts.id)
           FROM accounts WHERE id = $1",
    )
    .bind(i32::try_from(user_id)?)
    .fetch_one(pool)
    .await?;
    Ok(MallRecord {
        points: u32::try_from(points)?,
        premium: u8::try_from(premium)?,
        premium_until: until,
        billing_items: u32::try_from(billing)?,
    })
}

/// Changes an account's points by `change`, or sets them to `set`, never below 0, and tells the game servers (deviation 5 of the
/// item mall). The points now.
pub async fn change_points(pool: &PgPool, user_id: u32, change: i64, set: Option<u32>) -> Result<u32> {
    let mut tx = pool.begin().await?;
    let points: Option<i32> = sqlx::query_scalar(
        "UPDATE accounts SET points = COALESCE($3::int8, points + $2) WHERE id = $1 AND COALESCE($3::int8, points + $2) BETWEEN 0 AND 2147483647
         RETURNING points",
    )
    .bind(i32::try_from(user_id)?)
    .bind(change)
    .bind(set.map(i64::from))
    .fetch_optional(&mut *tx)
    .await?;
    let Some(points) = points else { bail!("the points would leave 0 to 2147483647") };
    sqlx::query("SELECT pg_notify('account_points', $1)").bind(user_id.to_string()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(u32::try_from(points)?)
}

/// Sets an account's premium: the billing type (0 ends it) for `days` from now, or with no end. Read at the next world entry.
pub async fn set_premium(pool: &PgPool, user_id: u32, premium: u8, days: Option<f64>) -> Result<()> {
    if premium > 3 {
        bail!("the premium type is 0 to 3");
    }
    sqlx::query(
        "UPDATE accounts SET premium = $2, premium_until = CASE WHEN $2 = 0 OR $3::float8 IS NULL THEN NULL ELSE now() + make_interval(secs => $3 * 86400) END
          WHERE id = $1",
    )
    .bind(i32::try_from(user_id)?)
    .bind(i16::from(premium))
    .bind(days)
    .execute(pool)
    .await?;
    Ok(())
}

/// Puts a product's items into an account's billing storage (deviation 8): one row per entry, a count above the stack split, in the
/// lowest free slots. The rows' `(slot, item id, count)`.
pub async fn deliver_product(pool: &PgPool, user_id: u32, code: &str) -> Result<Vec<(u8, u32, u8)>> {
    use sqlx::Row;
    let entries: Vec<String> = (1..=24).map(|i| format!("COALESCE(itemid{i}, 0)::int4, COALESCE(itemcount{i}, 0)::int2")).collect();
    let sql = format!("SELECT {} FROM defs.product_list WHERE productcode = $1", entries.join(", "));
    let mut tx = pool.begin().await?;
    // Built from constants only.
    let Some(row) = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(code).fetch_optional(&mut *tx).await? else {
        bail!("no product with code {code}")
    };
    let mut rows = Vec::new();
    for i in 0..24 {
        let (item, count): (i32, i16) = (row.try_get(2 * i)?, row.try_get(2 * i + 1)?);
        if item == 0 || count == 0 {
            continue;
        }
        let stack: Option<i16> =
            sqlx::query_scalar("SELECT count FROM defs.items WHERE itemid = $1").bind(item).fetch_optional(&mut *tx).await?;
        let stack = stack.filter(|&s| s > 0).with_context(|| format!("item {item} of {code} is not defined"))?;
        let mut left = count;
        while left > 0 {
            let piece = left.min(stack);
            rows.push((u32::try_from(item)?, u8::try_from(piece)?));
            left -= piece;
        }
    }
    let user = i32::try_from(user_id)?;
    sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE").bind(user).execute(&mut *tx).await?;
    let taken: Vec<i16> = sqlx::query_scalar("SELECT slot FROM billing_items WHERE user_id = $1").bind(user).fetch_all(&mut *tx).await?;
    let free: Vec<i16> = (0..240).filter(|s| !taken.contains(s)).take(rows.len()).collect();
    if free.len() < rows.len() {
        bail!("the billing storage has {} free slots, {} needed", 240 - taken.len(), rows.len());
    }
    let mut placed = Vec::new();
    for (&slot, &(item, count)) in free.iter().zip(&rows) {
        sqlx::query("INSERT INTO billing_items (user_id, slot, item_id, count, product_code) VALUES ($1, $2, $3, $4, $5)")
            .bind(user)
            .bind(slot)
            .bind(i32::try_from(item)?)
            .bind(i16::from(count))
            .bind(code)
            .execute(&mut *tx)
            .await?;
        placed.push((u8::try_from(slot)?, item, count));
    }
    tx.commit().await?;
    Ok(placed)
}
