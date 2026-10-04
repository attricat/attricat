use axum::{Json, http::StatusCode};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::repository::{Team, WorkspaceDirectory};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateTeamRequest {
    code: String,
    name: String,
    #[serde(default)]
    member_user_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateTeamRequest {
    name: Option<String>,
    member_user_ids: Option<Vec<Uuid>>,
}

pub(super) async fn directory(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<WorkspaceDirectory>, ApiError> {
    Ok(Json(repository.workspace_directory().await?))
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<Team>>, ApiError> {
    Ok(Json(repository.list_teams().await?))
}

pub(super) async fn create(
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateTeamRequest>,
) -> Result<(StatusCode, Json<Team>), ApiError> {
    let team = repository
        .create_team(&input.code, &input.name, &input.member_user_ids)
        .await?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateTeamRequest>,
) -> Result<Json<Team>, ApiError> {
    Ok(Json(
        repository
            .update_team(id, input.name.as_deref(), input.member_user_ids.as_deref())
            .await?,
    ))
}

pub(super) async fn delete(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository.delete_team(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
