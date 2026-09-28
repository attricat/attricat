use std::convert::Infallible;

use async_stream::stream;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    agents::{
        MAX_CONVERSATION_ATTACHMENTS, MAX_CONVERSATION_MESSAGE_BYTES, MAX_CONVERSATION_TITLE_BYTES,
    },
    repository::ApprovalDecision,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateConversation {
    #[serde(default)]
    title: String,
    #[serde(default)]
    entity_id: Option<Uuid>,
    #[serde(default)]
    context_id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SendMessage {
    pub content: String,
    #[serde(default)]
    pub attachment_ids: Vec<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateConversation {
    title: String,
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

fn configured(state: &AppState) -> Result<&crate::agents::AgentProviderConfig, ApiError> {
    state
        .agent_provider
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("agents are not configured"))
}

pub(super) async fn create_conversation(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateConversation>,
) -> Result<(StatusCode, Json<ConversationResponse>), ApiError> {
    if input.title.len() > MAX_CONVERSATION_TITLE_BYTES {
        return Err(ApiError::invalid_input(
            "title must be at most 512 characters".into(),
        ));
    }
    if input.context_id.is_some() && input.entity_id.is_none() {
        return Err(ApiError::invalid_input(
            "context_id requires entity_id".into(),
        ));
    }
    let conversation = if let Some(entity_id) = input.entity_id {
        if !repository
            .is_authorized(user, workspace, "entities.read", Some(entity_id), None)
            .await?
        {
            return Err(ApiError::forbidden());
        }
        repository
            .get_entity(entity_id)
            .await?
            .ok_or_else(|| ApiError::not_found("entity"))?;
        if let Some(context_id) = input.context_id {
            repository
                .get_context_by_id(context_id)
                .await?
                .ok_or_else(|| ApiError::invalid_input("unknown context".into()))?;
        }
        repository
            .create_entity_conversation(user, &input.title, entity_id, input.context_id)
            .await?
    } else {
        repository
            .create_conversation(Some(user), &input.title)
            .await?
    };
    Ok((
        StatusCode::CREATED,
        Json(ConversationResponse {
            id: conversation.id,
            title: conversation.title,
        }),
    ))
}

async fn readable_conversation(
    repository: &crate::repository::CatalogRepository,
    user: Uuid,
    workspace: Uuid,
    id: Uuid,
) -> Result<crate::repository::Conversation, ApiError> {
    let conversation = repository.get_conversation(id).await?;
    if let Some(entity_id) = conversation.entity_id {
        if !repository
            .is_authorized(user, workspace, "entities.read", Some(entity_id), None)
            .await?
        {
            return Err(ApiError::forbidden());
        }
    }
    Ok(conversation)
}

pub(super) async fn list_conversations(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<crate::repository::Conversation>>, ApiError> {
    let mut visible = Vec::new();
    for conversation in repository.list_conversations().await? {
        if let Some(entity_id) = conversation.entity_id {
            if !repository
                .is_authorized(user, workspace, "entities.read", Some(entity_id), None)
                .await?
            {
                continue;
            }
        }
        visible.push(conversation);
    }
    Ok(Json(visible))
}

pub(super) async fn get_conversation(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<crate::repository::Conversation>, ApiError> {
    Ok(Json(
        readable_conversation(&repository, user, workspace, conversation_id).await?,
    ))
}

pub(super) async fn update_conversation(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateConversation>,
) -> Result<Json<crate::repository::Conversation>, ApiError> {
    if input.title.len() > MAX_CONVERSATION_TITLE_BYTES {
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
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::ConversationMessage>>, ApiError> {
    readable_conversation(&repository, user, workspace, conversation_id).await?;
    Ok(Json(
        repository.conversation_messages(conversation_id).await?,
    ))
}

pub(super) async fn list_runs(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::AgentRun>>, ApiError> {
    readable_conversation(&repository, user, workspace, conversation_id).await?;
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
    if (input.content.trim().is_empty() && input.attachment_ids.is_empty())
        || input.content.len() > MAX_CONVERSATION_MESSAGE_BYTES
    {
        return Err(ApiError::invalid_input(
            "content must be between 1 and 32768 bytes unless attachments are included".into(),
        ));
    }
    if input.attachment_ids.len() > MAX_CONVERSATION_ATTACHMENTS {
        return Err(ApiError::invalid_input(
            "a message may include at most 16 attachments".into(),
        ));
    }
    if input
        .attachment_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != input.attachment_ids.len()
    {
        return Err(ApiError::invalid_input(
            "attachment_ids must not contain duplicates".into(),
        ));
    }
    readable_conversation(&repository, user, workspace_id, conversation_id).await?;
    let config = configured(&state)?;
    let run = repository
        .submit_agent_message(
            conversation_id,
            user,
            Value::String(input.content),
            &input.attachment_ids,
            config.base_url.as_str(),
            &config.model,
        )
        .await?;
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
    let _ = configured(state)?;
    let call = repository
        .decide_tool_call(tool_call_id, user, decision)
        .await?;
    let _ = workspace_id;
    Ok(Json(
        json!({"tool_call_id": call.id, "state": call.state, "run_id": call.run_id}),
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
