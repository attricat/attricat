//! Process-owned dispatcher for durable agent runs.
//!
//! HTTP only persists a queued run and sends its id here.  Losing a process
//! message is safe because startup recovery re-enqueues every queued run.
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    agent_provider::OpenAiCompatibleClient,
    agent_runner,
    agents::AgentProviderConfig,
    repository::{CatalogRepository, ClaimedTask},
    storage::ObjectStore,
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};

#[derive(Debug, Error)]
pub enum AgentDispatchError {
    #[error("agent dispatcher is stopped")]
    Stopped,
}

/// Compatibility signal for older in-process callers. Production does not
/// start it; when used, it still claims the durable task before execution.
#[derive(Clone)]
pub struct AgentDispatcher {
    sender: mpsc::Sender<(Uuid, Uuid)>,
}

/// Shared-queue handler. Provider work is deliberately terminal on every
/// error: an uncertain provider request is never retried by the task runtime.
pub struct AgentTaskHandler {
    repository: CatalogRepository,
    config: AgentProviderConfig,
    object_store: Arc<dyn ObjectStore>,
}

impl AgentTaskHandler {
    pub fn new(
        repository: CatalogRepository,
        config: AgentProviderConfig,
        object_store: Arc<dyn ObjectStore>,
    ) -> Self {
        Self {
            repository,
            config,
            object_store,
        }
    }
}

#[async_trait::async_trait]
impl TaskHandler for AgentTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::AgentRunV1
    }

    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let repository = self
            .repository
            .for_workspace(task.workspace_id)
            .await
            .map_err(|error| TaskHandlerError {
                code: "workspace",
                message: error.to_string(),
            })?;
        let repository = repository.for_agent_task(&task);
        let Some(run) = repository
            .claim_queued_agent_run(task.subject_id)
            .await
            .map_err(|error| TaskHandlerError {
                code: "claim",
                message: error.to_string(),
            })?
        else {
            return Ok(TaskOutcome::Complete);
        };
        let provider =
            OpenAiCompatibleClient::new(&self.config).map_err(|error| TaskHandlerError {
                code: "provider",
                message: error.to_string(),
            })?;
        let result = tokio::time::timeout(self.config.run_timeout, async {
            match repository.decided_agent_tool_calls(task.subject_id).await {
                Ok(calls) if calls.is_empty() => {
                    agent_runner::run_claimed(
                        &repository,
                        &provider,
                        &self.object_store,
                        task.subject_id,
                        run.conversation_id,
                    )
                    .await
                }
                Ok(_) => {
                    agent_runner::resume_claimed(
                        &repository,
                        &provider,
                        &self.object_store,
                        task.subject_id,
                    )
                    .await
                }
                Err(error) => Err(error.into()),
            }
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(TaskOutcome::Complete),
            Ok(Err(error)) => {
                repository
                    .transition_agent_run(
                        task.subject_id,
                        "failed",
                        Some("runner_error"),
                        Some(&error.to_string()),
                    )
                    .await
                    .map_err(|error| TaskHandlerError {
                        code: "transition",
                        message: error.to_string(),
                    })?;
                Ok(TaskOutcome::Complete)
            }
            Err(_) => {
                agent_runner::fail_run(
                    &repository,
                    task.subject_id,
                    "run_timeout",
                    "agent run exceeded configured timeout",
                )
                .await
                .map_err(|error| TaskHandlerError {
                    code: "timeout",
                    message: error.to_string(),
                })?;
                Ok(TaskOutcome::Complete)
            }
        }
    }

    async fn on_lease_lost(&self, _: ClaimedTask) {
        // The stale handler must not write lifecycle state after its token is
        // lost. The next fenced claimant terminalizes an uncertain run.
    }
}

impl AgentDispatcher {
    pub async fn enqueue(
        &self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> Result<(), AgentDispatchError> {
        self.sender
            .send((workspace_id, run_id))
            .await
            .map_err(|_| AgentDispatchError::Stopped)
    }
}

/// Legacy test/embedding entrypoint. It uses the same durable claim and
/// token-fenced handler as the shared worker, rather than process-local state.
pub async fn start(
    repository: CatalogRepository,
    config: AgentProviderConfig,
    object_store: Arc<dyn ObjectStore>,
) -> AgentDispatcher {
    let (sender, mut receiver) = mpsc::channel(config.dispatch_queue_capacity);
    let dispatcher = AgentDispatcher { sender };
    tokio::spawn(async move {
        while receiver.recv().await.is_some() {
            let Ok(Some(task)) = repository
                .claim_task_for_kinds(
                    "legacy-agent-dispatcher",
                    TaskKind::AgentRunV1.policy().lease_duration,
                    &[TaskKind::AgentRunV1],
                )
                .await
            else {
                continue;
            };
            let handler =
                AgentTaskHandler::new(repository.clone(), config.clone(), object_store.clone());
            match handler.handle(task.clone()).await {
                Ok(TaskOutcome::Complete) => {
                    let _ = repository
                        .complete_task(task.id, &task.lease_owner, task.lease_token)
                        .await;
                }
                Ok(TaskOutcome::DeadLettered) => {}
                Ok(TaskOutcome::Retry { .. } | TaskOutcome::Reschedule { .. }) | Err(_) => {
                    let _ = repository
                        .retry_task_at(
                            task.id,
                            &task.lease_owner,
                            task.lease_token,
                            chrono::Utc::now(),
                            "legacy_agent",
                            "legacy agent dispatcher failed",
                        )
                        .await;
                }
            }
        }
    });
    dispatcher
}
