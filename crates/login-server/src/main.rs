use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use login_server::config::Config;
use login_server::keys::KeyPool;
use login_server::pg::PgBackend;
use login_server::server::{Context, serve};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;

    let started = Instant::now();
    let pool_size = config.key_pool_size;
    let keys = tokio::task::spawn_blocking(move || KeyPool::generate(pool_size)).await?;
    tracing::info!(keys = pool_size, elapsed = ?started.elapsed(), "RSA key pool ready");

    let pool = PgPoolOptions::new().max_connections(16).connect(&config.database_url).await?;
    sqlx::migrate!("../../db/migrations").run(&pool).await?;

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "login server listening");

    let backend = PgBackend::new(pool, config.heartbeat_timeout);
    let ctx = Arc::new(Context::new(config, keys, backend));

    let cleanup = ctx.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tick.tick().await;
            match cleanup.store.purge_expired_sessions().await {
                Ok(0) => {}
                Ok(n) => tracing::debug!(removed = n, "expired sessions removed"),
                Err(error) => tracing::warn!(%error, "session cleanup failed"),
            }
        }
    });

    tokio::select! {
        result = serve(listener, ctx) => result,
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutting down");
            Ok(())
        }
    }
}
