use super::*;
use crate::{
    domain_events::DomainEvent,
    task_queue::{TaskInsert, TaskKind},
};
use chrono::{DateTime, Utc};
use serde_json::Value;
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
pub enum WorkflowActionResult {
    Executed,
    AlreadyCompleted,
    Cancelled,
}

pub struct ClaimedWorkflowRun {
    pub run: WorkflowRun,
    pub trigger_event: Value,
    pub compiled_plan: Value,
}

#[derive(sqlx::FromRow)]
struct WorkflowRunRow {
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

impl From<WorkflowRunRow> for ClaimedWorkflowRun {
    fn from(row: WorkflowRunRow) -> Self {
        Self {
            run: WorkflowRun {
                id: row.id,
                workflow_id: row.workflow_id,
                workflow_version: row.workflow_version,
                trigger_event_id: row.trigger_event_id,
                trigger_sequence: row.trigger_sequence,
                source: row.source,
                status: row.status,
                attempts: row.attempts,
                failed_at: row.failed_at,
                completed_at: row.completed_at,
                last_error: row.last_error,
                created_at: row.created_at,
                cancelled_at: row.cancelled_at,
                root_trigger_event_id: row.root_trigger_event_id,
                causal_depth: row.causal_depth,
            },
            trigger_event: row.trigger_event,
            compiled_plan: row.compiled_plan,
        }
    }
}

impl CatalogRepository {
    async fn enqueue_workflow_task(
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
                kind: TaskKind::WorkflowRunV1,
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

    /// Fan-out creates the immutable run and its delivery envelope in one
    /// transaction. The outbox handler is therefore only a producer.
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
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, i64, Value)> = sqlx::query_as("SELECT w.id, w.version, w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.status='published' AND l.enabled_version=w.version AND $2 > COALESCE(l.activation_sequence, 0) FOR SHARE OF l")
            .bind(ws).bind(event.sequence).fetch_all(&mut *tx).await?;
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
            let id = Uuid::new_v4();
            let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO workflow_runs (id, workspace_id, workflow_id, workflow_version, trigger_event_id, trigger_sequence, trigger_event, compiled_plan, root_trigger_event_id, causal_depth) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (workspace_id,workflow_id,workflow_version,trigger_event_id) DO NOTHING RETURNING id")
                .bind(id).bind(ws).bind(workflow_id).bind(version).bind(event.id).bind(event.sequence).bind(&snapshot).bind(plan).bind(root_trigger_event_id).bind(causal_depth).fetch_optional(&mut *tx).await?;
            if let Some(run_id) = run_id {
                self.enqueue_workflow_task(
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

    /// Loads only the domain row named by the current workflow task. A task is
    /// never allowed to execute a cross-workspace or mismatched subject.
    pub async fn workflow_run_for_task(
        &self,
        task: &ClaimedTask,
    ) -> Result<ClaimedWorkflowRun, RepositoryError> {
        let ws = self.workspace_id.0;
        if task.kind != TaskKind::WorkflowRunV1 || task.workspace_id != ws {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "workflow task workspace or kind mismatch".into(),
            ));
        }
        let row = sqlx::query_as::<_, WorkflowRunRow>("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth,trigger_event,compiled_plan FROM workflow_runs WHERE id=$1 AND workspace_id=$2")
            .bind(task.subject_id).bind(ws).fetch_optional(&self.pool).await?;
        row.map(Into::into).ok_or_else(|| {
            RepositoryError::InvalidWorkflowDefinition("workflow task subject is missing".into())
        })
    }

    /// The run completion is a domain write, so it validates and locks the
    /// current task token in the same transaction before acknowledging it.
    pub async fn complete_workflow_run_task(
        &self,
        task: &ClaimedTask,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let fenced = self.for_workflow_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        sqlx::query("UPDATE workflow_runs SET status='completed',completed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='pending'")
            .bind(task.subject_id).bind(ws).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn dead_letter_workflow_run_task(
        &self,
        task: &ClaimedTask,
        error: &str,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let fenced = self.for_workflow_task(task);
        fenced.ensure_task_fence(&mut tx).await?;
        let run_changed = sqlx::query("UPDATE workflow_runs SET status='dead_letter',failed_at=clock_timestamp(),last_error=$3,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='pending'")
            .bind(task.subject_id).bind(ws).bind(error).execute(&mut *tx).await?.rows_affected();
        if run_changed != 1 {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "workflow run is not available for its task".into(),
            ));
        }
        let changed = sqlx::query("UPDATE tasks SET status='dead_letter',failures=max_failures,lease_owner=NULL,lease_token=NULL,lease_until=NULL,last_error_code='workflow_run',last_error_message=$4,failed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>clock_timestamp()")
            .bind(task.id).bind(&task.lease_owner).bind(task.lease_token).bind(error).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(crate::repository::RepositoryError::Task(
                crate::repository::TaskError::LeaseLost,
            ));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_workflow_runs(&self) -> Result<Vec<WorkflowRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        Ok(sqlx::query_as("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth FROM workflow_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?)
    }

    pub async fn get_workflow_run(&self, id: Uuid) -> Result<Option<WorkflowRun>, RepositoryError> {
        let ws = self.workspace_id.0;
        sqlx::query_as("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth FROM workflow_runs WHERE workspace_id=$1 AND id=$2")
            .bind(ws).bind(id).fetch_optional(&self.pool).await.map_err(Into::into)
    }

    pub async fn workflow_runs_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<WorkflowRun>, bool), RepositoryError> {
        let ws = self.workspace_id.0;
        let mut rows = sqlx::query_as("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,source,status,attempts,failed_at,completed_at,last_error,created_at,cancelled_at,root_trigger_event_id,causal_depth FROM workflow_runs WHERE workspace_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3")
            .bind(ws).bind(limit + 1).bind(offset).fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows, has_more))
    }

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
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let row: Option<(i64, Value)> = sqlx::query_as("SELECT w.version,w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.id=$2 AND w.status='published' AND l.enabled_version=w.version FOR SHARE OF l").bind(ws).bind(workflow_id).fetch_optional(&mut *tx).await?;
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
        let snapshot = serde_json::to_value(&event).expect("domain event serializes");
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO workflow_runs(id,workspace_id,workflow_id,workflow_version,trigger_event,compiled_plan,source,idempotency_key,causal_depth) VALUES($1,$2,$3,$4,$5,$6,'manual',$7,0) ON CONFLICT (workspace_id,workflow_id,workflow_version,source,idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING RETURNING id")
            .bind(id).bind(ws).bind(workflow_id).bind(version).bind(snapshot).bind(plan).bind(idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some(run_id) = inserted {
            self.enqueue_workflow_task(&mut tx, ws, run_id, Some(event.correlation_id), None)
                .await?;
            self.commit_mutation(tx).await?;
            Ok(run_id)
        } else {
            let run_id: Uuid = sqlx::query_scalar("SELECT id FROM workflow_runs WHERE workspace_id=$1 AND workflow_id=$2 AND workflow_version=$3 AND source='manual' AND idempotency_key=$4").bind(ws).bind(workflow_id).bind(version).bind(idempotency_key).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(run_id)
        }
    }

    /// Advances schedule cursors; it is a producer only. Each created run and
    /// task envelope is committed together, while active task/run state keeps
    /// the existing no-overlap semantics.
    pub async fn schedule_workflow_runs(&self) -> Result<u64, RepositoryError> {
        use chrono::Duration as ChronoDuration;
        let ws = self.workspace_id.0;
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
                let schedule = catalog_workflow::parse_six_field_cron(cron).map_err(|_| {
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
                }
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
                    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflow_runs WHERE workspace_id=$1 AND workflow_id=$2 AND status='pending')").bind(ws).bind(workflow_id).fetch_one(&mut *tx).await?;
                    if missed || active {
                        skipped += 1;
                    } else {
                        let id = Uuid::new_v4();
                        let event = synthetic_trigger(id, ws, target, "schedule", None);
                        let snapshot =
                            serde_json::to_value(&event).expect("domain event serializes");
                        let run_id: Option<Uuid> = sqlx::query_scalar("INSERT INTO workflow_runs(id,workspace_id,workflow_id,workflow_version,trigger_event,compiled_plan,source,idempotency_key,causal_depth) VALUES($1,$2,$3,$4,$5,$6,'schedule',$7,0) ON CONFLICT (workspace_id,workflow_id,workflow_version,source,idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING RETURNING id").bind(id).bind(ws).bind(workflow_id).bind(version).bind(snapshot).bind(plan.clone()).bind(format!("{index}:{}", occurrence_due.to_rfc3339())).fetch_optional(&mut *tx).await?;
                        if let Some(run_id) = run_id {
                            self.enqueue_workflow_task(
                                &mut tx,
                                ws,
                                run_id,
                                Some(event.correlation_id),
                                None,
                            )
                            .await?;
                            created += 1;
                        }
                    }
                    occurrence_due = next;
                }
                sqlx::query("UPDATE workflow_schedule_states SET next_run_at=$5,misfires=misfires+$6,updated_at=clock_timestamp() WHERE workspace_id=$1 AND workflow_id=$2 AND workflow_version=$3 AND trigger_index=$4").bind(ws).bind(workflow_id).bind(version).bind(index as i32).bind(occurrence_due).bind(skipped).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(created)
    }

    /// Backfills envelopes for pre-cutover pending rows. No legacy claimer is
    /// started in this binary, so normalizing stale leases is safe and keeps a
    /// restart from stranding work created before the cutover.
    pub async fn backfill_workflow_tasks(&self) -> Result<u64, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE workflow_runs SET status='pending',lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND status='leased'").bind(ws).execute(&mut *tx).await?;
        let rows: Vec<(Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as("SELECT id,trigger_event_id,trigger_event_id FROM workflow_runs WHERE workspace_id=$1 AND status='pending' FOR UPDATE").bind(ws).fetch_all(&mut *tx).await?;
        let count = rows.len() as u64;
        for (run_id, correlation, causation) in rows {
            self.enqueue_workflow_task(&mut tx, ws, run_id, correlation, causation)
                .await?;
        }
        tx.commit().await?;
        Ok(count)
    }

    pub async fn replay_workflow_run(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query("UPDATE workflow_runs SET status='pending',attempts=0,failed_at=NULL,last_error=NULL,replayed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='dead_letter'").bind(id).bind(ws).execute(&mut *tx).await?.rows_affected();
        if updated == 0 {
            tx.commit().await?;
            return Ok(false);
        }
        let task_id: Uuid = sqlx::query_scalar("SELECT id FROM tasks WHERE workspace_id=$1 AND kind='workflow_run.v1' AND subject_id=$2 AND status='dead_letter' ORDER BY generation DESC LIMIT 1 FOR UPDATE").bind(ws).bind(id).fetch_one(&mut *tx).await?;
        self.replay_task(&mut tx, task_id).await?;
        tx.commit().await?;
        Ok(true)
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
