//! The operator console on the server machine (`docs/original/operator-console.md` section 6 step 6): a command for a game server,
//! carried out as the in-game `.command` of a level-1 GM is, and its answer printed.
//!
//!   console <server id> <command…>      e.g.  console 1 info      console 1 exp 1.5      console 1 help kick
//!
//! The command goes to the game server through PostgreSQL `NOTIFY` (`game_console`: `<server id> <request id> <command line>`) and
//! its answer comes back on `game_console_reply` (`<request id> <lines>`). Without an answer in 5 seconds the game server is not
//! running (or not listening yet). `console 1 help` lists the commands.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use sqlx::postgres::{PgListener, PgPoolOptions};

/// The game server's channels (`game_server::pg::CONSOLE_CHANNEL`, `CONSOLE_REPLY_CHANNEL`).
const CHANNEL: &str = "game_console";
const REPLY_CHANNEL: &str = "game_console_reply";
const ANSWER_WITHIN: Duration = Duration::from_secs(5);

const USAGE: &str = "usage: console <server id> <command…>   (console 1 help lists the commands)";

/// The answer lines in a reply to `request`, or `None` when it answers another request.
fn answer_to<'a>(request: &str, payload: &'a str) -> Option<&'a str> {
    payload.strip_prefix(request)?.strip_prefix(' ')
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server: u8 = args.first().context(USAGE)?.parse().context("the server id must be a number")?;
    if args.len() < 2 {
        bail!(USAGE);
    }
    let line = args[1..].join(" ");
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    let mut replies = PgListener::connect_with(&pool).await?;
    replies.listen(REPLY_CHANNEL).await?;
    let request = format!("{:08x}", rand::random::<u32>());
    sqlx::query("SELECT pg_notify($1, $2)").bind(CHANNEL).bind(format!("{server} {request} {line}")).execute(&pool).await?;
    let wait = async {
        loop {
            let reply = replies.recv().await?;
            if let Some(lines) = answer_to(&request, reply.payload()) {
                return Ok::<_, anyhow::Error>(lines.to_owned());
            }
        }
    };
    match tokio::time::timeout(ANSWER_WITHIN, wait).await {
        Ok(lines) => println!("{}", lines?),
        Err(_) => bail!("no answer (is game server {server} running?)"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_reply_to_the_request_counts() {
        assert_eq!(answer_to("0a1b2c3d", "0a1b2c3d players 1\nexp x1.000"), Some("players 1\nexp x1.000"));
        assert_eq!(answer_to("0a1b2c3d", "ffffffff players 1"), None);
        assert_eq!(answer_to("0a1b2c3d", "0a1b2c3dx"), None);
    }
}
