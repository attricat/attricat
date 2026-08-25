use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::repository::{Permission, WorkspaceGrantTarget, WorkspaceRole};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoleInput {
    code: String,
    permissions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DuplicateRoleInput {
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RetireRoleInput {
    replacement_role_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct CreatedRole {
    id: Uuid,
}

pub(super) async fn list(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<WorkspaceRole>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_workspace_roles(actor, workspace)
            .await?,
    ))
}

pub(super) async fn list_assignable_roles(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<WorkspaceRole>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_workspace_assignable_roles(actor, workspace)
            .await?,
    ))
}

pub(super) async fn list_permissions(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<Permission>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_workspace_permissions(actor, workspace)
            .await?,
    ))
}

pub(super) async fn list_token_permissions(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<Permission>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_workspace_token_permissions(actor, workspace)
            .await?,
    ))
}

pub(super) async fn list_grant_targets(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(scope_type): ApiPath<String>,
) -> Result<Json<Vec<WorkspaceGrantTarget>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_workspace_grant_targets(actor, workspace, &scope_type)
            .await?,
    ))
}

pub(super) async fn create(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<RoleInput>,
) -> Result<(StatusCode, Json<CreatedRole>), ApiError> {
    let id = state
        .repository
        .create_workspace_role(actor, workspace, &input.code, &input.permissions)
        .await?;
    Ok((StatusCode::CREATED, Json(CreatedRole { id })))
}

pub(super) async fn update(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(role_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RoleInput>,
) -> Result<StatusCode, ApiError> {
    state
        .repository
        .update_workspace_role(actor, workspace, role_id, &input.code, &input.permissions)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn duplicate(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(role_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<DuplicateRoleInput>,
) -> Result<(StatusCode, Json<CreatedRole>), ApiError> {
    let id = state
        .repository
        .duplicate_workspace_role(actor, workspace, role_id, &input.code)
        .await?;
    Ok((StatusCode::CREATED, Json(CreatedRole { id })))
}

pub(super) async fn retire(
    State(state): State<AppState>,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(role_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RetireRoleInput>,
) -> Result<StatusCode, ApiError> {
    state
        .repository
        .retire_workspace_role(actor, workspace, role_id, input.replacement_role_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
