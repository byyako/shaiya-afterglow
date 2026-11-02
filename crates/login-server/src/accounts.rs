use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use shaiya_proto::login::LoginStatus;
use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::status::{Decision, decide};

pub use shaiya_proto::login::{CREDENTIAL_MAX_LEN, PASSWORD_MAX_LEN};

/// Password hashes running at once. Each costs tens of milliseconds and about 19 MiB, so a flood of
/// logins queues here instead of using every core.
static HASH_SLOTS: Semaphore = Semaphore::const_new(4);

#[derive(Debug, PartialEq, Eq)]
pub enum AuthResult {
    Accepted {
        user_id: u32,
        privilege: u8,
    },
    /// Unknown name, or deleted account.
    UnknownAccount,
    WrongPassword,
    /// Password was right but the account status refuses the login.
    Refused(LoginStatus),
}

/// Where accounts live. The server only needs one question answered.
pub trait AccountStore: Send + Sync + 'static {
    fn authenticate(&self, username: &str, password: &str, ip: IpAddr) -> impl Future<Output = Result<AuthResult>> + Send;
}

/// A new password is at least this long (`docs/server/audit.md` R7); accounts made with shorter ones still log in.
pub const PASSWORD_MIN_LEN: usize = 6;

/// Names and passwords must fit what the client can send (the original cut both at 12; a password may have 16 now).
pub fn validate_credentials(username: &str, password: &str) -> Result<()> {
    let ok_name =
        !username.is_empty() && username.len() <= CREDENTIAL_MAX_LEN && username.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !ok_name {
        bail!("username must be 1-{CREDENTIAL_MAX_LEN} letters, digits or underscores");
    }
    if !(PASSWORD_MIN_LEN..=PASSWORD_MAX_LEN).contains(&password.len()) || !password.bytes().all(|b| b.is_ascii_graphic()) {
        bail!("password must be {PASSWORD_MIN_LEN}-{PASSWORD_MAX_LEN} printable ASCII characters");
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String> {
    Ok(Argon2::default().hash_password(password.as_bytes()).map_err(|e| anyhow::anyhow!("hashing failed: {e}"))?.to_string())
}

fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()).unwrap_or(false)
}

/// Verified against when the account does not exist, so a miss costs the same as a wrong password.
fn dummy_hash() -> &'static str {
    use std::sync::OnceLock;
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| hash_password("not-a-real-password").expect("argon2 works"))
}

async fn check_password(hash: String, password: String) -> Result<bool> {
    let _slot = HASH_SLOTS.acquire().await?;
    Ok(tokio::task::spawn_blocking(move || verify_password(&hash, &password)).await?)
}

/// The shared login decision: password first (so nobody learns an account's state without
/// knowing its password), then the status rules.
fn decide_login(user_id: u32, status: i16, password_ok: bool) -> AuthResult {
    if !password_ok {
        return AuthResult::WrongPassword;
    }
    match decide(status) {
        Decision::Allowed { privilege } => AuthResult::Accepted { user_id, privilege },
        Decision::Refused(LoginStatus::NoSuchAccount) => AuthResult::UnknownAccount,
        Decision::Refused(LoginStatus::BadCredentials) => AuthResult::WrongPassword,
        Decision::Refused(status) => AuthResult::Refused(status),
    }
}

pub struct PgAccountStore {
    pool: PgPool,
}

impl PgAccountStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `status` uses the original values: 0 normal, 16 to 80 staff (16 is full operator), others block.
    pub async fn create_account(&self, username: &str, password: &str, status: i16) -> Result<u32> {
        validate_credentials(username, password)?;
        let hash = tokio::task::spawn_blocking({
            let password = password.to_owned();
            move || hash_password(&password)
        })
        .await??;
        let id: i32 = sqlx::query_scalar("INSERT INTO accounts (username, password_hash, status) VALUES ($1, $2, $3) RETURNING id")
            .bind(username)
            .bind(hash)
            .bind(status)
            .fetch_one(&self.pool)
            .await
            .context("could not create account (name already taken?)")?;
        Ok(id as u32)
    }

    async fn log(&self, account_id: Option<i32>, username: &str, ip: IpAddr, outcome: &str) {
        let result = sqlx::query("INSERT INTO login_log (account_id, username, ip, outcome) VALUES ($1, $2, $3::inet, $4)")
            .bind(account_id)
            .bind(username)
            .bind(ip.to_string())
            .bind(outcome)
            .execute(&self.pool)
            .await;
        if let Err(error) = result {
            tracing::warn!(%error, "could not write login log");
        }
    }
}

impl AccountStore for PgAccountStore {
    async fn authenticate(&self, username: &str, password: &str, ip: IpAddr) -> Result<AuthResult> {
        // `block_expired` mirrors the original: a timed block (status 2 to 6) lifts itself once its end date passes.
        let row: Option<(i32, String, i16, bool, bool)> = sqlx::query_as(
            "SELECT id, password_hash, status,
                    COALESCE(blocked_until <= now(), false) AS block_expired, deleted
               FROM accounts WHERE lower(username) = lower($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        let hash = row.as_ref().map(|r| r.1.clone()).unwrap_or_else(|| dummy_hash().to_owned());
        let password_ok = check_password(hash, password.to_owned()).await?;

        let result = match &row {
            None | Some((_, _, _, _, true)) => AuthResult::UnknownAccount,
            Some((id, _, status, block_expired, _)) => {
                let mut status = *status;
                if password_ok && *block_expired && (2..=6).contains(&status) {
                    sqlx::query("UPDATE accounts SET status = 0, blocked_until = NULL WHERE id = $1").bind(id).execute(&self.pool).await?;
                    status = 0;
                }
                decide_login(*id as u32, status, password_ok)
            }
        };

        let (label, account_id) = match (&result, &row) {
            (AuthResult::Accepted { user_id, .. }, _) => ("ok".to_owned(), Some(*user_id as i32)),
            (AuthResult::Refused(status), Some(r)) => (format!("refused_{status:?}").to_lowercase(), Some(r.0)),
            (AuthResult::WrongPassword, _) => ("bad_password".to_owned(), None),
            _ => ("unknown".to_owned(), None),
        };
        self.log(account_id, username, ip, &label).await;
        Ok(result)
    }
}

/// In-memory accounts for tests and quick experiments.
#[derive(Default)]
pub struct MemoryAccountStore {
    accounts: Mutex<HashMap<String, (u32, String, i16)>>,
}

impl MemoryAccountStore {
    pub fn add(&self, username: &str, password: &str, status: i16) -> Result<u32> {
        validate_credentials(username, password)?;
        let mut accounts = self.accounts.lock().unwrap();
        let id = accounts.len() as u32 + 1;
        accounts.insert(username.to_lowercase(), (id, hash_password(password)?, status));
        Ok(id)
    }
}

impl AccountStore for MemoryAccountStore {
    async fn authenticate(&self, username: &str, password: &str, _ip: IpAddr) -> Result<AuthResult> {
        let found = self.accounts.lock().unwrap().get(&username.to_lowercase()).cloned();
        let hash = found.as_ref().map(|a| a.1.clone()).unwrap_or_else(|| dummy_hash().to_owned());
        let ok = check_password(hash, password.to_owned()).await?;
        Ok(match found {
            None => AuthResult::UnknownAccount,
            Some((user_id, _, status)) => decide_login(user_id, status, ok),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_password_never_reveals_the_account_status() {
        assert_eq!(decide_login(1, 2, false), AuthResult::WrongPassword);
        assert_eq!(decide_login(1, 0x80, false), AuthResult::WrongPassword);
    }

    #[test]
    fn status_rules_apply_once_the_password_is_right() {
        assert_eq!(decide_login(7, 0, true), AuthResult::Accepted { user_id: 7, privilege: 11 });
        assert_eq!(decide_login(7, 16, true), AuthResult::Accepted { user_id: 7, privilege: 1 });
        assert_eq!(decide_login(7, 2, true), AuthResult::Refused(LoginStatus::NotAllowed));
        assert_eq!(decide_login(7, -3, true), AuthResult::UnknownAccount);
        assert_eq!(decide_login(7, -1, true), AuthResult::WrongPassword);
    }

    #[test]
    fn new_passwords_need_six_to_sixteen_characters() {
        assert!(validate_credentials("alice", "abcde").is_err(), "5 is too short");
        assert!(validate_credentials("alice", "abcdef").is_ok());
        assert!(validate_credentials("alice", "abcdefghijklmnop").is_ok(), "16 is what the client sends");
        assert!(validate_credentials("alice", "abcdefghijklmnopq").is_err(), "17 is too long");
        assert!(MemoryAccountStore::default().add("alice", "abc", 0).is_err());
    }
}
