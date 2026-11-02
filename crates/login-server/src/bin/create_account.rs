//! Creates an account in the login database.
//!
//! usage: create_account <username> [status]
//! Status uses the original values: 0 normal account (default), 16 full GM, 32/48/64/80 lesser GM levels.
//! The password is read from the SHAIYA_PASSWORD environment variable (NEWSHAIYA_PASSWORD, its old name, still works), or from
//! standard input if neither is set, so it does not appear in the process list.

use std::io::BufRead;

use anyhow::{Context, Result};
use login_server::accounts::PgAccountStore;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let mut args = std::env::args().skip(1);
    let username = args.next().context("usage: create_account <username> [status]")?;
    let status: i16 = args.next().map(|p| p.parse()).transpose().context("status must be a number such as 0 or 16")?.unwrap_or(0);

    let password = match std::env::var("SHAIYA_PASSWORD").or_else(|_| std::env::var("NEWSHAIYA_PASSWORD")) {
        Ok(password) => password,
        Err(_) => {
            eprintln!("password for {username}:");
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            line.trim_end_matches(['\r', '\n']).to_owned()
        }
    };

    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = PgPoolOptions::new().max_connections(1).connect(&url).await?;
    sqlx::migrate!("../../db/migrations").run(&pool).await?;
    let id = PgAccountStore::new(pool).create_account(&username, &password, status).await?;
    println!("created account {username} with id {id}");
    Ok(())
}
