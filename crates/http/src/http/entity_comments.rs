use super::{
    auth::{AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::repository::{COMMENT_PAGE_SIZE, EntityComment};
use axum::{Json, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageQuery {
    before_time: Option<DateTime<Utc>>,
    before_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct CommentPage {
    items: Vec<EntityComment>,
    has_more: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateComment {
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateComment {
    body: String,
    revision: i64,
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(entity): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<CommentPage>, ApiError> {
    let before = match (query.before_time, query.before_id) {
        (Some(time), Some(id)) => Some((time, id)),
        (None, None) => None,
        _ => {
            return Err(ApiError::invalid_input(
                "both cursor fields are required".to_owned(),
            ));
        }
    };
    let mut items = repository.list_entity_comments(entity, before).await?;
    let has_more = items.len() > COMMENT_PAGE_SIZE as usize;
    items.truncate(COMMENT_PAGE_SIZE as usize);
    Ok(Json(CommentPage { items, has_more }))
}

pub(super) async fn create(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath(entity): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateComment>,
) -> Result<StatusCode, ApiError> {
    repository
        .create_entity_comment(entity, actor, &input.body)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath((entity, comment)): ApiPath<(Uuid, Uuid)>,
    ApiJson(input): ApiJson<UpdateComment>,
) -> Result<StatusCode, ApiError> {
    repository
        .update_entity_comment(entity, comment, actor, input.revision, &input.body)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
