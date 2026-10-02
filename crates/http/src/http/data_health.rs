use super::{AppState, auth::ActiveWorkspace, error::ApiError, extractors::ApiQuery};
use crate::{
    constants::{DEFAULT_STALE_AFTER_DAYS, MAX_STALE_AFTER_DAYS},
    repository::{CatalogRepository, RepositoryError},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_DATA_HEALTH_CACHE_ENTRIES: usize = 256;

pub(super) async fn background_processing_status(
    State(state): State<AppState>,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<Vec<crate::repository::BackgroundProcessingStatus>>, ApiError> {
    Ok(Json(
        state
            .repository
            .background_processing_status_for_workspace(workspace_id)
            .await?,
    ))
}

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
    let Ok(_permit) = state.readiness_permits.try_acquire() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready" })),
        )
            .into_response();
    };
    let deadline = Duration::from_secs(2);
    let (database, storage) = tokio::join!(
        tokio::time::timeout(deadline, state.repository.readiness()),
        tokio::time::timeout(deadline, state.object_store.readiness()),
    );
    let database_ready = matches!(database, Ok(Ok(())));
    let storage_ready = matches!(storage, Ok(Ok(())));
    metrics::gauge!("catalog_database_ready").set(if database_ready { 1.0 } else { 0.0 });
    // Object-store readiness is also recorded at the storage boundary, but set
    // it here so every readiness response has a complete dependency snapshot.
    metrics::gauge!("catalog_object_store_ready").set(if storage_ready { 1.0 } else { 0.0 });
    if database_ready && storage_ready {
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
/// Workspace-partitioned cache of aggregate data-health responses.
///
/// Each workspace has a generation that invalidation advances, so a load that
/// started before a mutation cannot repopulate the cache with stale data.
#[derive(Clone, Default)]
pub struct DataHealthCache(Arc<Mutex<DataHealthCacheEntries>>);

#[derive(Default)]
struct DataHealthCacheEntries {
    values: HashMap<(Uuid, String), (Instant, Value)>,
    generations: HashMap<Uuid, u64>,
}

impl DataHealthCache {
    fn lock(&self) -> std::sync::MutexGuard<'_, DataHealthCacheEntries> {
        // Entries are plain data; a panic mid-update cannot leave them unsafe
        // to read, so recover from poisoning rather than failing every request.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Returns the live cached value, or the generation a subsequent
    /// [`Self::insert`] must match.
    fn get(&self, workspace_id: Uuid, key: &str, ttl: Duration) -> Result<Value, u64> {
        let mut entries = self.lock();
        entries
            .values
            .retain(|_, (created_at, _)| created_at.elapsed() < ttl);
        match entries.values.get(&(workspace_id, key.to_owned())) {
            Some((_, value)) => Ok(value.clone()),
            None => Err(entries
                .generations
                .get(&workspace_id)
                .copied()
                .unwrap_or_default()),
        }
    }

    fn insert(&self, workspace_id: Uuid, key: String, value: Value, generation: u64) {
        let mut entries = self.lock();
        if entries
            .generations
            .get(&workspace_id)
            .copied()
            .unwrap_or_default()
            != generation
        {
            return;
        }
        if entries.values.len() >= MAX_DATA_HEALTH_CACHE_ENTRIES
            && let Some(oldest_key) = entries
                .values
                .iter()
                .min_by_key(|(_, (created_at, _))| *created_at)
                .map(|(key, _)| key.clone())
        {
            entries.values.remove(&oldest_key);
        }
        entries
            .values
            .insert((workspace_id, key), (Instant::now(), value));
    }

    fn invalidate(&self, workspace_id: Uuid) {
        let mut entries = self.lock();
        entries
            .values
            .retain(|(cached_workspace, _), _| *cached_workspace != workspace_id);
        *entries.generations.entry(workspace_id).or_default() += 1;
    }
}

async fn cached_data_health<T>(
    state: &AppState,
    workspace_id: Uuid,
    key: String,
    load: impl std::future::Future<Output = Result<T, RepositoryError>>,
) -> Result<Response, ApiError>
where
    T: Serialize,
{
    let (value, cache_status) = if state.data_health_cache_ttl_seconds > 0 {
        let ttl = Duration::from_secs(state.data_health_cache_ttl_seconds);
        match state.data_health_cache.get(workspace_id, &key, ttl) {
            Ok(value) => (value, "HIT"),
            Err(generation) => {
                let value = to_json(load.await?)?;
                state
                    .data_health_cache
                    .insert(workspace_id, key, value.clone(), generation);
                (value, "MISS")
            }
        }
    } else {
        (to_json(load.await?)?, "BYPASS")
    };
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
fn to_json(value: impl Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(value)
        .map_err(|_| ApiError::internal("data health response could not be serialized"))
}
/// Drops cached data-health responses for the repository's workspace only.
pub(super) fn invalidate_data_health(state: &AppState, repository: &CatalogRepository) {
    state
        .data_health_cache
        .invalidate(repository.workspace_id_for_runtime());
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
        workspace_id,
        format!("summary:{days}"),
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
        workspace_id,
        format!("blueprints:{days}"),
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
        workspace_id,
        "freshness".to_owned(),
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
        workspace_id,
        "completeness".to_owned(),
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
        workspace_id,
        "contexts".to_owned(),
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
        workspace_id,
        "relationships".to_owned(),
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
        workspace_id,
        "storage".to_owned(),
        repository.data_health_storage(),
    )
    .await
}
pub(super) async fn refresh_data_health(
    State(state): State<AppState>,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> StatusCode {
    state.data_health_cache.invalidate(workspace_id);
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    const TTL: Duration = Duration::from_secs(60);

    #[test]
    fn invalidation_is_scoped_to_one_workspace() {
        let cache = DataHealthCache::default();
        let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
        for workspace in [first, second] {
            let generation = cache.get(workspace, "storage", TTL).unwrap_err();
            cache.insert(workspace, "storage".to_owned(), json!(1), generation);
        }
        cache.invalidate(first);
        assert!(cache.get(first, "storage", TTL).is_err());
        assert_eq!(cache.get(second, "storage", TTL), Ok(json!(1)));
    }

    #[test]
    fn loads_started_before_invalidation_are_not_cached() {
        let cache = DataHealthCache::default();
        let workspace = Uuid::new_v4();
        let generation = cache.get(workspace, "summary:30", TTL).unwrap_err();
        cache.invalidate(workspace);
        cache.insert(
            workspace,
            "summary:30".to_owned(),
            json!("stale"),
            generation,
        );
        assert!(cache.get(workspace, "summary:30", TTL).is_err());
    }
}
