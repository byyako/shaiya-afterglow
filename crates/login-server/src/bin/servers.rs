//! Manage the game servers the login server lists.
//!
//!   servers list
//!   servers add <id> <name> <ipv4> [capacity] [client version]
//!   servers remove <id>
//!   servers enable <id> | disable <id>
//!   servers heartbeat <id> [players] [--every <seconds>]
//!
//! Every game server needs its own IPv4 address: the client always connects to port 30810 and
//! the login reply carries only an address. A server is listed as open while it is enabled and its
//! heartbeat is recent. `heartbeat` stands in for a game server until a real one exists.

use std::net::Ipv4Addr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use login_server::config::parse_u32;
use login_server::pg;
use sqlx::postgres::PgPoolOptions;

const USAGE: &str = "usage: servers list | add <id> <name> <ipv4> [capacity] [version] | remove <id> | enable <id> | disable <id> | heartbeat <id> [players] [--every <seconds>]";

fn id_arg(args: &[String], at: usize) -> Result<u8> {
    args.get(at).context(USAGE)?.parse().context("id must be a number from 0 to 255")
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).context(USAGE)?;
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = PgPoolOptions::new().max_connections(1).connect(&url).await?;
    sqlx::migrate!("../../db/migrations").run(&pool).await?;

    match command {
        "list" => {
            println!("{:>3}  {:<20} {:<15} {:>8} {:>10}  {:<8} heartbeat", "id", "name", "address", "players", "version", "enabled");
            for s in pg::list_servers(&pool).await? {
                let heartbeat = s.seen_ago.map_or("never".to_owned(), |t| format!("{t:.0}s ago"));
                let version = s.version.map_or("any".to_owned(), |v| format!("{v:#x}"));
                println!(
                    "{:>3}  {:<20} {:<15} {:>4}/{:<4} {:>10}  {:<8} {}",
                    s.id, s.name, s.address, s.players, s.capacity, version, s.enabled, heartbeat
                );
            }
        }
        "add" => {
            let id = id_arg(&args, 1)?;
            let name = args.get(2).context(USAGE)?;
            let address: Ipv4Addr = args.get(3).context(USAGE)?.parse().context("address must be an IPv4 address")?;
            let capacity: u16 = args.get(4).map(|c| c.parse()).transpose().context("capacity must be a number")?.unwrap_or(1000);
            let version = args.get(5).map(|v| parse_u32(v)).transpose()?;
            pg::add_server(&pool, id, name, address, capacity, version).await?;
            println!("added server {id} '{name}' at {address}");
        }
        "remove" => {
            pg::remove_server(&pool, id_arg(&args, 1)?).await?;
            println!("removed");
        }
        "enable" | "disable" => {
            pg::set_enabled(&pool, id_arg(&args, 1)?, command == "enable").await?;
            println!("{command}d");
        }
        "heartbeat" => {
            let id = id_arg(&args, 1)?;
            let mut players: u16 = 0;
            let mut every: Option<f64> = None;
            let mut rest = args[2..].iter();
            while let Some(arg) = rest.next() {
                if arg == "--every" {
                    every = Some(rest.next().context("--every needs seconds")?.parse()?);
                } else {
                    players = arg.parse().context("players must be a number")?;
                }
            }
            match every {
                None => {
                    pg::heartbeat(&pool, id, players).await?;
                    println!("heartbeat sent for server {id} ({players} players)");
                }
                Some(seconds) => {
                    if seconds <= 0.0 {
                        bail!("--every must be positive");
                    }
                    println!("sending a heartbeat for server {id} every {seconds}s ({players} players); Ctrl-C to stop");
                    let mut tick = tokio::time::interval(Duration::from_secs_f64(seconds));
                    loop {
                        tokio::select! {
                            _ = tick.tick() => pg::heartbeat(&pool, id, players).await?,
                            _ = tokio::signal::ctrl_c() => break,
                        }
                    }
                }
            }
        }
        _ => bail!(USAGE),
    }
    Ok(())
}
