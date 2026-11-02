//! The data editor (`docs/plans/done/data-editing.md`): changes to the game's data kept in the database with their history, applied again
//! after every fresh load of the original files, undone, exported and imported.

pub mod api;
pub mod apply;
pub mod console;
pub mod export;
pub mod images;
pub mod registry;
pub mod serve;
pub mod store;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;

pub use store::Editor;

/// Connects to `DATABASE_URL` and runs the migrations (which create the `edits` schema), as the other binaries do.
pub async fn connect() -> Result<Editor> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let pool = PgPoolOptions::new().max_connections(4).connect(&url).await.context("connecting to the database")?;
    sqlx::migrate!("../../db/migrations").run(&pool).await.context("running the migrations")?;
    Ok(Editor::new(pool, registry::Schemas::default()))
}
