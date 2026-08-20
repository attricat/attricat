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
    let max_preview_relationship_depth = std::env::var("PREVIEW_MAX_RELATIONSHIP_DEPTH")
        .unwrap_or_else(|_| "3".to_owned())
        .parse()?;
    let max_preview_relationship_items = std::env::var("PREVIEW_MAX_RELATIONSHIP_ITEMS")
        .unwrap_or_else(|_| "10".to_owned())
        .parse()?;
    let max_entity_page_size = std::env::var("ENTITY_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()?;
    let max_incoming_relationship_page_size = std::env::var("INCOMING_RELATIONSHIP_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| "50".to_owned())
        .parse()?;
    let max_relationship_facet_nodes = std::env::var("RELATIONSHIP_FACET_MAX_NODES")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    MIGRATOR.run(&pool).await?;

    let history_retention_days = std::env::var("ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS")
        .unwrap_or_else(|_| "90".to_owned())
        .parse()?;
    CatalogRepository::new(pool.clone())
        .purge_value_history(history_retention_days)
        .await?;

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    println!("API listening on http://{}", listener.local_addr()?);
    axum::serve(
        listener,
        router(AppState {
            repository: CatalogRepository::new(pool),
            max_preview_relationship_depth,
            max_preview_relationship_items,
            max_entity_page_size,
            max_incoming_relationship_page_size,
            max_relationship_facet_nodes,
            data_health_cache_ttl_seconds: std::env::var("DATA_HEALTH_CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_owned())
                .parse()?,
            data_health_cache: Default::default(),
        }),
    )
    .await?;

    Ok(())
}
