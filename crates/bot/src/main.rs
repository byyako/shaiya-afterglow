//! `bot`: the test bots' command line. Built so far: `probe`, `seed` and `unseed`; `swarm` comes in a later part.
//! Exit codes: 0 done, 1 an error, 2 refused (as `data-editor`).

use std::process::ExitCode;

use anyhow::Context;
use bot::seed::{self, Refused, cli};
use sqlx::postgres::PgPoolOptions;

const USAGE: &str = "usage: bot probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]\n       \
                     bot seed --count <n> [--prefix bot] [--map 18] [--level 60] [--server 1] [--template <character id>]...\n       \
                     bot unseed [--prefix bot]\n       \
                     bot swarm   (not built yet)";

/// The database of the game, `DATABASE_URL`, as the other tools read it.
async fn connect() -> anyhow::Result<sqlx::PgPool> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    Ok(PgPoolOptions::new().max_connections(4).connect(&url).await?)
}

async fn run_seed(args: &[String]) -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let password = std::env::var("SHAIYA_PASSWORD").context("SHAIYA_PASSWORD (every bot account's password) is not set")?;
    let options = cli::parse_seed(args, &password)?;
    let pool = connect().await?;
    let defs = seed::Defs::load(&pool).await?;
    let seeded = seed::seed(&pool, &defs, &options).await?;
    for bot in &seeded.bots {
        println!(
            "{} {} (id {}) {} race {} class {} level {} hp/mp/sp {:?} weapon {:?} range {} gear {} skills {} at {:?}",
            if bot.reset { "reset" } else { "made " },
            bot.username,
            bot.character_id,
            bot.name,
            bot.family,
            bot.job,
            bot.level,
            bot.health,
            bot.weapon,
            bot.range,
            bot.gear,
            bot.skills,
            bot.position
        );
    }
    println!("{} made, {} put back", seeded.created(), seeded.reset());
    Ok(())
}

async fn run_unseed(args: &[String]) -> anyhow::Result<()> {
    let prefix = cli::parse_unseed(args)?;
    let pool = connect().await?;
    let removed = seed::unseed(&pool, &prefix).await?;
    println!("{removed} bot account(s) removed");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("probe") => bot::probe::run(&args[1..]).await,
        Some("seed") => run_seed(&args[1..]).await,
        Some("unseed") => run_unseed(&args[1..]).await,
        Some("swarm") => Err(anyhow::anyhow!("bot swarm is not built yet")),
        _ => Err(anyhow::anyhow!("{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(if error.downcast_ref::<Refused>().is_some() { 2 } else { 1 })
        }
    }
}
