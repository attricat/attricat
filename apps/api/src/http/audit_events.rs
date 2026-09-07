use super::{auth::ScopedRepository, error::ApiError, extractors::ApiQuery};
use crate::{
    constants::DEFAULT_LIST_PAGE_SIZE,
    repository::{AuditEventFilter, AuditEventPage},
};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AuditEventsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    occurred_after: Option<DateTime<Utc>>,
    occurred_before: Option<DateTime<Utc>>,
    actor_user_id: Option<Uuid>,
    action_category: Option<String>,
    target_type: Option<String>,
    executor_type: Option<String>,
    agent_run_id: Option<Uuid>,
    agent_tool_call_id: Option<Uuid>,
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<AuditEventsQuery>,
) -> Result<Json<AuditEventPage>, ApiError> {
    let limit = query.limit.unwrap_or(i64::from(DEFAULT_LIST_PAGE_SIZE));
    let offset = query.offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || offset < 0 {
        return Err(ApiError::invalid_input(
            "limit must be between 1 and 100 and offset must not be negative".to_owned(),
        ));
    }
    if query
        .occurred_after
        .zip(query.occurred_before)
        .is_some_and(|(after, before)| after > before)
    {
        return Err(ApiError::invalid_input(
            "occurred_after must not be after occurred_before".to_owned(),
        ));
    }
    if let Some(executor_type) = &query.executor_type
        && !matches!(executor_type.as_str(), "human" | "agent")
    {
        return Err(ApiError::invalid_input(
            "executor_type must be human or agent".to_owned(),
        ));
    }
    for (name, value) in [
        ("action_category", query.action_category.as_deref()),
        ("target_type", query.target_type.as_deref()),
    ] {
        if value.is_some_and(|value| {
            value.is_empty()
                || !value
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        }) {
            return Err(ApiError::invalid_input(format!(
                "{name} must contain only lowercase letters, digits, underscores, or hyphens"
            )));
        }
    }
    Ok(Json(
        repository
            .list_audit_events(AuditEventFilter {
                limit,
                offset,
                occurred_after: query.occurred_after,
                occurred_before: query.occurred_before,
                actor_user_id: query.actor_user_id,
                action_category: query.action_category,
                target_type: query.target_type,
                executor_type: query.executor_type,
                agent_run_id: query.agent_run_id,
                agent_tool_call_id: query.agent_tool_call_id,
            })
            .await?,
    ))
}
