use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AppState,
    auth::{AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{agent_provider::OpenAiCompatibleClient, agent_runner, repository::ApprovalDecision};

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

pub(super) async fn create_conversation(
    super::auth::AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateConversation>,
) -> Result<(StatusCode, Json<ConversationResponse>), ApiError> {
    if input.title.len() > 512 {
        return Err(ApiError::invalid_input(
            "title must be at most 512 characters".to_owned(),
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

pub(super) async fn send_message(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<SendMessage>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    if input.content.trim().is_empty() || input.content.len() > 32 * 1024 {
        return Err(ApiError::invalid_input(
            "content must be between 1 and 32768 bytes".to_owned(),
        ));
    }
    let config = state
        .agent_provider
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("agents are not configured"))?;
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
    let provider = OpenAiCompatibleClient::new(config)
        .map_err(|_| ApiError::service_unavailable("agent provider is unavailable"))?;
    agent_runner::run(&repository, &provider, run.id, conversation_id)
        .await
        .map_err(|_| ApiError::service_unavailable("agent run failed"))?;
    Ok((
        StatusCode::ACCEPTED,
        Json(RunResponse {
            id: run.id,
            status: "submitted".to_owned(),
        }),
    ))
}

pub(super) async fn approve(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(tool_call_id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    decide_and_resume(
        state,
        repository,
        tool_call_id,
        user,
        ApprovalDecision::Approve,
    )
    .await
}
pub(super) async fn reject(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(tool_call_id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    decide_and_resume(
        state,
        repository,
        tool_call_id,
        user,
        ApprovalDecision::Reject,
    )
    .await
}
async fn decide_and_resume(
    state: AppState,
    repository: crate::repository::CatalogRepository,
    tool_call_id: Uuid,
    user: Uuid,
    decision: ApprovalDecision,
) -> Result<Json<Value>, ApiError> {
    let call = repository
        .decide_tool_call(tool_call_id, user, decision)
        .await?;
    let config = state
        .agent_provider
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("agents are not configured"))?;
    let provider = OpenAiCompatibleClient::new(config)
        .map_err(|_| ApiError::service_unavailable("agent provider is unavailable"))?;
    agent_runner::resume(&repository, &provider, call.run_id)
        .await
        .map_err(|_| ApiError::service_unavailable("agent run failed"))?;
    Ok(Json(
        json!({"tool_call_id":call.id,"state":call.state,"run_id":call.run_id}),
    ))
}
