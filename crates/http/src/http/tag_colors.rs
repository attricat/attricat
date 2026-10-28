use axum::Json;

use super::{
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::ApiJson,
};
use crate::repository::{TagColors, TagColorsPatch};

pub(super) async fn get(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<TagColors>, ApiError> {
    Ok(Json(repository.tag_colors().await?))
}

pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(patch): ApiJson<TagColorsPatch>,
) -> Result<Json<TagColors>, ApiError> {
    Ok(Json(
        repository
            .update_tag_colors(actor, workspace, patch)
            .await?,
    ))
}
