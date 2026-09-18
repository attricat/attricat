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

pub async fn start(
    repository: CatalogRepository,
    config: AgentProviderConfig,
    object_store: Arc<dyn ObjectStore>,
) -> AgentDispatcher {
    let (sender, mut receiver) = mpsc::channel(config.dispatch_queue_capacity);
    let dispatcher = AgentDispatcher { sender };
    // Start consuming before replaying durable work. Replaying into a bounded
    // channel first can fill it and wait forever for a receiver that has not
    // been spawned yet.
    let worker_repository = repository.clone();
    let worker_config = config.clone();
    let worker_object_store = object_store.clone();
    tokio::spawn(async move {
        while let Some((workspace_id, run_id)) = receiver.recv().await {
            let Ok(repository) = worker_repository.for_workspace(workspace_id).await else {
                tracing::error!(%workspace_id, %run_id, "could not scope agent run worker");
                continue;
            };
            let Ok(Some(run)) = repository.claim_queued_agent_run(run_id).await else {
                // Another API process claimed it first, or it is no longer queued.
                continue;
            };
            let Ok(provider) = OpenAiCompatibleClient::new(&worker_config) else {
                let _ = repository
                    .transition_agent_run(
                        run_id,
                        "failed",
                        Some("provider_unavailable"),
                        Some("agent provider is unavailable"),
                    )
                    .await;
                continue;
            };
            let result = tokio::time::timeout(worker_config.run_timeout, async {
                match repository.decided_agent_tool_calls(run_id).await {
                    Ok(calls) if calls.is_empty() => {
                        agent_runner::run_claimed(
                            &repository,
                            &provider,
                            &worker_object_store,
                            run_id,
                            run.conversation_id,
                        )
                        .await
                    }
                    Ok(_) => {
                        agent_runner::resume_claimed(
                            &repository,
                            &provider,
                            &worker_object_store,
                            run_id,
                        )
                        .await
                    }
                    Err(error) => Err(error.into()),
                }
            })
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::error!(%run_id, error = %error, "agent run failed");
                    let _ = repository
                        .transition_agent_run(
                            run_id,
                            "failed",
                            Some("runner_error"),
                            Some(&error.to_string()),
                        )
                        .await;
                }
                Err(_) => {
                    tracing::warn!(%run_id, "agent run exceeded configured timeout");
                    if let Err(error) = agent_runner::fail_run(
                        &repository,
                        run_id,
                        "run_timeout",
                        "agent run exceeded configured timeout",
                    )
                    .await
                    {
                        tracing::error!(%run_id, %error, "could not persist agent run timeout");
                    }
                }
            }
        }
    });

    if let Err(error) = repository.recover_interrupted_agent_runs().await {
        tracing::error!(%error, "could not recover interrupted agent runs");
    }
    for job in repository.queued_agent_runs().await.unwrap_or_default() {
        if let Err(error) = dispatcher.enqueue(job.0, job.1).await {
            tracing::error!(workspace_id = %job.0, run_id = %job.1, %error, "could not enqueue recovered agent run");
            break;
        }
    }
    dispatcher
}
