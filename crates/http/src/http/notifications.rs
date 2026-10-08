//! The caller's own notification inbox in the current workspace. Every route
//! acts on the authenticated user's notifications only.

use super::{
    auth::{AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::repository::{NOTIFICATION_PAGE_SIZE, Notification};
use axum::{Json, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListQuery {
    #[serde(default)]
    unread_only: bool,
    before_time: Option<DateTime<Utc>>,
    before_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct NotificationPage {
    items: Vec<Notification>,
    has_more: bool,
    unread_count: i64,
}

#[derive(Serialize)]
pub(super) struct UnreadCount {
    count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateNotification {
    read: bool,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MarkAllRead {
    /// Only notifications created at or before this instant are marked, so
    /// ones that arrived after the client loaded its list stay unread.
    up_to: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct MarkAllReadResult {
    updated: u64,
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ApiQuery(query): ApiQuery<ListQuery>,
) -> Result<Json<NotificationPage>, ApiError> {
    let before = match (query.before_time, query.before_id) {
        (Some(time), Some(id)) => Some((time, id)),
        (None, None) => None,
        _ => {
            return Err(ApiError::invalid_input(
                "both cursor fields are required".to_owned(),
            ));
        }
    };
    let mut items = repository
        .list_notifications(user, query.unread_only, before, NOTIFICATION_PAGE_SIZE)
        .await?;
    let has_more = items.len() > NOTIFICATION_PAGE_SIZE as usize;
    items.truncate(NOTIFICATION_PAGE_SIZE as usize);
    let unread_count = repository.count_unread_notifications(user).await?;
    Ok(Json(NotificationPage {
        items,
        has_more,
        unread_count,
    }))
}

pub(super) async fn unread_count(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
) -> Result<Json<UnreadCount>, ApiError> {
    let count = repository.count_unread_notifications(user).await?;
    Ok(Json(UnreadCount { count }))
}

pub(super) async fn get(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ApiPath(notification): ApiPath<Uuid>,
) -> Result<Json<Notification>, ApiError> {
    Ok(Json(repository.get_notification(user, notification).await?))
}

pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ApiPath(notification): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateNotification>,
) -> Result<StatusCode, ApiError> {
    repository
        .set_notification_read(user, notification, input.read)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn mark_all_read(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    input: Option<ApiJson<MarkAllRead>>,
) -> Result<Json<MarkAllReadResult>, ApiError> {
    let up_to = input.and_then(|ApiJson(input)| input.up_to);
    let updated = repository.mark_all_notifications_read(user, up_to).await?;
    Ok(Json(MarkAllReadResult { updated }))
}

pub(super) async fn delete(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ApiPath(notification): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository.delete_notification(user, notification).await?;
    Ok(StatusCode::NO_CONTENT)
}
