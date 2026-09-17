//! Durable, bounded evaluation for catalog-owned declarative rules.
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    repository::{CatalogRepository, RepositoryError},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

const PAGE_SIZE: i64 = 500;
const MAX_ATTEMPTS: i32 = 5;

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

#[derive(sqlx::FromRow)]
struct ClaimedRun {
    id: Uuid,
    rule_id: Uuid,
    rule_version: i64,
    dry_run: bool,
    scope_entity_id: Option<Uuid>,
    candidate_cursor: Option<Uuid>,
    attempts: i32,
    candidates_evaluated: i64,
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    compiled_plan: Value,
}
#[derive(sqlx::FromRow)]
struct Candidate {
    id: Uuid,
    system_tags: Vec<String>,
}

pub fn start(
    repository: CatalogRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            for workspace in repository.active_workspace_ids().await.unwrap_or_default() {
                match repository.for_workspace(workspace).await {
                    Ok(scoped) => {
                        if let Err(error) = scoped.schedule_rule_runs().await {
                            tracing::error!(%error, "rule schedule poll failed");
                        }
                        if let Err(error) = process_one(&scoped).await {
                            tracing::error!(%error, "rule run poll failed");
                        }
                    }
                    Err(error) => tracing::error!(%error, "rule workspace initialization failed"),
                }
            }
            tokio::select! { _ = tokio::time::sleep(Duration::from_millis(250)) => {}, _ = shutdown.changed() => return }
        }
    })
}

async fn process_one(repo: &CatalogRepository) -> Result<bool, RepositoryError> {
    let ws = repo.workspace_id_for_runtime();
    let pool = repo.pool_for_runtime();
    let owner = Uuid::new_v4().to_string();
    let run: Option<ClaimedRun> = sqlx::query_as(
        "WITH candidate AS (SELECT rr.id FROM rule_runs rr JOIN rule_lifecycles l ON l.rule_id=rr.rule_id AND l.workspace_id=rr.workspace_id AND l.enabled_version=rr.rule_version WHERE rr.workspace_id=$1 AND ((rr.status='pending' AND rr.next_attempt_at<=clock_timestamp()) OR (rr.status='leased' AND rr.lease_until<=clock_timestamp())) ORDER BY rr.next_attempt_at,rr.id FOR UPDATE SKIP LOCKED LIMIT 1), leased AS (UPDATE rule_runs rr SET status='leased',attempts=attempts+1,lease_owner=$2,lease_until=clock_timestamp()+interval '30 seconds',last_error=NULL,updated_at=clock_timestamp() FROM candidate c WHERE rr.id=c.id RETURNING rr.*) SELECT rr.id,rr.rule_id,rr.rule_version,rr.dry_run,rr.scope_entity_id,rr.candidate_cursor,rr.attempts,rr.candidates_evaluated,r.blueprint_id,r.blueprint_version,r.context_id,r.compiled_plan FROM leased rr JOIN rules r ON r.id=rr.rule_id AND r.version=rr.rule_version AND r.workspace_id=rr.workspace_id"
    ).bind(ws).bind(&owner).fetch_optional(&pool).await?;
    let Some(run) = run else {
        return Ok(false);
    };
    let result = evaluate_page(repo, &run).await;
    match result {
        Ok(done) => {
            if done {
                sqlx::query("UPDATE rule_runs SET status='completed',completed_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2").bind(run.id).bind(&owner).execute(&pool).await?;
            } else {
                sqlx::query("UPDATE rule_runs SET status='pending',lease_owner=NULL,lease_until=NULL,next_attempt_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2").bind(run.id).bind(&owner).execute(&pool).await?;
            }
        }
        Err(error) => {
            sqlx::query("UPDATE rule_runs SET status=CASE WHEN attempts >= $3 THEN 'dead_letter' ELSE 'pending' END,next_attempt_at=CASE WHEN attempts >= $3 THEN next_attempt_at ELSE clock_timestamp() + (($4::bigint) * interval '1 second') END,last_error=$5,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2")
                .bind(run.id).bind(&owner).bind(MAX_ATTEMPTS).bind((1_i64 << (run.attempts.saturating_sub(1) as u32).min(6)).min(60)).bind(error.to_string()).execute(&pool).await?;
        }
    }
    Ok(true)
}

async fn evaluate_page(
    repo: &CatalogRepository,
    run: &ClaimedRun,
) -> Result<bool, RepositoryError> {
    let compiled: catalog_rules::CompiledRule =
        serde_json::from_value(run.compiled_plan.clone())
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
    let pool = repo.pool_for_runtime();
    let ws = repo.workspace_id_for_runtime();
    let remaining =
        (catalog_rules::MAX_CANDIDATES_PER_RUN as i64 - run.candidates_evaluated).max(0);
    if remaining == 0 {
        return Ok(true);
    }
    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT id,system_tags FROM entities WHERE workspace_id=$1 AND blueprint_id=$2 AND blueprint_version=$3 AND deleted_at IS NULL AND ($4::uuid IS NULL OR id>$4) AND ($5::uuid IS NULL OR id=$5) ORDER BY id LIMIT $6"
    ).bind(ws).bind(run.blueprint_id).bind(run.blueprint_version).bind(run.candidate_cursor).bind(run.scope_entity_id).bind(PAGE_SIZE.min(remaining)).fetch_all(&pool).await?;
    let mut created = 0_i64;
    let mut resolved = 0_i64;
    for candidate in &candidates {
        let (failed, message, evidence) =
            predicate_failure(&pool, ws, run, candidate, &compiled.predicate).await?;
        if run.dry_run {
            if failed {
                created += 1;
            }
            continue;
        }
        if failed {
            let outcome: bool = sqlx::query_scalar("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,entity_id,context_id,evaluation_key,severity,message,evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(workspace_id,rule_id,entity_id,context_id,evaluation_key) DO UPDATE SET rule_version=EXCLUDED.rule_version,severity=EXCLUDED.severity,message=EXCLUDED.message,evidence=EXCLUDED.evidence,state='open',resolved_at=NULL,resolved_by_run_id=NULL,updated_at=clock_timestamp() RETURNING (xmax=0)").bind(Uuid::new_v4()).bind(ws).bind(run.rule_id).bind(run.rule_version).bind(candidate.id).bind(run.context_id).bind(&compiled.raw_definition_hash).bind(format!("{:?}",compiled.severity).to_lowercase()).bind(message).bind(evidence).fetch_one(&pool).await?;
            if outcome {
                created += 1;
            }
        } else {
            resolved += sqlx::query("UPDATE rule_findings SET state='resolved',resolved_at=clock_timestamp(),resolved_by_run_id=$4,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND entity_id=$3 AND context_id IS NOT DISTINCT FROM $5 AND evaluation_key=$6 AND state <> 'resolved'").bind(ws).bind(run.rule_id).bind(candidate.id).bind(run.id).bind(run.context_id).bind(&compiled.raw_definition_hash).execute(&pool).await?.rows_affected() as i64;
        }
    }
    let next = candidates.last().map(|candidate| candidate.id);
    let done = candidates.len() < PAGE_SIZE.min(remaining) as usize
        || run.scope_entity_id.is_some()
        || run.candidates_evaluated + candidates.len() as i64
            >= catalog_rules::MAX_CANDIDATES_PER_RUN as i64;
    sqlx::query("UPDATE rule_runs SET candidate_cursor=$2,candidates_evaluated=candidates_evaluated+$3,findings_created=findings_created+$4,findings_resolved=findings_resolved+$5,updated_at=clock_timestamp() WHERE id=$1").bind(run.id).bind(next).bind(candidates.len() as i64).bind(created).bind(resolved).execute(&pool).await?;
    Ok(done)
}
async fn predicate_failure(
    pool: &sqlx::PgPool,
    ws: Uuid,
    run: &ClaimedRun,
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
            let present:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id WHERE av.workspace_id=$1 AND av.entity_id=$2 AND a.code=$3 AND a.blueprint_id=$4 AND a.blueprint_version=$5 AND av.latest AND av.active AND av.context_id IS NOT DISTINCT FROM $6)").bind(ws).bind(candidate.id).bind(attribute_code).bind(run.blueprint_id).bind(run.blueprint_version).bind(run.context_id).fetch_one(pool).await?;
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
            let changed:Option<DateTime<Utc>>=sqlx::query_scalar("SELECT max(av.created_at) FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id WHERE av.workspace_id=$1 AND av.entity_id=$2 AND a.code=$3 AND a.blueprint_id=$4 AND a.blueprint_version=$5 AND av.latest AND av.active AND av.context_id IS NOT DISTINCT FROM $6").bind(ws).bind(candidate.id).bind(attribute_code).bind(run.blueprint_id).bind(run.blueprint_version).bind(run.context_id).fetch_one(pool).await?;
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
