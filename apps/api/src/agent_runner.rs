//! Durable single-run orchestration.
use std::sync::Arc;

use crate::{
    agent_provider::{
        AssistantMessage, ChatMessage, OpenAiCompatibleClient, ProviderError, ToolCall,
    },
    agent_tools::{self, ToolKind},
    repository::{CatalogRepository, RepositoryError},
    storage::ObjectStore,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use uuid::Uuid;

const MAX_INLINE_ATTACHMENT_BYTES: i64 = 5 * 1024 * 1024;

const SYSTEM_PROMPT: &str = "You are a catalogue assistant. Use tools for catalogue facts. Before drafting a blueprint, call blueprint_authoring_guide and use create_blueprint with complete TOML; every entity blueprint must include a views.dropdown_option definition. To modify a blueprint, use create_blueprint_revision with its id and a complete revised TOML definition. New blueprints and revisions are drafts: use publish_blueprint with the returned id and version before creating entities from them. Never put blueprint attributes or a definition in create_entity. Use list_blueprints to find an existing blueprint before creating an entity. When a conversation attachment should be retained on an entity, use link_file with its file_id and an applicable file attribute. Never claim a mutation happened until its tool result says so. All mutations require human approval.";

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

pub async fn run(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
    object_store: &Arc<dyn ObjectStore>,
    run_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), RunError> {
    repository
        .transition_agent_run(run_id, "running", None, None)
        .await?;
    run_claimed(repository, provider, object_store, run_id, conversation_id).await
}

/// Drives a run whose queued-to-running transition was atomically claimed by
/// the process-owned dispatcher.
pub async fn run_claimed(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
    object_store: &Arc<dyn ObjectStore>,
    run_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), RunError> {
    drive(
        repository,
        provider,
        object_store,
        run_id,
        conversation_id,
        0,
    )
    .await
}

async fn drive(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
    object_store: &Arc<dyn ObjectStore>,
    run_id: Uuid,
    conversation_id: Uuid,
    rounds: u8,
) -> Result<(), RunError> {
    if rounds >= 8 {
        fail(
            repository,
            run_id,
            "tool_limit",
            "tool-call round limit exceeded",
        )
        .await?;
        return Ok(());
    }
    let messages = repository.conversation_messages(conversation_id).await?;
    let mut request = vec![ChatMessage {
        role: "system".into(),
        content: Value::String(SYSTEM_PROMPT.into()),
        tool_call_id: None,
        tool_calls: None,
    }];
    for message in messages {
        request.push(request_message(repository, object_store, message).await);
    }
    let mut deltas = String::new();
    let answer = match provider
        .stream(request, agent_tools::definitions(), |delta| {
            deltas.push_str(delta);
        })
        .await
    {
        Ok(answer) => answer,
        Err(error) => {
            tracing::warn!(%run_id, %error, "agent provider request failed");
            fail(repository, run_id, "provider_error", &error.to_string()).await?;
            return Ok(());
        }
    };
    if !deltas.is_empty() {
        repository
            .append_run_event(run_id, "message_delta", json!({"text": deltas}))
            .await?;
    }
    let AssistantMessage {
        content,
        tool_calls,
    } = answer;
    if let Some(content) = content {
        repository
            .append_conversation_message(
                conversation_id,
                Some(run_id),
                "assistant",
                Value::String(content.clone()),
            )
            .await?;
        repository
            .append_run_event(run_id, "message_completed", json!({"text": content}))
            .await?;
    }
    if !tool_calls.is_empty() {
        repository
            .append_conversation_message(
                conversation_id,
                Some(run_id),
                "assistant",
                json!({"tool_calls": tool_calls}),
            )
            .await?;
    }
    let mut executed_read = false;
    for call in tool_calls {
        let arguments: Value = match serde_json::from_str::<Value>(&call.function.arguments) {
            Ok(value) if value.is_object() => value,
            _ => {
                fail(
                    repository,
                    run_id,
                    "invalid_tool_arguments",
                    "provider supplied invalid tool arguments",
                )
                .await?;
                return Ok(());
            }
        };
        let kind = match agent_tools::kind(&call.function.name) {
            Ok(kind) => kind,
            Err(_) => {
                fail(
                    repository,
                    run_id,
                    "unknown_tool",
                    "provider requested an unknown tool",
                )
                .await?;
                return Ok(());
            }
        };
        if kind == ToolKind::Mutation {
            let summary = match agent_tools::change_summary(&call.function.name, &arguments) {
                Ok(value) => value,
                Err(_) => {
                    fail(
                        repository,
                        run_id,
                        "invalid_tool_arguments",
                        "provider supplied invalid mutation arguments",
                    )
                    .await?;
                    return Ok(());
                }
            };
            let tool = repository
                .create_agent_tool_call(
                    run_id,
                    Some(&call.id),
                    &call.function.name,
                    arguments,
                    Some(&summary),
                    "pending_approval",
                )
                .await?;
            repository.append_run_event(run_id, "tool_call", json!({"tool_call_id":tool.id,"name":tool.tool_name,"state":"pending_approval"})).await?;
            repository
                .append_run_event(
                    run_id,
                    "approval_required",
                    json!({"tool_call_id":tool.id,"change_summary":summary}),
                )
                .await?;
            repository
                .transition_agent_run(run_id, "awaiting_approval", None, None)
                .await?;
            return Ok(());
        }
        executed_read = true;
        let tool = repository
            .create_agent_tool_call(
                run_id,
                Some(&call.id),
                &call.function.name,
                arguments.clone(),
                None,
                "pending_approval",
            )
            .await?;
        let result = agent_tools::execute_read(repository, &call.function.name, arguments)
            .await
            .map_err(|error| json!({"code":"tool_error","message":error.to_string()}));
        let result_message = match &result {
            Ok(value) => value.clone(),
            Err(value) => value.clone(),
        };
        repository.complete_agent_tool_call(tool.id, result).await?;
        repository
            .append_conversation_message(
                conversation_id,
                Some(run_id),
                "tool",
                json!({"tool_call_id":call.id,"name":call.function.name,"result":result_message}),
            )
            .await?;
    }
    if executed_read {
        return Box::pin(drive(
            repository,
            provider,
            object_store,
            run_id,
            conversation_id,
            rounds + 1,
        ))
        .await;
    }
    repository
        .transition_agent_run(run_id, "completed", None, None)
        .await?;
    Ok(())
}
async fn request_message(
    repository: &CatalogRepository,
    object_store: &Arc<dyn ObjectStore>,
    message: crate::repository::ConversationMessage,
) -> ChatMessage {
    if message.role == "assistant" {
        if let Some(tool_calls) = message.content.get("tool_calls") {
            if let Ok(tool_calls) = serde_json::from_value::<Vec<ToolCall>>(tool_calls.clone()) {
                return ChatMessage {
                    role: message.role,
                    content: Value::Null,
                    tool_call_id: None,
                    tool_calls: Some(tool_calls),
                };
            }
        }
    }
    if message.role == "tool" {
        if let Some(tool_call_id) = message.content.get("tool_call_id").and_then(Value::as_str) {
            let content = message
                .content
                .get("result")
                .cloned()
                .unwrap_or(Value::Null);
            return ChatMessage {
                role: message.role,
                content: Value::String(content.to_string()),
                tool_call_id: Some(tool_call_id.to_owned()),
                tool_calls: None,
            };
        }
    }
    if message.attachments.is_empty() {
        return ChatMessage {
            role: message.role,
            content: message.content,
            tool_call_id: None,
            tool_calls: None,
        };
    }
    let mut parts = vec![json!({
        "type": "text",
        "text": message.content.as_str().unwrap_or_default(),
    })];
    for attachment in message.attachments {
        let note = format!(
            "Attached file: {} ({}) with file_id: {}",
            attachment.filename, attachment.mime_type, attachment.id
        );
        let Ok(file) = repository.file_object(attachment.id, None).await else {
            parts.push(json!({"type":"text","text":note}));
            continue;
        };
        if attachment.mime_type.starts_with("image/")
            && file.byte_size <= MAX_INLINE_ATTACHMENT_BYTES
        {
            match object_store.get(&file.object_key).await {
                Ok(object) if object.bytes.len() as i64 <= MAX_INLINE_ATTACHMENT_BYTES => {
                    let data_url = format!(
                        "data:{};base64,{}",
                        attachment.mime_type,
                        STANDARD.encode(object.bytes),
                    );
                    parts.push(json!({"type":"text","text":note}));
                    parts.push(json!({"type":"image_url","image_url":{"url":data_url}}));
                }
                _ => parts.push(json!({"type":"text","text":format!("{note} could not be read.")})),
            }
        } else {
            parts.push(json!({"type":"text","text":note}));
        }
    }
    ChatMessage {
        role: message.role,
        content: Value::Array(parts),
        tool_call_id: None,
        tool_calls: None,
    }
}

/// Continues a paused run after its durable decision. Approved mutations are
/// executed once; rejected calls become structured tool results. The run is
/// then sent back to the provider with that result in thread history.
pub async fn resume(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
    object_store: &Arc<dyn ObjectStore>,
    run_id: Uuid,
) -> Result<(), RunError> {
    repository
        .transition_agent_run(run_id, "running", None, None)
        .await?;
    resume_claimed(repository, provider, object_store, run_id).await
}

/// Resumes a decision-bearing run after the dispatcher claimed it.
pub async fn resume_claimed(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
    object_store: &Arc<dyn ObjectStore>,
    run_id: Uuid,
) -> Result<(), RunError> {
    let agent_run = repository.get_agent_run(run_id).await?;
    let (actor, workspace) = repository.agent_run_initiator(run_id).await?;
    for call in repository.decided_agent_tool_calls(run_id).await? {
        let result = if call.state == "approved" {
            if !mutation_authorized(
                repository,
                actor,
                workspace,
                &call.tool_name,
                &call.arguments,
            )
            .await?
            {
                Err(
                    json!({"code":"forbidden","message":"The initiating user is no longer authorized to make this change."}),
                )
            } else {
                agent_tools::execute_mutation(repository, &call.tool_name, call.arguments.clone())
                    .await
                    .map_err(|error| json!({"code":"tool_error","message":error.to_string()}))
            }
        } else {
            Err(
                json!({"code":"rejected","message":"The requested change was rejected by a human approver."}),
            )
        };
        let payload = match &result {
            Ok(value) | Err(value) => value.clone(),
        };
        repository.complete_agent_tool_call(call.id, result).await?;
        repository.append_conversation_message(agent_run.conversation_id, Some(run_id), "tool", json!({"tool_call_id":call.provider_call_id,"name":call.tool_name,"result":payload})).await?;
    }
    run_claimed(
        repository,
        provider,
        object_store,
        run_id,
        agent_run.conversation_id,
    )
    .await
}

async fn mutation_authorized(
    repository: &CatalogRepository,
    actor: Uuid,
    workspace: Uuid,
    name: &str,
    arguments: &Value,
) -> Result<bool, RepositoryError> {
    let (permission, target_id) = match name {
        "create_blueprint" | "create_blueprint_revision" => ("blueprints.write", None),
        "publish_blueprint" => ("blueprints.publish", None),
        "create_entity" | "link_file" => ("entities.write", None),
        "set_entity_values" => (
            "entities.write",
            arguments
                .get("entity_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse().ok()),
        ),
        "delete_entity" => (
            "entities.delete",
            arguments
                .get("entity_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse().ok()),
        ),
        "create_context" => ("contexts.write", None),
        _ => return Ok(false),
    };
    repository
        .is_authorized(actor, workspace, permission, target_id, None)
        .await
}

async fn fail(
    repository: &CatalogRepository,
    run_id: Uuid,
    code: &str,
    message: &str,
) -> Result<(), RepositoryError> {
    repository
        .append_run_event(run_id, "error", json!({"code":code,"message":message}))
        .await?;
    repository
        .transition_agent_run(run_id, "failed", Some(code), Some(message))
        .await?;
    Ok(())
}
