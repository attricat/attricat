use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use crate::{
    domain_events::DomainEvent,
    model::{CreateManualRuleRun, CreateRule, Rule, RuleFinding, RuleRun},
    task_queue::{TaskInsert, TaskKind},
};
use chrono::Utc;
use uuid::Uuid;

const RULE_FIELDS: &str = "r.id,r.blueprint_id,r.blueprint_version,r.context_id,r.code,r.name,r.version,r.status,r.definition,r.definition_hash,r.compiled_plan,r.published_at,r.created_at,l.enabled_version";

type RuleRunTaskRow = (
    String,
    Uuid,
    Uuid,
    i64,
    bool,
    Option<Uuid>,
    Option<Uuid>,
    i64,
    Uuid,
    i64,
    Option<Uuid>,
    serde_json::Value,
);

/// Immutable rule execution input loaded only for the task subject named by a
/// currently leased rule envelope.
pub struct ClaimedRuleRun {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub rule_version: i64,
    pub dry_run: bool,
    pub scope_entity_id: Option<Uuid>,
    pub candidate_cursor: Option<Uuid>,
    pub candidates_evaluated: i64,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
    pub compiled_plan: serde_json::Value,
}

/// Evaluation reads happen before the fenced checkpoint transaction. This is
/// deliberately data-only: all findings and run progress are persisted later
/// under the task token.
pub struct RuleCandidateResult {
    pub entity_id: Uuid,
    pub failed: bool,
    pub message: String,
    pub evidence: serde_json::Value,
    pub evaluation_key: String,
    pub severity: String,
}

impl CatalogRepository {
    async fn enqueue_rule_task(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        workspace_id: Uuid,
        run_id: Uuid,
        correlation_id: Option<Uuid>,
        causation_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        self.enqueue_task(
            tx,
            TaskInsert {
                workspace_id,
                kind: TaskKind::RuleRunV1,
                subject_id: run_id,
                generation: 0,
                payload: serde_json::json!({}),
                correlation_id,
                causation_id,
            },
        )
        .await?;
        Ok(())
    }

    pub async fn list_rules(
        &self,
        blueprint_id: Option<Uuid>,
    ) -> Result<Vec<Rule>, RepositoryError> {
        let ws = self.workspace_id.0;
        let query = format!(
            "SELECT {RULE_FIELDS} FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id WHERE r.workspace_id=$1 AND ($2::uuid IS NULL OR r.blueprint_id=$2) ORDER BY r.created_at DESC,r.version DESC"
        );
        Ok(sqlx::query_as::<_, Db<Rule>>(&query)
            .bind(ws)
            .bind(blueprint_id)
            .fetch_all(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn get_rule_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Option<Rule>, RepositoryError> {
        let ws = self.workspace_id.0;
        let q = format!(
            "SELECT {RULE_FIELDS} FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id WHERE r.workspace_id=$1 AND r.id=$2 AND r.version=$3"
        );
        Ok(sqlx::query_as::<_, Db<Rule>>(&q)
            .bind(ws)
            .bind(id)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?
            .into_domain())
    }

    pub async fn create_rule(&self, input: CreateRule) -> Result<Rule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let id = self
            .create_rule_in_transaction(&mut tx, Uuid::new_v4(), input)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }

    /// Shared mutation seam for callers that create a rule with a chosen id
    /// as part of a larger transaction, such as seed application.
    pub(super) async fn create_rule_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: CreateRule,
    ) -> Result<Uuid, RepositoryError> {
        let compiled = catalog_rules::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        let ws = self.workspace_id.0;
        let valid_blueprint:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM blueprints WHERE id=$1 AND version=$2 AND workspace_id=$3 AND status='published' AND deleted_at IS NULL)").bind(input.blueprint_id).bind(input.blueprint_version).bind(ws).fetch_one(&mut **tx).await?;
        if !valid_blueprint {
            return Err(RepositoryError::BlueprintNotPublished);
        }
        if let Some(context_id) = input.context_id {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM attribute_contexts WHERE id=$1 AND workspace_id=$2)",
            )
            .bind(context_id)
            .bind(ws)
            .fetch_one(&mut **tx)
            .await?;
            if !exists {
                return Err(RepositoryError::InvalidContext);
            }
        }
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("rule-code:{ws}:{}", compiled.code))
            .execute(&mut **tx)
            .await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM rules WHERE workspace_id=$1 AND code=$2)",
        )
        .bind(ws)
        .bind(&compiled.code)
        .fetch_one(&mut **tx)
        .await?;
        if exists {
            return Err(RepositoryError::RuleCodeTaken);
        }
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO rules(id,workspace_id,blueprint_id,blueprint_version,context_id,code,name,version,definition,definition_hash,compiled_plan) VALUES($1,$2,$3,$4,$5,$6,$7,1,$8,$9,$10)").bind(id).bind(ws).bind(input.blueprint_id).bind(input.blueprint_version).bind(input.context_id).bind(&compiled.code).bind(&compiled.name).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO rule_lifecycles(rule_id,workspace_id) VALUES($1,$2)")
            .bind(id)
            .bind(ws)
            .execute(&mut **tx)
            .await?;
        Ok(id)
    }
    pub async fn create_rule_revision(
        &self,
        id: Uuid,
        input: CreateRule,
    ) -> Result<Rule, RepositoryError> {
        let compiled = catalog_rules::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let previous: Option<(i64, String, Uuid, i64, Option<Uuid>)> = sqlx::query_as("SELECT version,code,blueprint_id,blueprint_version,context_id FROM rules WHERE workspace_id=$1 AND id=$2 ORDER BY version DESC LIMIT 1 FOR UPDATE").bind(ws).bind(id).fetch_optional(&mut *tx).await?;
        let Some((version, code, blueprint_id, blueprint_version, context_id)) = previous else {
            return Err(RepositoryError::NotFound("rule"));
        };
        if code != compiled.code
            || blueprint_id != input.blueprint_id
            || blueprint_version != input.blueprint_version
            || context_id != input.context_id
        {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule code, blueprint revision, and context cannot change across revisions".into(),
            ));
        }
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO rules(id,workspace_id,blueprint_id,blueprint_version,context_id,code,name,version,definition,definition_hash,compiled_plan) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(id).bind(ws).bind(blueprint_id).bind(blueprint_version).bind(context_id).bind(&compiled.code).bind(&compiled.name).bind(version+1).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }
    pub async fn get_rule(&self, id: Uuid) -> Result<Option<Rule>, RepositoryError> {
        let ws = self.workspace_id.0;
        let q = format!(
            "SELECT {RULE_FIELDS} FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id WHERE r.workspace_id=$1 AND r.id=$2 ORDER BY r.version DESC LIMIT 1"
        );
        Ok(sqlx::query_as::<_, Db<Rule>>(&q)
            .bind(ws)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn publish_rule(&self, id: Uuid, version: i64) -> Result<Rule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.publish_rule_in_transaction(&mut tx, id, version)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }
    pub(super) async fn publish_rule_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        if sqlx::query("UPDATE rules SET status='published',published_at=COALESCE(published_at,now()) WHERE workspace_id=$1 AND id=$2 AND version=$3").bind(ws).bind(id).bind(version).execute(&mut **tx).await?.rows_affected()==0{return Err(RepositoryError::NotFound("rule revision"));}
        Ok(())
    }
    pub async fn enable_rule(&self, id: Uuid, version: i64) -> Result<Rule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.enable_rule_in_transaction(&mut tx, id, version)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }
    pub(super) async fn enable_rule_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rules WHERE workspace_id=$1 AND id=$2 AND version=$3 AND status='published')").bind(ws).bind(id).bind(version).fetch_one(&mut **tx).await?;
        if !exists {
            return Err(RepositoryError::RuleNotPublished);
        }
        sqlx::query("UPDATE rule_lifecycles SET enabled_version=$3,activation_sequence=(SELECT COALESCE(max(sequence),0) FROM domain_events WHERE workspace_id=$1),enabled_at=now(),disabled_at=NULL,updated_at=now() WHERE workspace_id=$1 AND rule_id=$2").bind(ws).bind(id).bind(version).execute(&mut **tx).await?;
        let plan: serde_json::Value = sqlx::query_scalar(
            "SELECT compiled_plan FROM rules WHERE workspace_id=$1 AND id=$2 AND version=$3",
        )
        .bind(ws)
        .bind(id)
        .bind(version)
        .fetch_one(&mut **tx)
        .await?;
        let compiled: catalog_rules::CompiledRule = serde_json::from_value(plan)
            .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
        for (index, trigger) in compiled.triggers.iter().enumerate() {
            let catalog_rules::Trigger::Schedule { cron, .. } = trigger else {
                continue;
            };
            let schedule = catalog_rules::parse_six_field_cron(cron).map_err(|_| {
                RepositoryError::InvalidRuleDefinition("stored schedule cron is invalid".into())
            })?;
            let next = schedule.after(&Utc::now()).next().ok_or_else(|| {
                RepositoryError::InvalidRuleDefinition("schedule has no future occurrence".into())
            })?;
            sqlx::query("INSERT INTO rule_schedule_states(workspace_id,rule_id,rule_version,trigger_index,next_run_at) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING").bind(ws).bind(id).bind(version).bind(index as i32).bind(next).execute(&mut **tx).await?;
        }
        Ok(())
    }
    pub async fn disable_rule(&self, id: Uuid) -> Result<Rule, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        if sqlx::query("UPDATE rule_lifecycles SET enabled_version=NULL,disabled_at=now(),updated_at=now() WHERE workspace_id=$1 AND rule_id=$2").bind(ws).bind(id).execute(&mut *tx).await?.rows_affected()==0{return Err(RepositoryError::NotFound("rule"));}
        sqlx::query("UPDATE rule_runs SET status='cancelled',cancelled_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND status IN ('pending','leased')").bind(ws).bind(id).execute(&mut *tx).await?;
        let queued_tasks: Vec<Uuid> = sqlx::query_scalar("SELECT t.id FROM tasks t JOIN rule_runs rr ON rr.id=t.subject_id WHERE t.workspace_id=$1 AND t.kind='rule_run.v1' AND t.status='queued' AND rr.rule_id=$2 FOR UPDATE OF t")
            .bind(ws).bind(id).fetch_all(&mut *tx).await?;
        for task_id in queued_tasks {
            self.cancel_queued_task(&mut tx, task_id).await?;
        }
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }
    /// Creates a durable, idempotent manual run for the currently enabled revision.
    /// An optional entity scope is checked before enqueueing, so callers cannot turn a
    /// manual request into an arbitrary cross-workspace scan.
    pub async fn create_manual_rule_run(
        &self,
        rule_id: Uuid,
        input: CreateManualRuleRun,
    ) -> Result<Uuid, RepositoryError> {
        if input.idempotency_key.is_empty()
            || input.idempotency_key.len() > 128
            || !input.idempotency_key.is_ascii()
        {
            return Err(RepositoryError::InvalidRuleDefinition(
                "manual idempotency_key must be 1-128 ASCII bytes".into(),
            ));
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let version: Option<i64> = sqlx::query_scalar(
            "SELECT r.version FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id AND l.workspace_id=r.workspace_id WHERE r.workspace_id=$1 AND r.id=$2 AND r.status='published' AND l.enabled_version=r.version FOR SHARE OF l",
        ).bind(ws).bind(rule_id).fetch_optional(&mut *tx).await?;
        let Some(version) = version else {
            return Err(RepositoryError::RuleNotPublished);
        };
        if let Some(entity_id) = input.entity_id {
            let matches: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entities e JOIN rules r ON r.id=$3 AND r.workspace_id=$1 AND r.version=$4 WHERE e.id=$2 AND e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=r.blueprint_id AND e.blueprint_version=r.blueprint_version)")
                .bind(ws).bind(entity_id).bind(rule_id).bind(version).fetch_one(&mut *tx).await?;
            if !matches {
                return Err(RepositoryError::NotFound("entity in rule blueprint"));
            }
        }
        let id = Uuid::new_v4();
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,dry_run,scope_entity_id,idempotency_key) VALUES($1,$2,$3,$4,'manual',$5,$6,$7) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
            .bind(id).bind(ws).bind(rule_id).bind(version).bind(input.dry_run).bind(input.entity_id).bind(&input.idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some(id) = inserted {
            self.enqueue_rule_task(&mut tx, ws, id, None, None).await?;
            self.commit_mutation(tx).await?;
            Ok(id)
        } else {
            let existing: Uuid = sqlx::query_scalar("SELECT id FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND source='manual' AND idempotency_key=$4").bind(ws).bind(rule_id).bind(version).bind(&input.idempotency_key).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(existing)
        }
    }
    /// Event intake only creates bounded entity-scoped work. It never executes on the outbox lease.
    pub async fn fan_out_rule_runs(&self, event: &DomainEvent) -> Result<u64, RepositoryError> {
        if event.aggregate_kind != "entity" || event.source_name.starts_with("rule:") {
            return Ok(0);
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, i64, serde_json::Value)> = sqlx::query_as("SELECT r.id,r.version,r.compiled_plan FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id AND l.workspace_id=r.workspace_id JOIN entities e ON e.id=$3 AND e.workspace_id=r.workspace_id AND e.blueprint_id=r.blueprint_id AND e.blueprint_version=r.blueprint_version AND e.deleted_at IS NULL WHERE r.workspace_id=$1 AND r.status='published' AND l.enabled_version=r.version AND $2 > COALESCE(l.activation_sequence,0) FOR SHARE OF l").bind(ws).bind(event.sequence).bind(event.aggregate_id).fetch_all(&mut *tx).await?;
        let mut inserted = 0;
        for (rule_id, version, plan) in rows {
            let compiled: catalog_rules::CompiledRule = serde_json::from_value(plan)
                .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
            if !compiled.triggers.iter().any(|trigger| matches!(trigger, catalog_rules::Trigger::Event { event_type } if event_type == &event.event_type)) { continue; }
            let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,scope_entity_id,idempotency_key) VALUES($1,$2,$3,$4,'event',$5,$6) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
                .bind(Uuid::new_v4()).bind(ws).bind(rule_id).bind(version).bind(event.aggregate_id).bind(event.id.to_string()).fetch_optional(&mut *tx).await?;
            if let Some(run_id) = run_id {
                self.enqueue_rule_task(
                    &mut tx,
                    ws,
                    run_id,
                    Some(event.correlation_id),
                    Some(event.id),
                )
                .await?;
                inserted += 1;
            }
        }
        tx.commit().await?;
        Ok(inserted)
    }
    /// Materialize due cron occurrences as durable runs. The occurrence timestamp is the
    /// idempotency key, while schedule state is the only timer/cursor held by the system.
    pub async fn schedule_rule_runs(&self) -> Result<u64, RepositoryError> {
        let ws = self.workspace_id.0;
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, i64, serde_json::Value)> = sqlx::query_as("SELECT r.id,r.version,r.compiled_plan FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id AND l.workspace_id=r.workspace_id WHERE r.workspace_id=$1 AND r.status='published' AND l.enabled_version=r.version FOR SHARE OF l").bind(ws).fetch_all(&mut *tx).await?;
        let mut created = 0;
        for (rule_id, version, plan) in rows {
            let compiled: catalog_rules::CompiledRule = serde_json::from_value(plan)
                .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
            for (index, trigger) in compiled.triggers.iter().enumerate() {
                let catalog_rules::Trigger::Schedule { cron, .. } = trigger else {
                    continue;
                };
                let schedule = catalog_rules::parse_six_field_cron(cron).map_err(|_| {
                    RepositoryError::InvalidRuleDefinition("stored schedule cron is invalid".into())
                })?;
                let state: Option<(chrono::DateTime<Utc>,)> = sqlx::query_as("SELECT next_run_at FROM rule_schedule_states WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND trigger_index=$4 FOR UPDATE").bind(ws).bind(rule_id).bind(version).bind(index as i32).fetch_optional(&mut *tx).await?;
                let Some((due,)) = state else {
                    continue;
                };
                if due > now {
                    continue;
                }
                let next = schedule.after(&due).next().ok_or_else(|| {
                    RepositoryError::InvalidRuleDefinition(
                        "schedule has no future occurrence".into(),
                    )
                })?;
                let missed = due < now - chrono::Duration::minutes(5);
                let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND status IN ('pending','leased'))").bind(ws).bind(rule_id).fetch_one(&mut *tx).await?;
                if !missed && !active {
                    let key = format!("{index}:{}", due.to_rfc3339());
                    let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,idempotency_key) VALUES($1,$2,$3,$4,'schedule',$5) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
                        .bind(Uuid::new_v4()).bind(ws).bind(rule_id).bind(version).bind(key).fetch_optional(&mut *tx).await?;
                    if let Some(run_id) = run_id {
                        self.enqueue_rule_task(&mut tx, ws, run_id, None, None)
                            .await?;
                        created += 1;
                    }
                }
                sqlx::query("UPDATE rule_schedule_states SET next_run_at=$5,misfires=misfires+$6,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND trigger_index=$4").bind(ws).bind(rule_id).bind(version).bind(index as i32).bind(next).bind(if missed || active { 1_i64 } else { 0 }).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(created)
    }
    pub async fn list_rule_runs(&self) -> Result<Vec<RuleRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,source,dry_run,scope_entity_id,status,candidate_cursor,candidates_evaluated,findings_created,findings_resolved,attempts,last_error,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?.into_domain())
    }
    pub async fn list_rule_findings(
        &self,
        entity_id: Option<Uuid>,
    ) -> Result<Vec<RuleFinding>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,entity_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND ($2::uuid IS NULL OR entity_id=$2) ORDER BY updated_at DESC").bind(ws).bind(entity_id).fetch_all(&self.pool).await?.into_domain())
    }
    pub async fn rule_runs_page(
        &self,
        rule_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleRun>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,source,dry_run,scope_entity_id,status,candidate_cursor,candidates_evaluated,findings_created,findings_resolved,attempts,last_error,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 AND ($2::uuid IS NULL OR rule_id=$2) ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(ws).bind(rule_id).bind(limit + 1).bind(offset).fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    pub async fn rule_findings_page(
        &self,
        entity_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleFinding>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,entity_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND ($2::uuid IS NULL OR entity_id=$2) ORDER BY updated_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(ws).bind(entity_id).bind(limit + 1).bind(offset).fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    pub async fn acknowledge_rule_finding(&self, id: Uuid) -> Result<RuleFinding, RepositoryError> {
        let ws = self.workspace_id.0;
        let actor = self
            .audit_context
            .as_ref()
            .and_then(|context| context.actor_user_id);
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query("UPDATE rule_findings SET state='acknowledged',acknowledged_at=COALESCE(acknowledged_at,clock_timestamp()),acknowledged_by_user_id=COALESCE(acknowledged_by_user_id,$3),updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND state='open'").bind(ws).bind(id).bind(actor).execute(&mut *tx).await?;
        if updated.rows_affected() == 0 {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM rule_findings WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(ws)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                return Err(RepositoryError::NotFound("rule finding"));
            }
        }
        let finding = sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,entity_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND id=$2").bind(ws).bind(id).fetch_one(&mut *tx).await?.into_domain();
        self.commit_mutation(tx).await?;
        Ok(finding)
    }

    /// Claims the domain row only while the shared task token remains current.
    /// `rule_runs.lease_*` is retained for API compatibility; it is no longer a
    /// concurrency authority after the cutover.
    pub async fn begin_rule_run_task(
        &self,
        task: &super::ClaimedTask,
    ) -> Result<Option<ClaimedRuleRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        if task.kind != TaskKind::RuleRunV1 || task.workspace_id != ws {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule task workspace or kind mismatch".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let fenced = self.for_rule_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let row: Option<RuleRunTaskRow> = sqlx::query_as(
            "SELECT rr.status,rr.id,rr.rule_id,rr.rule_version,rr.dry_run,rr.scope_entity_id,rr.candidate_cursor,rr.candidates_evaluated,r.blueprint_id,r.blueprint_version,r.context_id,r.compiled_plan FROM rule_runs rr JOIN rules r ON r.id=rr.rule_id AND r.version=rr.rule_version AND r.workspace_id=rr.workspace_id WHERE rr.id=$1 AND rr.workspace_id=$2 FOR UPDATE OF rr",
        ).bind(task.subject_id).bind(ws).fetch_optional(&mut *tx).await?;
        let Some((
            status,
            id,
            rule_id,
            rule_version,
            dry_run,
            scope_entity_id,
            candidate_cursor,
            candidates_evaluated,
            blueprint_id,
            blueprint_version,
            context_id,
            compiled_plan,
        )) = row
        else {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule task subject is missing".into(),
            ));
        };
        if matches!(status.as_str(), "cancelled" | "completed" | "dead_letter") {
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query("UPDATE rule_runs SET status='leased',lease_owner=$3,lease_until=$4,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2")
            .bind(id).bind(ws).bind(&task.lease_owner).bind(task.lease_until).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(ClaimedRuleRun {
            id,
            rule_id,
            rule_version,
            dry_run,
            scope_entity_id,
            candidate_cursor,
            candidates_evaluated,
            blueprint_id,
            blueprint_version,
            context_id,
            compiled_plan,
        }))
    }

    pub async fn rule_run_is_dead_letter_for_task(
        &self,
        task: &super::ClaimedTask,
    ) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        if task.kind != TaskKind::RuleRunV1 || task.workspace_id != ws {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule task workspace or kind mismatch".into(),
            ));
        }
        Ok(sqlx::query_scalar(
            "SELECT status='dead_letter' FROM rule_runs WHERE id=$1 AND workspace_id=$2",
        )
        .bind(task.subject_id)
        .bind(ws)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(false))
    }

    /// Inserts or resolves the whole candidate page and advances its cursor in
    /// the same token-fenced transaction. A lost task lease therefore rolls
    /// back both findings and the page checkpoint.
    pub async fn checkpoint_rule_page_task(
        &self,
        task: &super::ClaimedTask,
        run: &ClaimedRuleRun,
        results: Vec<RuleCandidateResult>,
        next_cursor: Option<Uuid>,
        done: bool,
    ) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let fenced = self.for_rule_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM rule_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.id)
        .bind(ws)
        .fetch_optional(&mut *tx)
        .await?;
        match status.as_deref() {
            Some("cancelled") | Some("completed") | Some("dead_letter") => {
                tx.commit().await?;
                return Ok(true);
            }
            Some("leased") => {}
            _ => {
                return Err(RepositoryError::InvalidRuleDefinition(
                    "rule run is not available for its task".into(),
                ));
            }
        }
        let mut created = 0_i64;
        let mut resolved = 0_i64;
        for result in &results {
            if run.dry_run {
                created += i64::from(result.failed);
            } else if result.failed {
                let inserted: bool = sqlx::query_scalar("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,entity_id,context_id,evaluation_key,severity,message,evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(workspace_id,rule_id,entity_id,context_id,evaluation_key) DO UPDATE SET rule_version=EXCLUDED.rule_version,severity=EXCLUDED.severity,message=EXCLUDED.message,evidence=EXCLUDED.evidence,state='open',resolved_at=NULL,resolved_by_run_id=NULL,updated_at=clock_timestamp() RETURNING (xmax=0)")
                    .bind(Uuid::new_v4()).bind(ws).bind(run.rule_id).bind(run.rule_version).bind(result.entity_id).bind(run.context_id).bind(&result.evaluation_key).bind(&result.severity).bind(&result.message).bind(&result.evidence).fetch_one(&mut *tx).await?;
                created += i64::from(inserted);
            } else {
                resolved += sqlx::query("UPDATE rule_findings SET state='resolved',resolved_at=clock_timestamp(),resolved_by_run_id=$4,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND entity_id=$3 AND context_id IS NOT DISTINCT FROM $5 AND evaluation_key=$6 AND state <> 'resolved'")
                    .bind(ws).bind(run.rule_id).bind(result.entity_id).bind(run.id).bind(run.context_id).bind(&result.evaluation_key).execute(&mut *tx).await?.rows_affected() as i64;
            }
        }
        sqlx::query("UPDATE rule_runs SET candidate_cursor=$2,candidates_evaluated=candidates_evaluated+$3,findings_created=findings_created+$4,findings_resolved=findings_resolved+$5,status=CASE WHEN $6 THEN 'completed' ELSE 'pending' END,completed_at=CASE WHEN $6 THEN clock_timestamp() ELSE completed_at END,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$7")
            .bind(run.id).bind(next_cursor).bind(results.len() as i64).bind(created).bind(resolved).bind(done).bind(ws).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Records retry-visible domain state under the same token as the generic
    /// task retry. The generic task worker still owns the failure counter.
    pub async fn fail_rule_run_task(
        &self,
        task: &super::ClaimedTask,
        error: &str,
    ) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        let terminal = task.failures + 1 >= task.max_failures;
        let mut tx = self.pool.begin().await?;
        let fenced = self.for_rule_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let changed = sqlx::query("UPDATE rule_runs SET status=CASE WHEN $3 THEN 'dead_letter' ELSE 'pending' END,last_error=$4,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status IN ('pending','leased')")
            .bind(task.subject_id).bind(ws).bind(terminal).bind(error).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule run is not available for its task".into(),
            ));
        }
        if terminal {
            let changed = sqlx::query("UPDATE tasks SET status='dead_letter',failures=max_failures,lease_owner=NULL,lease_token=NULL,lease_until=NULL,last_error_code='rule_run',last_error_message=$4,failed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>clock_timestamp()")
                .bind(task.id).bind(&task.lease_owner).bind(task.lease_token).bind(error).execute(&mut *tx).await?.rows_affected();
            if changed != 1 {
                return Err(RepositoryError::Task(super::TaskError::LeaseLost));
            }
        }
        tx.commit().await?;
        Ok(terminal)
    }

    /// Backfills envelopes for rows created before the task cutover. No legacy
    /// claimer is started with this binary, so normalising its stale leases is
    /// safe and avoids stranding work after deployment or restart.
    pub async fn backfill_rule_tasks(&self) -> Result<u64, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE rule_runs rr SET status='pending',lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE rr.workspace_id=$1 AND rr.status='leased' AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.workspace_id=rr.workspace_id AND t.kind='rule_run.v1' AND t.subject_id=rr.id AND t.status='leased' AND t.lease_until>clock_timestamp())")
            .bind(ws).execute(&mut *tx).await?;
        let rows: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM rule_runs WHERE workspace_id=$1 AND status='pending' FOR UPDATE",
        )
        .bind(ws)
        .fetch_all(&mut *tx)
        .await?;
        let count = rows.len() as u64;
        for run_id in rows {
            self.enqueue_rule_task(&mut tx, ws, run_id, None, None)
                .await?;
        }
        tx.commit().await?;
        Ok(count)
    }

    /// Replays a dead-lettered rule run through a fresh task generation while
    /// keeping finding idempotency and the candidate cursor durable.
    pub async fn replay_rule_run(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        if sqlx::query("UPDATE rule_runs SET status='pending',last_error=NULL,completed_at=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='dead_letter'")
            .bind(id).bind(ws).execute(&mut *tx).await?.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(false);
        }
        let task_id: Uuid = sqlx::query_scalar("SELECT id FROM tasks WHERE workspace_id=$1 AND kind='rule_run.v1' AND subject_id=$2 AND status='dead_letter' ORDER BY generation DESC LIMIT 1 FOR UPDATE")
            .bind(ws).bind(id).fetch_one(&mut *tx).await?;
        self.replay_task(&mut tx, task_id).await?;
        tx.commit().await?;
        Ok(true)
    }
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    pub async fn ensure_rule_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions(code,description) VALUES ('rules.read','Read workspace data quality rules and findings'),('rules.manage','Create and operate workspace data quality rules') ON CONFLICT(code) DO NOTHING").execute(&mut *tx).await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions(role_id,permission_code) SELECT $1,code FROM permissions WHERE code IN ('rules.read','rules.manage') ON CONFLICT DO NOTHING").bind(role_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
