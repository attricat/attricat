use axum::Json;

use super::{
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::ApiJson,
};
use crate::repository::AgentSettings;

pub(super) async fn configured(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<AgentSettings>, ApiError> {
    Ok(Json(
        repository
            .configured_agent_settings(actor, workspace)
            .await?,
    ))
}

pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<AgentSettings>,
) -> Result<Json<AgentSettings>, ApiError> {
    Ok(Json(
        repository
            .update_agent_settings(actor, workspace, input)
            .await?,
    ))
}
