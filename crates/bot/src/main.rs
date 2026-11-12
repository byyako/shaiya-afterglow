//! `bot`: the test bots' command line: `probe`, `seed`, `unseed` and `swarm`.
//! Exit codes: 0 done, 1 an error, 2 refused (as `data-editor`).

use std::process::ExitCode;

use anyhow::Context;
use bot::seed::{self, Refused, cli};
use bot::swarm;
use sqlx::postgres::PgPoolOptions;

const USAGE: &str = "usage: bot probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]\n       \
                     bot seed --count <n> [--prefix bot] [--map 18] [--level 60] [--server 1] [--template <character id>]...\n       \
                     bot unseed [--prefix bot]\n       \
                     bot swarm --login <host:30800> --count <n> [--first 1] [--prefix bot] [--server 1] [--ramp 5] [--at <map,x,z>] [--radius 60]\n       \
                     [--duration 10m] [--steps-per-sec 2] [--skill-every 6s] [--rebirth-after 5s] [--idle-share 0] [--report report.json]";

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

async fn run_swarm(args: &[String]) -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let password = std::env::var("SHAIYA_PASSWORD").context("SHAIYA_PASSWORD (every bot account's password) is not set")?;
    let options = swarm::cli::parse_swarm(args, &password)?;
    let pool = connect().await?;
    let defs = seed::Defs::load(&pool).await?;
    let setup = swarm::prepare(&pool, &defs, &options).await?;
    pool.close().await;
    let armed = setup.plans.iter().filter(|p| p.config.weapon_range > 0).count();
    println!(
        "{} bots ({} to {}), battle at map {} ({:.0}, {:.0}), {} a second, {} s; {} with a weapon range above 0, {} skills for the first bot",
        setup.plans.len(),
        options.first,
        options.first + options.count - 1,
        setup.map,
        setup.point[0],
        setup.point[1],
        options.ramp,
        options.duration.as_secs(),
        armed,
        setup.plans[0].config.skills.len()
    );
    let note = std::sync::Arc::new(std::sync::OnceLock::new());
    let enter = swarm::login_entry(&options, note.clone());
    let args = options.args_json(setup.map, setup.point);
    let stop = async {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("stopping: logging the bots out");
    };
    let report = swarm::run(setup.plans, enter, &options, args, &note, stop).await;
    std::fs::write(&options.report, serde_json::to_string_pretty(&report.json())?)
        .with_context(|| format!("writing {}", options.report.display()))?;
    println!("{report}");
    println!("report: {}", options.report.display());
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
        Some("swarm") => run_swarm(&args[1..]).await,
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
