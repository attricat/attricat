use api::{MIGRATOR, config::Config, constants::MAINTENANCE_POOL_CONNECTIONS};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    let pool = PgPoolOptions::new()
        .max_connections(MAINTENANCE_POOL_CONNECTIONS)
        .connect(config.database.url.expose_secret())
        .await?;
    MIGRATOR.run(&pool).await?;
    pool.close().await;
    Ok(())
}
