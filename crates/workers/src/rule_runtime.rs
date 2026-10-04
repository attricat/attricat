//! Rule intake and schedule advancement are producers. Evaluation is owned by
//! the shared task worker after the rule task cutover.
use crate::repository::CoordinatorLeadership;
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    repository::{
        CatalogRepository, ClaimedRuleRun, RepositoryError, RuleCandidateResult, SystemRepository,
    },
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use async_trait::async_trait;
use catalog_repository::round_trips::measure;
use chrono::Utc;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use uuid::Uuid;

const PAGE_SIZE: i64 = 500;

pub struct RuleIntakeHandler;
#[async_trait]
impl EventHandler for RuleIntakeHandler {
    fn name(&self) -> &'static str {
        "catalog.rules"
    }

    fn event_types(&self) -> &'static [&'static str] {
        ALL_EVENT_TYPES_V1
    }

    async fn handle(
        &self,
        event: DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        context.repository().fan_out_rule_runs(&event).await?;
        Ok(())
    }
}

pub fn add_to_registry(
    registry: crate::event_dispatcher::EventHandlerRegistry,
) -> crate::event_dispatcher::EventHandlerRegistry {
    registry
        .with_handler(Arc::new(RuleIntakeHandler))
        .expect("rule handler name is unique")
}

pub struct RuleTaskHandler {
    repository: SystemRepository,
}

pub fn task_handler(repository: impl Into<SystemRepository>) -> Arc<dyn TaskHandler> {
    Arc::new(RuleTaskHandler {
        repository: repository.into(),
    })
}

#[async_trait]
impl TaskHandler for RuleTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::RuleRunV1
    }

    async fn handle(
        &self,
        task: crate::repository::ClaimedTask,
    ) -> Result<TaskOutcome, TaskHandlerError> {
        let scoped = self
            .repository
            .for_workspace(task.workspace_id)
            .await
            .map_err(task_error)?;
        let run = match scoped.begin_rule_run_task(&task).await {
            Ok(run) => run,
            Err(error) => return fail_rule_run(&scoped, &task, error).await,
        };
        let Some(run) = run else {
            // A legacy split terminal transition is repaired by dead-lettering
            // the reclaimed generic envelope, never by acknowledging it.
            if scoped
                .rule_run_is_dead_letter_for_task(&task)
                .await
                .map_err(task_error)?
            {
                scoped
                    .dead_letter_task(&task, "rule_run", "rule run is already dead-lettered")
                    .await
                    .map_err(|error| task_error(error.into()))?;
                return Ok(TaskOutcome::DeadLettered);
            }
            return Ok(TaskOutcome::Complete);
        };
        match evaluate_page(&scoped, &run).await {
            Ok((results, next, done)) => match scoped
                .checkpoint_rule_page_task(&task, &run, results, next, done)
                .await
            {
                Ok(outcome) if outcome => Ok(TaskOutcome::Complete),
                Ok(_) => {
                    // Yield after every page. The shared queue keeps this from
                    // monopolising a worker or consuming a failure budget.
                    Ok(TaskOutcome::Reschedule { at: Utc::now() })
                }
                Err(error) => fail_rule_run(&scoped, &task, error).await,
            },
            Err(error) => fail_rule_run(&scoped, &task, error).await,
        }
    }
}

async fn fail_rule_run(
    scoped: &CatalogRepository,
    task: &crate::repository::ClaimedTask,
    error: RepositoryError,
) -> Result<TaskOutcome, TaskHandlerError> {
    let message = bounded_error_message(&error.to_string());
    if scoped
        .fail_rule_run_task(task, &message)
        .await
        .map_err(task_error)?
    {
        Ok(TaskOutcome::DeadLettered)
    } else {
        // The generic task worker owns retries before the failure budget is
        // exhausted. The repository has returned the run to pending first.
        Err(TaskHandlerError {
            code: "rule_run",
            message,
        })
    }
}

fn bounded_error_message(message: &str) -> String {
    let mut end = message.len().min(1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}

fn task_error(error: RepositoryError) -> TaskHandlerError {
    TaskHandlerError {
        code: "rule_run",
        message: error.to_string(),
    }
}

async fn evaluate_page(
    repo: &CatalogRepository,
    run: &ClaimedRuleRun,
) -> Result<(Vec<RuleCandidateResult>, Option<Uuid>, bool), RepositoryError> {
    let compiled: catalog_rules::CompiledRule =
        serde_json::from_value(run.compiled_plan.clone())
            .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
    let remaining =
        (catalog_rules::MAX_CANDIDATES_PER_RUN as i64 - run.candidates_evaluated).max(0);
    if remaining == 0 {
        return Ok((Vec::new(), run.candidate_cursor, true));
    }
    let pool = repo.pool_for_runtime();
    let candidates: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM entities WHERE workspace_id=$1 AND blueprint_id=$2 AND blueprint_version=$3 AND deleted_at IS NULL AND ($4::uuid IS NULL OR id>$4) AND ($5::uuid IS NULL OR id=$5) ORDER BY id LIMIT $6",
    )
    .bind(repo.workspace_id_for_runtime())
    .bind(run.blueprint_id)
    .bind(run.blueprint_version)
    .bind(run.candidate_cursor)
    .bind(run.scope_entity_id)
    .bind(PAGE_SIZE.min(remaining))
    .fetch_all(&pool)
    .await?;
    let results = repo
        .evaluate_rule_candidates(run.context_id, &compiled, &candidates)
        .await?;
    let page_size = PAGE_SIZE.min(remaining) as usize;
    let done = candidates.len() < page_size
        || run.scope_entity_id.is_some()
        || run.candidates_evaluated + candidates.len() as i64
            >= catalog_rules::MAX_CANDIDATES_PER_RUN as i64;
    Ok((results, candidates.last().copied(), done))
}

/// The only rule schedule coordinator. It never claims rule runs; task-worker
/// instances are the sole execution claimers.
pub fn start_schedule_coordinator(
    repository: SystemRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Workspaces this coordinator has prepared, with the last full pass.
        let mut prepared: HashMap<Uuid, Instant> = HashMap::new();
        // Only one replica runs schedules; the others stand by.
        let mut leadership = CoordinatorLeadership::new("rule_schedule");
        loop {
            if !leadership.is_leader(&repository).await {
                tokio::select! { _ = tokio::time::sleep(Duration::from_secs(1)) => continue, _ = shutdown.changed() => return }
            }
            let delay = measure("worker:rule_schedule", async {
                let (workspaces, delay) = match repository.polled_workspace_ids().await {
                    Ok(workspaces) => {
                        let workspaces = workspaces.as_ref().clone();
                        metrics::gauge!("catalog_schedule_coordinator_healthy", "kind" => "rule")
                            .set(1.0);
                        (workspaces, Duration::from_millis(250))
                    }
                    Err(error) => {
                        metrics::gauge!("catalog_schedule_coordinator_healthy", "kind" => "rule")
                            .set(0.0);
                        tracing::warn!(%error, "rule scheduler cannot discover workspaces");
                        (Vec::new(), Duration::from_secs(5))
                    }
                };
                for workspace in workspaces {
                    match repository.for_workspace(workspace).await {
                        Ok(scoped) => {
                            // Pre-cutover rows get task envelopes once per
                            // workspace; afterwards an idle tick only checks
                            // whether any schedule cursor is due.
                            let first = prepared.insert(workspace, Instant::now()).is_none();
                            if first && let Err(error) = scoped.backfill_rule_tasks().await {
                                tracing::error!(%error, "rule task backfill failed");
                            }
                            let run = if first {
                                Ok(true)
                            } else {
                                scoped.rule_schedules_due().await
                            };
                            let result = match run {
                                Ok(true) => scoped.schedule_rule_runs().await.map(drop),
                                Ok(false) => Ok(()),
                                Err(error) => Err(error),
                            };
                            if let Err(error) = result {
                                tracing::error!(%error, "rule schedule poll failed");
                            }
                        }
                        Err(error) => {
                            tracing::error!(%error, "rule scheduler workspace scope failed")
                        }
                    }
                }
                delay
            })
            .await;
            tokio::select! { _ = tokio::time::sleep(delay) => {}, _ = shutdown.changed() => return }
        }
    })
}
