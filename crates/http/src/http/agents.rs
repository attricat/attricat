use std::convert::Infallible;

use async_stream::stream;
use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, AuthenticatedSession, ScopedRepository},
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
    record_id: Option<Uuid>,
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
            "title must be at most 512 bytes".into(),
        ));
    }
    if input.context_id.is_some() && input.record_id.is_none() {
        return Err(ApiError::invalid_input(
            "context_id requires record_id".into(),
        ));
    }
    let conversation = if let Some(record_id) = input.record_id {
        if !repository
            .principal_may(
                request_actor(&repository, user),
                workspace,
                "records.read",
                Some(record_id),
                None,
            )
            .await?
        {
            return Err(ApiError::forbidden());
        }
        repository
            .get_record(record_id)
            .await?
            .ok_or_else(|| ApiError::not_found("record"))?;
        if let Some(context_id) = input.context_id {
            repository
                .get_context_by_id(context_id)
                .await?
                .ok_or_else(|| ApiError::invalid_input("unknown context".into()))?;
        }
        repository
            .create_record_conversation(user, &input.title, record_id, input.context_id)
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

fn request_actor(
    repository: &crate::repository::CatalogRepository,
    user_id: Uuid,
) -> crate::repository::AuthorizationActor {
    repository
        .authorization_actor()
        .unwrap_or(crate::repository::AuthorizationActor {
            user_id,
            token_id: None,
        })
}

pub(super) async fn readable_conversation(
    repository: &crate::repository::CatalogRepository,
    user: Uuid,
    workspace: Uuid,
    id: Uuid,
) -> Result<crate::repository::Conversation, ApiError> {
    let conversation = repository.get_conversation(id).await?;
    if let Some(record_id) = conversation.record_id
        && !repository
            .principal_may(
                request_actor(repository, user),
                workspace,
                "records.read",
                Some(record_id),
                None,
            )
            .await?
    {
        return Err(ApiError::forbidden());
    }
    Ok(conversation)
}

/// Keeps items that are either not record-bound or bound to a record the
/// user may read, authorizing every record in one query.
async fn retain_readable<T>(
    repository: &crate::repository::CatalogRepository,
    user: Uuid,
    workspace: Uuid,
    items: Vec<T>,
    record_of: impl Fn(&T) -> Option<Uuid>,
) -> Result<Vec<T>, ApiError> {
    let mut record_ids = items.iter().filter_map(&record_of).collect::<Vec<_>>();
    record_ids.sort_unstable();
    record_ids.dedup();
    let readable = if repository
        .principal_token_permits(request_actor(repository, user), workspace, "records.read")
        .await?
    {
        repository
            .authorized_record_ids(user, workspace, "records.read", &record_ids)
            .await?
    } else {
        Default::default()
    };
    Ok(items
        .into_iter()
        .filter(|item| record_of(item).is_none_or(|record_id| readable.contains(&record_id)))
        .collect())
}

pub(super) async fn list_conversations(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<crate::repository::Conversation>>, ApiError> {
    let conversations = repository.list_conversations().await?;
    Ok(Json(
        retain_readable(&repository, user, workspace, conversations, |c| c.record_id).await?,
    ))
}

#[derive(Deserialize, Default)]
pub(super) struct ConversationSearchQuery {
    #[serde(default)]
    q: String,
    cursor: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ConversationSearchPage {
    items: Vec<crate::repository::Conversation>,
    next_cursor: Option<String>,
}

pub(super) async fn search_conversations(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ConversationSearchQuery>,
) -> Result<Json<ConversationSearchPage>, ApiError> {
    let search = query.q.trim();
    if search.len() > 120 || search.contains('\0') {
        return Err(ApiError::invalid_input(
            "invalid conversation search".into(),
        ));
    }
    if query
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.len() > 128)
    {
        return Err(ApiError::invalid_input("invalid cursor".into()));
    }
    let before = query
        .cursor
        .map(|cursor| {
            let (date, id) = cursor
                .split_once('|')
                .ok_or_else(|| ApiError::invalid_input("invalid cursor".into()))?;
            let date = date
                .parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|_| ApiError::invalid_input("invalid cursor".into()))?;
            let id = id
                .parse::<Uuid>()
                .map_err(|_| ApiError::invalid_input("invalid cursor".into()))?;
            Ok::<_, ApiError>((date, id))
        })
        .transpose()?;
    const PAGE_SIZE: usize = 30;
    let mut rows = repository
        .search_conversations(search, before, (PAGE_SIZE + 1) as i64)
        .await?;
    let has_more = rows.len() > PAGE_SIZE;
    rows.truncate(PAGE_SIZE);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| format!("{}|{}", row.updated_at.to_rfc3339(), row.id))
    } else {
        None
    };
    let items = retain_readable(&repository, user, workspace, rows, |c| c.record_id).await?;
    Ok(Json(ConversationSearchPage { items, next_cursor }))
}

#[derive(Serialize)]
pub(super) struct ConversationDetail {
    #[serde(flatten)]
    conversation: crate::repository::Conversation,
    /// The agent can only read in this deployment, so the client can say so.
    read_only: bool,
}

pub(super) async fn get_conversation(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<Json<ConversationDetail>, ApiError> {
    Ok(Json(ConversationDetail {
        conversation: readable_conversation(&repository, user, workspace, conversation_id).await?,
        read_only: state
            .agent_provider
            .as_ref()
            .is_some_and(|config| config.read_only),
    }))
}

pub(super) async fn update_conversation(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateConversation>,
) -> Result<Json<crate::repository::Conversation>, ApiError> {
    if input.title.len() > MAX_CONVERSATION_TITLE_BYTES {
        return Err(ApiError::invalid_input(
            "title must be at most 512 bytes".into(),
        ));
    }
    readable_conversation(&repository, user, workspace, conversation_id).await?;
    Ok(Json(
        repository
            .update_conversation_title(conversation_id, &input.title)
            .await?,
    ))
}

pub(super) async fn delete_conversation(
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    readable_conversation(&repository, user, workspace, conversation_id).await?;
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
    principal @ AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
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
        .with_authorization_actor(principal.actor())
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
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiQuery(query): ApiQuery<ApprovalQuery>,
) -> Result<Json<Vec<crate::repository::AgentToolCall>>, ApiError> {
    if let Some(id) = query.conversation_id {
        readable_conversation(&repository, user, workspace, id).await?;
    }
    let calls = repository
        .pending_agent_tool_calls(query.conversation_id)
        .await?;
    let mut run_ids = calls.iter().map(|call| call.run_id).collect::<Vec<_>>();
    run_ids.sort_unstable();
    run_ids.dedup();
    let run_records = repository.run_conversation_records(&run_ids).await?;
    // Calls whose conversation is gone or archived are not shown.
    let calls = calls
        .into_iter()
        .filter(|call| run_records.contains_key(&call.run_id))
        .collect();
    Ok(Json(
        retain_readable(&repository, user, workspace, calls, |call| {
            run_records.get(&call.run_id).copied().flatten()
        })
        .await?,
    ))
}

pub(super) async fn approve(
    State(state): State<AppState>,
    principal @ AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(tool_call_id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    decide_and_enqueue(
        &state,
        repository.with_authorization_actor(principal.actor()),
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
    let call = repository.get_agent_tool_call(tool_call_id).await?;
    let run = repository.get_agent_run(call.run_id).await?;
    readable_conversation(&repository, user, workspace_id, run.conversation_id).await?;
    if matches!(decision, ApprovalDecision::Approve)
        && !catalog_agent_runtime::agent_runner::mutation_authorized(
            &repository,
            user,
            workspace_id,
            &call.tool_name,
            &call.arguments,
        )
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let _ = configured(state)?;
    let call = repository
        .decide_tool_call(tool_call_id, user, decision)
        .await?;
    Ok(Json(
        json!({"tool_call_id": call.id, "state": call.state, "run_id": call.run_id}),
    ))
}

#[derive(Serialize)]
struct RunEventData<'a> {
    id: Uuid,
    run_id: Uuid,
    sequence: i64,
    #[serde(rename = "type")]
    event_type: &'a str,
    payload: &'a Value,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Tells the client the stream ended abnormally rather than because the run
/// finished; `EventSource` surfaces this through its `error` handler.
fn stream_failed() -> Event {
    Event::default()
        .event("error")
        .data("agent run event stream failed")
}

const STREAM_POLL_MIN: std::time::Duration = std::time::Duration::from_millis(250);
const STREAM_POLL_MAX: std::time::Duration = std::time::Duration::from_secs(2);

async fn authorize_event_stream(
    repository: &crate::repository::CatalogRepository,
    principal: AuthenticatedPrincipal,
    session: Option<&AuthenticatedSession>,
    workspace: Uuid,
    conversation: Uuid,
) -> Result<(), ApiError> {
    if let Some(session) = session
        && !repository
            .validate_browser_session(&session.0)
            .await?
            .is_some_and(|active| {
                active.user_id == principal.0
                    && active.workspace_id == workspace
                    && active.workspace_active
            })
    {
        return Err(ApiError::unauthenticated());
    }
    // Unlike request middleware, a response body can outlive its credential
    // and grants. principal_may also checks live PAT revocation and expiry.
    if !repository
        .principal_may(principal.actor(), workspace, "agents.run", None, None)
        .await?
    {
        return Err(ApiError::forbidden());
    }
    readable_conversation(repository, principal.0, workspace, conversation).await?;
    Ok(())
}

/// Streams durable events in sequence order. Event ids are database UUIDs, so a
/// reconnecting client can use Last-Event-ID without relying on process memory.
pub(super) async fn stream_events(
    State(state): State<AppState>,
    principal @ AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    session: Option<Extension<AuthenticatedSession>>,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(run_id): ApiPath<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let run = repository.get_agent_run(run_id).await?;
    let session = session.map(|Extension(session)| session);
    authorize_event_stream(
        &repository,
        principal,
        session.as_ref(),
        workspace,
        run.conversation_id,
    )
    .await?;
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
    let permit = state
        .stream_control
        .acquire(workspace, user)
        .ok_or_else(|| ApiError::service_unavailable("too many active event streams"))?;
    let mut shutdown = state.stream_control.subscribe();
    let lifetime = state.stream_control.lifetime();
    let events = stream! {
        let mut sequence = after;
        let mut draining = false;
        // An idle run is polled progressively less often; new events reset it.
        let mut idle_delay = STREAM_POLL_MIN;
        loop {
            let batch = repository.agent_run_events_after(run_id, sequence).await;
            // Authorize after fetching, before exposing any new batch. An
            // idle stream must also close when access is withdrawn rather
            // than retaining its admission permit until the lifetime limit.
            if let Err(error) = authorize_event_stream(&repository, principal, session.as_ref(), workspace, run.conversation_id).await {
                tracing::debug!(?error, %run_id, "agent run event stream authorization ended");
                yield Ok(stream_failed());
                break;
            }
            match batch {
                Ok(events) if events.is_empty() => {
                    if draining { break; }
                    match repository.get_agent_run(run_id).await {
                        Ok(run) if matches!(run.status.as_str(), "completed" | "failed" | "cancelled" | "skipped") => {
                            // Completion and its terminal event commit atomically.
                            // Re-read after observing completion to close the race
                            // between the preceding event read and status read.
                            draining = true;
                            continue;
                        }
                        Ok(_) => {
                            tokio::time::sleep(idle_delay).await;
                            idle_delay = (idle_delay * 2).min(STREAM_POLL_MAX);
                        }
                        Err(error) => {
                            tracing::error!(%error, %run_id, "agent run event stream failed");
                            yield Ok(stream_failed());
                            break;
                        }
                    }
                }
                Ok(events) => for event in events {
                    idle_delay = STREAM_POLL_MIN;
                    sequence = event.sequence;
                    let frame = Event::default()
                        .id(event.id.to_string())
                        .event(&event.event_type)
                        .json_data(RunEventData {
                            id: event.id,
                            run_id: event.run_id,
                            sequence: event.sequence,
                            event_type: &event.event_type,
                            payload: &event.payload,
                            created_at: event.created_at,
                        });
                    match frame {
                        Ok(frame) => {
                            yield Ok(frame);
                            if event.event_type == "terminal" { return; }
                        },
                        Err(error) => {
                            tracing::error!(%error, %run_id, "agent run event could not be encoded");
                            yield Ok(stream_failed());
                            return;
                        }
                    }
                },
                Err(error) => {
                    tracing::error!(%error, %run_id, "agent run event stream failed");
                    yield Ok(stream_failed());
                    break;
                }
            }
        }
    };
    let bounded = stream! {
        // Captured by the body, not the handler future. A slow client,
        // disconnect, deadline or shutdown all release this same permit.
        let _permit = permit;
        tokio::pin!(events);
        let deadline = tokio::time::sleep(lifetime);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                biased;
                _ = async { let _ = shutdown.wait_for(|stopping| *stopping).await; } => break,
                _ = &mut deadline => break,
                event = events.next() => match event {
                    Some(event) => yield event,
                    None => break,
                }
            }
        }
    };
    Ok(Sse::new(bounded).keep_alive(KeepAlive::default()))
}
