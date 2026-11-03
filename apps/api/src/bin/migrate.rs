use api::{
    MIGRATOR, config::Config, constants::MAINTENANCE_POOL_CONNECTIONS, migration_connect_options,
};
use secrecy::ExposeSecret;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    let pool = PgPoolOptions::new()
        .max_connections(MAINTENANCE_POOL_CONNECTIONS)
        .connect_with(migration_connect_options(
            config
                .database
                .url
                .expose_secret()
                .parse::<PgConnectOptions>()?,
        ))
        .await?;
    MIGRATOR.run(&pool).await?;
    pool.close().await;
    Ok(())
}
