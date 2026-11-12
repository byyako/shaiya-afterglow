//! `bot`: the test bots' command line. Part 1 has `probe`; `seed`, `unseed` and `swarm` come in later parts.
//! Exit codes: 0 done, 1 an error, 2 refused (as `data-editor`).

use std::process::ExitCode;

const USAGE: &str = "usage: bot probe <login host:port> <username> <password> [server id] [create <name> | delete <id> | restore <id>]\n       \
                     bot seed | unseed | swarm   (not built yet)";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("probe") => bot::probe::run(&args[1..]).await,
        Some(command @ ("seed" | "unseed" | "swarm")) => Err(anyhow::anyhow!("bot {command} is not built yet")),
        _ => Err(anyhow::anyhow!("{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(1)
        }
    }
}
