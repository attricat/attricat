//! Direct database reads and fixtures for state the API does not expose.

use std::{future::Future, time::Duration};

use sqlx::PgPool;
use uuid::Uuid;

use super::bootstrap_workspace_id;

/// Runs a `SELECT count(*)`-style query and returns its single value.
pub async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Polls `condition` until it holds, returning false if it still does not
/// after ten seconds. Callers panic with their own diagnostics.
pub async fn wait_until<F, Fut>(mut condition: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if condition().await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// Makes `channel` an enabled publication channel of the bootstrap workspace
/// and records `entity_id` as published in it by a fresh user.
pub async fn publish_in_channel(pool: &PgPool, entity_id: Uuid, channel: Uuid) {
    let workspace = bootstrap_workspace_id();
    sqlx::query(
        "INSERT INTO publication_channels(workspace_id,context_id,enabled) VALUES($1,$2,true)",
    )
    .bind(workspace)
    .bind(channel)
    .execute(pool)
    .await
    .unwrap();
    let publisher = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email) VALUES($1,$2)")
        .bind(publisher)
        .bind(format!("{publisher}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO entity_channel_publications(workspace_id,entity_id,context_id,published_at,published_by_user_id) VALUES($1,$2,$3,now(),$4)")
        .bind(workspace)
        .bind(entity_id)
        .bind(channel)
        .bind(publisher)
        .execute(pool)
        .await
        .unwrap();
}

/// Whether the entity is currently published in any channel.
pub async fn is_published(pool: &PgPool, entity_id: Uuid) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM entity_channel_publications WHERE entity_id=$1 AND published_at IS NOT NULL)",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .unwrap()
}
