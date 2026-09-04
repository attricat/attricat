use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, header},
    response::Response,
};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::{AppState, auth::ScopedRepository, error::ApiError};
use crate::{
    extensions::{UiContributionKind, UiOutlet},
    storage::ObjectStoreError,
};

#[derive(Serialize)]
pub(super) struct RuntimeContribution {
    extension_id: String,
    release_id: Uuid,
    configuration: Value,
    capabilities: Vec<String>,
    id: String,
    version: u32,
    kind: UiContributionKind,
    outlet: Option<UiOutlet>,
    title: Option<String>,
    element: String,
}

/// Returns only contributions from currently enabled installations. The client
/// loads executable bytes through the separate contribution-bound endpoint.
pub(super) async fn runtime(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<RuntimeContribution>>, ApiError> {
    let contributions = repository
        .client_extension_contributions()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        contributions
            .into_iter()
            .map(|item| RuntimeContribution {
                extension_id: item.extension_id,
                release_id: item.installed_release_id,
                configuration: item.configuration,
                capabilities: item.capabilities,
                id: item.id,
                version: item.version,
                kind: item.kind,
                outlet: item.outlet,
                title: item.title,
                element: item.element,
            })
            .collect(),
    ))
}

/// Artifact bytes are never addressed by object key or release ID in the URL.
/// Resolving the enabled contribution before storage access closes the stale
/// artifact path after disable, quarantine, or upgrade.
pub(super) async fn artifact(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    Path((extension_id, contribution_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let contribution = repository
        .client_extension_contribution(&extension_id, &contribution_id)
        .await
        .map_err(ApiError::from)?;
    let object = state
        .object_store
        .get(&contribution.artifact_key)
        .await
        .map_err(|error| match error {
            ObjectStoreError::Unavailable | ObjectStoreError::TimedOut(_) => {
                ApiError::service_unavailable("extension artifact storage is unavailable")
            }
            ObjectStoreError::Operation(_) => {
                ApiError::internal("extension artifact could not be loaded")
            }
        })?;
    let mut response = Response::new(Body::from(object.bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
