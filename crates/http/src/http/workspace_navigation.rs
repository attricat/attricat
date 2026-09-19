use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::ApiJson,
};
use crate::repository::{ExploreNavigationEntry, ExploreNavigationItem};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NavigationInput {
    explore_navigation: Vec<ExploreNavigationEntry>,
}

pub(super) async fn configured(
    State(_state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<ExploreNavigationEntry>>, ApiError> {
    Ok(Json(
        repository
            .configured_explore_navigation(actor, workspace)
            .await?,
    ))
}

pub(super) async fn sidebar(
    State(_state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<ExploreNavigationItem>>, ApiError> {
    Ok(Json(
        repository.list_explore_navigation(actor, workspace).await?,
    ))
}

pub(super) async fn update(
    State(_state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<NavigationInput>,
) -> Result<StatusCode, ApiError> {
    repository
        .update_explore_navigation(actor, workspace, &input.explore_navigation)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
