use super::*;
use crate::domain_events::DomainEvent;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub workflow_version: i64,
    pub trigger_event_id: Option<Uuid>,
    pub trigger_sequence: Option<i64>,
    pub source: String,
    pub status: String,
    pub attempts: i32,
    pub failed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub root_trigger_event_id: Option<Uuid>,
    pub causal_depth: i32,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkflowActionResult {
    Executed,
    AlreadyCompleted,
    Cancelled,
}

#[derive(sqlx::FromRow)]
struct ClaimedWorkflowRunRow {
    id: Uuid,
    workflow_id: Uuid,
    workflow_version: i64,
    trigger_event_id: Option<Uuid>,
    trigger_sequence: Option<i64>,
    source: String,
    status: String,
    attempts: i32,
    failed_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    cancelled_at: Option<DateTime<Utc>>,
    root_trigger_event_id: Option<Uuid>,
    causal_depth: i32,
    trigger_event: Value,
    compiled_plan: Value,
}

pub(crate) struct ClaimedWorkflowRun {
    pub run: WorkflowRun,
    pub trigger_event: Value,
    pub compiled_plan: Value,
    pub(crate) lease_owner: String,
}

impl CatalogRepository {
    /// Fan-out is deliberately limited to durable intake. No action is executed
    /// on the shared domain-event dispatcher lease.
    pub async fn fan_out_workflow_runs(&self, event: &DomainEvent) -> Result<u64, RepositoryError> {
        if event.aggregate_kind != "entity"
            || event.source_name.starts_with("workflow:")
            || event
                .metadata
                .get("workflow_causal_depth")
                .and_then(Value::as_u64)
                .is_some_and(|depth| depth > 8)
        {
            return Ok(0);
        }
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, i64, Value)> = sqlx::query_as(
            // Hold a shared lifecycle lock through insertion. disable/enable
            // acquire the conflicting row lock, so a run selected before a
            // disable cannot be committed after that disable.
            "SELECT w.id, w.version, w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.status='published' AND l.enabled_version=w.version AND $2 > COALESCE(l.activation_sequence, 0) FOR SHARE OF l"
        ).bind(ws).bind(event.sequence).fetch_all(&mut *tx).await?;
        let snapshot = serde_json::to_value(event).expect("domain event serializes");
        let mut inserted = 0;
        for (workflow_id, version, plan) in rows {
            let compiled: catalog_workflow::CompiledWorkflow = serde_json::from_value(plan.clone())
                .map_err(|e| {
                    RepositoryError::InvalidWorkflowDefinition(format!(
                        "stored compiled workflow is invalid: {e}"
                    ))
                })?;
            if !compiled
                .triggers
                .iter()
                .any(|trigger| workflow_trigger_matches(trigger, event))
            {
                continue;
            }
            let root_trigger_event_id = event
                .metadata
                .get("workflow_root_trigger_event_id")
                .and_then(Value::as_str)
                .and_then(|id| id.parse::<Uuid>().ok())
                .unwrap_or(event.id);
            let causal_depth = event
                .metadata
                .get("workflow_causal_depth")
                .and_then(Value::as_i64)
                .filter(|depth| (0..=8).contains(depth))
                .unwrap_or(0) as i32;
            let result = sqlx::query("INSERT INTO workflow_runs (id, workspace_id, workflow_id, workflow_version, trigger_event_id, trigger_sequence, trigger_event, compiled_plan, root_trigger_event_id, causal_depth) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (workspace_id,workflow_id,workflow_version,trigger_event_id) DO NOTHING")
                .bind(Uuid::new_v4()).bind(ws).bind(workflow_id).bind(version).bind(event.id).bind(event.sequence).bind(&snapshot).bind(plan).bind(root_trigger_event_id).bind(causal_depth).execute(&mut *tx).await?;
            inserted += result.rows_affected();
        }
        tx.commit().await?;
        Ok(inserted)
    }

    pub(crate) async fn claim_workflow_run(
        &self,
        owner: &str,
        lease: Duration,
    ) -> Result<Option<ClaimedWorkflowRun>, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let row = sqlx::query_as::<_, ClaimedWorkflowRunRow>(
            "WITH candidate AS (SELECT id FROM workflow_runs WHERE workspace_id=$1 AND ((status='pending' AND next_attempt_at<=clock_timestamp()) OR (status='leased' AND lease_until<=clock_timestamp())) ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT 1), leased AS (UPDATE workflow_runs r SET status='leased', attempts=attempts+1, lease_owner=$2, lease_until=clock_timestamp()+($3 * interval '1 millisecond'), last_error=NULL, updated_at=clock_timestamp() FROM candidate c WHERE r.id=c.id RETURNING r.*) SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth,trigger_event,compiled_plan FROM leased"
        ).bind(ws).bind(owner).bind(lease.as_millis() as i64).fetch_optional(&self.pool).await?;
        Ok(row.map(|r| ClaimedWorkflowRun {
            run: WorkflowRun {
                id: r.id,
                workflow_id: r.workflow_id,
                workflow_version: r.workflow_version,
                trigger_event_id: r.trigger_event_id,
                trigger_sequence: r.trigger_sequence,
                source: r.source,
                status: r.status,
                attempts: r.attempts,
                failed_at: r.failed_at,
                completed_at: r.completed_at,
                last_error: r.last_error,
                created_at: r.created_at,
                cancelled_at: r.cancelled_at,
                root_trigger_event_id: r.root_trigger_event_id,
                causal_depth: r.causal_depth,
            },
            trigger_event: r.trigger_event,
            compiled_plan: r.compiled_plan,
            lease_owner: owner.to_owned(),
        }))
    }
    pub(crate) async fn complete_workflow_run(
        &self,
        run: &ClaimedWorkflowRun,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE workflow_runs SET status='completed',completed_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2").bind(run.run.id).bind(&run.lease_owner).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            let cancelled: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM workflow_runs WHERE id=$1 AND status='cancelled')",
            )
            .bind(run.run.id)
            .fetch_one(&self.pool)
            .await?;
            if !cancelled {
                return Err(RepositoryError::InvalidWorkflowDefinition(
                    "workflow run lease was lost".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) async fn retry_workflow_run(
        &self,
        run: &ClaimedWorkflowRun,
        error: &str,
        delay: Duration,
        max_attempts: i32,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE workflow_runs SET status=CASE WHEN attempts >= $4 THEN 'dead_letter' ELSE 'pending' END,next_attempt_at=CASE WHEN attempts >= $4 THEN next_attempt_at ELSE clock_timestamp()+($3 * interval '1 millisecond') END,failed_at=clock_timestamp(),last_error=$2,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$5").bind(run.run.id).bind(error).bind(delay.as_millis() as i64).bind(max_attempts).bind(&run.lease_owner).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "workflow run lease was lost or cancelled".into(),
            ));
        }
        Ok(())
    }
    pub async fn list_workflow_runs(&self) -> Result<Vec<WorkflowRun>, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth FROM workflow_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?)
    }
    /// Create a durable manual run only for the exact currently enabled revision.
    /// The management route cannot select a draft or historical revision.
    pub async fn create_manual_workflow_run(
        &self,
        workflow_id: Uuid,
        entity_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Uuid, RepositoryError> {
        if idempotency_key.is_empty() || idempotency_key.len() > 128 || !idempotency_key.is_ascii()
        {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "manual idempotency_key must be 1-128 ASCII bytes".into(),
            ));
        }
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let row: Option<(i64, Value)> = sqlx::query_as(
            "SELECT w.version,w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.id=$2 AND w.status='published' AND l.enabled_version=w.version FOR SHARE OF l",
        ).bind(ws).bind(workflow_id).fetch_optional(&mut *tx).await?;
        let Some((version, plan)) = row else {
            return Err(RepositoryError::WorkflowNotPublished);
        };
        let compiled: catalog_workflow::CompiledWorkflow = serde_json::from_value(plan.clone())
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        if !compiled
            .triggers
            .iter()
            .any(|trigger| matches!(trigger, catalog_workflow::Trigger::Manual))
        {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "enabled revision does not allow manual runs".into(),
            ));
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM entities WHERE id=$1 AND workspace_id=$2 AND deleted_at IS NULL)").bind(entity_id).bind(ws).fetch_one(&mut *tx).await?;
        if !exists {
            return Err(RepositoryError::NotFound("entity"));
        }
        let id = Uuid::new_v4();
        let event = synthetic_trigger(id, ws, entity_id, "manual", self.audit_context.as_ref());
        let snapshot = serde_json::to_value(event).expect("domain event serializes");
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO workflow_runs(id,workspace_id,workflow_id,workflow_version,trigger_event,compiled_plan,source,idempotency_key,causal_depth) VALUES($1,$2,$3,$4,$5,$6,'manual',$7,0) ON CONFLICT (workspace_id,workflow_id,workflow_version,source,idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING RETURNING id")
            .bind(id).bind(ws).bind(workflow_id).bind(version).bind(snapshot).bind(plan).bind(idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some(run_id) = inserted {
            self.commit_mutation(tx).await?;
            Ok(run_id)
        } else {
            // The unique key is the durable retry boundary; do not audit or enqueue a second run.
            let run_id: Uuid = sqlx::query_scalar("SELECT id FROM workflow_runs WHERE workspace_id=$1 AND workflow_id=$2 AND workflow_version=$3 AND source='manual' AND idempotency_key=$4")
                .bind(ws).bind(workflow_id).bind(version).bind(idempotency_key).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(run_id)
        }
    }

    /// Advances durable UTC schedule cursors. Misfires older than five minutes are recorded and
    /// skipped; a workflow with an active run does not overlap and skips that occurrence.
    pub async fn schedule_workflow_runs(&self) -> Result<u64, RepositoryError> {
        use chrono::Duration as ChronoDuration;
        use cron::Schedule;
        use std::str::FromStr;
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;
        let mut created = 0;
        let rows: Vec<(Uuid,i64,Value)>=sqlx::query_as("SELECT w.id,w.version,w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.status='published' AND l.enabled_version=w.version FOR SHARE OF l").bind(ws).fetch_all(&mut *tx).await?;
        for (workflow_id, version, plan) in rows {
            let compiled: catalog_workflow::CompiledWorkflow = serde_json::from_value(plan.clone())
                .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
            for (index, trigger) in compiled.triggers.iter().enumerate() {
                let catalog_workflow::Trigger::Schedule {
                    cron,
                    target_entity_id,
                    ..
                } = trigger
                else {
                    continue;
                };
                let schedule = Schedule::from_str(cron).map_err(|_| {
                    RepositoryError::InvalidWorkflowDefinition(
                        "stored schedule cron is invalid".into(),
                    )
                })?;
                let target = target_entity_id.parse::<Uuid>().map_err(|_| {
                    RepositoryError::InvalidWorkflowDefinition(
                        "stored schedule target is invalid".into(),
                    )
                })?;
                let state:Option<(DateTime<Utc>,)>=sqlx::query_as("SELECT next_run_at FROM workflow_schedule_states WHERE workspace_id=$1 AND workflow_id=$2 AND workflow_version=$3 AND trigger_index=$4 FOR UPDATE").bind(ws).bind(workflow_id).bind(version).bind(index as i32).fetch_optional(&mut *tx).await?;
                let Some((due,)) = state else {
                    let next = schedule.after(&now).next().ok_or_else(|| {
                        RepositoryError::InvalidWorkflowDefinition(
                            "schedule has no future occurrence".into(),
                        )
                    })?;
                    sqlx::query("INSERT INTO workflow_schedule_states(workspace_id,workflow_id,workflow_version,trigger_index,next_run_at) VALUES($1,$2,$3,$4,$5)").bind(ws).bind(workflow_id).bind(version).bind(index as i32).bind(next).execute(&mut *tx).await?;
                    continue;
                };
                if due > now {
                    continue;
                };
                // Advance occurrence-by-occurrence, rather than jumping from an old
                // cursor to now. This makes every overlap/misfire observable. The
                // compiler's six-field cron admits a one-second cadence, so cap a
                // single transaction at five minutes' worth plus a small margin.
                let mut occurrence_due = due;
                let mut skipped = 0_i64;
                for _ in 0..512 {
                    if occurrence_due > now {
                        break;
                    }
                    let next = schedule.after(&occurrence_due).next().ok_or_else(|| {
                        RepositoryError::InvalidWorkflowDefinition(
                            "schedule has no future occurrence".into(),
                        )
                    })?;
                    let missed = occurrence_due < now - ChronoDuration::minutes(5);
                    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_runs WHERE workspace_id=$1 AND workflow_id=$2 AND status IN ('pending','leased'))")
                        .bind(ws).bind(workflow_id).fetch_one(&mut *tx).await?;
                    if missed || active {
                        skipped += 1;
                    } else {
                        let id = Uuid::new_v4();
                        let event = synthetic_trigger(id, ws, target, "schedule", None);
                        let snapshot =
                            serde_json::to_value(event).expect("domain event serializes");
                        let result = sqlx::query("INSERT INTO workflow_runs(id,workspace_id,workflow_id,workflow_version,trigger_event,compiled_plan,source,idempotency_key,causal_depth) VALUES($1,$2,$3,$4,$5,$6,'schedule',$7,0) ON CONFLICT (workspace_id,workflow_id,workflow_version,source,idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING")
                            .bind(id).bind(ws).bind(workflow_id).bind(version).bind(snapshot).bind(plan.clone()).bind(format!("{index}:{}", occurrence_due.to_rfc3339())).execute(&mut *tx).await?;
                        created += result.rows_affected();
                    }
                    occurrence_due = next;
                }
                sqlx::query("UPDATE workflow_schedule_states SET next_run_at=$5,misfires=misfires+$6,updated_at=clock_timestamp() WHERE workspace_id=$1 AND workflow_id=$2 AND workflow_version=$3 AND trigger_index=$4")
                    .bind(ws).bind(workflow_id).bind(version).bind(index as i32).bind(occurrence_due).bind(skipped).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(created)
    }

    pub async fn replay_workflow_run(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query("UPDATE workflow_runs SET status='pending',attempts=0,next_attempt_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,failed_at=NULL,last_error=NULL,replayed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='dead_letter'").bind(id).bind(ws).execute(&self.pool).await?.rows_affected()==1)
    }
}

fn synthetic_trigger(
    id: Uuid,
    workspace_id: Uuid,
    entity_id: Uuid,
    source: &str,
    audit: Option<&AuditContext>,
) -> DomainEvent {
    let mut metadata = serde_json::json!({"workflow_synthetic": true});
    if let Some(actor) = audit.and_then(|context| context.actor_user_id) {
        metadata["initiating_actor_user_id"] = Value::String(actor.to_string());
    }
    if let Some(token) = audit.and_then(|context| context.actor_token_id) {
        metadata["initiating_actor_token_id"] = Value::String(token.to_string());
    }
    DomainEvent {
        id,
        sequence: 0,
        workspace_id,
        occurred_at: Utc::now(),
        event_type: format!("workflow.{source}.v1"),
        aggregate_kind: "entity".into(),
        aggregate_id: entity_id,
        correlation_id: id,
        causation_id: None,
        source_kind: "workflow".into(),
        source_name: format!("workflow:{source}"),
        metadata,
        payload: serde_json::json!({"facts": []}),
    }
}

fn workflow_trigger_matches(trigger: &catalog_workflow::Trigger, event: &DomainEvent) -> bool {
    let catalog_workflow::Trigger::Event {
        event_type,
        envelope,
        facts,
    } = trigger
    else {
        return false;
    };
    event_type == &event.event_type
        && envelope.iter().all(|(key, expected)| match key.as_str() {
            "event_type" => expected == &Value::String(event.event_type.clone()),
            "aggregate_kind" => expected == &Value::String(event.aggregate_kind.clone()),
            "source_kind" => expected == &Value::String(event.source_kind.clone()),
            "source_name" => expected == &Value::String(event.source_name.clone()),
            _ => key
                .strip_prefix("metadata.")
                .and_then(|path| value_path(&event.metadata, path))
                .is_some_and(|actual| actual == expected),
        })
        && facts.iter().all(|(key, expected)| {
            value_path(&event.payload, key).is_some_and(|actual| actual == expected)
        })
}
fn value_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |v, key| match v {
        Value::Object(_) => v.get(key),
        Value::Array(values) => key
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn matching_uses_exact_event_and_immutable_facts() {
        let trigger = catalog_workflow::Trigger::Event {
            event_type: "entity.updated.v1".into(),
            envelope: [("source_kind".into(), json!("api"))].into(),
            facts: [("facts.0.attribute_code".into(), json!("title"))].into(),
        };
        let event = DomainEvent {
            id: Uuid::new_v4(),
            sequence: 1,
            workspace_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            event_type: "entity.updated.v1".into(),
            aggregate_kind: "entity".into(),
            aggregate_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            causation_id: None,
            source_kind: "api".into(),
            source_name: "catalog_api".into(),
            metadata: json!({}),
            payload: json!({"facts":[{"attribute_code":"title"}]}),
        };
        assert!(workflow_trigger_matches(&trigger, &event));
        let different = DomainEvent {
            source_kind: "worker".into(),
            ..event.clone()
        };
        assert!(!workflow_trigger_matches(&trigger, &different));

        let metadata_trigger = catalog_workflow::Trigger::Event {
            event_type: "entity.updated.v1".into(),
            envelope: [("metadata.tenant_hint".into(), json!("north"))].into(),
            facts: Default::default(),
        };
        assert!(!workflow_trigger_matches(&metadata_trigger, &event));
        let metadata_event = DomainEvent {
            metadata: json!({"tenant_hint":"north"}),
            ..event
        };
        assert!(workflow_trigger_matches(&metadata_trigger, &metadata_event));
    }
}
