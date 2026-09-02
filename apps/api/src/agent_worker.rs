//! Process-owned dispatcher for durable agent runs and UTC schedules.
//!
//! HTTP only persists a queued run and sends its id here.  Losing a process
//! message is safe because startup recovery re-enqueues every queued run.
use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    agent_provider::OpenAiCompatibleClient, agent_runner, agents::AgentProviderConfig,
    repository::CatalogRepository,
};

#[derive(Clone)]
pub struct AgentDispatcher {
    sender: mpsc::Sender<(Uuid, Uuid)>,
}

impl AgentDispatcher {
    pub async fn enqueue(&self, workspace_id: Uuid, run_id: Uuid) -> Result<(), ()> {
        self.sender
            .send((workspace_id, run_id))
            .await
            .map_err(|_| ())
    }
}

pub async fn start(repository: CatalogRepository, config: AgentProviderConfig) -> AgentDispatcher {
    let (sender, mut receiver) = mpsc::channel(256);
    let dispatcher = AgentDispatcher { sender };
    if let Err(error) = repository.recover_interrupted_agent_runs().await {
        tracing::error!(%error, "could not recover interrupted agent runs");
    }
    for job in repository.queued_agent_runs().await.unwrap_or_default() {
        let _ = dispatcher.enqueue(job.0, job.1).await;
    }

    let worker_repository = repository.clone();
    let worker_config = config.clone();
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
            let result = match repository.decided_agent_tool_calls(run_id).await {
                Ok(calls) if calls.is_empty() => {
                    agent_runner::run_claimed(&repository, &provider, run_id, run.conversation_id)
                        .await
                }
                Ok(_) => agent_runner::resume_claimed(&repository, &provider, run_id).await,
                Err(error) => Err(error.into()),
            };
            if let Err(error) = result {
                tracing::error!(%run_id, error = %error, "agent run failed");
                let _ = repository
                    .transition_agent_run(
                        run_id,
                        "failed",
                        Some("runner_error"),
                        Some("agent run failed"),
                    )
                    .await;
            }
        }
    });

    let scheduler_repository = repository;
    let scheduler_dispatcher = dispatcher.clone();
    tokio::spawn(async move {
        // A scheduler panic or unexpected exit must not silently disable future
        // runs. This supervisor owns restart backoff; normal poll failures are
        // handled inside the scheduled loop without dropping due work.
        loop {
            let repository = scheduler_repository.clone();
            let dispatcher = scheduler_dispatcher.clone();
            let config = config.clone();
            match tokio::spawn(async move { run_scheduler(repository, dispatcher, config).await })
                .await
            {
                Ok(()) => tracing::error!("agent scheduler stopped; restarting"),
                Err(error) => tracing::error!(%error, "agent scheduler task failed; restarting"),
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    dispatcher
}

async fn run_scheduler(
    repository: CatalogRepository,
    dispatcher: AgentDispatcher,
    config: AgentProviderConfig,
) {
    let mut interval = tokio::time::interval(config.scheduler_poll_interval);
    loop {
        interval.tick().await;
        match repository
            .claim_due_agent_schedules(Utc::now(), config.base_url.as_str(), &config.model)
            .await
        {
            Ok(runs) => {
                for (workspace_id, run_id) in runs {
                    if dispatcher.enqueue(workspace_id, run_id).await.is_err() {
                        tracing::error!(%workspace_id, %run_id, "agent scheduler dispatcher stopped");
                        return;
                    }
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "agent scheduler poll failed");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
