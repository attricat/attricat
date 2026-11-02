//! Supervised executor for the API task envelope.
//!
//! Kinds are registered only when their producer and token-fenced domain
//! handler have cut over. An empty registry is intentional during the initial
//! rollout: the worker remains supervised but never leases unhandled work.

use std::{
    collections::HashMap,
    panic::AssertUnwindSafe,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use attricat_repository::round_trips::measure;
use chrono::Utc;
use futures_util::FutureExt;
use metrics::{counter, gauge, histogram};
use tokio::{sync::watch, task::JoinSet, time};
use uuid::Uuid;

use crate::{
    repository::{ClaimedTask, SystemRepository, TaskError},
    task_queue::{MAX_TASK_ERROR_CODE_BYTES, MAX_TASK_ERROR_MESSAGE_BYTES, TaskKind, TaskStatus},
};

const MAX_WORKER_ID_BYTES: usize = 128;
/// How often queue-depth gauges are refreshed.
const METRICS_INTERVAL: Duration = Duration::from_secs(5);
/// The pause after a failed claim before the worker polls again.
const CLAIM_RETRY_DELAY: Duration = Duration::from_secs(1);
/// How long a task waits after its first handler failure. Each further failure
/// waits four times longer, so a short database outage or a burst of deadlocks
/// cannot spend a whole failure budget within seconds.
const HANDLER_FAILURE_RETRY_DELAY: chrono::TimeDelta = chrono::TimeDelta::seconds(1);
const HANDLER_FAILURE_BACKOFF_FACTOR: i32 = 4;
/// How long a task waits to try again after its domain row could not be settled
/// when its failure budget ran out.
const EXHAUSTED_SETTLE_RETRY_DELAY: chrono::TimeDelta = chrono::TimeDelta::seconds(30);
/// The error code of a handler that panicked.
const HANDLER_PANIC_CODE: &str = "handler_panic";
/// The error code of a task whose leases kept expiring without it settling.
const LEASE_EXPIRED_CODE: &str = "lease_expired";

#[derive(Debug, Clone)]
pub struct TaskWorkerConfig {
    pub worker_id: String,
    pub concurrency: usize,
    pub poll_interval: Duration,
    pub shutdown_grace: Duration,
}

impl TaskWorkerConfig {
    /// Parses the `TASK_WORKER_*` settings, defaulting each unset one.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let positive = |name: &str, default: u64| -> Result<u64, String> {
            match get(name) {
                Some(value) => value
                    .parse()
                    .ok()
                    .filter(|value: &u64| *value > 0)
                    .ok_or_else(|| format!("{name} must be a positive integer")),
                None => Ok(default),
            }
        };
        let worker_id = get("TASK_WORKER_ID").unwrap_or_else(|| Uuid::new_v4().to_string());
        if worker_id.is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
            return Err(format!(
                "TASK_WORKER_ID must be 1-{MAX_WORKER_ID_BYTES} bytes"
            ));
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
    /// The handler settled its domain row and envelope itself without
    /// dead-lettering them, for example by scheduling a retry; `outcome`
    /// labels the task metric (`retry` or `rescheduled`).
    Settled {
        outcome: &'static str,
    },
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
    /// Called instead of retrying when the task's failure budget is spent, so
    /// the domain row and the envelope end together. Return `true` after
    /// dead-lettering both, `false` when only the envelope remains to end. An
    /// error is a transient failure to settle: the worker tries again later
    /// without dead-lettering anything.
    async fn on_exhausted(
        &self,
        _task: &ClaimedTask,
        _message: &str,
    ) -> Result<bool, TaskHandlerError> {
        Ok(false)
    }
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
    repository: impl Into<SystemRepository>,
    registry: TaskHandlerRegistry,
    config: TaskWorkerConfig,
    shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<Result<(), TaskError>> {
    let repository = repository.into();
    tokio::spawn(async move { run(repository, registry, config, shutdown).await })
}

async fn run(
    repository: SystemRepository,
    registry: TaskHandlerRegistry,
    config: TaskWorkerConfig,
    mut shutdown: watch::Receiver<()>,
) -> Result<(), TaskError> {
    let kinds = registry.kinds();
    gauge!("attricat_task_worker_active_kinds").set(kinds.len() as f64);
    if kinds.is_empty() {
        tracing::info!(
            "task worker started with no active kinds; no task will be leased before a safe cutover"
        );
    } else {
        tracing::info!(kinds = ?kinds, concurrency = config.concurrency, "task worker started");
    }
    let leases: Vec<(TaskKind, Duration)> = kinds
        .iter()
        .map(|kind| (*kind, kind.policy().lease_duration))
        .collect();
    // Tasks written before enqueueing created fairness rows still need one.
    if let Err(error) = repository.materialize_task_workspace_service().await {
        tracing::warn!(%error, "could not prepare task claim fairness rows");
    }
    let mut running = JoinSet::new();
    let mut stopping = false;
    let mut metrics_tick = time::interval(METRICS_INTERVAL);
    metrics_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    loop {
        if stopping {
            break;
        }
        let mut claim_failed = false;
        while running.len() < config.concurrency {
            // One statement claims the next due task of any registered kind,
            // leased for that kind's registered policy.
            let claimed = match measure(
                "worker:task_claim",
                repository.claim_task_with_leases(&config.worker_id, &leases),
            )
            .await
            {
                Ok(claimed) => claimed,
                Err(error) => {
                    tracing::warn!(%error, "task claim failed; retrying after backoff");
                    claim_failed = true;
                    None
                }
            };
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
            counter!("attricat_tasks_total", "kind" => task.kind.as_str(), "outcome" => "claimed")
                .increment(1);
            let task_repository = repository.clone();
            running.spawn(execute(task_repository, handler, task));
        }
        if claim_failed {
            // Keep the executor alive across transient database failures;
            // respect shutdown even when the queue cannot be reached.
            tokio::select! {
                _ = time::sleep(CLAIM_RETRY_DELAY) => {}
                _ = shutdown.changed() => { stopping = true; }
            }
            continue;
        }
        tokio::select! {
            _ = shutdown.changed() => {
                // A closed channel also means nobody can supervise this worker.
                // Drain rather than spinning on the immediately-ready error.
                stopping = true;
                tracing::info!(running = running.len(), "task worker stopped claiming; draining active tasks");
            }
            joined = running.join_next(), if !running.is_empty() => {
                if let Some(Err(error)) = joined {
                    tracing::error!(error = %error, "task worker task panicked");
                }
            }
            _ = metrics_tick.tick() => {
                if let Err(error) = measure("worker:task_metrics", async {
                    repository.materialize_task_workspace_service().await?;
                    record_queue_metrics(&repository, &kinds).await
                }).await {
                    tracing::warn!(%error, "could not refresh task queue metrics");
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

async fn record_queue_metrics(
    repository: &SystemRepository,
    kinds: &[TaskKind],
) -> Result<(), TaskError> {
    const STATUSES: [&str; 3] = [
        TaskStatus::Queued.as_str(),
        TaskStatus::Leased.as_str(),
        TaskStatus::DeadLetter.as_str(),
    ];
    for kind in kinds {
        for status in STATUSES {
            gauge!("attricat_task_queue_depth", "kind" => kind.as_str(), "status" => status)
                .set(0.0);
            gauge!("attricat_task_queue_oldest_age_seconds", "kind" => kind.as_str(), "status" => status).set(0.0);
            gauge!("attricat_task_queue_retries", "kind" => kind.as_str(), "status" => status)
                .set(0.0);
        }
    }
    for (kind, status, count, oldest_age, retries) in repository.task_queue_health(kinds).await? {
        gauge!("attricat_task_queue_depth", "kind" => kind.clone(), "status" => status.clone())
            .set(count as f64);
        gauge!("attricat_task_queue_oldest_age_seconds", "kind" => kind.clone(), "status" => status.clone())
            .set(oldest_age.max(0.0));
        gauge!("attricat_task_queue_retries", "kind" => kind, "status" => status)
            .set(retries as f64);
    }

    const OPERATION_STATUSES: [&str; 3] = ["pending", "leased", "dead_letter"];
    for status in OPERATION_STATUSES {
        gauge!("attricat_extension_operation_runs", "status" => status).set(0.0);
        gauge!("attricat_extension_operation_oldest_age_seconds", "status" => status).set(0.0);
        gauge!("attricat_extension_operation_retries", "status" => status).set(0.0);
    }
    for (status, count, oldest_age, retries) in repository.extension_operation_health().await? {
        gauge!("attricat_extension_operation_runs", "status" => status.clone()).set(count as f64);
        gauge!("attricat_extension_operation_oldest_age_seconds", "status" => status.clone())
            .set(oldest_age.max(0.0));
        gauge!("attricat_extension_operation_retries", "status" => status).set(retries as f64);
    }
    Ok(())
}

fn truncate_error(value: &str, max_bytes: usize) -> String {
    value[..value.floor_char_boundary(max_bytes)].to_owned()
}

fn bounded_error(code: &str, message: &str) -> (String, String) {
    (
        truncate_error(code, MAX_TASK_ERROR_CODE_BYTES),
        truncate_error(message, MAX_TASK_ERROR_MESSAGE_BYTES),
    )
}

/// When a task that has failed `failures` times may run again.
fn handler_failure_retry_at(failures: i32) -> chrono::DateTime<Utc> {
    let delay = HANDLER_FAILURE_RETRY_DELAY
        * HANDLER_FAILURE_BACKOFF_FACTOR.saturating_pow(failures.clamp(0, 8) as u32);
    Utc::now() + delay
}

/// Ends a task whose failure budget is spent: its domain row first, then the
/// envelope. If the domain row cannot be settled now, the task runs again later
/// with its last failure unspent, so the two can't end up in different states.
async fn dead_letter_exhausted(
    repository: &SystemRepository,
    handler: &dyn TaskHandler,
    task: &ClaimedTask,
    code: &str,
    message: &str,
) -> Result<&'static str, TaskError> {
    match handler.on_exhausted(task, message).await {
        Ok(true) => Ok("dead_letter"),
        Ok(false) => repository
            .dead_letter_task(task, code, message)
            .await
            .map(|()| "dead_letter"),
        Err(error) => {
            tracing::warn!(task_id = %task.id, code = error.code, error = %error.message, "could not settle a task that spent its failure budget");
            repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    Utc::now() + EXHAUSTED_SETTLE_RETRY_DELAY,
                )
                .await
                .map(|()| "rescheduled")
        }
    }
}

/// The metric outcome of a retried task: exhausting its failure budget
/// dead-letters it.
fn retry_outcome(status: TaskStatus) -> &'static str {
    if status == TaskStatus::DeadLetter {
        "dead_letter"
    } else {
        "retry"
    }
}

async fn execute(repository: SystemRepository, handler: Arc<dyn TaskHandler>, task: ClaimedTask) {
    let started = Instant::now();
    let kind = task.kind;
    gauge!("attricat_task_worker_tasks_in_flight", "kind" => kind.as_str()).increment(1.0);
    let lease_duration = kind.policy().lease_duration;
    let heartbeat_every = lease_duration
        .checked_div(3)
        .unwrap_or(Duration::from_millis(1));
    // Reclaiming an expired lease spends a failure, so a task that keeps killing
    // its process or never settles stops here instead of running forever.
    if task.failures >= task.max_failures {
        let (code, message) = bounded_error(
            LEASE_EXPIRED_CODE,
            "task lease kept expiring before the task settled",
        );
        tracing::error!(task_id = %task.id, kind = %kind, "task lease kept expiring; dead-lettering it");
        let result =
            dead_letter_exhausted(&repository, handler.as_ref(), &task, &code, &message).await;
        record_task_result(kind, started, result);
        return;
    }
    // A panic counts as a failed run rather than leaving the lease to expire.
    let handled = measure(
        format!("task:{kind}"),
        AssertUnwindSafe(handler.handle(task.clone())).catch_unwind(),
    )
    .map(|result| {
        result.unwrap_or_else(|_| {
            Err(TaskHandlerError {
                code: HANDLER_PANIC_CODE,
                message: "task handler panicked".to_owned(),
            })
        })
    });
    let renewals = async {
        let mut heartbeat = time::interval(heartbeat_every);
        heartbeat.tick().await;
        let mut lease_ends = time::Instant::now() + lease_duration;
        loop {
            heartbeat.tick().await;
            // A database error says nothing about the lease, which stays valid
            // until it ends. Only then is this execution certainly out of time.
            let attempted_at = time::Instant::now();
            match time::timeout_at(
                lease_ends,
                repository.heartbeat_task(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    lease_duration,
                ),
            )
            .await
            {
                Ok(Ok(())) => lease_ends = attempted_at + lease_duration,
                Ok(Err(TaskError::LeaseLost)) | Err(_) => return TaskError::LeaseLost,
                Ok(Err(error)) if time::Instant::now() >= lease_ends => return error,
                Ok(Err(error)) => {
                    tracing::warn!(task_id = %task.id, kind = %kind, error = %error, "task heartbeat failed; retrying");
                }
            }
        }
    };
    let outcome = match crate::heartbeat::with_heartbeat(handled, renewals).await {
        Ok(outcome) => outcome,
        Err(error) => {
            counter!("attricat_tasks_total", "kind" => kind.as_str(), "outcome" => "lease_lost")
                .increment(1);
            tracing::warn!(task_id = %task.id, kind = %kind, error = %error, "task heartbeat lost its lease");
            // The execution future has already dropped its transactions and
            // other resources before lease-loss cleanup starts.
            handler.on_lease_lost(task.clone()).await;
            histogram!("attricat_task_worker_task_duration_seconds", "kind" => kind.as_str(), "outcome" => "lease_lost").record(started.elapsed().as_secs_f64());
            gauge!("attricat_task_worker_tasks_in_flight", "kind" => kind.as_str()).decrement(1.0);
            return;
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
            .map(retry_outcome),
        Ok(TaskOutcome::Reschedule { at }) => repository
            .reschedule_task_at(task.id, &task.lease_owner, task.lease_token, at)
            .await
            .map(|_| "rescheduled"),
        Ok(TaskOutcome::DeadLettered) => Ok("dead_letter"),
        Ok(TaskOutcome::Settled { outcome }) => Ok(outcome),
        Err(error) => {
            let (code, message) = bounded_error(error.code, &error.message);
            tracing::error!(task_id = %task.id, kind = %kind, %code, %message, "task handler failed");
            if task.failures + 1 >= task.max_failures {
                dead_letter_exhausted(&repository, handler.as_ref(), &task, &code, &message).await
            } else {
                repository
                    .retry_task_at(
                        task.id,
                        &task.lease_owner,
                        task.lease_token,
                        handler_failure_retry_at(task.failures),
                        &code,
                        &message,
                    )
                    .await
                    .map(retry_outcome)
            }
        }
    };
    record_task_result(kind, started, result);
}

/// Records the metrics of a task whose outcome was committed, or could not be.
fn record_task_result(kind: TaskKind, started: Instant, result: Result<&'static str, TaskError>) {
    let metric_outcome = match result {
        Ok(outcome) => {
            counter!("attricat_tasks_total", "kind" => kind.as_str(), "outcome" => outcome)
                .increment(1);
            outcome
        }
        Err(error) => {
            counter!("attricat_tasks_total", "kind" => kind.as_str(), "outcome" => "lease_lost")
                .increment(1);
            tracing::warn!(kind = %kind, error = %error, "task outcome could not be committed");
            "lease_lost"
        }
    };
    histogram!("attricat_task_worker_task_duration_seconds", "kind" => kind.as_str(), "outcome" => metric_outcome)
        .record(started.elapsed().as_secs_f64());
    gauge!("attricat_task_worker_tasks_in_flight", "kind" => kind.as_str()).decrement(1.0);
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

    #[tokio::test]
    async fn claim_failure_does_not_stop_the_worker() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(100))
            .connect_lazy("postgres://postgres@127.0.0.1:1/attricat")
            .unwrap();
        let repository = crate::repository::AttricatRepository::system(pool);
        let registry = TaskHandlerRegistry::new(vec![Arc::new(Duplicate)]).unwrap();
        let (shutdown, receiver) = watch::channel(());
        let worker = start(
            repository,
            registry,
            TaskWorkerConfig {
                worker_id: "test".into(),
                concurrency: 1,
                poll_interval: Duration::from_millis(10),
                shutdown_grace: Duration::from_millis(10),
            },
            receiver,
        );
        time::sleep(Duration::from_millis(250)).await;
        assert!(
            !worker.is_finished(),
            "transient claim failure must not kill the worker"
        );
        shutdown.send(()).unwrap();
        time::timeout(Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn handler_errors_are_utf8_safe_and_bounded_before_retry() {
        let (code, message) = bounded_error("x", &"é".repeat(600));
        assert_eq!(code, "x");
        assert!(message.len() <= MAX_TASK_ERROR_MESSAGE_BYTES);
        assert!(std::str::from_utf8(message.as_bytes()).is_ok());
    }
}
