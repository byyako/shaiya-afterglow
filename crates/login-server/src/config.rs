use std::net::SocketAddr;

use std::time::Duration;

use anyhow::{Context, Result};

use crate::limits::LimitConfig;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    /// A game server that has not sent a heartbeat for this long is listed as closed.
    pub heartbeat_timeout: Duration,
    /// How long a new connection may take to finish the key exchange, counted from the accept; pings do not extend it
    /// (`docs/server/audit.md` R10).
    pub key_timeout: Duration,
    /// How long a connection may take from the key exchange to the login, pings not extending it; once logged in, how long it may
    /// sit at the server list without sending anything.
    pub idle_timeout: Duration,
    /// Enforce `GameServer::version`. Off by default, as in the original's default setup; the client's
    /// value is always logged.
    pub check_versions: bool,
    pub limits: LimitConfig,
    pub key_pool_size: usize,
    pub max_connections: usize,
}

impl Config {
    /// Reads `LOGIN_*` environment variables, with defaults for a local test setup.
    pub fn from_env() -> Result<Self> {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        Ok(Self {
            bind: var("LOGIN_BIND", "0.0.0.0:30800").parse().context("LOGIN_BIND")?,
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            heartbeat_timeout: Duration::from_secs(var("LOGIN_HEARTBEAT_SECS", "15").parse().context("LOGIN_HEARTBEAT_SECS")?),
            key_timeout: Duration::from_secs(var("LOGIN_KEY_TIMEOUT_SECS", "30").parse().context("LOGIN_KEY_TIMEOUT_SECS")?),
            idle_timeout: Duration::from_secs(var("LOGIN_IDLE_SECS", "600").parse().context("LOGIN_IDLE_SECS")?),
            check_versions: flag(&var("LOGIN_CHECK_VERSION", "false")),
            limits: LimitConfig {
                max_connections: var("LOGIN_MAX_PER_IP", "20").parse().context("LOGIN_MAX_PER_IP")?,
                connects_per_minute: var("LOGIN_CONNECTS_PER_MINUTE", "30").parse().context("LOGIN_CONNECTS_PER_MINUTE")?,
                failure_limit: var("LOGIN_FAILURE_LIMIT", "20").parse().context("LOGIN_FAILURE_LIMIT")?,
                window: Duration::from_secs(var("LOGIN_FAILURE_WINDOW_SECS", "600").parse().context("LOGIN_FAILURE_WINDOW_SECS")?),
                lockout: Duration::from_secs(var("LOGIN_LOCKOUT_SECS", "600").parse().context("LOGIN_LOCKOUT_SECS")?),
                pair_failure_limit: var("LOGIN_PAIR_FAILURES", "5").parse().context("LOGIN_PAIR_FAILURES")?,
                pair_window: Duration::from_secs(var("LOGIN_PAIR_WINDOW_SECS", "900").parse().context("LOGIN_PAIR_WINDOW_SECS")?),
                pair_lockout: Duration::from_secs(var("LOGIN_PAIR_LOCKOUT_SECS", "900").parse().context("LOGIN_PAIR_LOCKOUT_SECS")?),
                account_failure_limit: var("LOGIN_ACCOUNT_FAILURES", "30").parse().context("LOGIN_ACCOUNT_FAILURES")?,
                account_window: Duration::from_secs(var("LOGIN_ACCOUNT_WINDOW_SECS", "3600").parse().context("LOGIN_ACCOUNT_WINDOW_SECS")?),
                tracked_accounts: var("LOGIN_TRACKED_ACCOUNTS", "100000").parse().context("LOGIN_TRACKED_ACCOUNTS")?,
            },
            key_pool_size: var("LOGIN_KEY_POOL", "8").parse().context("LOGIN_KEY_POOL")?,
            max_connections: var("LOGIN_MAX_CONNECTIONS", "5000").parse().context("LOGIN_MAX_CONNECTIONS")?,
        })
    }
}

fn flag(text: &str) -> bool {
    matches!(text.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

/// Decimal, or hexadecimal with a `0x` prefix.
pub fn parse_u32(text: &str) -> Result<u32> {
    let text = text.trim();
    Ok(match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16)?,
        None => text.parse()?,
    })
}

/// The original's rule: checking is a global switch, `0xFFFFFFFF` on either side means "any".
pub fn version_allowed(check: bool, server_version: Option<u32>, client_version: u32) -> bool {
    !check || client_version == u32::MAX || server_version.is_none_or(|v| v == u32::MAX || v == client_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_rules_match_the_original() {
        assert!(version_allowed(false, Some(5), 9), "checking off accepts everything");
        assert!(version_allowed(true, None, 9), "a server without a version accepts any client");
        assert!(version_allowed(true, Some(u32::MAX), 9), "0xFFFFFFFF on the server is a wildcard");
        assert!(version_allowed(true, Some(5), u32::MAX), "0xFFFFFFFF from the client is a wildcard");
        assert!(version_allowed(true, Some(5), 5));
        assert!(!version_allowed(true, Some(5), 9));
    }

    #[test]
    fn numbers_parse_as_decimal_or_hex() {
        assert_eq!(parse_u32("108").unwrap(), 108);
        assert_eq!(parse_u32("0x10042600").unwrap(), 268707328);
        assert!(parse_u32("nope").is_err());
    }
}
