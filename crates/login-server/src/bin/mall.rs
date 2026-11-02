//! The item mall's operator side (`docs/original/item-mall.md` section 10, deviations 4, 5 and 8).
//!
//!   mall show <user>
//!   mall points <user> add <n> | take <n> | set <n>
//!   mall premium <user> <type 0 to 3> [<days>]
//!   mall deliver <user> <product code>
//!
//! Points belong to the account on every server; a character in the world sees a change at once. Premium (1: the warehouse's
//! second half; 2 and 3: also no wear, 1.5× experience and lapis links × 1.5) is read at the next world entry; without days it has
//! no end, type 0 ends it. `deliver` puts a product's items into the account's billing storage, seen at the next world entry.

use anyhow::{Context, Result, bail};
use login_server::pg;
use sqlx::postgres::PgPoolOptions;

const USAGE: &str =
    "usage: mall show <user> | points <user> add|take|set <n> | premium <user> <type 0 to 3> [<days>] | deliver <user> <product code>";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).context(USAGE)?;
    let user = args.get(1).context(USAGE)?;
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = PgPoolOptions::new().max_connections(1).connect(&url).await?;
    sqlx::migrate!("../../db/migrations").run(&pool).await?;
    let user_id = pg::account_by_name(&pool, user).await?;

    match command {
        "show" => {}
        "points" => {
            let n: u32 = args.get(3).context(USAGE)?.parse().context("the points must be a number")?;
            let (change, set) = match args.get(2).map(String::as_str) {
                Some("add") => (i64::from(n), None),
                Some("take") => (-i64::from(n), None),
                Some("set") => (0, Some(n)),
                _ => bail!(USAGE),
            };
            let points = pg::change_points(&pool, user_id, change, set).await?;
            println!("{user}: {points} points");
            return Ok(());
        }
        "premium" => {
            let premium: u8 = args.get(2).context(USAGE)?.parse().context("the type must be 0 to 3")?;
            let days: Option<f64> = args.get(3).map(|d| d.parse()).transpose().context("days must be a number")?;
            pg::set_premium(&pool, user_id, premium, days).await?;
        }
        "deliver" => {
            let code = args.get(2).context(USAGE)?;
            for (slot, item, count) in pg::deliver_product(&pool, user_id, code).await? {
                println!("billing storage slot {slot}: item {item} × {count}");
            }
        }
        _ => bail!(USAGE),
    }
    let r = pg::mall_record(&pool, user_id).await?;
    let until = match (r.premium, r.premium_until) {
        (0, _) => String::new(),
        (_, None) => " (no end)".to_owned(),
        (_, Some(until)) => format!(" (until {until})"),
    };
    println!("{user}: {} points, premium {}{until}, {} billing storage rows", r.points, r.premium, r.billing_items);
    Ok(())
}
