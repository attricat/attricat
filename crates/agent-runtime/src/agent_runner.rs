//! Durable single-run orchestration.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    agent_provider::{
        AssistantMessage, ChatMessage, OpenAiCompatibleClient, ProviderError, ToolCall,
    },
    agent_tools::{self, ToolKind},
    agents::{MAX_INLINE_ATTACHMENT_BYTES, MAX_TOOL_CALL_ROUNDS},
    repository::{AgentAuditAttribution, AuditContext, CatalogRepository, RepositoryError},
    storage::ObjectStore,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use uuid::Uuid;

const MAX_INLINE_TOOL_IMAGE_BYTES: i64 = 1024 * 1024;
const MAX_INLINE_TOOL_TEXT_BYTES: i64 = 64 * 1024;

const SYSTEM_PROMPT: &str = "You are a catalogue assistant. Use tools for catalogue facts. Before drafting a blueprint, call blueprint_authoring_guide and use create_blueprint with complete TOML; every entity blueprint must include a views.dropdown_option definition. To modify a blueprint, use create_blueprint_revision with its id and a complete revised TOML definition. New blueprints and revisions are drafts: use publish_blueprint with the returned id and version before creating entities from them. Entity edits are not channel exports: inspect publication status and explicitly publish an entity to a requested channel only after human approval. Never put blueprint attributes or a definition in create_entity. Use list_blueprints to find an existing blueprint before creating an entity. Use search_entities to find matching entities; set outdated to true when looking for entities that need a blueprint upgrade. Use get_blueprint_revision to inspect an exact blueprint revision. Use preview_entity_migration to assess an upgrade without proposing a write. To change relationship sets, inspect the entity first; replace_entity_relationships supplies the complete target set, while remove_entity_relationships unlinks only named targets. Use get_entity_preview_link for each entity you cite and include its returned link as a Markdown link in your reply. When asked to save a named Explorer search, first use list_saved_searches and get_saved_search to check for an existing owned search; use update_saved_search for changes to an existing search instead of creating duplicates. Use create_saved_search only for a new search. Include the returned link after approval. Use data_health_summary, list_rule_findings, and list_workflow_runs for diagnostic questions; use get_rule_definition, get_workflow_definition, list_rule_runs, or get_workflow_run when a user needs more context. For extension or blueprint connector operations, use list_extension_operation_runs, get_extension_operation_run, and list_blueprint_connector_jobs only when the initiating user has extension management access. These tools are read-only and do not authorize replay or management actions. Use get_entity_changes and get_value_history with pagination to inspect history. Before proposing update_entity_annotations, inspect the entity; when changing contexts, inspect get_context first. Before proposing remove_entity_values or restore_entity_value, inspect the entity and the specific history entry; restored history can change current values. Use preview_entity_migration first; use migrate_entity only when the user requests the upgrade and approval is appropriate. Report issues if it needs input. Use view_image with an image file ID from get_entity when visual inspection is needed, or read_file for UTF-8 text files. When a conversation attachment should be retained on an entity, use link_file with its file_id and an applicable file attribute. Never claim a mutation happened until its tool result says so. All mutations require human approval.";

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
    if rounds >= MAX_TOOL_CALL_ROUNDS {
        fail_run(
            repository,
            run_id,
            "tool_limit",
            "tool-call round limit exceeded",
        )
        .await?;
        return Ok(());
    }
    let conversation = repository.get_conversation(conversation_id).await?;
    let messages = repository.conversation_messages(conversation_id).await?;
    let context_prompt = match conversation.entity_id {
        Some(entity_id) => format!(
            " This conversation is anchored to entity {entity_id} in attribute context {:?}. Before answering questions about what its preview shows, call get_entity_context_preview with this entity and context ID to see resolved inherited values (or get_entity when no context is selected). Ground all responses in this entity and its selected context, and do not assume values from another context apply here. For edits, always confirm before persisting changes.",
            conversation.context_id
        ),
        None => String::new(),
    };
    let mut request = vec![ChatMessage {
        role: "system".into(),
        content: Value::String(format!("{SYSTEM_PROMPT}{context_prompt}")),
        tool_call_id: None,
        tool_calls: None,
    }];
    for message in messages {
        request.push(request_message(repository, object_store, message).await);
    }
    let pending = Arc::new(Mutex::new(String::new()));
    let stream_pending = pending.clone();
    let mut stream = Box::pin(
        provider.stream(request, agent_tools::definitions(), move |delta| {
            stream_pending
                .lock()
                .expect("agent delta buffer poisoned")
                .push_str(delta);
        }),
    );
    // The provider callback is synchronous. Flush while its future waits for
    // network frames so subscribers see text before generation completes,
    // without a database write for every token.
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    tick.tick().await;
    let result = loop {
        tokio::select! {
            result = &mut stream => break result,
            _ = tick.tick() => flush_deltas(repository, run_id, &pending).await?,
        }
    };
    flush_deltas(repository, run_id, &pending).await?;
    let answer = match result {
        Ok(answer) => answer,
        Err(error) => {
            tracing::warn!(%run_id, %error, "agent provider request failed");
            fail_run(repository, run_id, "provider_error", &error.to_string()).await?;
            return Ok(());
        }
    };
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
    let mut awaiting_approval = false;
    // Providers are asked not to parallelize calls, but persist every mutation
    // defensively if one still returns several. This keeps the assistant's
    // complete tool-call list matched by either an approval or a tool result.
    for call in tool_calls {
        let arguments: Value = match serde_json::from_str::<Value>(&call.function.arguments) {
            Ok(value) if value.is_object() => value,
            _ => {
                fail_run(
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
                fail_run(
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
                    fail_run(
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
            awaiting_approval = true;
            continue;
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
        let (actor, workspace) = repository.agent_run_initiator(run_id).await?;
        let file_attachments = if matches!(call.function.name.as_str(), "view_image" | "read_file")
        {
            arguments
                .get("file_id")
                .and_then(Value::as_str)
                .and_then(|value| value.parse().ok())
                .map(|file_id| vec![file_id])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let result =
            agent_tools::execute_read(repository, actor, workspace, &call.function.name, arguments)
                .await
                .map_err(|error| match error {
                    agent_tools::ToolError::Forbidden => json!({
                        "code":"forbidden",
                        "message":"The initiating user is not authorized to read this catalog data."
                    }),
                    error => json!({"code":"tool_error","message":error.to_string()}),
                });
        let result_message = match &result {
            Ok(value) => value.clone(),
            Err(value) => value.clone(),
        };
        repository.complete_agent_tool_call(tool.id, result).await?;
        repository
            .append_conversation_message_with_attachments(
                conversation_id,
                Some(run_id),
                "tool",
                json!({"tool_call_id":call.id,"name":call.function.name,"result":result_message}),
                if result_message.get("code").is_some() {
                    &[]
                } else {
                    &file_attachments
                },
            )
            .await?;
    }
    if awaiting_approval {
        repository
            .transition_agent_run(run_id, "awaiting_approval", None, None)
            .await?;
        return Ok(());
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
    let title_repository = repository.clone();
    let title_provider = provider.clone();
    tokio::spawn(async move {
        crate::conversation_title::maybe_generate_title(
            &title_repository,
            &title_provider,
            conversation_id,
        )
        .await;
    });
    Ok(())
}
async fn flush_deltas(
    repository: &CatalogRepository,
    run_id: Uuid,
    pending: &Mutex<String>,
) -> Result<(), RepositoryError> {
    let text = std::mem::take(&mut *pending.lock().expect("agent delta buffer poisoned"));
    if !text.is_empty() {
        repository
            .append_run_event(run_id, "message_delta", json!({"text": text}))
            .await?;
    }
    Ok(())
}

async fn request_message(
    repository: &CatalogRepository,
    object_store: &Arc<dyn ObjectStore>,
    message: crate::repository::ConversationMessage,
) -> ChatMessage {
    if message.role == "assistant"
        && let Some(proposal) = message.content.get("draft_proposal")
    {
        return ChatMessage {
            role: message.role,
            content: Value::String(format!("Draft-only proposal (not saved): {proposal}")),
            tool_call_id: None,
            tool_calls: None,
        };
    }
    if message.role == "assistant"
        && let Some(tool_calls) = message.content.get("tool_calls")
        && let Ok(tool_calls) = serde_json::from_value::<Vec<ToolCall>>(tool_calls.clone())
    {
        return ChatMessage {
            role: message.role,
            content: Value::Null,
            tool_call_id: None,
            tool_calls: Some(tool_calls),
        };
    }
    if message.role == "tool"
        && let Some(tool_call_id) = message.content.get("tool_call_id").and_then(Value::as_str)
    {
        let content = message
            .content
            .get("result")
            .cloned()
            .unwrap_or(Value::Null);
        let mut parts = vec![json!({"type":"text","text":content.to_string()})];
        let tool_name = message.content.get("name").and_then(Value::as_str);
        for attachment in message.attachments {
            if attachment.mime_type.starts_with("image/") {
                let file = match repository.file_object(attachment.id, Some("display")).await {
                    Ok(file) => file,
                    Err(_) => match repository.file_object(attachment.id, None).await {
                        Ok(file) => file,
                        Err(_) => continue,
                    },
                };
                if file.byte_size > MAX_INLINE_TOOL_IMAGE_BYTES {
                    continue;
                }
                if let Ok(object) = object_store.get(&file.object_key).await
                    && object.bytes.len() as i64 <= MAX_INLINE_TOOL_IMAGE_BYTES
                {
                    let data_url = format!(
                        "data:{};base64,{}",
                        file.mime_type,
                        STANDARD.encode(object.bytes),
                    );
                    parts.push(json!({"type":"image_url","image_url":{"url":data_url}}));
                }
            } else if tool_name == Some("read_file") {
                let Ok(file) = repository.file_object(attachment.id, None).await else {
                    continue;
                };
                if file.byte_size > MAX_INLINE_TOOL_TEXT_BYTES {
                    continue;
                }
                if let Ok(object) = object_store.get(&file.object_key).await
                    && object.bytes.len() as i64 <= MAX_INLINE_TOOL_TEXT_BYTES
                    && let Ok(text) = std::str::from_utf8(&object.bytes)
                {
                    parts.push(json!({
                        "type":"text",
                        "text":format!("Contents of {}:\n{text}", file.display_filename),
                    }));
                }
            }
        }
        return ChatMessage {
            role: message.role,
            content: Value::Array(parts),
            tool_call_id: Some(tool_call_id.to_owned()),
            tool_calls: None,
        };
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
                let audited_repository = repository
                    .clone()
                    .with_audit_context(agent_audit_context(actor, &agent_run, &call));
                agent_tools::execute_mutation(
                    &audited_repository,
                    actor,
                    &call.tool_name,
                    call.arguments.clone(),
                )
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
    // A decision queues the run, even when sibling calls from this response
    // still need a human decision. Do not send a partial tool-result set back
    // to the provider: wait until every call has been resolved.
    if repository.has_pending_agent_tool_calls(run_id).await? {
        repository
            .transition_agent_run(run_id, "awaiting_approval", None, None)
            .await?;
        return Ok(());
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

fn mutation_authorization(name: &str, arguments: &Value) -> Option<(&'static str, Option<Uuid>)> {
    Some(match name {
        "create_blueprint" | "create_blueprint_revision" => ("blueprints.write", None),
        "publish_blueprint" => ("blueprints.publish", None),
        "create_entity" | "link_file" => ("entities.write", None),
        "publish_entity" | "unpublish_entity" | "publish_entity_to_all_channels" => (
            "entities.publish",
            arguments
                .get("entity_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse().ok()),
        ),
        "set_entity_values"
        | "remove_entity_values"
        | "restore_entity_value"
        | "replace_entity_relationships"
        | "remove_entity_relationships"
        | "migrate_entity"
        | "update_entity_annotations" => (
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
        "update_context" | "delete_context" => (
            "contexts.write",
            arguments
                .get("context_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse().ok()),
        ),
        "create_saved_search" | "update_saved_search" => ("entities.read", None),
        _ => return None,
    })
}

async fn mutation_authorized(
    repository: &CatalogRepository,
    actor: Uuid,
    workspace: Uuid,
    name: &str,
    arguments: &Value,
) -> Result<bool, RepositoryError> {
    let Some((permission, target_id)) = mutation_authorization(name, arguments) else {
        return Ok(false);
    };
    repository
        .is_authorized(actor, workspace, permission, target_id, None)
        .await
}

fn agent_audit_context(
    actor: Uuid,
    run: &crate::repository::AgentRun,
    call: &crate::repository::AgentToolCall,
) -> AuditContext {
    let (permission, target_id) = mutation_authorization(&call.tool_name, &call.arguments)
        .expect("only known mutation tools are executed");
    AuditContext {
        actor_user_id: Some(actor),
        actor_token_id: None,
        // Agent execution has no HTTP request. The durable call and run IDs
        // provide its request/correlation identity without retaining prompts.
        request_id: call.id,
        correlation_id: run.id,
        action: format!("catalog.agent.{}", call.tool_name),
        authorization_scope: json!({"permission": permission}),
        target: match target_id {
            Some(id) => json!({"id": id}),
            None => json!({"tool": call.tool_name}),
        },
        metadata: json!({}),
        agent: Some(AgentAuditAttribution {
            run_id: run.id,
            conversation_id: run.conversation_id,
            tool_call_id: call.id,
            tool_name: call.tool_name.clone(),
            approval_decision: Some(call.state.clone()),
            approved_by_user_id: call.decided_by_user_id,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::mutation_authorization;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn relationship_mutations_require_entity_scoped_write_permission() {
        let id = Uuid::new_v4();
        for name in [
            "replace_entity_relationships",
            "remove_entity_relationships",
            "remove_entity_values",
            "restore_entity_value",
            "update_entity_annotations",
        ] {
            assert_eq!(
                mutation_authorization(name, &json!({"entity_id":id})),
                Some(("entities.write", Some(id)))
            );
            assert_eq!(
                mutation_authorization(name, &json!({"entity_id":"invalid"})),
                Some(("entities.write", None))
            );
        }
    }

    #[test]
    fn context_edits_require_context_scoped_write_permission() {
        let id = Uuid::new_v4();
        for name in ["update_context", "delete_context"] {
            assert_eq!(
                mutation_authorization(name, &json!({"context_id":id})),
                Some(("contexts.write", Some(id)))
            );
        }
    }
}

pub(crate) async fn fail_run(
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
