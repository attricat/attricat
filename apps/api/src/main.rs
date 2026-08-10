use std::net::SocketAddr;

use api::{
    MIGRATOR,
    http::{AppState, router},
    repository::CatalogRepository,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to start the API")?;
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    MIGRATOR.run(&pool).await?;

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    println!("API listening on http://{}", listener.local_addr()?);
    axum::serve(
        listener,
        router(AppState {
            repository: CatalogRepository::new(pool),
        }),
    )
    .await?;

    Ok(())
}
