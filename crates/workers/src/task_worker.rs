//! Supervised executor for the API task envelope.
//!
//! Kinds are registered only when their producer and token-fenced domain
//! handler have cut over. An empty registry is intentional during the initial
//! rollout: the worker remains supervised but never leases unhandled work.

use std::{collections::HashMap, env, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use metrics::{counter, gauge};
use tokio::{sync::watch, task::JoinSet, time};
use uuid::Uuid;

use crate::{
    repository::{CatalogRepository, ClaimedTask, TaskError},
    task_queue::TaskKind,
};

#[derive(Debug, Clone)]
pub struct TaskWorkerConfig {
    pub worker_id: String,
    pub concurrency: usize,
    pub poll_interval: Duration,
    pub shutdown_grace: Duration,
}

impl TaskWorkerConfig {
    pub fn from_env() -> Result<Self, String> {
        fn positive(name: &str, default: u64) -> Result<u64, String> {
            match env::var(name) {
                Ok(value) => value
                    .parse()
                    .ok()
                    .filter(|value: &u64| *value > 0)
                    .ok_or_else(|| format!("{name} must be a positive integer")),
                Err(_) => Ok(default),
            }
        }
        let worker_id = env::var("TASK_WORKER_ID").unwrap_or_else(|_| Uuid::new_v4().to_string());
        if worker_id.is_empty() || worker_id.len() > 128 {
            return Err("TASK_WORKER_ID must be 1-128 bytes".to_owned());
        }
        Ok(Self {
            worker_id,
            concurrency: positive("TASK_WORKER_CONCURRENCY", 8)?
                .try_into()
                .map_err(|_| "TASK_WORKER_CONCURRENCY is too large".to_owned())?,
            poll_interval: Duration::from_millis(positive("TASK_WORKER_POLL_MILLIS", 250)?),
            shutdown_grace: Duration::from_secs(positive(
                "TASK_WORKER_SHUTDOWN_GRACE_SECONDS",
                30,
            )?),
        })
    }
}

#[derive(Debug, Clone)]
pub enum TaskOutcome {
    Complete,
    Retry {
        delay: Duration,
        code: &'static str,
    },
    /// A cooperative checkpoint. This never consumes the failure budget.
    Reschedule {
        at: chrono::DateTime<Utc>,
    },
    /// The handler atomically dead-lettered its domain row and envelope.
    DeadLettered,
}

#[derive(Debug)]
pub struct TaskHandlerError {
    pub code: &'static str,
    pub message: String,
}

#[async_trait]
pub trait TaskHandler: Send + Sync {
    fn kind(&self) -> TaskKind;
    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError>;
    /// Called when heartbeat fencing proves this execution no longer owns its
    /// task. Non-resumable handlers can record a terminal domain interruption.
    async fn on_lease_lost(&self, _: ClaimedTask) {}
}

#[derive(Clone, Default)]
pub struct TaskHandlerRegistry {
    handlers: HashMap<TaskKind, Arc<dyn TaskHandler>>,
}

impl TaskHandlerRegistry {
    pub fn new(handlers: Vec<Arc<dyn TaskHandler>>) -> Result<Self, &'static str> {
        let mut registered = HashMap::new();
        for handler in handlers {
            if registered.insert(handler.kind(), handler).is_some() {
                return Err("task handlers must have unique registered kinds");
            }
        }
        Ok(Self {
            handlers: registered,
        })
    }

    fn kinds(&self) -> Vec<TaskKind> {
        self.handlers.keys().copied().collect()
    }

    fn handler(&self, kind: TaskKind) -> Option<Arc<dyn TaskHandler>> {
        self.handlers.get(&kind).cloned()
    }
}

pub fn start(
    repository: CatalogRepository,
    registry: TaskHandlerRegistry,
    config: TaskWorkerConfig,
    shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<Result<(), TaskError>> {
    tokio::spawn(async move { run(repository, registry, config, shutdown).await })
}

async fn run(
    repository: CatalogRepository,
    registry: TaskHandlerRegistry,
    config: TaskWorkerConfig,
    mut shutdown: watch::Receiver<()>,
) -> Result<(), TaskError> {
    let kinds = registry.kinds();
    gauge!("catalog_task_worker_active_kinds").set(kinds.len() as f64);
    if kinds.is_empty() {
        tracing::info!(
            "task worker started with no active kinds; no task will be leased before a safe cutover"
        );
    } else {
        tracing::info!(kinds = ?kinds, concurrency = config.concurrency, "task worker started");
    }
    let mut running = JoinSet::new();
    let mut next_kind = 0;
    let mut stopping = false;
    loop {
        if stopping {
            break;
        }
        while running.len() < config.concurrency {
            // Claim one registered kind at a time so the lease used at claim
            // is the registered per-kind policy, not a truncated global value.
            let mut claimed = None;
            for offset in 0..kinds.len() {
                let index = (next_kind + offset) % kinds.len();
                let kind = kinds[index];
                if let Some(task) = repository
                    .claim_task_for_kinds(&config.worker_id, kind.policy().lease_duration, &[kind])
                    .await?
                {
                    next_kind = (index + 1) % kinds.len();
                    claimed = Some(task);
                    break;
                }
            }
            let Some(task) = claimed else {
                break;
            };
            let Some(handler) = registry.handler(task.kind) else {
                // The query is restricted to the registry, so this can only
                // happen if the registry was internally corrupted. Leave the
                // lease for normal expiry rather than acknowledging unknown work.
                tracing::error!(kind = %task.kind, task_id = %task.id, "claimed task has no handler");
                break;
            };
            counter!("catalog_tasks_total", "kind" => task.kind.as_str(), "outcome" => "claimed")
                .increment(1);
            let task_repository = repository.clone();
            running.spawn(execute(task_repository, handler, task));
        }
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_ok() {
                    stopping = true;
                    tracing::info!(running = running.len(), "task worker stopped claiming; draining active tasks");
                }
            }
            joined = running.join_next(), if !running.is_empty() => {
                if let Some(Err(error)) = joined {
                    tracing::error!(error = %error, "task worker task panicked");
                }
            }
            _ = time::sleep(config.poll_interval) => {}
        }
    }
    // Handlers keep heartbeating during this bounded grace period. Tasks that
    // do not complete are aborted and their token-fenced lease is allowed to
    // expire; no unsafe acknowledgement is made during shutdown.
    if time::timeout(config.shutdown_grace, async {
        while running.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        tracing::warn!(
            remaining = running.len(),
            "task worker drain deadline elapsed; allowing leases to expire"
        );
        running.abort_all();
        while running.join_next().await.is_some() {}
    }
    Ok(())
}

fn truncate_error(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn bounded_error(code: &str, message: &str) -> (String, String) {
    (truncate_error(code, 128), truncate_error(message, 1024))
}

async fn execute(repository: CatalogRepository, handler: Arc<dyn TaskHandler>, task: ClaimedTask) {
    let kind = task.kind;
    let lease_duration = kind.policy().lease_duration;
    let heartbeat_every = lease_duration
        .checked_div(3)
        .unwrap_or(Duration::from_millis(1));
    let mut heartbeat = time::interval(heartbeat_every);
    heartbeat.tick().await;
    let handled = handler.handle(task.clone());
    tokio::pin!(handled);
    let outcome = loop {
        tokio::select! {
            outcome = &mut handled => break outcome,
            _ = heartbeat.tick() => {
                if let Err(error) = repository.heartbeat_task(task.id, &task.lease_owner, task.lease_token, lease_duration).await {
                    counter!("catalog_tasks_total", "kind" => kind.as_str(), "outcome" => "lease_lost").increment(1);
                    tracing::warn!(task_id = %task.id, kind = %kind, error = %error, "task heartbeat lost its lease");
                    handler.on_lease_lost(task.clone()).await;
                    return;
                }
            }
        }
    };
    let result = match outcome {
        Ok(TaskOutcome::Complete) => repository
            .complete_task(task.id, &task.lease_owner, task.lease_token)
            .await
            .map(|_| "completed"),
        Ok(TaskOutcome::Retry { delay, code }) => repository
            .retry_task_at(
                task.id,
                &task.lease_owner,
                task.lease_token,
                Utc::now() + chrono::Duration::from_std(delay).unwrap_or_default(),
                code,
                "handler requested retry",
            )
            .await
            .map(|status| {
                if status.as_str() == "dead_letter" {
                    "dead_letter"
                } else {
                    "retry"
                }
            }),
        Ok(TaskOutcome::Reschedule { at }) => repository
            .reschedule_task_at(task.id, &task.lease_owner, task.lease_token, at)
            .await
            .map(|_| "rescheduled"),
        Ok(TaskOutcome::DeadLettered) => Ok("dead_letter"),
        Err(error) => {
            let (code, message) = bounded_error(error.code, &error.message);
            repository
                .retry_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    Utc::now() + chrono::Duration::seconds(1),
                    &code,
                    &message,
                )
                .await
                .map(|status| {
                    if status.as_str() == "dead_letter" {
                        "dead_letter"
                    } else {
                        "retry"
                    }
                })
        }
    };
    match result {
        Ok(outcome) => {
            counter!("catalog_tasks_total", "kind" => kind.as_str(), "outcome" => outcome)
                .increment(1)
        }
        Err(error) => {
            counter!("catalog_tasks_total", "kind" => kind.as_str(), "outcome" => "lease_lost")
                .increment(1);
            tracing::warn!(task_id = %task.id, kind = %kind, error = %error, "task outcome could not be committed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Duplicate;
    #[async_trait]
    impl TaskHandler for Duplicate {
        fn kind(&self) -> TaskKind {
            TaskKind::RuleRunV1
        }
        async fn handle(&self, _: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
            Ok(TaskOutcome::Complete)
        }
    }

    #[test]
    fn registry_rejects_duplicate_kinds() {
        assert!(TaskHandlerRegistry::new(vec![Arc::new(Duplicate), Arc::new(Duplicate)]).is_err());
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn handler_errors_are_utf8_safe_and_bounded_before_retry() {
        let (code, message) = bounded_error("x", &"é".repeat(600));
        assert_eq!(code, "x");
        assert!(message.len() <= 1024);
        assert!(std::str::from_utf8(message.as_bytes()).is_ok());
    }
}
