#![allow(dead_code, unused_imports)]

use std::net::SocketAddr;

use api::{
    http::{AppState, router},
    repository::CatalogRepository,
    telemetry::init_metrics,
};
pub use reqwest::{Client, StatusCode};
pub use serde_json::{Value, json};
pub use sqlx::PgPool;
pub use tokio::{net::TcpListener, task::JoinHandle};
pub use uuid::Uuid;

pub async fn start_server(pool: PgPool) -> (String, JoinHandle<()>) {
    start_server_with_data_health_cache_ttl(pool, 0).await
}

pub async fn start_server_with_data_health_cache_ttl(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let router = router(AppState {
        repository: CatalogRepository::new(pool),
        metrics: init_metrics().unwrap(),
        max_preview_relationship_depth: 3,
        max_preview_relationship_items: 10,
        max_entity_page_size: 100,
        max_incoming_relationship_page_size: 50,
        max_relationship_facet_nodes: 100,
        data_health_cache_ttl_seconds,
        data_health_cache: Default::default(),
    });
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    (format!("http://{address}"), server)
}

pub async fn create_blueprint(client: &Client, base_url: &str, definition: &str) -> Value {
    let blueprint: Value = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{}/versions/{}/publish",
            blueprint["blueprint"]["id"].as_str().unwrap(),
            blueprint["blueprint"]["version"].as_i64().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn create_entity(client: &Client, base_url: &str, blueprint: &Value) -> Value {
    client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"],
            },
            "values": [],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
