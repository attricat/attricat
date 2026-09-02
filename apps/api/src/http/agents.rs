use std::convert::Infallible;

use async_stream::stream;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{agent_service::next_utc_schedule_run, repository::ApprovalDecision};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateConversation {
    #[serde(default)]
    title: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SendMessage {
    pub content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateConversation {
    title: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateSchedule {
    conversation_id: Uuid,
    cron_expression: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateSchedule {
    cron_expression: Option<String>,
    enabled: Option<bool>,
}
#[derive(Deserialize, Default)]
pub(super) struct ScheduleQuery {
    conversation_id: Option<Uuid>,
}
#[derive(Deserialize, Default)]
pub(super) struct ApprovalQuery {
    conversation_id: Option<Uuid>,
}
#[derive(Serialize)]
pub(super) struct ConversationResponse {
    id: Uuid,
    title: String,
}
#[derive(Serialize)]
pub(super) struct RunResponse {
    id: Uuid,
    status: String,
}

fn configured(
    state: &AppState,
) -> Result<
    (
        &crate::agents::AgentProviderConfig,
        &crate::agent_worker::AgentDispatcher,
    ),
    ApiError,
> {
    match (&state.agent_provider, &state.agent_dispatcher) {
        (Some(config), Some(dispatcher)) => Ok((config, dispatcher)),
        _ => Err(ApiError::service_unavailable("agents are not configured")),
    }
}

pub(super) async fn create_conversation(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateConversation>,
) -> Result<(StatusCode, Json<ConversationResponse>), ApiError> {
    if input.title.len() > 512 {
        return Err(ApiError::invalid_input(
            "title must be at most 512 characters".into(),
        ));
    }
    let conversation = repository
        .create_conversation(Some(user), &input.title)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ConversationResponse {
            id: conversation.id,
            title: conversation.title,
        }),
    ))
}

pub(super) async fn list_conversations(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<crate::repository::Conversation>>, ApiError> {
    Ok(Json(repository.list_conversations().await?))
}

pub(super) async fn get_conversation(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<crate::repository::Conversation>, ApiError> {
    Ok(Json(repository.get_conversation(conversation_id).await?))
}

pub(super) async fn update_conversation(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateConversation>,
) -> Result<Json<crate::repository::Conversation>, ApiError> {
    if input.title.len() > 512 {
        return Err(ApiError::invalid_input(
            "title must be at most 512 characters".into(),
        ));
    }
    Ok(Json(
        repository
            .update_conversation_title(conversation_id, &input.title)
            .await?,
    ))
}

pub(super) async fn delete_conversation(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository.archive_conversation(conversation_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn list_messages(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::ConversationMessage>>, ApiError> {
    repository.get_conversation(conversation_id).await?;
    Ok(Json(
        repository.conversation_messages(conversation_id).await?,
    ))
}

pub(super) async fn list_runs(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::AgentRun>>, ApiError> {
    repository.get_conversation(conversation_id).await?;
    Ok(Json(
        repository
            .agent_runs_for_conversation(conversation_id)
            .await?,
    ))
}

pub(super) async fn send_message(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<SendMessage>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    if input.content.trim().is_empty() || input.content.len() > 32 * 1024 {
        return Err(ApiError::invalid_input(
            "content must be between 1 and 32768 bytes".into(),
        ));
    }
    let (config, dispatcher) = configured(&state)?;
    repository
        .append_conversation_message(conversation_id, None, "user", Value::String(input.content))
        .await?;
    let run = repository
        .create_agent_run_for_user(
            conversation_id,
            user,
            config.base_url.as_str(),
            &config.model,
        )
        .await?;
    dispatcher
        .enqueue(workspace_id, run.id)
        .await
        .map_err(|_| ApiError::service_unavailable("agent worker is unavailable"))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(RunResponse {
            id: run.id,
            status: run.status,
        }),
    ))
}

pub(super) async fn list_pending_approvals(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ApprovalQuery>,
) -> Result<Json<Vec<crate::repository::AgentToolCall>>, ApiError> {
    Ok(Json(
        repository
            .pending_agent_tool_calls(query.conversation_id)
            .await?,
    ))
}

pub(super) async fn approve(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(tool_call_id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    decide_and_enqueue(
        &state,
        repository,
        tool_call_id,
        user,
        workspace_id,
        ApprovalDecision::Approve,
    )
    .await
}
pub(super) async fn reject(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(tool_call_id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    decide_and_enqueue(
        &state,
        repository,
        tool_call_id,
        user,
        workspace_id,
        ApprovalDecision::Reject,
    )
    .await
}
async fn decide_and_enqueue(
    state: &AppState,
    repository: crate::repository::CatalogRepository,
    tool_call_id: Uuid,
    user: Uuid,
    workspace_id: Uuid,
    decision: ApprovalDecision,
) -> Result<Json<Value>, ApiError> {
    let (_, dispatcher) = configured(state)?;
    let call = repository
        .decide_tool_call(tool_call_id, user, decision)
        .await?;
    dispatcher
        .enqueue(workspace_id, call.run_id)
        .await
        .map_err(|_| ApiError::service_unavailable("agent worker is unavailable"))?;
    Ok(Json(
        json!({"tool_call_id": call.id, "state": call.state, "run_id": call.run_id}),
    ))
}

pub(super) async fn create_schedule(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateSchedule>,
) -> Result<(StatusCode, Json<crate::repository::AgentSchedule>), ApiError> {
    let _ = configured(&state)?;
    if input.cron_expression.len() > 256 {
        return Err(ApiError::invalid_input(
            "cron_expression must be at most 256 characters".into(),
        ));
    }
    let next = next_utc_schedule_run(&input.cron_expression, Utc::now()).map_err(|_| {
        ApiError::invalid_input(
            "cron_expression must be a valid UTC cron expression with a future occurrence".into(),
        )
    })?;
    Ok((
        StatusCode::CREATED,
        Json(
            repository
                .create_agent_schedule(input.conversation_id, user, &input.cron_expression, next)
                .await?,
        ),
    ))
}
pub(super) async fn list_schedules(
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ScheduleQuery>,
) -> Result<Json<Vec<crate::repository::AgentSchedule>>, ApiError> {
    Ok(Json(
        repository
            .list_agent_schedules(query.conversation_id)
            .await?,
    ))
}
pub(super) async fn update_schedule(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(schedule_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateSchedule>,
) -> Result<Json<crate::repository::AgentSchedule>, ApiError> {
    let _ = configured(&state)?;
    if input
        .cron_expression
        .as_deref()
        .is_some_and(|value| value.len() > 256)
    {
        return Err(ApiError::invalid_input(
            "cron_expression must be at most 256 characters".into(),
        ));
    }
    let existing = repository.get_agent_schedule(schedule_id).await?;
    let expression = input
        .cron_expression
        .as_deref()
        .unwrap_or(&existing.cron_expression);
    let next = if input.cron_expression.is_some() || input.enabled == Some(true) {
        Some(next_utc_schedule_run(expression, Utc::now()).map_err(|_| {
            ApiError::invalid_input(
                "cron_expression must be a valid UTC cron expression with a future occurrence"
                    .into(),
            )
        })?)
    } else {
        None
    };
    Ok(Json(
        repository
            .update_agent_schedule(
                schedule_id,
                input.cron_expression.as_deref(),
                input.enabled,
                next,
            )
            .await?,
    ))
}
pub(super) async fn delete_schedule(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(schedule_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository.delete_agent_schedule(schedule_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn run_schedule_now(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(schedule_id): ApiPath<Uuid>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    let (config, dispatcher) = configured(&state)?;
    let run = repository
        .create_manual_agent_run(schedule_id, user, config.base_url.as_str(), &config.model)
        .await?;
    dispatcher
        .enqueue(workspace_id, run.id)
        .await
        .map_err(|_| ApiError::service_unavailable("agent worker is unavailable"))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(RunResponse {
            id: run.id,
            status: run.status,
        }),
    ))
}

/// Streams durable events in sequence order. Event ids are database UUIDs, so a
/// reconnecting client can use Last-Event-ID without relying on process memory.
pub(super) async fn stream_events(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(run_id): ApiPath<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    repository.get_agent_run(run_id).await?;
    let after = match headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            repository
                .agent_event_sequence(
                    run_id,
                    value.parse().map_err(|_| {
                        ApiError::invalid_input("Last-Event-ID must be an event UUID".into())
                    })?,
                )
                .await?
        }
        None => -1,
    };
    let events = stream! {
        let mut sequence = after;
        loop {
            match repository.agent_run_events_after(run_id, sequence).await {
                Ok(events) => for event in events {
                    sequence = event.sequence;
                    let data = json!({"id": event.id, "run_id": event.run_id, "sequence": event.sequence, "type": event.event_type, "payload": event.payload, "created_at": event.created_at});
                    yield Ok(Event::default().id(event.id.to_string()).event(event.event_type).data(data.to_string()));
                },
                Err(_) => break,
            }
            match repository.get_agent_run(run_id).await {
                Ok(run) if matches!(run.status.as_str(), "completed" | "failed" | "cancelled" | "skipped") => break,
                Ok(_) => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
                Err(_) => break,
            }
        }
    };
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}
