//! Durable single-run orchestration.
use crate::{
    agent_provider::{ChatMessage, OpenAiCompatibleClient, ProviderError},
    agent_tools::{self, ToolKind},
    repository::{CatalogRepository, RepositoryError},
};
use serde_json::{Value, json};
use uuid::Uuid;

const SYSTEM_PROMPT: &str = "You are a catalogue assistant. Use tools for catalogue facts. Never claim a mutation happened until its tool result says so. All mutations require human approval.";

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
    run_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), RunError> {
    repository
        .transition_agent_run(run_id, "running", None, None)
        .await?;
    drive(repository, provider, run_id, conversation_id, 0).await
}

async fn drive(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
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
    }];
    request.extend(messages.into_iter().map(|message| ChatMessage {
        role: message.role,
        content: message.content,
    }));
    let mut deltas = String::new();
    let answer = match provider
        .stream(request, agent_tools::definitions(), |delta| {
            deltas.push_str(delta);
        })
        .await
    {
        Ok(answer) => answer,
        Err(error) => {
            fail(repository, run_id, "provider_error", &error.to_string()).await?;
            return Ok(());
        }
    };
    if !deltas.is_empty() {
        repository
            .append_run_event(run_id, "message_delta", json!({"text": deltas}))
            .await?;
    }
    if let Some(content) = answer.content {
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
    let mut executed_read = false;
    for call in answer.tool_calls {
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
                "approved",
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
            run_id,
            conversation_id,
            rounds + 1,
        ))
        .await;
    }
    repository
        .transition_agent_run(run_id, "completed", None, None)
        .await?;
    repository
        .append_run_event(run_id, "terminal", json!({"status":"completed"}))
        .await?;
    Ok(())
}
/// Continues a paused run after its durable decision. Approved mutations are
/// executed once; rejected calls become structured tool results. The run is
/// then sent back to the provider with that result in thread history.
pub async fn resume(
    repository: &CatalogRepository,
    provider: &OpenAiCompatibleClient,
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
    run(repository, provider, run_id, agent_run.conversation_id).await
}

async fn mutation_authorized(
    repository: &CatalogRepository,
    actor: Uuid,
    workspace: Uuid,
    name: &str,
    arguments: &Value,
) -> Result<bool, RepositoryError> {
    let (permission, target_id) = match name {
        "create_entity" => ("entities.write", None),
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
    repository
        .append_run_event(run_id, "terminal", json!({"status":"failed","code":code}))
        .await?;
    Ok(())
}
