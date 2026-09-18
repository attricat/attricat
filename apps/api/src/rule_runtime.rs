//! Rule intake and schedule advancement are producers. Evaluation is owned by
//! the shared task worker after the rule task cutover.
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    repository::{CatalogRepository, ClaimedRuleRun, RepositoryError, RuleCandidateResult},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
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
    repository: CatalogRepository,
}

pub fn task_handler(repository: CatalogRepository) -> Arc<dyn TaskHandler> {
    Arc::new(RuleTaskHandler { repository })
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
        let Some(run) = scoped
            .begin_rule_run_task(&task)
            .await
            .map_err(task_error)?
        else {
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
            Ok((results, next, done)) => {
                let outcome = scoped
                    .checkpoint_rule_page_task(&task, &run, results, next, done)
                    .await
                    .map_err(task_error)?;
                if outcome {
                    Ok(TaskOutcome::Complete)
                } else {
                    // Yield after every page. The shared queue keeps this from
                    // monopolising a worker or consuming a failure budget.
                    Ok(TaskOutcome::Reschedule { at: Utc::now() })
                }
            }
            Err(error) => {
                let message = bounded_error_message(&error.to_string());
                if scoped
                    .fail_rule_run_task(&task, &message)
                    .await
                    .map_err(task_error)?
                {
                    return Ok(TaskOutcome::DeadLettered);
                }
                Err(TaskHandlerError {
                    code: "rule_run",
                    message,
                })
            }
        }
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
    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT id,system_tags FROM entities WHERE workspace_id=$1 AND blueprint_id=$2 AND blueprint_version=$3 AND deleted_at IS NULL AND ($4::uuid IS NULL OR id>$4) AND ($5::uuid IS NULL OR id=$5) ORDER BY id LIMIT $6",
    )
    .bind(repo.workspace_id_for_runtime())
    .bind(run.blueprint_id)
    .bind(run.blueprint_version)
    .bind(run.candidate_cursor)
    .bind(run.scope_entity_id)
    .bind(PAGE_SIZE.min(remaining))
    .fetch_all(&pool)
    .await?;
    let mut results = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let (failed, message, evidence) = predicate_failure(
            &pool,
            repo.workspace_id_for_runtime(),
            run,
            candidate,
            &compiled.predicate,
        )
        .await?;
        results.push(RuleCandidateResult {
            entity_id: candidate.id,
            failed,
            message,
            evidence,
            evaluation_key: compiled.raw_definition_hash.clone(),
            severity: format!("{:?}", compiled.severity).to_lowercase(),
        });
    }
    let page_size = PAGE_SIZE.min(remaining) as usize;
    let done = candidates.len() < page_size
        || run.scope_entity_id.is_some()
        || run.candidates_evaluated + candidates.len() as i64
            >= catalog_rules::MAX_CANDIDATES_PER_RUN as i64;
    Ok((
        results,
        candidates.last().map(|candidate| candidate.id),
        done,
    ))
}

#[derive(sqlx::FromRow)]
struct Candidate {
    id: Uuid,
    system_tags: Vec<String>,
}

async fn predicate_failure(
    pool: &sqlx::PgPool,
    ws: Uuid,
    run: &ClaimedRuleRun,
    candidate: &Candidate,
    predicate: &catalog_rules::Predicate,
) -> Result<(bool, String, Value), RepositoryError> {
    match predicate {
        catalog_rules::Predicate::HasTag { tag } => Ok((
            !candidate.system_tags.iter().any(|item| item == tag),
            format!("Entity is missing required system tag '{tag}'"),
            json!({"tag":tag}),
        )),
        catalog_rules::Predicate::MissingTag { tag } => Ok((
            candidate.system_tags.iter().any(|item| item == tag),
            format!("Entity has prohibited system tag '{tag}'"),
            json!({"tag":tag}),
        )),
        catalog_rules::Predicate::Required { attribute_code } => {
            let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id WHERE av.workspace_id=$1 AND av.entity_id=$2 AND a.code=$3 AND a.blueprint_id=$4 AND a.blueprint_version=$5 AND av.latest AND av.active AND av.context_id IS NOT DISTINCT FROM $6)")
                .bind(ws).bind(candidate.id).bind(attribute_code).bind(run.blueprint_id).bind(run.blueprint_version).bind(run.context_id).fetch_one(pool).await?;
            Ok((
                !present,
                format!("Required attribute '{attribute_code}' has no value"),
                json!({"attribute_code":attribute_code,"context_id":run.context_id}),
            ))
        }
        catalog_rules::Predicate::Stale {
            attribute_code,
            max_age_seconds,
        } => {
            let changed: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT max(av.created_at) FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id WHERE av.workspace_id=$1 AND av.entity_id=$2 AND a.code=$3 AND a.blueprint_id=$4 AND a.blueprint_version=$5 AND av.latest AND av.active AND av.context_id IS NOT DISTINCT FROM $6")
                .bind(ws).bind(candidate.id).bind(attribute_code).bind(run.blueprint_id).bind(run.blueprint_version).bind(run.context_id).fetch_one(pool).await?;
            let stale = changed
                .is_none_or(|time| (Utc::now() - time).num_seconds() > *max_age_seconds as i64);
            Ok((
                stale,
                format!("Attribute '{attribute_code}' is older than {max_age_seconds} seconds"),
                json!({"attribute_code":attribute_code,"last_changed_at":changed,"max_age_seconds":max_age_seconds}),
            ))
        }
    }
}

/// The only rule schedule coordinator. It never claims rule runs; task-worker
/// instances are the sole execution claimers.
pub fn start_schedule_coordinator(
    repository: CatalogRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            for workspace in repository.active_workspace_ids().await.unwrap_or_default() {
                match repository.for_workspace(workspace).await {
                    Ok(scoped) => {
                        if let Err(error) = scoped.backfill_rule_tasks().await {
                            tracing::error!(%error, "rule task backfill failed");
                        }
                        if let Err(error) = scoped.schedule_rule_runs().await {
                            tracing::error!(%error, "rule schedule poll failed");
                        }
                    }
                    Err(error) => tracing::error!(%error, "rule scheduler workspace scope failed"),
                }
            }
            tokio::select! { _ = tokio::time::sleep(Duration::from_millis(250)) => {}, _ = shutdown.changed() => return }
        }
    })
}
