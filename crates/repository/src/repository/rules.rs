use super::generations::{Generation, advance_generation};
use super::notifications::{
    NewNotification, NotificationSubject, NotificationSubjectKind, kinds as notification_kinds,
};
use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use crate::{
    domain_events::DomainEvent,
    model::{
        CreateManualRuleRun, CreateRule, EnableRule, RevisionNotApplicable, Rule, RuleDetails,
        RuleDryRunFailure, RuleFinding, RuleRevision, RuleRun, RuleRunDetails, RuleWarning,
        UpdateRule,
    },
    task_queue::{TaskInsert, TaskKind},
};
use attricat_domain::states::{RevisionStatus, RuleFindingSeverity, RuleFindingState, RunStatus};
use attricat_rules::CompiledRule;
use chrono::Utc;
use uuid::Uuid;

/// How many of the records a dry run flags it names. The rest are counted.
pub const DRY_RUN_FAILURE_SAMPLE: i64 = 50;

/// A rule as its current revision, with whether it is enabled. Select it
/// from [`CURRENT_RULES`].
const RULE_FIELDS: &str = "r.id,r.blueprint_id,r.context_id,r.code,r.name,r.version,r.definition,r.definition_hash,r.compiled_plan,l.enabled,(SELECT min(x.created_at) FROM rules x WHERE x.workspace_id=r.workspace_id AND x.id=r.id) AS created_at,r.created_at AS updated_at";
/// Each rule joined to its current revision.
const CURRENT_RULES: &str = "rule_lifecycles l JOIN rules r ON r.workspace_id=l.workspace_id AND r.id=l.rule_id AND r.version=l.current_version";
const REVISION_FIELDS: &str =
    "id AS rule_id,version,name,definition,definition_hash,created_by_user_id,created_at";

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
    pub context_id: Option<Uuid>,
    /// The definition the run evaluates: its rule revision's, or the unsaved
    /// one a dry run was given.
    pub compiled_plan: serde_json::Value,
}

/// Evaluation reads happen before the fenced checkpoint transaction. This is
/// deliberately data-only: all findings and run progress are persisted later
/// under the task token.
pub struct RuleCandidateResult {
    pub record_id: Uuid,
    pub failed: bool,
    /// The record's blueprint revision does not fit the rule. It is neither
    /// checked nor reported, and passes.
    pub skipped: bool,
    pub message: String,
    pub evidence: serde_json::Value,
    pub evaluation_key: String,
    pub severity: String,
}

/// A durable run to create: `(rule, rule version, definition hash, scope
/// record, idempotency key)`.
type NewRuleRun = (Uuid, i64, String, Uuid, String);

impl AttricatRepository {
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

    /// Rules, optionally of one blueprint, with the revisions they skip.
    pub async fn list_rules(
        &self,
        blueprint_id: Option<Uuid>,
    ) -> Result<Vec<RuleDetails>, RepositoryError> {
        let ws = self.workspace_id.0;
        let rules: Vec<Rule> = sqlx::query_as::<_, Db<Rule>>(&format!(
            "SELECT {RULE_FIELDS} FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND ($2::uuid IS NULL OR r.blueprint_id=$2) ORDER BY r.name,r.id"
        ))
        .bind(ws)
        .bind(blueprint_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain();
        let mut conn = self.pool.acquire().await?;
        with_applicability(&mut conn, ws, rules).await
    }

    /// The rules of a record's blueprint, with the revisions they skip, so
    /// the record's readers can name the rules its findings come from.
    pub async fn list_record_rules(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<RuleDetails>, RepositoryError> {
        let blueprint_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT blueprint_id FROM records WHERE workspace_id=$1 AND id=$2 AND deleted_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(blueprint_id) = blueprint_id else {
            return Err(RepositoryError::NotFound("record"));
        };
        // The counts span records the reader may not see.
        let mut rules = self.list_rules(Some(blueprint_id)).await?;
        for rule in &mut rules {
            rule.open_findings = None;
            rule.acknowledged_findings = None;
        }
        Ok(rules)
    }

    pub async fn get_rule(&self, id: Uuid) -> Result<Option<Rule>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<Rule>>(&format!(
            "SELECT {RULE_FIELDS} FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.rule_id=$2"
        ))
        .bind(self.workspace_id.0)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .into_domain())
    }

    /// A rule with the revisions of its blueprint it skips.
    pub async fn get_rule_details(&self, id: Uuid) -> Result<Option<RuleDetails>, RepositoryError> {
        let Some(rule) = self.get_rule(id).await? else {
            return Ok(None);
        };
        let mut conn = self.pool.acquire().await?;
        Ok(
            with_applicability(&mut conn, self.workspace_id.0, vec![rule])
                .await?
                .pop(),
        )
    }

    /// Every saved revision of a rule, newest first: its changelog.
    pub async fn list_rule_revisions(
        &self,
        id: Uuid,
    ) -> Result<Vec<RuleRevision>, RepositoryError> {
        let revisions: Vec<RuleRevision> = sqlx::query_as::<_, Db<RuleRevision>>(&format!(
            "SELECT {REVISION_FIELDS} FROM rules WHERE workspace_id=$1 AND id=$2 ORDER BY version DESC"
        ))
        .bind(self.workspace_id.0)
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .into_domain();
        if revisions.is_empty() {
            return Err(RepositoryError::NotFound("rule"));
        }
        Ok(revisions)
    }

    pub async fn get_rule_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Option<RuleRevision>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<RuleRevision>>(&format!(
            "SELECT {REVISION_FIELDS} FROM rules WHERE workspace_id=$1 AND id=$2 AND version=$3"
        ))
        .bind(self.workspace_id.0)
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

    /// Runs the checks `create_rule` applies to a definition and its blueprint
    /// and context, without storing anything.
    pub async fn validate_rule(&self, input: &CreateRule) -> Result<CompiledRule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let compiled = self.check_new_rule(&mut tx, input).await?;
        tx.rollback().await?;
        Ok(compiled)
    }

    /// Compiles a new rule and checks it against its blueprint and optional
    /// context.
    async fn check_new_rule(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: &CreateRule,
    ) -> Result<CompiledRule, RepositoryError> {
        let compiled = compile_rule(&input.definition)?;
        let ws = self.workspace_id.0;
        check_rule_fits(tx, ws, input.blueprint_id, &compiled).await?;
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
        Ok(compiled)
    }

    /// Serializes every writer of a rule code in this workspace until the
    /// transaction ends and returns the rules that already use it. A rule
    /// code names one rule per workspace.
    pub(super) async fn lock_rule_code(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        code: &str,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let ws = self.workspace_id.0;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("rule-code:{ws}:{code}"))
            .execute(&mut **tx)
            .await?;
        Ok(
            sqlx::query_scalar("SELECT DISTINCT id FROM rules WHERE workspace_id=$1 AND code=$2")
                .bind(ws)
                .bind(code)
                .fetch_all(&mut **tx)
                .await?,
        )
    }

    /// Shared mutation seam for callers that create a rule with a chosen id
    /// as part of a larger transaction, such as seed application. The rule
    /// starts disabled with its first revision current.
    pub(super) async fn create_rule_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: CreateRule,
    ) -> Result<Uuid, RepositoryError> {
        let compiled = self.check_new_rule(tx, &input).await?;
        let ws = self.workspace_id.0;
        if !self.lock_rule_code(tx, &compiled.code).await?.is_empty() {
            return Err(RepositoryError::CodeTaken(CodedResource::Rule));
        }
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO rules(id,workspace_id,blueprint_id,context_id,code,name,version,definition,definition_hash,compiled_plan,created_by_user_id) VALUES($1,$2,$3,$4,$5,$6,1,$7,$8,$9,$10)").bind(id).bind(ws).bind(input.blueprint_id).bind(input.context_id).bind(&compiled.code).bind(&compiled.name).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).bind(self.actor_user_id()).execute(&mut **tx).await?;
        sqlx::query(
            "INSERT INTO rule_lifecycles(rule_id,workspace_id,current_version) VALUES($1,$2,1)",
        )
        .bind(id)
        .bind(ws)
        .execute(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Saves a changed definition as the rule's new current revision. Findings
    /// of earlier definitions are resolved; an enabled rule moves its
    /// schedules to the new revision and re-checks every record. An enforcing
    /// definition of an enabled rule passes the same dry-run gate as enabling.
    pub async fn update_rule(&self, id: Uuid, input: UpdateRule) -> Result<Rule, RepositoryError> {
        let compiled = compile_rule(&input.definition)?;
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let current: Option<(i64, String, Uuid, String, bool)> = sqlx::query_as(&format!(
            "SELECT r.version,r.code,r.blueprint_id,r.definition_hash,l.enabled FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.rule_id=$2 FOR UPDATE OF l"
        ))
        .bind(ws)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((version, code, blueprint_id, definition_hash, enabled)) = current else {
            return Err(RepositoryError::NotFound("rule"));
        };
        if code != compiled.code {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule code cannot change across revisions".into(),
            ));
        }
        // Only the current revision counts, as for blueprints: returning to
        // an earlier definition is a change.
        if definition_hash == compiled.raw_definition_hash {
            return Err(RepositoryError::RuleRevisionUnchanged { version });
        }
        check_rule_fits(&mut tx, ws, blueprint_id, &compiled).await?;
        if enabled && compiled.enforcement.is_some() {
            self.dry_run_gate(
                &mut tx,
                id,
                blueprint_id,
                &compiled.raw_definition_hash,
                input.accept_existing_violations,
            )
            .await?;
        }
        let next: i64 =
            sqlx::query_scalar("SELECT max(version)+1 FROM rules WHERE workspace_id=$1 AND id=$2")
                .bind(ws)
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO rules(id,workspace_id,blueprint_id,context_id,code,name,version,definition,definition_hash,compiled_plan,created_by_user_id) SELECT id,workspace_id,blueprint_id,context_id,$3,$4,$5,$6,$7,$8,$9 FROM rules WHERE workspace_id=$1 AND id=$2 AND version=$10").bind(ws).bind(id).bind(&compiled.code).bind(&compiled.name).bind(next).bind(&input.definition).bind(&compiled.raw_definition_hash).bind(plan).bind(self.actor_user_id()).bind(version).execute(&mut *tx).await?;
        sqlx::query("UPDATE rule_lifecycles SET current_version=$3,updated_at=now() WHERE workspace_id=$1 AND rule_id=$2")
            .bind(ws).bind(id).bind(next).execute(&mut *tx).await?;
        // Findings belong to the definition that reported them.
        let superseded: Vec<Uuid> = sqlx::query_scalar("UPDATE rule_findings SET state=$4,resolved_at=clock_timestamp(),resolved_by_run_id=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND evaluation_key<>$3 AND state<>$4 RETURNING record_id")
            .bind(ws).bind(id).bind(&compiled.raw_definition_hash).bind(RuleFindingState::Resolved.as_str()).fetch_all(&mut *tx).await?;
        sync_needs_attention_tags(&mut tx, ws, &superseded).await?;
        // Unfinished runs would report against the previous definition.
        self.cancel_unfinished_rule_runs(&mut tx, id, false).await?;
        if enabled {
            sqlx::query("DELETE FROM rule_schedule_states WHERE workspace_id=$1 AND rule_id=$2")
                .bind(ws)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            self.start_rule_schedules(&mut tx, id, next, &compiled)
                .await?;
            let run_id = Uuid::new_v4();
            sqlx::query("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,definition_hash,source,idempotency_key) VALUES($1,$2,$3,$4,$5,'manual',$6)")
                .bind(run_id).bind(ws).bind(id).bind(next).bind(&compiled.raw_definition_hash).bind(format!("revision:{next}")).execute(&mut *tx).await?;
            self.enqueue_rule_task(&mut tx, ws, run_id, None, None)
                .await?;
        }
        advance_generation(&mut tx, ws, Generation::Attricat).await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }

    /// Enables a rule's current revision. An enforcing rule first needs a
    /// completed full dry run of that definition, unless its blueprint has no
    /// live records; existing violations must be accepted explicitly.
    pub async fn enable_rule(
        &self,
        id: Uuid,
        options: EnableRule,
    ) -> Result<Rule, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.enable_rule_in_transaction(&mut tx, id, options.accept_existing_violations)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }

    /// The enable path shared by every caller, gate included.
    pub(super) async fn enable_rule_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        accept_existing_violations: bool,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let current: Option<(i64, Uuid, serde_json::Value)> = sqlx::query_as(&format!(
            "SELECT r.version,r.blueprint_id,r.compiled_plan FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.rule_id=$2 FOR UPDATE OF l"
        ))
        .bind(ws)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((version, blueprint_id, plan)) = current else {
            return Err(RepositoryError::NotFound("rule"));
        };
        let compiled = stored_rule(plan)?;
        if compiled.enforcement.is_some() {
            self.dry_run_gate(
                tx,
                id,
                blueprint_id,
                &compiled.raw_definition_hash,
                accept_existing_violations,
            )
            .await?;
        }
        sqlx::query("UPDATE rule_lifecycles SET enabled=true,activation_sequence=(SELECT COALESCE(max(sequence),0) FROM domain_events WHERE workspace_id=$1),enabled_at=now(),disabled_at=NULL,updated_at=now() WHERE workspace_id=$1 AND rule_id=$2").bind(ws).bind(id).execute(&mut **tx).await?;
        advance_generation(tx, ws, Generation::Attricat).await?;
        self.start_rule_schedules(tx, id, version, &compiled).await
    }

    /// The dry-run gate for an enforcing definition, by its hash. A blueprint
    /// without live records needs none; otherwise the latest completed full
    /// dry run of the definition decides.
    async fn dry_run_gate(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        rule_id: Uuid,
        blueprint_id: Uuid,
        definition_hash: &str,
        accept_existing_violations: bool,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let has_records: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND blueprint_id=$2 AND deleted_at IS NULL)")
            .bind(ws).bind(blueprint_id).fetch_one(&mut **tx).await?;
        if !has_records {
            return Ok(());
        }
        let latest: Option<(i64, bool)> = sqlx::query_as("SELECT findings_created,truncated FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND definition_hash=$3 AND dry_run AND scope_record_id IS NULL AND status='completed' ORDER BY completed_at DESC LIMIT 1")
            .bind(ws).bind(rule_id).bind(definition_hash).fetch_optional(&mut **tx).await?;
        dry_run_verdict(latest, accept_existing_violations)
    }

    /// Starts the schedule cursors of a rule revision's schedule triggers.
    async fn start_rule_schedules(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
        compiled: &CompiledRule,
    ) -> Result<(), RepositoryError> {
        for (index, trigger) in compiled.triggers.iter().enumerate() {
            let attricat_rules::Trigger::Schedule { cron, .. } = trigger else {
                continue;
            };
            let schedule = attricat_rules::parse_six_field_cron(cron).map_err(|_| {
                RepositoryError::InvalidRuleDefinition("stored schedule cron is invalid".into())
            })?;
            let next = schedule.after(&Utc::now()).next().ok_or_else(|| {
                RepositoryError::InvalidRuleDefinition("schedule has no future occurrence".into())
            })?;
            sqlx::query("INSERT INTO rule_schedule_states(workspace_id,rule_id,rule_version,trigger_index,next_run_at) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING").bind(self.workspace_id.0).bind(id).bind(version).bind(index as i32).bind(next).execute(&mut **tx).await?;
        }
        Ok(())
    }

    pub async fn disable_rule(&self, id: Uuid) -> Result<Rule, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        if sqlx::query("UPDATE rule_lifecycles SET enabled=false,disabled_at=now(),updated_at=now() WHERE workspace_id=$1 AND rule_id=$2").bind(ws).bind(id).execute(&mut *tx).await?.rows_affected()==0{return Err(RepositoryError::NotFound("rule"));}
        advance_generation(&mut tx, ws, Generation::Attricat).await?;
        self.cancel_unfinished_rule_runs(&mut tx, id, true).await?;
        self.commit_mutation(tx).await?;
        self.get_rule(id)
            .await?
            .ok_or(RepositoryError::NotFound("rule"))
    }

    /// Cancels a rule's pending and leased runs, and their queued tasks.
    /// Dry runs are kept unless `dry_runs` is set: they report nothing.
    async fn cancel_unfinished_rule_runs(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        dry_runs: bool,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        sqlx::query("UPDATE rule_runs SET status='cancelled',cancelled_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND rule_id=$2 AND status IN ('pending','leased') AND ($3 OR NOT dry_run)").bind(ws).bind(id).bind(dry_runs).execute(&mut **tx).await?;
        let queued_tasks: Vec<Uuid> = sqlx::query_scalar("SELECT t.id FROM tasks t JOIN rule_runs rr ON rr.id=t.subject_id WHERE t.workspace_id=$1 AND t.kind='rule_run.v1' AND t.status='queued' AND rr.rule_id=$2 AND rr.status='cancelled' FOR UPDATE OF t")
            .bind(ws).bind(id).fetch_all(&mut **tx).await?;
        for task_id in queued_tasks {
            self.cancel_queued_task(tx, task_id).await?;
        }
        Ok(())
    }

    /// Creates a durable, idempotent manual run of the rule's current
    /// revision. A normal run needs the rule enabled; a dry run may instead
    /// check an unsaved `definition`. An optional record scope is checked
    /// before enqueueing, so callers cannot turn a manual request into an
    /// arbitrary cross-workspace scan.
    pub async fn create_manual_rule_run(
        &self,
        rule_id: Uuid,
        input: CreateManualRuleRun,
    ) -> Result<Uuid, RepositoryError> {
        if !super::workflow_runs::valid_manual_idempotency_key(&input.idempotency_key) {
            return Err(RepositoryError::InvalidRuleDefinition(
                "manual idempotency_key must be 1-128 ASCII bytes".into(),
            ));
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let current: Option<(i64, String, Uuid, String, bool)> = sqlx::query_as(&format!(
            "SELECT r.version,r.code,r.blueprint_id,r.definition_hash,l.enabled FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.rule_id=$2 FOR SHARE OF l"
        ))
        .bind(ws)
        .bind(rule_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((version, code, blueprint_id, current_hash, enabled)) = current else {
            return Err(RepositoryError::NotFound("rule"));
        };
        let (definition_hash, plan) = match &input.definition {
            Some(_) if !input.dry_run => {
                return Err(RepositoryError::InvalidRuleDefinition(
                    "only a dry run can check an unsaved definition".into(),
                ));
            }
            Some(definition) => {
                let compiled = compile_rule(definition)?;
                if compiled.code != code {
                    return Err(RepositoryError::InvalidRuleDefinition(
                        "rule code cannot change across revisions".into(),
                    ));
                }
                check_rule_fits(&mut tx, ws, blueprint_id, &compiled).await?;
                let plan = serde_json::to_value(&compiled)
                    .map_err(|e| RepositoryError::InvalidRuleDefinition(e.to_string()))?;
                (compiled.raw_definition_hash, Some(plan))
            }
            None => (current_hash, None),
        };
        if !input.dry_run && !enabled {
            return Err(RepositoryError::RuleNotEnabled);
        }
        if let Some(record_id) = input.record_id {
            let matches: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM records WHERE id=$2 AND workspace_id=$1 AND deleted_at IS NULL AND blueprint_id=$3)")
                .bind(ws).bind(record_id).bind(blueprint_id).fetch_one(&mut *tx).await?;
            if !matches {
                return Err(RepositoryError::NotFound("record in rule blueprint"));
            }
        }
        let id = Uuid::new_v4();
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,definition_hash,compiled_plan,source,dry_run,scope_record_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,'manual',$7,$8,$9) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
            .bind(id).bind(ws).bind(rule_id).bind(version).bind(&definition_hash).bind(plan).bind(input.dry_run).bind(input.record_id).bind(&input.idempotency_key).fetch_optional(&mut *tx).await?;
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

    /// Re-checks one record against every enabled rule of its blueprint
    /// with idempotent record-scoped manual runs. Returns the run of each
    /// rule, including runs an earlier request with the same key created.
    pub async fn create_record_rule_runs(
        &self,
        record_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        if !super::workflow_runs::valid_manual_idempotency_key(idempotency_key) {
            return Err(RepositoryError::InvalidRuleDefinition(
                "manual idempotency_key must be 1-128 ASCII bytes".into(),
            ));
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND id=$2 AND deleted_at IS NULL)",
        )
        .bind(ws)
        .bind(record_id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(RepositoryError::NotFound("record"));
        }
        let rules: Vec<(Uuid, i64, String)> = sqlx::query_as(&format!("SELECT r.id,r.version,r.definition_hash FROM {CURRENT_RULES} JOIN records e ON e.id=$2 AND e.workspace_id=l.workspace_id AND e.blueprint_id=r.blueprint_id WHERE l.workspace_id=$1 AND l.enabled ORDER BY r.id FOR SHARE OF l"))
            .bind(ws).bind(record_id).fetch_all(&mut *tx).await?;
        let mut ids = Vec::with_capacity(rules.len());
        let mut created = false;
        for (rule_id, version, definition_hash) in rules {
            let id = Uuid::new_v4();
            let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,definition_hash,source,scope_record_id,idempotency_key) VALUES($1,$2,$3,$4,$5,'manual',$6,$7) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
                .bind(id).bind(ws).bind(rule_id).bind(version).bind(&definition_hash).bind(record_id).bind(idempotency_key).fetch_optional(&mut *tx).await?;
            match inserted {
                Some(id) => {
                    self.enqueue_rule_task(&mut tx, ws, id, None, None).await?;
                    created = true;
                    ids.push(id);
                }
                None => ids.push(sqlx::query_scalar("SELECT id FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3 AND source='manual' AND idempotency_key=$4").bind(ws).bind(rule_id).bind(version).bind(idempotency_key).fetch_one(&mut *tx).await?),
            }
        }
        if created {
            self.commit_mutation(tx).await?;
        } else {
            tx.commit().await?;
        }
        Ok(ids)
    }
    /// Event intake only creates bounded record-scoped work. It never executes on the outbox lease.
    pub async fn fan_out_rule_runs(&self, event: &DomainEvent) -> Result<u64, RepositoryError> {
        if event.aggregate_kind != "record" || event.source_name.starts_with("rule:") {
            return Ok(0);
        }
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        // Every enabled rule in one read: whether it is a rule of the event
        // record's blueprint, and that record's blueprint code.
        let rows: Vec<EventRuleRow> = sqlx::query_as(&format!("SELECT r.id, r.version, r.definition_hash, r.compiled_plan, r.blueprint_id, (e.id IS NOT NULL) AS direct, (SELECT b.code FROM records x JOIN blueprints b ON b.id=x.blueprint_id AND b.version=x.blueprint_version WHERE x.workspace_id=$1 AND x.id=$3) AS event_blueprint FROM {CURRENT_RULES} LEFT JOIN records e ON e.id=$3 AND e.workspace_id=l.workspace_id AND e.blueprint_id=r.blueprint_id AND e.deleted_at IS NULL WHERE l.workspace_id=$1 AND l.enabled AND $2 > COALESCE(l.activation_sequence,0) FOR SHARE OF l")).bind(ws).bind(event.sequence).bind(event.aggregate_id).fetch_all(&mut *tx).await?;
        // A change to the record re-evaluates it for every enabled rule of its
        // blueprint; dependents re-run only for rules triggered by the event.
        let record_changed = attricat_rules::is_record_change_event(&event.event_type);
        let mut runs: Vec<NewRuleRun> = Vec::new();
        let mut rules = Vec::with_capacity(rows.len());
        for row in rows {
            let compiled = stored_rule(row.compiled_plan.clone())?;
            let triggered = compiled.triggers.iter().any(|trigger| matches!(trigger, attricat_rules::Trigger::Event { event_type } if event_type == &event.event_type));
            if row.direct && (triggered || record_changed) {
                runs.push((
                    row.id,
                    row.version,
                    row.definition_hash.clone(),
                    event.aggregate_id,
                    event.id.to_string(),
                ));
            }
            if triggered {
                rules.push((row, compiled));
            }
        }
        let mut inserted = self.insert_event_rule_runs(&mut tx, event, &runs).await?;
        runs.clear();
        for (row, compiled) in &rules {
            for dependent in self.rule_dependents(&mut tx, event, row, compiled).await? {
                runs.push((
                    row.id,
                    row.version,
                    row.definition_hash.clone(),
                    dependent,
                    format!("{}:{dependent}", event.id),
                ));
            }
        }
        inserted += self.insert_event_rule_runs(&mut tx, event, &runs).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// Creates event-sourced runs and their task envelopes with one
    /// statement; existing runs are skipped.
    async fn insert_event_rule_runs(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &DomainEvent,
        runs: &[NewRuleRun],
    ) -> Result<u64, RepositoryError> {
        if runs.is_empty() {
            return Ok(0);
        }
        let ids: Vec<Uuid> = runs.iter().map(|_| Uuid::new_v4()).collect();
        let task_ids: Vec<Uuid> = runs.iter().map(|_| Uuid::new_v4()).collect();
        let rule_ids: Vec<Uuid> = runs.iter().map(|run| run.0).collect();
        let versions: Vec<i64> = runs.iter().map(|run| run.1).collect();
        let hashes: Vec<&str> = runs.iter().map(|run| run.2.as_str()).collect();
        let scopes: Vec<Uuid> = runs.iter().map(|run| run.3).collect();
        let keys: Vec<&str> = runs.iter().map(|run| run.4.as_str()).collect();
        let kind = TaskKind::RuleRunV1;
        let created: i64 = sqlx::query_scalar("WITH candidates AS (SELECT * FROM UNNEST($2::uuid[], $3::uuid[], $4::uuid[], $5::bigint[], $6::text[], $7::uuid[], $8::text[]) AS c(id, task_id, rule_id, rule_version, definition_hash, scope_record_id, idempotency_key)), created AS (INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,definition_hash,source,scope_record_id,idempotency_key) SELECT id, $1, rule_id, rule_version, definition_hash, 'event', scope_record_id, idempotency_key FROM candidates ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id), service AS (INSERT INTO task_workspace_service (workspace_id) VALUES ($1) ON CONFLICT (workspace_id) DO NOTHING), queued AS (INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, payload, max_failures, correlation_id, causation_id) SELECT c.task_id, $1, $9, 1, c.id, 0, '{}'::jsonb, $10, $11, $12 FROM created JOIN candidates c ON c.id = created.id RETURNING id) SELECT count(*) FROM queued")
            .bind(self.workspace_id.0)
            .bind(&ids)
            .bind(&task_ids)
            .bind(&rule_ids)
            .bind(&versions)
            .bind(&hashes)
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
        compiled: &CompiledRule,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        const MAX_DEPENDENTS: i64 = 100;
        let ws = self.workspace_id.0;
        let requirements =
            attricat_validation::predicate::Requirements::for_predicates([&compiled.predicate]);
        let mut dependents: Vec<Uuid> = Vec::new();
        // The rule evaluates records of its blueprint; their relationship may
        // be a revision field or an additional attribute.
        for relationship in &requirements.linked {
            dependents.extend(
                super::references::referencing_record_ids(
                    tx,
                    ws,
                    super::references::ReferenceQuery {
                        target: event.aggregate_id,
                        attribute_code: relationship,
                        referrers: super::references::Referrers::Blueprint(rule.blueprint_id),
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
            dependents.extend(sqlx::query_scalar::<_, Uuid>("SELECT DISTINCT av.relationship_target_record_id FROM attribute_values av JOIN attributes a ON a.id=av.attribute_id AND a.code=$3 JOIN records t ON t.id=av.relationship_target_record_id AND t.workspace_id=$1 AND t.deleted_at IS NULL AND t.blueprint_id=$4 WHERE av.record_id=$2 AND av.active LIMIT $5")
                .bind(ws).bind(event.aggregate_id).bind(relationship).bind(rule.blueprint_id).bind(MAX_DEPENDENTS).fetch_all(&mut **tx).await?);
            // A record the write stopped referencing (a removed or re-pointed
            // relationship) no longer appears above, yet its count changed.
            let released = changed_relationship_targets(event, relationship);
            if !released.is_empty() {
                dependents.extend(sqlx::query_scalar::<_, Uuid>("SELECT id FROM records WHERE workspace_id=$1 AND id=ANY($2) AND deleted_at IS NULL AND blueprint_id=$3")
                    .bind(ws).bind(&released).bind(rule.blueprint_id).fetch_all(&mut **tx).await?);
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
            "SELECT EXISTS (SELECT 1 FROM rule_schedule_states s JOIN rule_lifecycles l ON l.rule_id = s.rule_id AND l.workspace_id = s.workspace_id AND l.current_version = s.rule_version AND l.enabled WHERE s.workspace_id = $1 AND s.next_run_at <= $2)",
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
        let rows: Vec<(Uuid, i64, String, serde_json::Value)> = sqlx::query_as(&format!("SELECT r.id,r.version,r.definition_hash,r.compiled_plan FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.enabled FOR SHARE OF l")).bind(ws).fetch_all(&mut *tx).await?;
        let mut created = 0;
        for (rule_id, version, definition_hash, plan) in rows {
            let compiled = stored_rule(plan)?;
            // Whether the rule has an unfinished run; read on first need and
            // kept current as this pass creates runs.
            let mut active: Option<bool> = None;
            for (index, trigger) in compiled.triggers.iter().enumerate() {
                let attricat_rules::Trigger::Schedule { cron, .. } = trigger else {
                    continue;
                };
                let schedule = attricat_rules::parse_six_field_cron(cron).map_err(|_| {
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
                let missed = due < now - super::workflow_runs::MISSED_SCHEDULE_GRACE;
                let is_active = match active {
                    Some(active) => active,
                    None => {
                        let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rule_runs WHERE workspace_id=$1 AND rule_id=$2 AND status IN ('pending','leased'))").bind(ws).bind(rule_id).fetch_one(&mut *tx).await?;
                        *active.insert(current)
                    }
                };
                if !missed && !is_active {
                    let key = format!("{index}:{}", due.to_rfc3339());
                    let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO rule_runs(id,workspace_id,rule_id,rule_version,definition_hash,source,idempotency_key) VALUES($1,$2,$3,$4,$5,'schedule',$6) ON CONFLICT(workspace_id,rule_id,rule_version,source,idempotency_key) DO NOTHING RETURNING id")
                        .bind(Uuid::new_v4()).bind(ws).bind(rule_id).bind(version).bind(&definition_hash).bind(key).fetch_optional(&mut *tx).await?;
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

    /// Finds the enabled rules of a blueprint that do not fit a revision just
    /// published, and tells every member who manages rules, except the
    /// publisher. Publishing goes ahead; the rules skip the revision's records.
    pub(super) async fn warn_about_rules_not_fitting(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        blueprint: &crate::model::Blueprint,
    ) -> Result<Vec<RuleWarning>, RepositoryError> {
        let ws = self.workspace_id.0;
        let rules: Vec<(Uuid, String, String, serde_json::Value)> = sqlx::query_as(&format!(
            "SELECT r.id,r.code,r.name,r.compiled_plan FROM {CURRENT_RULES} WHERE l.workspace_id=$1 AND l.enabled AND r.blueprint_id=$2 ORDER BY r.name,r.id"
        ))
        .bind(ws)
        .bind(blueprint.id)
        .fetch_all(&mut **tx)
        .await?;
        if rules.is_empty() {
            return Ok(Vec::new());
        }
        let attributes = super::checks::revision_attributes(tx, ws, &[blueprint.id])
            .await?
            .remove(&(blueprint.id, blueprint.version))
            .unwrap_or_default();
        let mut warnings = Vec::new();
        for (rule_id, code, name, plan) in rules {
            if let Err(error) = attricat_rules::applicability(&stored_rule(plan)?, &attributes) {
                warnings.push(RuleWarning {
                    rule_id,
                    code,
                    name,
                    reason: error.reason,
                    message: error.message,
                });
            }
        }
        if warnings.is_empty() {
            return Ok(warnings);
        }
        let managers = Self::workspace_members_with_permission_on(
            tx,
            ws,
            attricat_domain::permissions::Permission::RulesManage,
        )
        .await?;
        for warning in &warnings {
            let notification = NewNotification {
                kind: notification_kinds::RULE_NOT_APPLICABLE,
                title: format!(
                    "Revision {} of {} no longer fits the data quality rule \"{}\"",
                    blueprint.version, blueprint.name, warning.name
                ),
                body: Some(warning.message.clone()),
                actor_user_id: self.actor_user_id(),
                subject: Some(NotificationSubject {
                    kind: NotificationSubjectKind::Rule,
                    id: warning.rule_id,
                }),
                data: serde_json::json!({
                    "rule_name": warning.name,
                    "blueprint_id": blueprint.id,
                    "blueprint_name": blueprint.name,
                    "blueprint_version": blueprint.version,
                    "reason": warning.reason,
                }),
            };
            self.notify_on(tx, &managers, &notification).await?;
        }
        Ok(warnings)
    }

    /// The signed-in user who causes this request's writes, if any.
    fn actor_user_id(&self) -> Option<Uuid> {
        self.audit_context
            .as_ref()
            .and_then(|context| context.actor_user_id)
    }

    pub async fn list_rule_runs(&self) -> Result<Vec<RuleRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,definition_hash,source,dry_run,scope_record_id,status,candidate_cursor,candidates_evaluated,candidates_skipped,findings_created,findings_resolved,attempts,last_error,truncated,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?.into_domain())
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
        record_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleRun>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,definition_hash,source,dry_run,scope_record_id,status,candidate_cursor,candidates_evaluated,candidates_skipped,findings_created,findings_resolved,attempts,last_error,truncated,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 AND ($2::uuid IS NULL OR rule_id=$2) AND ($5::uuid IS NULL OR scope_record_id=$5) ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(ws).bind(rule_id).bind(limit + 1).bind(offset).bind(record_id).fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    /// Findings matching `filter`, most severe first and then newest first.
    /// The order does not depend on re-checks, so offset pages stay stable
    /// while rules run again.
    pub async fn rule_findings_page(
        &self,
        filter: &RuleFindingFilter,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<RuleFinding>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as::<_, Db<RuleFinding>>("SELECT f.id,f.rule_id,f.rule_version,f.record_id,f.context_id,f.severity,f.message,f.evidence,f.state,f.acknowledged_at,f.resolved_at,f.created_at,f.updated_at FROM rule_findings f WHERE f.workspace_id=$1 AND ($2::uuid IS NULL OR f.record_id=$2) AND (NOT $3 OR f.state <> $4) AND ($5::uuid IS NULL OR f.rule_id=$5) AND ($6::uuid IS NULL OR f.context_id=$6) AND ($7::text IS NULL OR f.severity=$7) AND ($8::text IS NULL OR f.state=$8) AND ($9::uuid IS NULL OR EXISTS (SELECT 1 FROM records r WHERE r.workspace_id=f.workspace_id AND r.id=f.record_id AND r.blueprint_id=$9)) ORDER BY array_position($12::text[],f.severity),f.created_at DESC,f.id DESC LIMIT $10 OFFSET $11")
            .bind(ws)
            .bind(filter.record_id)
            .bind(filter.unresolved)
            .bind(RuleFindingState::Resolved.as_str())
            .bind(filter.rule_id)
            .bind(filter.context_id)
            .bind(filter.severity.map(RuleFindingSeverity::as_str))
            .bind(filter.state.map(RuleFindingState::as_str))
            .bind(filter.blueprint_id)
            .bind(limit + 1)
            .bind(offset)
            .bind(severities_most_severe_first())
            .fetch_all(&self.pool)
            .await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    /// Open and acknowledged findings, optionally of one record.
    pub async fn unresolved_rule_finding_count(
        &self,
        record_id: Option<Uuid>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM rule_findings WHERE workspace_id=$1 AND ($2::uuid IS NULL OR record_id=$2) AND state <> 'resolved'")
            .bind(self.workspace_id.0).bind(record_id).fetch_one(&self.pool).await?)
    }

    pub async fn acknowledge_rule_finding(&self, id: Uuid) -> Result<RuleFinding, RepositoryError> {
        let ws = self.workspace_id.0;
        let actor = self.actor_user_id();
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
        sync_needs_attention_tags(&mut tx, ws, &[finding.record_id]).await?;
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
        let fenced = self.for_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let row: Option<RuleRunTaskRow> = sqlx::query_as(
            "SELECT rr.status,rr.id,rr.rule_id,rr.rule_version,rr.dry_run,rr.scope_record_id,rr.candidate_cursor,rr.candidates_evaluated,r.blueprint_id,r.context_id,COALESCE(rr.compiled_plan,r.compiled_plan) FROM rule_runs rr JOIN rules r ON r.id=rr.rule_id AND r.version=rr.rule_version AND r.workspace_id=rr.workspace_id WHERE rr.id=$1 AND rr.workspace_id=$2 FOR UPDATE OF rr",
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
            context_id,
            compiled_plan,
        )) = row
        else {
            return Err(RepositoryError::InvalidRuleDefinition(
                "rule task subject is missing".into(),
            ));
        };
        if stored_code::<RunStatus>(status)?.is_terminal() {
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
        let fenced = self.for_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM rule_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.id)
        .bind(ws)
        .fetch_optional(&mut *tx)
        .await?;
        match status.map(stored_code::<RunStatus>).transpose()? {
            Some(status) if status.is_terminal() => {
                tx.commit().await?;
                return Ok(true);
            }
            Some(RunStatus::Leased) => {}
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
            self.keep_dry_run_failures(&mut tx, run.id, &results)
                .await?;
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
        if !run.dry_run {
            let record_ids = results
                .iter()
                .map(|result| result.record_id)
                .collect::<Vec<_>>();
            sync_needs_attention_tags(&mut tx, ws, &record_ids).await?;
        }
        let skipped = results.iter().filter(|result| result.skipped).count() as i64;
        sqlx::query("UPDATE rule_runs SET candidate_cursor=$2,candidates_evaluated=candidates_evaluated+$3,candidates_skipped=candidates_skipped+$9,findings_created=findings_created+$4,findings_resolved=findings_resolved+$5,status=CASE WHEN $6 THEN 'completed' ELSE 'pending' END,completed_at=CASE WHEN $6 THEN clock_timestamp() ELSE completed_at END,truncated=$8,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$7")
            .bind(run.id).bind(next_cursor).bind(results.len() as i64).bind(created).bind(resolved).bind(done).bind(ws).bind(truncated).bind(skipped).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Keeps the first [`DRY_RUN_FAILURE_SAMPLE`] records a dry run flags, in
    /// the order it checks them, so its result can name them.
    async fn keep_dry_run_failures(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        run_id: Uuid,
        results: &[RuleCandidateResult],
    ) -> Result<(), RepositoryError> {
        let failed: Vec<&RuleCandidateResult> =
            results.iter().filter(|result| result.failed).collect();
        if failed.is_empty() {
            return Ok(());
        }
        let kept: i64 =
            sqlx::query_scalar("SELECT count(*) FROM rule_dry_run_failures WHERE run_id=$1")
                .bind(run_id)
                .fetch_one(&mut **tx)
                .await?;
        let room = usize::try_from(DRY_RUN_FAILURE_SAMPLE - kept).unwrap_or(0);
        let sample: Vec<&RuleCandidateResult> = failed.into_iter().take(room).collect();
        if sample.is_empty() {
            return Ok(());
        }
        sqlx::query("INSERT INTO rule_dry_run_failures(run_id,workspace_id,record_id,evaluation_key,message,evidence,position) SELECT $1,$2,f.record_id,f.evaluation_key,f.message,f.evidence,$3+f.ordinality-1 FROM unnest($4::uuid[],$5::text[],$6::text[],$7::jsonb[]) WITH ORDINALITY AS f(record_id,evaluation_key,message,evidence,ordinality) ON CONFLICT DO NOTHING")
            .bind(run_id)
            .bind(self.workspace_id.0)
            .bind(kept)
            .bind(sample.iter().map(|result| result.record_id).collect::<Vec<_>>())
            .bind(sample.iter().map(|result| result.evaluation_key.as_str()).collect::<Vec<_>>())
            .bind(sample.iter().map(|result| result.message.as_str()).collect::<Vec<_>>())
            .bind(sample.iter().map(|result| &result.evidence).collect::<Vec<_>>())
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// One run with, for a dry run, the records it would flag.
    pub async fn get_rule_run_details(
        &self,
        id: Uuid,
    ) -> Result<Option<RuleRunDetails>, RepositoryError> {
        let ws = self.workspace_id.0;
        let Some(run) = sqlx::query_as::<_, Db<RuleRun>>("SELECT id,rule_id,rule_version,definition_hash,source,dry_run,scope_record_id,status,candidate_cursor,candidates_evaluated,candidates_skipped,findings_created,findings_resolved,attempts,last_error,truncated,completed_at,created_at FROM rule_runs WHERE workspace_id=$1 AND id=$2")
            .bind(ws)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let dry_run_failures = sqlx::query_as::<_, Db<RuleDryRunFailure>>("SELECT record_id,message,evidence FROM rule_dry_run_failures WHERE workspace_id=$1 AND run_id=$2 ORDER BY position")
            .bind(ws)
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .into_domain();
        Ok(Some(RuleRunDetails {
            run: run.into_domain(),
            dry_run_failures,
        }))
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
        let fenced = self.for_task(task);
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
    definition_hash: String,
    compiled_plan: serde_json::Value,
    blueprint_id: Uuid,
    /// The rule is a rule of the event record's blueprint.
    direct: bool,
    /// The event record's blueprint code.
    event_blueprint: Option<String>,
}

impl AttricatRepository {
    /// Evaluates a rule for a candidate page of its blueprint's records with
    /// the shared predicate engine. A rule without a context checks every
    /// resolved context and fails when any of them fails. Records on a
    /// revision the rule does not fit are skipped.
    pub async fn evaluate_rule_candidates(
        &self,
        blueprint_id: Uuid,
        context_id: Option<Uuid>,
        rule: &CompiledRule,
        candidates: &[Uuid],
    ) -> Result<Vec<RuleCandidateResult>, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut conn = self.pool.acquire().await?;
        let revisions = super::checks::revision_attributes(&mut conn, ws, &[blueprint_id]).await?;
        let scope = super::checks::CheckScope::load(&mut conn, ws).await?;
        let contexts = context_id
            .map(|context| vec![context])
            .unwrap_or_else(|| scope.context_ids());
        let mut records = super::checks::load_current_records(&mut conn, ws, candidates).await?;
        let requirements =
            attricat_validation::predicate::Requirements::for_predicates([&rule.predicate]);
        let prefetch =
            super::checks::prefetch_related(&mut conn, &scope, &records, &contexts, &requirements)
                .await?;
        let severity = rule.severity.as_str();
        let mut results = Vec::with_capacity(candidates.len());
        for record_id in candidates {
            let Some(subject) = records.remove(record_id) else {
                continue;
            };
            let fits = revisions
                .get(&(subject.blueprint_id, subject.blueprint_version))
                .is_some_and(|attributes| attricat_rules::applicability(rule, attributes).is_ok());
            if !fits {
                results.push(RuleCandidateResult {
                    record_id: *record_id,
                    failed: false,
                    skipped: true,
                    message: String::new(),
                    evidence: serde_json::json!({}),
                    evaluation_key: rule.raw_definition_hash.clone(),
                    severity: severity.to_owned(),
                });
                continue;
            }
            let mut failure: Option<attricat_validation::predicate::Failure> = None;
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
                    attricat_validation::predicate::describe_predicate(&rule.predicate),
                    serde_json::json!({}),
                ),
            };
            if let Some(object) = evidence.as_object_mut() {
                object.insert("contexts".into(), serde_json::json!(failing_contexts));
            }
            results.push(RuleCandidateResult {
                record_id: *record_id,
                failed,
                skipped: false,
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

/// Decides the dry-run gate from the latest completed full dry run, given as
/// its violation count and whether it stopped at the candidate cap. A
/// truncated dry run missed candidates, so it counts only when existing
/// violations are accepted.
fn dry_run_verdict(
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

fn compile_rule(definition: &str) -> Result<CompiledRule, RepositoryError> {
    attricat_rules::compile(definition)
        .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))
}

/// A rule revision's stored compiled plan.
fn stored_rule(plan: serde_json::Value) -> Result<CompiledRule, RepositoryError> {
    serde_json::from_value(plan)
        .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))
}

/// The newest published revision of a blueprint, which a rule being saved
/// must fit.
async fn latest_published_revision(
    conn: &mut sqlx::PgConnection,
    workspace_id: Uuid,
    blueprint_id: Uuid,
) -> Result<i64, RepositoryError> {
    sqlx::query_scalar::<_, Option<i64>>(
        "SELECT max(version) FROM blueprints WHERE workspace_id=$1 AND id=$2 AND status=$3 AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(blueprint_id)
    .bind(RevisionStatus::Published.as_str())
    .fetch_one(conn)
    .await?
    .ok_or(RepositoryError::BlueprintNotPublished)
}

/// Checks a rule being saved against its blueprint's latest published
/// revision. Earlier revisions it does not fit are skipped, not refused.
async fn check_rule_fits(
    conn: &mut sqlx::PgConnection,
    workspace_id: Uuid,
    blueprint_id: Uuid,
    rule: &CompiledRule,
) -> Result<(), RepositoryError> {
    let version = latest_published_revision(conn, workspace_id, blueprint_id).await?;
    let attributes = super::checks::revision_attributes(conn, workspace_id, &[blueprint_id])
        .await?
        .remove(&(blueprint_id, version))
        .unwrap_or_default();
    attricat_rules::applicability(rule, &attributes)
        .map_err(|error| RepositoryError::InvalidRuleDefinition(error.message))
}

/// Adds to each rule the revisions of its blueprint it does not fit, among
/// the latest published revision and those with live records.
async fn with_applicability(
    conn: &mut sqlx::PgConnection,
    workspace_id: Uuid,
    rules: Vec<Rule>,
) -> Result<Vec<RuleDetails>, RepositoryError> {
    let mut blueprint_ids: Vec<Uuid> = rules.iter().map(|rule| rule.blueprint_id).collect();
    blueprint_ids.sort();
    blueprint_ids.dedup();
    let revisions: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT b.id, b.version FROM blueprints b
          WHERE b.workspace_id = $1 AND b.id = ANY($2) AND b.deleted_at IS NULL
            AND (EXISTS (SELECT 1 FROM records e
                          WHERE e.workspace_id = b.workspace_id AND e.blueprint_id = b.id
                            AND e.blueprint_version = b.version AND e.deleted_at IS NULL)
                 OR b.version = (SELECT max(p.version) FROM blueprints p
                                  WHERE p.workspace_id = b.workspace_id AND p.id = b.id
                                    AND p.status = $3 AND p.deleted_at IS NULL))
          ORDER BY b.id, b.version",
    )
    .bind(workspace_id)
    .bind(&blueprint_ids)
    .bind(RevisionStatus::Published.as_str())
    .fetch_all(&mut *conn)
    .await?;
    let attributes = super::checks::revision_attributes(conn, workspace_id, &blueprint_ids).await?;
    let rule_ids: Vec<Uuid> = rules.iter().map(|rule| rule.id).collect();
    let finding_counts: std::collections::HashMap<Uuid, (i64, i64)> =
        sqlx::query_as::<_, (Uuid, i64, i64)>(
            "SELECT rule_id, count(*) FILTER (WHERE state = $3), count(*) FILTER (WHERE state = $4)
           FROM rule_findings
          WHERE workspace_id = $1 AND rule_id = ANY($2) AND state IN ($3, $4)
          GROUP BY rule_id",
        )
        .bind(workspace_id)
        .bind(&rule_ids)
        .bind(RuleFindingState::Open.as_str())
        .bind(RuleFindingState::Acknowledged.as_str())
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(rule_id, open, acknowledged)| (rule_id, (open, acknowledged)))
        .collect();
    let none = attricat_rules::RevisionAttributes::default();
    rules
        .into_iter()
        .map(|rule| {
            let compiled = stored_rule(rule.compiled_plan.clone())?;
            let not_applicable = revisions
                .iter()
                .filter(|(blueprint_id, _)| *blueprint_id == rule.blueprint_id)
                .filter_map(|key| {
                    let revision = attributes.get(key).unwrap_or(&none);
                    attricat_rules::applicability(&compiled, revision)
                        .err()
                        .map(|error| RevisionNotApplicable {
                            blueprint_version: key.1,
                            reason: error.reason,
                            message: error.message,
                        })
                })
                .collect();
            let (open_findings, acknowledged_findings) =
                finding_counts.get(&rule.id).copied().unwrap_or_default();
            Ok(RuleDetails {
                rule,
                not_applicable,
                open_findings: Some(open_findings),
                acknowledged_findings: Some(acknowledged_findings),
            })
        })
        .collect()
}

/// Which findings a page lists. Every set field narrows it.
#[derive(Clone, Debug, Default)]
pub struct RuleFindingFilter {
    pub record_id: Option<Uuid>,
    pub rule_id: Option<Uuid>,
    /// Findings about records of this blueprint.
    pub blueprint_id: Option<Uuid>,
    /// Findings for this context; record-wide findings have none.
    pub context_id: Option<Uuid>,
    pub severity: Option<RuleFindingSeverity>,
    pub state: Option<RuleFindingState>,
    /// Leaves out resolved findings.
    pub unresolved: bool,
}

/// Finding severity codes in the order findings are listed. A code outside
/// them has no position and sorts last.
fn severities_most_severe_first() -> Vec<&'static str> {
    RuleFindingSeverity::ALL
        .iter()
        .rev()
        .map(|severity| severity.as_str())
        .collect()
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Brings every record's needs-attention tag in line with its findings,
    /// in pages. [`Self::initialize_workspace`] runs it at startup, which
    /// tags records whose findings predate the tag. Safe to run repeatedly.
    pub async fn reconcile_needs_attention_tags(
        &self,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        loop {
            let record_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT DISTINCT f.record_id FROM rule_findings f
                   JOIN records e ON e.workspace_id = f.workspace_id AND e.id = f.record_id
                  WHERE f.workspace_id = $1 AND f.state = $2
                    AND NOT e.system_tags @> ARRAY[$3]
                 UNION
                 SELECT e.id FROM records e
                  WHERE e.workspace_id = $1 AND e.system_tags @> ARRAY[$3]
                    AND NOT EXISTS (SELECT 1 FROM rule_findings f
                                     WHERE f.workspace_id = e.workspace_id
                                       AND f.record_id = e.id AND f.state = $2)
                 LIMIT $4",
            )
            .bind(workspace_id)
            .bind(RuleFindingState::Open.as_str())
            .bind(NEEDS_ATTENTION_SYSTEM_TAG)
            .bind(NEEDS_ATTENTION_RECONCILE_PAGE)
            .fetch_all(&self.pool)
            .await?;
            if record_ids.is_empty() {
                return Ok(());
            }
            let mut tx = self.pool.begin().await?;
            sync_needs_attention_tags(&mut tx, workspace_id, &record_ids).await?;
            tx.commit().await?;
        }
    }
}

/// Records [`AttricatRepository::reconcile_needs_attention_tags`] fixes per
/// transaction.
const NEEDS_ATTENTION_RECONCILE_PAGE: i64 = 1_000;

/// Adds or removes [`NEEDS_ATTENTION_SYSTEM_TAG`] on `record_ids` so it shows
/// whether each record has an open finding. Rows are locked in ID order so
/// concurrent rule pages cannot deadlock. `updated_at` stays: the tag follows
/// the findings and is not an edit.
///
/// The lock and the update are separate statements on purpose: under READ
/// COMMITTED a single statement evaluates the open-finding `EXISTS` against
/// its starting snapshot even after waiting for a newer row version, so a
/// concurrent run that committed a finding meanwhile would be missed and the
/// tag dropped. Locking first makes the second statement's fresh snapshot see
/// every finding committed by the transactions it waited for.
pub(super) async fn sync_needs_attention_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    record_ids: &[Uuid],
) -> Result<(), RepositoryError> {
    if record_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "SELECT e.id FROM records e
          WHERE e.workspace_id = $1 AND e.id = ANY($2)
          ORDER BY e.id
            FOR NO KEY UPDATE",
    )
    .bind(workspace_id)
    .bind(record_ids)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "WITH state AS (
             SELECT e.id, $3 = ANY(e.system_tags) AS tagged,
                    EXISTS (SELECT 1 FROM rule_findings f
                             WHERE f.workspace_id = e.workspace_id AND f.record_id = e.id
                               AND f.state = $4) AS open
               FROM records e
              WHERE e.workspace_id = $1 AND e.id = ANY($2))
         UPDATE records e
            SET system_tags = CASE WHEN state.open THEN array_append(e.system_tags, $3)
                                   ELSE array_remove(e.system_tags, $3) END
           FROM state
          WHERE e.workspace_id = $1 AND e.id = state.id AND state.tagged <> state.open",
    )
    .bind(workspace_id)
    .bind(record_ids)
    .bind(NEEDS_ATTENTION_SYSTEM_TAG)
    .bind(RuleFindingState::Open.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn findings_list_the_most_severe_first() {
        assert_eq!(
            severities_most_severe_first(),
            ["critical", "error", "warning", "info"]
        );
    }

    #[test]
    fn dry_run_gate_requires_a_complete_dry_run_or_accepted_violations() {
        assert!(matches!(
            dry_run_verdict(None, true),
            Err(RepositoryError::RuleDryRunRequired)
        ));
        assert!(dry_run_verdict(Some((0, false)), false).is_ok());
        assert!(matches!(
            dry_run_verdict(Some((3, false)), false),
            Err(RepositoryError::RuleHasExistingViolations(3))
        ));
        assert!(dry_run_verdict(Some((3, false)), true).is_ok());
        // A dry run that stopped at the candidate cap never proves the
        // remaining records pass.
        assert!(matches!(
            dry_run_verdict(Some((0, true)), false),
            Err(RepositoryError::RuleDryRunTruncated(0))
        ));
        assert!(matches!(
            dry_run_verdict(Some((3, true)), false),
            Err(RepositoryError::RuleDryRunTruncated(3))
        ));
        assert!(dry_run_verdict(Some((0, true)), true).is_ok());
    }
}
