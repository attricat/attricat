//! Bounded periodic retention and cancellation-safe object cleanup.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use attricat_repository::repository::{RepositoryError, RetentionDays, SystemRepository};
use attricat_repository::round_trips::measure;
use attricat_storage::{ObjectStore, ObjectStoreError};
use tokio::sync::watch;

/// The upload cleanup's share of every tick, separate from the retention budget.
const UPLOAD_CLEANUP_BUDGET: Duration = Duration::from_secs(10);
/// The most abandoned uploads one sweep deletes.
const MAX_UPLOAD_CLEANUPS_PER_SWEEP: usize = 100;

const DEFAULT_INTERVAL_SECONDS: u64 = 60;
const MAX_INTERVAL_SECONDS: u64 = 86_400;
const DEFAULT_SWEEP_BUDGET_SECONDS: u64 = 10;
const MAX_SWEEP_BUDGET_SECONDS: u64 = 3_600;
const DEFAULT_BATCH_ROWS: u64 = 1_000;
const MAX_BATCH_ROWS: u64 = 10_000;

/// One table family the retention sweep purges. Each has its own repository
/// batch, retention setting and metric label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionFamily {
    ValueHistory,
    Tasks,
    RuleRuns,
    WorkflowRuns,
    ExtensionOperationRuns,
    AgentRunEvents,
    FileProcessingJobs,
    SolutionPackCheckRuns,
    DomainEvents,
    BrowserSessions,
    UserActionTokens,
    WorkspaceInvitations,
    RateLimitWindows,
    AuditEvents,
    ReadNotifications,
}

impl RetentionFamily {
    pub const ALL: [Self; 15] = [
        Self::ValueHistory,
        Self::Tasks,
        Self::RuleRuns,
        Self::WorkflowRuns,
        Self::ExtensionOperationRuns,
        Self::AgentRunEvents,
        Self::FileProcessingJobs,
        Self::SolutionPackCheckRuns,
        Self::DomainEvents,
        Self::BrowserSessions,
        Self::UserActionTokens,
        Self::WorkspaceInvitations,
        Self::RateLimitWindows,
        Self::AuditEvents,
        Self::ReadNotifications,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ValueHistory => "value_history",
            Self::Tasks => "tasks",
            Self::RuleRuns => "rule_runs",
            Self::WorkflowRuns => "workflow_runs",
            Self::ExtensionOperationRuns => "extension_operation_runs",
            Self::AgentRunEvents => "agent_run_events",
            Self::FileProcessingJobs => "file_processing_jobs",
            Self::SolutionPackCheckRuns => "solution_pack_check_runs",
            Self::DomainEvents => "domain_events",
            Self::BrowserSessions => "browser_sessions",
            Self::UserActionTokens => "user_action_tokens",
            Self::WorkspaceInvitations => "workspace_invitations",
            Self::RateLimitWindows => "rate_limit_windows",
            Self::AuditEvents => "audit_events",
            Self::ReadNotifications => "read_notifications",
        }
    }
}

/// The retention sweep's schedule, batch size and per-family retention. User
/// history (`audit_events`, `read_notifications`) is kept unless configured.
#[derive(Clone, Copy, Debug)]
pub struct RetentionConfig {
    pub interval: Duration,
    pub sweep_budget: Duration,
    pub batch_rows: i64,
    pub value_history: RetentionDays,
    pub tasks: RetentionDays,
    pub record_rule_runs: RetentionDays,
    pub rule_runs: RetentionDays,
    pub workflow_runs: RetentionDays,
    pub extension_operation_runs: RetentionDays,
    pub agent_run_events: RetentionDays,
    pub file_processing_jobs: RetentionDays,
    pub solution_pack_check_runs: RetentionDays,
    pub domain_events: RetentionDays,
    pub browser_sessions: RetentionDays,
    pub user_action_tokens: RetentionDays,
    pub workspace_invitations: RetentionDays,
    pub audit_events: Option<RetentionDays>,
    pub read_notifications: Option<RetentionDays>,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self::from_values(|_| None).expect("retention defaults are valid")
    }
}

impl RetentionConfig {
    /// Parses the retention settings, defaulting each unset one.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let bounded = |name: &str, default: u64, maximum: u64| -> Result<u64, String> {
            match get(name) {
                Some(value) => value
                    .parse::<u64>()
                    .ok()
                    .filter(|value| (1..=maximum).contains(value))
                    .ok_or_else(|| format!("{name} must be an integer between 1 and {maximum}")),
                None => Ok(default),
            }
        };
        let optional_days = |name: &str| -> Result<Option<RetentionDays>, String> {
            get(name)
                .map(|value| value.parse().map_err(|error| format!("{name} {error}")))
                .transpose()
        };
        let days = |name: &str, default: i64| -> Result<RetentionDays, String> {
            Ok(optional_days(name)?
                .unwrap_or_else(|| default.to_string().parse().expect("default is positive")))
        };
        let interval = bounded(
            "RETENTION_SWEEP_INTERVAL_SECONDS",
            DEFAULT_INTERVAL_SECONDS,
            MAX_INTERVAL_SECONDS,
        )?;
        let sweep_budget = bounded(
            "RETENTION_SWEEP_BUDGET_SECONDS",
            DEFAULT_SWEEP_BUDGET_SECONDS,
            MAX_SWEEP_BUDGET_SECONDS,
        )?;
        if sweep_budget > interval {
            return Err(
                "RETENTION_SWEEP_BUDGET_SECONDS must not exceed RETENTION_SWEEP_INTERVAL_SECONDS"
                    .to_owned(),
            );
        }
        Ok(Self {
            interval: Duration::from_secs(interval),
            sweep_budget: Duration::from_secs(sweep_budget),
            batch_rows: bounded("RETENTION_BATCH_ROWS", DEFAULT_BATCH_ROWS, MAX_BATCH_ROWS)?
                .try_into()
                .expect("bounded batch size fits i64"),
            value_history: days("ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS", 90)?,
            tasks: days("TASK_RETENTION_DAYS", 7)?,
            record_rule_runs: days("RULE_RUN_RECORD_RETENTION_DAYS", 7)?,
            rule_runs: days("RULE_RUN_RETENTION_DAYS", 90)?,
            workflow_runs: days("WORKFLOW_RUN_RETENTION_DAYS", 90)?,
            extension_operation_runs: days("EXTENSION_OPERATION_RUN_RETENTION_DAYS", 90)?,
            agent_run_events: days("AGENT_RUN_EVENT_RETENTION_DAYS", 90)?,
            file_processing_jobs: days("FILE_PROCESSING_JOB_RETENTION_DAYS", 30)?,
            solution_pack_check_runs: days("SOLUTION_PACK_CHECK_RUN_RETENTION_DAYS", 90)?,
            domain_events: days("DOMAIN_EVENT_RETENTION_DAYS", 90)?,
            browser_sessions: days("BROWSER_SESSION_RETENTION_DAYS", 7)?,
            user_action_tokens: days("USER_ACTION_TOKEN_RETENTION_DAYS", 7)?,
            workspace_invitations: days("WORKSPACE_INVITATION_RETENTION_DAYS", 30)?,
            audit_events: optional_days("AUDIT_EVENT_RETENTION_DAYS")?,
            read_notifications: optional_days("READ_NOTIFICATION_RETENTION_DAYS")?,
        })
    }

    /// The families this configuration sweeps, in round-robin order.
    pub fn families(&self) -> Vec<RetentionFamily> {
        RetentionFamily::ALL
            .into_iter()
            .filter(|family| match family {
                RetentionFamily::AuditEvents => self.audit_events.is_some(),
                RetentionFamily::ReadNotifications => self.read_notifications.is_some(),
                _ => true,
            })
            .collect()
    }
}

pub fn start(
    repository: SystemRepository,
    store: Arc<dyn ObjectStore>,
    retention: RetentionConfig,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(retention.interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = tick.tick() => {
                    tokio::select! {
                        _ = shutdown.changed() => break,
                        _ = measure("worker:maintenance", run_once(&repository, store.as_ref(), &retention)) => {}
                    }
                }
            }
        }
    })
}

async fn run_once(
    repository: &SystemRepository,
    store: &dyn ObjectStore,
    retention: &RetentionConfig,
) {
    sweep_retention(repository, retention).await;

    let _ = tokio::time::timeout(UPLOAD_CLEANUP_BUDGET, async {
        for _ in 0..MAX_UPLOAD_CLEANUPS_PER_SWEEP {
            match cleanup_upload_once(repository, store).await {
                Ok(true) => {},
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(%error, "upload cleanup failed; durable intent retained for retry");
                    metrics::counter!("attricat_upload_cleanup_total", "outcome" => "failed").increment(1);
                    break;
                }
            }
        }
    }).await;
}

/// The outcome of one family in one sweep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilySweep {
    pub family: RetentionFamily,
    pub purged: u64,
    /// `success` (nothing expired remains), `failed` or `budget_exhausted`.
    pub outcome: &'static str,
}

/// Runs one batch per family in turn until every family is drained, has
/// failed, or the shared budget runs out, so one large backlog cannot starve
/// the others. A failed family is logged and retried on the next tick.
pub async fn sweep_retention(
    repository: &SystemRepository,
    config: &RetentionConfig,
) -> Vec<FamilySweep> {
    struct Progress {
        sweep: FamilySweep,
        elapsed: Duration,
        done: bool,
    }
    let deadline = Instant::now() + config.sweep_budget;
    let mut progress: Vec<Progress> = config
        .families()
        .into_iter()
        .map(|family| Progress {
            sweep: FamilySweep {
                family,
                purged: 0,
                outcome: "budget_exhausted",
            },
            elapsed: Duration::ZERO,
            done: false,
        })
        .collect();
    while progress.iter().any(|entry| !entry.done) {
        for entry in progress.iter_mut().filter(|entry| !entry.done) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                entry.done = true;
                continue;
            }
            let family = entry.sweep.family;
            let started = Instant::now();
            let batch =
                tokio::time::timeout(remaining, purge_batch(repository, config, family)).await;
            entry.elapsed += started.elapsed();
            match batch {
                Ok(Ok(deleted)) => {
                    entry.sweep.purged += deleted;
                    metrics::counter!("attricat_retention_rows_purged_total", "family" => family.label())
                        .increment(deleted);
                    if deleted < config.batch_rows.unsigned_abs() {
                        entry.sweep.outcome = "success";
                        entry.done = true;
                    }
                }
                Ok(Err(error)) => {
                    tracing::error!(family = family.label(), %error, "retention batch failed; will retry");
                    entry.sweep.outcome = "failed";
                    entry.done = true;
                }
                Err(_) => entry.done = true,
            }
            tokio::task::yield_now().await;
        }
    }
    progress
        .into_iter()
        .map(|entry| {
            let family = entry.sweep.family.label();
            metrics::counter!("attricat_retention_cleanup_total", "family" => family, "outcome" => entry.sweep.outcome)
                .increment(1);
            metrics::histogram!("attricat_retention_cleanup_duration_seconds", "family" => family)
                .record(entry.elapsed.as_secs_f64());
            entry.sweep
        })
        .collect()
}

async fn purge_batch(
    repository: &SystemRepository,
    config: &RetentionConfig,
    family: RetentionFamily,
) -> Result<u64, RepositoryError> {
    let limit = config.batch_rows;
    match family {
        RetentionFamily::ValueHistory => {
            repository
                .purge_value_history_batch(config.value_history, limit)
                .await
        }
        RetentionFamily::Tasks => repository.purge_tasks_batch(config.tasks, limit).await,
        RetentionFamily::RuleRuns => {
            repository
                .purge_rule_runs_batch(config.record_rule_runs, config.rule_runs, limit)
                .await
        }
        RetentionFamily::WorkflowRuns => {
            repository
                .purge_workflow_runs_batch(config.workflow_runs, limit)
                .await
        }
        RetentionFamily::ExtensionOperationRuns => {
            repository
                .purge_extension_operation_runs_batch(config.extension_operation_runs, limit)
                .await
        }
        RetentionFamily::AgentRunEvents => {
            repository
                .purge_agent_run_events_batch(config.agent_run_events, limit)
                .await
        }
        RetentionFamily::FileProcessingJobs => {
            repository
                .purge_file_processing_jobs_batch(config.file_processing_jobs, limit)
                .await
        }
        RetentionFamily::SolutionPackCheckRuns => {
            repository
                .purge_solution_pack_check_runs_batch(config.solution_pack_check_runs, limit)
                .await
        }
        RetentionFamily::DomainEvents => {
            repository
                .purge_domain_events_batch(config.domain_events, limit)
                .await
        }
        RetentionFamily::BrowserSessions => {
            repository
                .purge_browser_sessions_batch(config.browser_sessions, limit)
                .await
        }
        RetentionFamily::UserActionTokens => {
            repository
                .purge_user_action_tokens_batch(config.user_action_tokens, limit)
                .await
        }
        RetentionFamily::WorkspaceInvitations => {
            repository
                .purge_workspace_invitations_batch(config.workspace_invitations, limit)
                .await
        }
        RetentionFamily::RateLimitWindows => repository.purge_rate_limit_windows_batch(limit).await,
        RetentionFamily::AuditEvents => match config.audit_events {
            Some(days) => repository.purge_audit_events_batch(days, limit).await,
            None => Ok(0),
        },
        RetentionFamily::ReadNotifications => match config.read_notifications {
            Some(days) => repository.purge_read_notifications_batch(days, limit).await,
            None => Ok(0),
        },
    }
}

/// One idempotent external deletion. A failure or process crash leaves the
/// claimed intent reclaimable after five minutes. No DB lock spans S3 I/O.
pub async fn cleanup_upload_once(
    repository: &SystemRepository,
    store: &dyn ObjectStore,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(upload) = repository.claim_abandoned_upload().await? else {
        return Ok(false);
    };
    match store.delete(&upload.object_key).await {
        Ok(()) | Err(ObjectStoreError::NotFound) => {}
        Err(error) => return Err(error.into()),
    }
    repository.complete_abandoned_upload(&upload).await?;
    metrics::counter!("attricat_upload_cleanup_total", "outcome" => "success").increment(1);
    Ok(true)
}
