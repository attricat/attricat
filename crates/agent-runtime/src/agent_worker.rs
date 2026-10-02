//! Shared-task handler for durable agent runs.
use std::sync::Arc;

use crate::{
    agent_provider::OpenAiCompatibleClient,
    agent_runner,
    agents::AgentProviderConfig,
    repository::{ClaimedTask, SystemRepository},
    storage::ObjectStore,
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};

/// Shared-queue handler. Provider work is deliberately terminal on every
/// error: an uncertain provider request is never retried by the task runtime.
pub struct AgentTaskHandler {
    repository: SystemRepository,
    config: AgentProviderConfig,
    object_store: Arc<dyn ObjectStore>,
}

impl AgentTaskHandler {
    pub fn new(
        repository: impl Into<SystemRepository>,
        config: AgentProviderConfig,
        object_store: Arc<dyn ObjectStore>,
    ) -> Self {
        Self {
            repository: repository.into(),
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
