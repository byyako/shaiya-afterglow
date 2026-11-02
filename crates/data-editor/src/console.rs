//! One request to a game server's operator console through PostgreSQL `NOTIFY`, as `login-server`'s `console` tool does
//! (`docs/original/operator-console.md` section 6): `game_console` carries `<server id> <request id> <command line>`, the answer comes
//! back on `game_console_reply` as `<request id> <lines>`.

use std::time::Duration;

use anyhow::{Result, bail};
use sqlx::PgPool;
use sqlx::postgres::PgListener;

/// The game server's channels (`game_server::pg::CONSOLE_CHANNEL`, `CONSOLE_REPLY_CHANNEL`).
const CHANNEL: &str = "game_console";
const REPLY_CHANNEL: &str = "game_console_reply";
/// A `reload` reads the database, so the wait is longer than the console tool's 5 seconds.
pub const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// The answer lines in a reply to `request`, or `None` when it answers another request.
pub fn answer_to<'a>(request: &str, payload: &'a str) -> Option<&'a str> {
    payload.strip_prefix(request)?.strip_prefix(' ')
}

/// Sends `line` to game server `server` and waits for its answer.
pub async fn request(pool: &PgPool, server: u8, line: &str) -> Result<String> {
    request_within(pool, server, line, ANSWER_WITHIN).await
}

/// The same with a shorter or longer wait (the page asks `info` of every server and cannot wait 15 seconds for one that is down).
pub async fn request_within(pool: &PgPool, server: u8, line: &str, within: Duration) -> Result<String> {
    let mut replies = PgListener::connect_with(pool).await?;
    replies.listen(REPLY_CHANNEL).await?;
    let id = format!("{:08x}", rand::random::<u32>());
    sqlx::query("SELECT pg_notify($1, $2)").bind(CHANNEL).bind(format!("{server} {id} {line}")).execute(pool).await?;
    let wait = async {
        loop {
            let reply = replies.recv().await?;
            if let Some(lines) = answer_to(&id, reply.payload()) {
                return Ok::<_, anyhow::Error>(lines.to_owned());
            }
        }
    };
    match tokio::time::timeout(within, wait).await {
        Ok(lines) => lines,
        Err(_) => bail!("no answer (is game server {server} running?)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_reply_to_the_request_counts() {
        assert_eq!(answer_to("0a1b2c3d", "0a1b2c3d reloaded: 1"), Some("reloaded: 1"));
        assert_eq!(answer_to("0a1b2c3d", "ffffffff reloaded: 1"), None);
        assert_eq!(answer_to("0a1b2c3d", "0a1b2c3dx"), None);
    }
}
