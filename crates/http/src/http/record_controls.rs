//! Controlled-record reads and file retention holds: which status transitions
//! the caller may take, approval history, and retention holds.
use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::repository::{
    FileRetentionHold, RECORD_CONTROL_PAGE_SIZE, RecordApproval, StatusTransitionAccess,
};
use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
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

/// Continues a newest-first list before the timestamp and ID of its last item.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageQuery {
    before_time: Option<DateTime<Utc>>,
    before_id: Option<Uuid>,
}

impl PageQuery {
    fn before(self) -> Result<Option<(DateTime<Utc>, Uuid)>, ApiError> {
        match (self.before_time, self.before_id) {
            (Some(time), Some(id)) => Ok(Some((time, id))),
            (None, None) => Ok(None),
            _ => Err(ApiError::invalid_input(
                "both cursor fields are required".to_owned(),
            )),
        }
    }
}

#[derive(Serialize)]
pub(super) struct Page<T> {
    items: Vec<T>,
    has_more: bool,
}

impl<T> Page<T> {
    /// Splits a [`RECORD_CONTROL_PAGE_SIZE`] page from one extra item.
    fn new(mut items: Vec<T>) -> Self {
        let has_more = items.len() > RECORD_CONTROL_PAGE_SIZE as usize;
        items.truncate(RECORD_CONTROL_PAGE_SIZE as usize);
        Self { items, has_more }
    }
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
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<Page<RecordApproval>>, ApiError> {
    let before = query.before()?;
    repository
        .get_record(record_id)
        .await?
        .ok_or(ApiError::not_found("record"))?;
    Ok(Json(Page::new(
        repository
            .record_approvals(record_id, before, RECORD_CONTROL_PAGE_SIZE + 1)
            .await?,
    )))
}

pub(super) async fn record_holds(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<Page<FileRetentionHold>>, ApiError> {
    let before = query.before()?;
    repository
        .get_record(record_id)
        .await?
        .ok_or(ApiError::not_found("record"))?;
    Ok(Json(Page::new(
        repository
            .record_retention_holds(record_id, before, RECORD_CONTROL_PAGE_SIZE + 1)
            .await?,
    )))
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
