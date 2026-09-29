use super::{auth::ScopedRepository, error::ApiError, extractors::ApiPath};
use crate::repository::FailedEventDelivery;
use axum::Json;
use uuid::Uuid;

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<FailedEventDelivery>>, ApiError> {
    Ok(Json(repository.list_failed_event_deliveries().await?))
}
pub(super) async fn replay(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((consumer_id, event_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !repository
        .replay_event_delivery(consumer_id, event_id)
        .await?
    {
        return Err(ApiError::not_found("event delivery"));
    }
    Ok(Json(
        serde_json::json!({"consumer_id": consumer_id, "event_id": event_id, "status": "pending"}),
    ))
}
