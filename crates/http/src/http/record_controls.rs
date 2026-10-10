//! Controlled-record reads and file retention holds: which status transitions
//! the caller may take, approval history, and retention holds.
use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::repository::{FileRetentionHold, RecordApproval, StatusTransitionAccess};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TransitionQuery {
    context_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct Items<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlaceHold {
    days: i64,
    reason: String,
}

pub(super) async fn status_transitions(
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<TransitionQuery>,
) -> Result<Json<Items<StatusTransitionAccess>>, ApiError> {
    let items = repository
        .status_transition_access(record_id, query.context_id, principal.actor())
        .await?;
    Ok(Json(Items { items }))
}

pub(super) async fn approvals(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Items<RecordApproval>>, ApiError> {
    repository
        .get_record(record_id)
        .await?
        .ok_or(ApiError::not_found("record"))?;
    Ok(Json(Items {
        items: repository.record_approvals(record_id).await?,
    }))
}

pub(super) async fn record_holds(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Items<FileRetentionHold>>, ApiError> {
    repository
        .get_record(record_id)
        .await?
        .ok_or(ApiError::not_found("record"))?;
    Ok(Json(Items {
        items: repository.record_retention_holds(record_id).await?,
    }))
}

pub(super) async fn file_holds(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiPath(file_id): ApiPath<Uuid>,
) -> Result<Json<Items<FileRetentionHold>>, ApiError> {
    super::files::authorize_read(
        &state,
        &repository,
        user_id,
        workspace_id,
        file_id,
        |file_id, record_id, blueprint_id| crate::file_access::FileAccessOperation::ReadMetadata {
            file_id,
            record_id,
            blueprint_id,
        },
    )
    .await?;
    Ok(Json(Items {
        items: repository.file_retention_holds(file_id).await?,
    }))
}

pub(super) async fn place_hold(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ApiPath(file_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PlaceHold>,
) -> Result<(StatusCode, Json<FileRetentionHold>), ApiError> {
    let hold = repository
        .place_file_retention_hold(file_id, input.days, &input.reason, user_id)
        .await?;
    Ok((StatusCode::CREATED, Json(hold)))
}

pub(super) async fn release_hold(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ApiPath((file_id, hold_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<FileRetentionHold>, ApiError> {
    Ok(Json(
        repository
            .release_file_retention_hold(file_id, hold_id, user_id)
            .await?,
    ))
}
