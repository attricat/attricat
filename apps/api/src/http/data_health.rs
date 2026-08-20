use super::{AppState, error::ApiError, extractors::ApiQuery};
use crate::repository::RepositoryError;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

pub(super) async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DataHealthQuery {
    stale_after_days: Option<u16>,
}
fn stale_after_days(query: DataHealthQuery) -> Result<u16, ApiError> {
    let days = query.stale_after_days.unwrap_or(90);
    if !(1..=3650).contains(&days) {
        return Err(ApiError::invalid_input(
            "stale_after_days must be between 1 and 3650".to_owned(),
        ));
    }
    Ok(days)
}
async fn cached_data_health<T>(
    state: &AppState,
    key: String,
    load: impl std::future::Future<Output = Result<T, RepositoryError>>,
) -> Result<Json<Value>, ApiError>
where
    T: Serialize,
{
    if state.data_health_cache_ttl_seconds > 0 {
        if let Some((created_at, value)) = state.data_health_cache.lock().await.get(&key).cloned() {
            if created_at.elapsed() < Duration::from_secs(state.data_health_cache_ttl_seconds) {
                return Ok(Json(value));
            }
        }
    }
    let value = serde_json::to_value(load.await?).expect("data health response serializes");
    if state.data_health_cache_ttl_seconds > 0 {
        state
            .data_health_cache
            .lock()
            .await
            .insert(key, (std::time::Instant::now(), value.clone()));
    }
    Ok(Json(value))
}
pub(super) async fn invalidate_data_health(state: &AppState) {
    state.data_health_cache.lock().await.clear();
}
pub(super) async fn data_health_summary(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Json<Value>, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("summary:{days}"),
        state.repository.data_health_summary(days.into()),
    )
    .await
}
pub(super) async fn data_health_blueprints(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Json<Value>, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("blueprints:{days}"),
        state.repository.data_health_blueprints(days.into()),
    )
    .await
}
pub(super) async fn data_health_freshness(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "freshness".to_owned(),
        state.repository.data_health_freshness(),
    )
    .await
}
pub(super) async fn data_health_completeness(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "completeness".to_owned(),
        state.repository.data_health_completeness(),
    )
    .await
}
pub(super) async fn data_health_contexts(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "contexts".to_owned(),
        state.repository.data_health_contexts(),
    )
    .await
}
pub(super) async fn data_health_relationships(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "relationships".to_owned(),
        state.repository.data_health_relationships(),
    )
    .await
}
pub(super) async fn data_health_storage(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "storage".to_owned(),
        state.repository.data_health_storage(),
    )
    .await
}
pub(super) async fn refresh_data_health(State(state): State<AppState>) -> StatusCode {
    invalidate_data_health(&state).await;
    StatusCode::NO_CONTENT
}
