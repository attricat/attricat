use super::generations::{Generation, advance_generation};
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
    pub scope_record_id: Option<Uuid>,
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
    pub record_id: Uuid,
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

    /// Serializes every writer of a rule code in this workspace until the
    /// transaction ends and returns the families that already use it, as
    /// `(rule_id, blueprint_id, latest_version)`. A rule code names one family
    /// per workspace; callers must reject a code owned by another family.
    pub(super) async fn lock_rule_code(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        code: &str,
    ) -> Result<Vec<(Uuid, Uuid, i64)>, RepositoryError> {
        let ws = self.workspace_id.0;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("rule-code:{ws}:{code}"))
            .execute(&mut **tx)
            .await?;
        Ok(sqlx::query_as(
            "SELECT id, blueprint_id, max(version) FROM rules WHERE workspace_id=$1 AND code=$2 GROUP BY id, blueprint_id",
        )
        .bind(ws)
        .bind(code)
        .fetch_all(&mut **tx)
        .await?)
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
        Self::validate_rule_attributes(tx, &compiled, input.blueprint_id, input.blueprint_version)
            .await?;
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
        if !self.lock_rule_code(tx, &compiled.code).await?.is_empty() {
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
        Self::validate_rule_attributes(&mut tx, &compiled, blueprint_id, blueprint_version).await?;
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
        advance_generation(tx, ws, Generation::Catalog).await?;
        Ok(())
    }
    /// Type-checks a standalone rule against its blueprint revision's attributes.
    async fn validate_rule_attributes(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        compiled: &catalog_rules::CompiledRule,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<(), RepositoryError> {
        let types: std::collections::HashMap<String, String> = sqlx::query_as(
            "SELECT code, value_type FROM attributes WHERE blueprint_id=$1 AND blueprint_version=$2 AND deleted_at IS NULL",
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect();
        catalog_rules::validate_against_attributes(compiled, &types)
            .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))
    }

    pub async fn enable_rule(&self, id: Uuid, version: i64) -> Result<Rule, RepositoryError> {
        self.enable_rule_with(id, version, crate::model::EnableRule::default())
            .await
    }

    /// Enables a published revision. An enforcing rule first needs a
    /// completed full dry run of that revision, unless its blueprint revision
    /// has no live records; existing violations must be accepted explicitly.
    pub async fn enable_rule_with(
        &self,
        id: Uuid,
        version: i64,
        options: crate::model::EnableRule,
    ) -> Result<Rule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_rule_enable_allowed(&mut tx, id, version, options.accept_existing_violations)
            .await?;
        self.enable_rule_in_transaction(&mut tx, id, version)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }
    /// The enable gate shared by every path that enables a rule revision: it
    /// must be published, and an enforcing rule whose blueprint revision has
    /// live records needs a completed full dry run of that revision; existing
    /// violations, and a dry run that stopped at the candidate cap, must be
    /// accepted explicitly.
    pub(super) async fn ensure_rule_enable_allowed(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
        accept_existing_violations: bool,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let target: Option<(serde_json::Value, Uuid, i64)> = sqlx::query_as("SELECT compiled_plan,blueprint_id,blueprint_version FROM rules WHERE workspace_id=$1 AND id=$2 AND version=$3 AND status='published'").bind(ws).bind(id).bind(version).fetch_optional(&mut **tx).await?;
        let Some((target_plan, blueprint_id, blueprint_version)) = target else {
            return Err(RepositoryError::RuleNotPublished);
        };
        let target_rule: catalog_rules::CompiledRule = serde_json::from_value(target_plan)
            .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
        if target_rule.enforcement.is_some() {
            let has_records: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND blueprint_id=$2 AND blueprint_version=$3 AND deleted_at IS NULL)")
                .bind(ws).bind(blueprint_id).bind(blueprint_version).fetch_one(&mut **tx).await?;
            if has_records {
                let latest: Option<(i64, bool)> = sqlx::query_as("SELECT findings_created,truncated FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND dry_run AND scope_record_id IS NULL AND status='completed' ORDER BY completed_at DESC LIMIT 1")
                    .bind(ws).bind(id).bind(version).fetch_optional(&mut **tx).await?;
                dry_run_enable_gate(latest, accept_existing_violations)?;
            }
        }
        Ok(())
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
        advance_generation(tx, ws, Generation::Catalog).await?;
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
        advance_generation(&mut tx, ws, Generation::Catalog).await?;
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
    /// An optional record scope is checked before enqueueing, so callers cannot turn a
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
        // Only dry runs may target a published revision that is not enabled,
        // so an enforcing rule can report existing violations before enabling.
        let version: Option<i64> = sqlx::query_scalar(
            "SELECT r.version FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id AND l.workspace_id=r.workspace_id WHERE r.workspace_id=$1 AND r.id=$2 AND r.status='published' AND (CASE WHEN $3 THEN ($4::bigint IS NULL AND l.enabled_version IS NOT DISTINCT FROM r.version) OR r.version=$4 ELSE l.enabled_version=r.version AND ($4::bigint IS NULL OR r.version=$4) END) FOR SHARE OF l",
        ).bind(ws).bind(rule_id).bind(input.dry_run).bind(input.version).fetch_optional(&mut *tx).await?;
        let version = match version {
            Some(version) => Some(version),
            None if input.dry_run && input.version.is_none() => sqlx::query_scalar(
                "SELECT max(version) FROM rules WHERE workspace_id=$1 AND id=$2 AND status='published'",
            )
            .bind(ws)
            .bind(rule_id)
            .fetch_one(&mut *tx)
            .await?,
            None => None,
        };
        let Some(version) = version else {
            // A normal run needs the enabled revision; only report a missing
            // publication when there is no published revision to enable.
            let published: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM rules WHERE workspace_id=$1 AND id=$2 AND status='published')",
            )
            .bind(ws)
            .bind(rule_id)
            .fetch_one(&mut *tx)
            .await?;
            return Err(if published && !input.dry_run {
                RepositoryError::RuleNotEnabled
            } else {
                RepositoryError::RuleNotPublished
            });
        };
        if let Some(record_id) = input.record_id {
            let matches: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM records e JOIN rules r ON r.id=$3 AND r.workspace_id=$1 AND r.version=$4 WHERE e.id=$2 AND e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=r.blueprint_id AND e.blueprint_version=r.blueprint_version)")
                .bind(ws).bind(record_id).bind(rule_id).bind(version).fetch_one(&mut *tx).await?;
            if !matches {
                return Err(RepositoryError::NotFound("record in rule blueprint"));
            }
        }
        let id = Uuid::new_v4();
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,dry_run,scope_record_id,idempotency_key) VALUES($1,$2,$3,$4,'manual',$5,$6,$7) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
            .bind(id).bind(ws).bind(rule_id).bind(version).bind(input.dry_run).bind(input.record_id).bind(&input.idempotency_key).fetch_optional(&mut *tx).await?;
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
    /// Event intake only creates bounded record-scoped work. It never executes on the outbox lease.
    pub async fn fan_out_rule_runs(&self, event: &DomainEvent) -> Result<u64, RepositoryError> {
        if event.aggregate_kind != "record" || event.source_name.starts_with("rule:") {
            return Ok(0);
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        // Every enabled rule in one read: whether it targets the event
        // record's own revision, and that record's blueprint code.
        let rows: Vec<EventRuleRow> = sqlx::query_as("SELECT r.id, r.version, r.compiled_plan, r.blueprint_id, r.blueprint_version, (e.id IS NOT NULL) AS direct, (SELECT b.code FROM records x JOIN blueprints b ON b.id=x.blueprint_id AND b.version=x.blueprint_version WHERE x.workspace_id=$1 AND x.id=$3) AS event_blueprint FROM rules r JOIN rule_lifecycles l ON l.rule_id=r.id AND l.workspace_id=r.workspace_id LEFT JOIN records e ON e.id=$3 AND e.workspace_id=r.workspace_id AND e.blueprint_id=r.blueprint_id AND e.blueprint_version=r.blueprint_version AND e.deleted_at IS NULL WHERE r.workspace_id=$1 AND r.status='published' AND l.enabled_version=r.version AND $2 > COALESCE(l.activation_sequence,0) FOR SHARE OF l").bind(ws).bind(event.sequence).bind(event.aggregate_id).fetch_all(&mut *tx).await?;
        let mut rules = Vec::with_capacity(rows.len());
        for row in rows {
            let compiled: catalog_rules::CompiledRule =
                serde_json::from_value(row.compiled_plan.clone())
                    .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?;
            if compiled.triggers.iter().any(|trigger| matches!(trigger, catalog_rules::Trigger::Event { event_type } if event_type == &event.event_type)) {
                rules.push((row, compiled));
            }
        }
        let mut runs: Vec<(Uuid, i64, Uuid, String)> = rules
            .iter()
            .filter(|(row, _)| row.direct)
            .map(|(row, _)| {
                (
                    row.id,
                    row.version,
                    event.aggregate_id,
                    event.id.to_string(),
                )
            })
            .collect();
        let mut inserted = self.insert_event_rule_runs(&mut tx, event, &runs).await?;
        runs.clear();
        for (row, compiled) in &rules {
            for dependent in self.rule_dependents(&mut tx, event, row, compiled).await? {
                runs.push((
                    row.id,
                    row.version,
                    dependent,
                    format!("{}:{dependent}", event.id),
                ));
            }
        }
        inserted += self.insert_event_rule_runs(&mut tx, event, &runs).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// Creates event-sourced runs `(rule, version, scope record, key)` and
    /// their task envelopes with one statement; existing runs are skipped.
    async fn insert_event_rule_runs(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &DomainEvent,
        runs: &[(Uuid, i64, Uuid, String)],
    ) -> Result<u64, RepositoryError> {
        if runs.is_empty() {
            return Ok(0);
        }
        let ids: Vec<Uuid> = runs.iter().map(|_| Uuid::new_v4()).collect();
        let task_ids: Vec<Uuid> = runs.iter().map(|_| Uuid::new_v4()).collect();
        let rule_ids: Vec<Uuid> = runs.iter().map(|run| run.0).collect();
        let versions: Vec<i64> = runs.iter().map(|run| run.1).collect();
        let scopes: Vec<Uuid> = runs.iter().map(|run| run.2).collect();
        let keys: Vec<String> = runs.iter().map(|run| run.3.clone()).collect();
        let kind = TaskKind::RuleRunV1;
        let created: i64 = sqlx::query_scalar("WITH candidates AS (SELECT * FROM UNNEST($2::uuid[], $3::uuid[], $4::uuid[], $5::bigint[], $6::uuid[], $7::text[]) AS c(id, task_id, rule_id, rule_version, scope_record_id, idempotency_key)), created AS (INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,scope_record_id,idempotency_key) SELECT id, $1, rule_id, rule_version, 'event', scope_record_id, idempotency_key FROM candidates ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id), service AS (INSERT INTO task_workspace_service (workspace_id) VALUES ($1) ON CONFLICT (workspace_id) DO NOTHING), queued AS (INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, payload, max_failures, correlation_id, causation_id) SELECT c.task_id, $1, $8, 1, c.id, 0, '{}'::jsonb, $9, $10, $11 FROM created JOIN candidates c ON c.id = created.id RETURNING id) SELECT count(*) FROM queued")
            .bind(self.workspace_id.0)
            .bind(&ids)
            .bind(&task_ids)
            .bind(&rule_ids)
            .bind(&versions)
            .bind(&scopes)
            .bind(&keys)
            .bind(kind.as_str())
            .bind(kind.policy().max_failures)
            .bind(event.correlation_id)
            .bind(event.id)
            .fetch_one(&mut **tx)
            .await?;
        Ok(created as u64)
    }

    /// A change to a linked or referencing record can invalidate another
    /// record's `linked` or `referenced_by` predicate. Event-triggered rules
    /// re-evaluate those dependents (at most 100 per rule and event), so the
    /// change is reported as a finding rather than rejected.
    async fn rule_dependents(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &DomainEvent,
        rule: &EventRuleRow,
        compiled: &catalog_rules::CompiledRule,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        const MAX_DEPENDENTS: i64 = 100;
        let ws = self.workspace_id.0;
        let requirements =
            catalog_validation::predicate::Requirements::for_predicates([&compiled.predicate]);
        let mut dependents: Vec<Uuid> = Vec::new();
        // The rule evaluates only records pinned to its revision; their
        // relationship may be a revision field or an additional attribute.
        for relationship in &requirements.linked {
            dependents.extend(
                super::references::referencing_record_ids(
                    tx,
                    ws,
                    super::references::ReferenceQuery {
                        target: event.aggregate_id,
                        attribute_code: relationship,
                        referrers: super::references::Referrers::Revision {
                            blueprint_id: rule.blueprint_id,
                            version: rule.blueprint_version,
                        },
                        only: None,
                        exclude_target: true,
                        limit: MAX_DEPENDENTS,
                    },
                )
                .await?,
            );
        }
        for (blueprint_code, relationship) in &requirements.referenced_by {
            if rule.event_blueprint.as_deref() != Some(blueprint_code.as_str()) {
                continue;
            }
            dependents.extend(sqlx::query_scalar::<_, Uuid>("SELECT DISTINCT av.relationship_target_record_id FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id AND a.code=$3 JOIN records t ON t.id=av.relationship_target_record_id AND t.workspace_id=$1 AND t.deleted_at IS NULL AND t.blueprint_id=$4 AND t.blueprint_version=$5 WHERE av.record_id=$2 AND av.active LIMIT $6")
                .bind(ws).bind(event.aggregate_id).bind(relationship).bind(rule.blueprint_id).bind(rule.blueprint_version).bind(MAX_DEPENDENTS).fetch_all(&mut **tx).await?);
            // A record the write stopped referencing (a removed or re-pointed
            // relationship) no longer appears above, yet its count changed.
            let released = changed_relationship_targets(event, relationship);
            if !released.is_empty() {
                dependents.extend(sqlx::query_scalar::<_, Uuid>("SELECT id FROM records WHERE workspace_id=$1 AND id=ANY($2) AND deleted_at IS NULL AND blueprint_id=$3 AND blueprint_version=$4")
                    .bind(ws).bind(&released).bind(rule.blueprint_id).bind(rule.blueprint_version).fetch_all(&mut **tx).await?);
            }
        }
        dependents.sort();
        dependents.dedup();
        dependents.retain(|dependent| *dependent != event.aggregate_id);
        dependents.truncate(MAX_DEPENDENTS as usize);
        Ok(dependents)
    }
    /// Whether any enabled rule's schedule cursor is due. Schedulers check
    /// this first so an idle tick costs one statement.
    pub async fn rule_schedules_due(&self) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM rule_schedule_states s JOIN rule_lifecycles l ON l.rule_id = s.rule_id AND l.workspace_id = s.workspace_id AND l.enabled_version = s.rule_version WHERE s.workspace_id = $1 AND s.next_run_at <= $2)",
        )
        .bind(self.workspace_id.0)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?)
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
            // Whether the rule has an unfinished run; read on first need and
            // kept current as this pass creates runs.
            let mut active: Option<bool> = None;
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
                let is_active = match active {
                    Some(active) => active,
                    None => {
                        let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND status IN ('pending','leased'))").bind(ws).bind(rule_id).fetch_one(&mut *tx).await?;
                        *active.insert(current)
                    }
                };
                if !missed && !is_active {
                    let key = format!("{index}:{}", due.to_rfc3339());
                    let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,source,idempotency_key) VALUES($1,$2,$3,$4,'schedule',$5) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
                        .bind(Uuid::new_v4()).bind(ws).bind(rule_id).bind(version).bind(key).fetch_optional(&mut *tx).await?;
                    if let Some(run_id) = run_id {
                        self.enqueue_rule_task(&mut tx, ws, run_id, None, None)
                            .await?;
                        created += 1;
                        active = Some(true);
                    }
                }
                sqlx::query("UPDATE rule_schedule_states SET next_run_at=$5,misfires=misfires+$6,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND trigger_index=$4").bind(ws).bind(rule_id).bind(version).bind(index as i32).bind(next).bind(if missed || is_active { 1_i64 } else { 0 }).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(created)
    }
    pub async fn list_rule_runs(&self) -> Result<Vec<RuleRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,source,dry_run,scope_record_id,status,candidate_cursor,candidates_evaluated,findings_created,findings_resolved,attempts,last_error,truncated,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?.into_domain())
    }
    pub async fn list_rule_findings(
        &self,
        record_id: Option<Uuid>,
    ) -> Result<Vec<RuleFinding>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,record_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND ($2::uuid IS NULL OR record_id=$2) ORDER BY updated_at DESC").bind(ws).bind(record_id).fetch_all(&self.pool).await?.into_domain())
    }
    pub async fn rule_runs_page(
        &self,
        rule_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleRun>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,source,dry_run,scope_record_id,status,candidate_cursor,candidates_evaluated,findings_created,findings_resolved,attempts,last_error,truncated,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 AND ($2::uuid IS NULL OR rule_id=$2) ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(ws).bind(rule_id).bind(limit + 1).bind(offset).fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    pub async fn rule_findings_page(
        &self,
        record_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleFinding>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,record_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND ($2::uuid IS NULL OR record_id=$2) ORDER BY updated_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(ws).bind(record_id).bind(limit + 1).bind(offset).fetch_all(&self.pool).await?;
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
        let finding = sqlx::query_as::<_, Db<RuleFinding>>("SELECT id,rule_id,rule_version,record_id,context_id,severity,message,evidence,state,acknowledged_at,resolved_at,created_at,updated_at FROM rule_findings WHERE workspace_id=$1 AND id=$2").bind(ws).bind(id).fetch_one(&mut *tx).await?.into_domain();
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
            "SELECT rr.status,rr.id,rr.rule_id,rr.rule_version,rr.dry_run,rr.scope_record_id,rr.candidate_cursor,rr.candidates_evaluated,r.blueprint_id,r.blueprint_version,r.context_id,r.compiled_plan FROM rule_runs rr JOIN rules r ON r.id=rr.rule_id AND r.version=rr.rule_version AND r.workspace_id=rr.workspace_id WHERE rr.id=$1 AND rr.workspace_id=$2 FOR UPDATE OF rr",
        ).bind(task.subject_id).bind(ws).fetch_optional(&mut *tx).await?;
        let Some((
            status,
            id,
            rule_id,
            rule_version,
            dry_run,
            scope_record_id,
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
            scope_record_id,
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
        truncated: bool,
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
        let distinct = results
            .iter()
            .map(|result| (result.record_id, result.evaluation_key.as_str()))
            .collect::<std::collections::HashSet<_>>()
            .len()
            == results.len();
        if run.dry_run {
            created = results.iter().map(|result| i64::from(result.failed)).sum();
        } else if distinct {
            // Each finding is written by one statement per outcome; one row
            // per finding key keeps them equivalent to writing in order.
            let (failed, passed): (Vec<_>, Vec<_>) =
                results.iter().partition(|result| result.failed);
            if !failed.is_empty() {
                created = sqlx::query_scalar("WITH upserted AS (INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,record_id,context_id,evaluation_key,severity,message,evidence) SELECT f.id,$1,$2,$3,f.record_id,$4,f.evaluation_key,f.severity,f.message,f.evidence FROM unnest($5::uuid[],$6::uuid[],$7::text[],$8::text[],$9::text[],$10::jsonb[]) AS f(id,record_id,evaluation_key,severity,message,evidence) ON CONFLICT(workspace_id,rule_id,record_id,context_id,evaluation_key) DO UPDATE SET rule_version=EXCLUDED.rule_version,severity=EXCLUDED.severity,message=EXCLUDED.message,evidence=EXCLUDED.evidence,state='open',resolved_at=NULL,resolved_by_run_id=NULL,updated_at=clock_timestamp() RETURNING (xmax=0) AS inserted) SELECT count(*) FILTER (WHERE inserted) FROM upserted")
                    .bind(ws)
                    .bind(run.rule_id)
                    .bind(run.rule_version)
                    .bind(run.context_id)
                    .bind(failed.iter().map(|_| Uuid::new_v4()).collect::<Vec<_>>())
                    .bind(failed.iter().map(|result| result.record_id).collect::<Vec<_>>())
                    .bind(failed.iter().map(|result| result.evaluation_key.as_str()).collect::<Vec<_>>())
                    .bind(failed.iter().map(|result| result.severity.as_str()).collect::<Vec<_>>())
                    .bind(failed.iter().map(|result| result.message.as_str()).collect::<Vec<_>>())
                    .bind(failed.iter().map(|result| &result.evidence).collect::<Vec<_>>())
                    .fetch_one(&mut *tx)
                    .await?;
            }
            if !passed.is_empty() {
                resolved = sqlx::query("UPDATE rule_findings f SET state='resolved',resolved_at=clock_timestamp(),resolved_by_run_id=$3,updated_at=clock_timestamp() FROM unnest($5::uuid[],$6::text[]) AS p(record_id,evaluation_key) WHERE f.workspace_id=$1 AND f.rule_id=$2 AND f.record_id=p.record_id AND f.context_id IS NOT DISTINCT FROM $4 AND f.evaluation_key=p.evaluation_key AND f.state <> 'resolved'")
                    .bind(ws)
                    .bind(run.rule_id)
                    .bind(run.id)
                    .bind(run.context_id)
                    .bind(passed.iter().map(|result| result.record_id).collect::<Vec<_>>())
                    .bind(passed.iter().map(|result| result.evaluation_key.as_str()).collect::<Vec<_>>())
                    .execute(&mut *tx)
                    .await?
                    .rows_affected() as i64;
            }
        } else {
            for result in &results {
                if result.failed {
                    let inserted: bool = sqlx::query_scalar("INSERT INTO rule_findings(id,workspace_id,rule_id,rule_version,record_id,context_id,evaluation_key,severity,message,evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(workspace_id,rule_id,record_id,context_id,evaluation_key) DO UPDATE SET rule_version=EXCLUDED.rule_version,severity=EXCLUDED.severity,message=EXCLUDED.message,evidence=EXCLUDED.evidence,state='open',resolved_at=NULL,resolved_by_run_id=NULL,updated_at=clock_timestamp() RETURNING (xmax=0)")
                        .bind(Uuid::new_v4()).bind(ws).bind(run.rule_id).bind(run.rule_version).bind(result.record_id).bind(run.context_id).bind(&result.evaluation_key).bind(&result.severity).bind(&result.message).bind(&result.evidence).fetch_one(&mut *tx).await?;
                    created += i64::from(inserted);
                } else {
                    resolved += sqlx::query("UPDATE rule_findings SET state='resolved',resolved_at=clock_timestamp(),resolved_by_run_id=$4,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND record_id=$3 AND context_id IS NOT DISTINCT FROM $5 AND evaluation_key=$6 AND state <> 'resolved'")
                        .bind(ws).bind(run.rule_id).bind(result.record_id).bind(run.id).bind(run.context_id).bind(&result.evaluation_key).execute(&mut *tx).await?.rows_affected() as i64;
                }
            }
        }
        sqlx::query("UPDATE rule_runs SET candidate_cursor=$2,candidates_evaluated=candidates_evaluated+$3,findings_created=findings_created+$4,findings_resolved=findings_resolved+$5,status=CASE WHEN $6 THEN 'completed' ELSE 'pending' END,completed_at=CASE WHEN $6 THEN clock_timestamp() ELSE completed_at END,truncated=$8,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$7")
            .bind(run.id).bind(next_cursor).bind(results.len() as i64).bind(created).bind(resolved).bind(done).bind(ws).bind(truncated).execute(&mut *tx).await?;
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
            self.dead_letter_task_in_transaction(&mut tx, task, "rule_run", error)
                .await?;
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

/// An enabled rule as seen by event fan-out.
#[derive(sqlx::FromRow)]
struct EventRuleRow {
    id: Uuid,
    version: i64,
    compiled_plan: serde_json::Value,
    blueprint_id: Uuid,
    blueprint_version: i64,
    /// The rule targets the event record's own revision.
    direct: bool,
    /// The event record's blueprint code.
    event_blueprint: Option<String>,
}

impl CatalogRepository {
    /// Evaluates a rule for a candidate page with the shared predicate
    /// engine. A rule without a context checks every resolved context and
    /// fails when any of them fails.
    pub async fn evaluate_rule_candidates(
        &self,
        context_id: Option<Uuid>,
        rule: &catalog_rules::CompiledRule,
        candidates: &[Uuid],
    ) -> Result<Vec<RuleCandidateResult>, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut conn = self.pool.acquire().await?;
        let scope = super::checks::CheckScope::load(&mut conn, ws).await?;
        let contexts = context_id
            .map(|context| vec![context])
            .unwrap_or_else(|| scope.context_ids());
        let mut records = super::checks::load_current_records(&mut conn, ws, candidates).await?;
        let requirements =
            catalog_validation::predicate::Requirements::for_predicates([&rule.predicate]);
        let prefetch =
            super::checks::prefetch_related(&mut conn, &scope, &records, &contexts, &requirements)
                .await?;
        let severity = severity_code(&rule.severity);
        let mut results = Vec::with_capacity(candidates.len());
        for record_id in candidates {
            let Some(subject) = records.remove(record_id) else {
                continue;
            };
            let mut failure: Option<catalog_validation::predicate::Failure> = None;
            let mut failing_contexts = Vec::new();
            for context in &contexts {
                let outcomes = super::checks::evaluate_in_context_with(
                    &mut conn,
                    &scope,
                    &subject,
                    *context,
                    &[],
                    &[&rule.predicate],
                    Some(&prefetch),
                )
                .await?;
                if let Some(Err(error)) = outcomes.into_iter().next() {
                    failing_contexts.push(scope.code(*context));
                    failure.get_or_insert(error);
                }
            }
            let (failed, message, mut evidence) = match failure {
                Some(failure) => (true, failure.message, failure.evidence),
                None => (
                    false,
                    catalog_validation::predicate::describe_predicate(&rule.predicate),
                    serde_json::json!({}),
                ),
            };
            if let Some(object) = evidence.as_object_mut() {
                object.insert("contexts".into(), serde_json::json!(failing_contexts));
            }
            results.push(RuleCandidateResult {
                record_id: *record_id,
                failed,
                message,
                evidence,
                evaluation_key: rule.raw_definition_hash.clone(),
                severity: severity.to_owned(),
            });
        }
        Ok(results)
    }
}

/// The relationship targets of `attribute_code` that the event's facts
/// record as added or removed.
fn changed_relationship_targets(event: &DomainEvent, attribute_code: &str) -> Vec<Uuid> {
    let has_code = |entry: &&Value| {
        entry.get("attribute_code").and_then(Value::as_str) == Some(attribute_code)
    };
    let as_uuid = |id: &Value| id.as_str().and_then(|id| id.parse().ok());
    let entries = |field: &str| {
        event
            .payload
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(has_code)
    };
    // Record writes list changed targets in their facts; a migration, which
    // carries no facts, lists the targets it released.
    let mut targets: Vec<Uuid> = entries("facts")
        .filter_map(|fact| fact.get("relationship_target_record_id").and_then(as_uuid))
        .chain(
            entries("released_relationships")
                .filter_map(|released| released.get("target_record_ids").and_then(Value::as_array))
                .flatten()
                .filter_map(as_uuid),
        )
        .collect();
    targets.sort();
    targets.dedup();
    targets
}

/// Decides the enable gate from the latest completed full dry run, given as
/// its violation count and whether it stopped at the candidate cap. A
/// truncated dry run missed candidates, so it counts only when existing
/// violations are accepted.
fn dry_run_enable_gate(
    latest: Option<(i64, bool)>,
    accept_existing_violations: bool,
) -> Result<(), RepositoryError> {
    match latest {
        None => Err(RepositoryError::RuleDryRunRequired),
        Some((count, true)) if !accept_existing_violations => {
            Err(RepositoryError::RuleDryRunTruncated(count))
        }
        Some((count, _)) if count > 0 && !accept_existing_violations => {
            Err(RepositoryError::RuleHasExistingViolations(count))
        }
        Some(_) => Ok(()),
    }
}

/// The `rule_findings.severity` code, spelled as in rule definitions.
fn severity_code(severity: &catalog_rules::Severity) -> &'static str {
    match severity {
        catalog_rules::Severity::Info => "info",
        catalog_rules::Severity::Warning => "warning",
        catalog_rules::Severity::Error => "error",
        catalog_rules::Severity::Critical => "critical",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_gate_requires_a_complete_dry_run_or_accepted_violations() {
        assert!(matches!(
            dry_run_enable_gate(None, true),
            Err(RepositoryError::RuleDryRunRequired)
        ));
        assert!(dry_run_enable_gate(Some((0, false)), false).is_ok());
        assert!(matches!(
            dry_run_enable_gate(Some((3, false)), false),
            Err(RepositoryError::RuleHasExistingViolations(3))
        ));
        assert!(dry_run_enable_gate(Some((3, false)), true).is_ok());
        // A dry run that stopped at the candidate cap never proves the
        // remaining records pass.
        assert!(matches!(
            dry_run_enable_gate(Some((0, true)), false),
            Err(RepositoryError::RuleDryRunTruncated(0))
        ));
        assert!(matches!(
            dry_run_enable_gate(Some((3, true)), false),
            Err(RepositoryError::RuleDryRunTruncated(3))
        ));
        assert!(dry_run_enable_gate(Some((0, true)), true).is_ok());
    }
}
