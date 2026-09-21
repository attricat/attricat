use super::{AppState, auth::ActiveWorkspace, error::ApiError, extractors::ApiQuery};
use crate::{
    constants::{DEFAULT_STALE_AFTER_DAYS, MAX_STALE_AFTER_DAYS},
    repository::RepositoryError,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

const MAX_DATA_HEALTH_CACHE_ENTRIES: usize = 256;

/// Liveness only establishes that this process can serve HTTP. It never
/// probes dependencies, so orchestration does not restart a healthy process
/// during a database or object-store outage.
pub(super) async fn liveness() -> Json<Value> {
    Json(json!({ "status": "live" }))
}

/// Readiness requires every synchronous request dependency. A failed probe is
/// deliberately a generic 503 response: dependency topology and credentials
/// are operational details, not public API data.
pub(super) async fn readiness(State(state): State<AppState>) -> Response {
    let database = state.repository.readiness().await;
    let storage = state.object_store.readiness().await;
    if database.is_ok() && storage.is_ok() {
        (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready" })),
        )
            .into_response()
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DataHealthQuery {
    stale_after_days: Option<u16>,
}
fn stale_after_days(query: DataHealthQuery) -> Result<u16, ApiError> {
    let days = query.stale_after_days.unwrap_or(DEFAULT_STALE_AFTER_DAYS);
    if !(1..=MAX_STALE_AFTER_DAYS).contains(&days) {
        return Err(ApiError::invalid_input(format!(
            "stale_after_days must be between 1 and {MAX_STALE_AFTER_DAYS}"
        )));
    }
    Ok(days)
}
async fn cached_data_health<T>(
    state: &AppState,
    key: String,
    load: impl std::future::Future<Output = Result<T, RepositoryError>>,
) -> Result<Response, ApiError>
where
    T: Serialize,
{
    let (value, cache_status) = if state.data_health_cache_ttl_seconds > 0 {
        let ttl = Duration::from_secs(state.data_health_cache_ttl_seconds);
        let cached = {
            let mut cache = state.data_health_cache.lock().await;
            cache.retain(|_, (created_at, _)| created_at.elapsed() < ttl);
            cache.get(&key).cloned()
        };
        if let Some((_, value)) = cached {
            (value, "HIT")
        } else {
            let value = serde_json::to_value(load.await?).expect("data health response serializes");
            (value, "MISS")
        }
    } else {
        let value = serde_json::to_value(load.await?).expect("data health response serializes");
        (value, "BYPASS")
    };
    if state.data_health_cache_ttl_seconds > 0 && cache_status == "MISS" {
        let mut cache = state.data_health_cache.lock().await;
        if cache.len() >= MAX_DATA_HEALTH_CACHE_ENTRIES
            && let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, (created_at, _))| *created_at)
                .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest_key);
        }
        cache.insert(key, (std::time::Instant::now(), value.clone()));
    }
    metrics::counter!("catalog_data_health_cache_total", "status" => cache_status).increment(1);
    let mut response = Json(value).into_response();
    response.headers_mut().append(
        HeaderName::from_static("server-timing"),
        HeaderValue::from_static(match cache_status {
            "HIT" => "cache;desc=HIT",
            "MISS" => "cache;desc=MISS",
            "BYPASS" => "cache;desc=BYPASS",
            _ => unreachable!("cache status is fixed above"),
        }),
    );
    Ok(response)
}
pub(super) async fn invalidate_data_health(state: &AppState) {
    state.data_health_cache.lock().await.clear();
}
pub(super) async fn data_health_summary(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Response, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("{workspace_id}:summary:{days}"),
        repository.data_health_summary(days.into()),
    )
    .await
}
pub(super) async fn data_health_blueprints(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Response, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("{workspace_id}:blueprints:{days}"),
        repository.data_health_blueprints(days.into()),
    )
    .await
}
pub(super) async fn data_health_freshness(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    cached_data_health(
        &state,
        format!("{workspace_id}:freshness"),
        repository.data_health_freshness(),
    )
    .await
}
pub(super) async fn data_health_completeness(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    cached_data_health(
        &state,
        format!("{workspace_id}:completeness"),
        repository.data_health_completeness(),
    )
    .await
}
pub(super) async fn data_health_contexts(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    cached_data_health(
        &state,
        format!("{workspace_id}:contexts"),
        repository.data_health_contexts(),
    )
    .await
}
pub(super) async fn data_health_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    cached_data_health(
        &state,
        format!("{workspace_id}:relationships"),
        repository.data_health_relationships(),
    )
    .await
}
pub(super) async fn data_health_storage(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    cached_data_health(
        &state,
        format!("{workspace_id}:storage"),
        repository.data_health_storage(),
    )
    .await
}
pub(super) async fn refresh_data_health(State(state): State<AppState>) -> StatusCode {
    invalidate_data_health(&state).await;
    StatusCode::NO_CONTENT
}
