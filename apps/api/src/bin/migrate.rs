use api::MIGRATOR;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to run migrations")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    MIGRATOR.run(&pool).await?;
    pool.close().await;
    Ok(())
}
