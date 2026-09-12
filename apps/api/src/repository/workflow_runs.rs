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
    pub trigger_event_id: Uuid,
    pub trigger_sequence: i64,
    pub status: String,
    pub attempts: i32,
    pub failed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
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
        if event.aggregate_kind != "entity" || event.source_name.starts_with("workflow:") {
            return Ok(0);
        }
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, i64, Value)> = sqlx::query_as(
            "SELECT w.id, w.version, w.compiled_plan FROM workflows w JOIN workflow_lifecycles l ON l.workflow_id=w.id AND l.workspace_id=w.workspace_id WHERE w.workspace_id=$1 AND w.status='published' AND l.enabled_version=w.version AND $2 > COALESCE(l.activation_sequence, 0)"
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
            let result = sqlx::query("INSERT INTO workflow_runs (id, workspace_id, workflow_id, workflow_version, trigger_event_id, trigger_sequence, trigger_event, compiled_plan) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (workspace_id,workflow_id,workflow_version,trigger_event_id) DO NOTHING")
                .bind(Uuid::new_v4()).bind(ws).bind(workflow_id).bind(version).bind(event.id).bind(event.sequence).bind(&snapshot).bind(plan).execute(&mut *tx).await?;
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
        let row = sqlx::query_as::<_, (Uuid,Uuid,i64,Uuid,i64,String,i32,Option<DateTime<Utc>>,Option<DateTime<Utc>>,Option<String>,DateTime<Utc>,Value,Value)>(
            "WITH candidate AS (SELECT id FROM workflow_runs WHERE workspace_id=$1 AND ((status='pending' AND next_attempt_at<=clock_timestamp()) OR (status='leased' AND lease_until<=clock_timestamp())) ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT 1), leased AS (UPDATE workflow_runs r SET status='leased', attempts=attempts+1, lease_owner=$2, lease_until=clock_timestamp()+($3 * interval '1 millisecond'), last_error=NULL, updated_at=clock_timestamp() FROM candidate c WHERE r.id=c.id RETURNING r.*) SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,status,attempts,failed_at,completed_at,last_error,created_at,trigger_event,compiled_plan FROM leased"
        ).bind(ws).bind(owner).bind(lease.as_millis() as i64).fetch_optional(&self.pool).await?;
        Ok(row.map(|r| ClaimedWorkflowRun {
            run: WorkflowRun {
                id: r.0,
                workflow_id: r.1,
                workflow_version: r.2,
                trigger_event_id: r.3,
                trigger_sequence: r.4,
                status: r.5,
                attempts: r.6,
                failed_at: r.7,
                completed_at: r.8,
                last_error: r.9,
                created_at: r.10,
            },
            trigger_event: r.11,
            compiled_plan: r.12,
            lease_owner: owner.to_owned(),
        }))
    }
    pub(crate) async fn complete_workflow_run(
        &self,
        run: &ClaimedWorkflowRun,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE workflow_runs SET status='completed',completed_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2").bind(run.run.id).bind(&run.lease_owner).execute(&self.pool).await?;
        Ok(())
    }
    pub(crate) async fn retry_workflow_run(
        &self,
        run: &ClaimedWorkflowRun,
        error: &str,
        delay: Duration,
        max_attempts: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE workflow_runs SET status=CASE WHEN attempts >= $4 THEN 'dead_letter' ELSE 'pending' END,next_attempt_at=CASE WHEN attempts >= $4 THEN next_attempt_at ELSE clock_timestamp()+($3 * interval '1 millisecond') END,failed_at=clock_timestamp(),last_error=$2,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$5").bind(run.run.id).bind(error).bind(delay.as_millis() as i64).bind(max_attempts).bind(&run.lease_owner).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn list_workflow_runs(&self) -> Result<Vec<WorkflowRun>, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as("SELECT id,workflow_id,workflow_version,trigger_event_id,trigger_sequence,status,attempts,failed_at,completed_at,last_error,created_at FROM workflow_runs WHERE workspace_id=$1 ORDER BY created_at DESC").bind(ws).fetch_all(&self.pool).await?)
    }
    pub async fn replay_workflow_run(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query("UPDATE workflow_runs SET status='pending',attempts=0,next_attempt_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,failed_at=NULL,last_error=NULL,replayed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='dead_letter'").bind(id).bind(ws).execute(&self.pool).await?.rows_affected()==1)
    }
    pub(crate) async fn workflow_action_is_completed(
        &self,
        run_id: Uuid,
        index: i32,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workflow_run_actions WHERE run_id=$1 AND action_index=$2)",
        )
        .bind(run_id)
        .bind(index)
        .fetch_one(&self.pool)
        .await?)
    }
    pub(crate) async fn complete_workflow_action(
        &self,
        run_id: Uuid,
        index: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO workflow_run_actions(run_id,action_index) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(run_id).bind(index).execute(&self.pool).await?;
        Ok(())
    }
}

fn workflow_trigger_matches(trigger: &catalog_workflow::Trigger, event: &DomainEvent) -> bool {
    trigger.event_type == event.event_type
        && trigger
            .envelope
            .iter()
            .all(|(key, expected)| match key.as_str() {
                "event_type" => expected == &Value::String(event.event_type.clone()),
                "aggregate_kind" => expected == &Value::String(event.aggregate_kind.clone()),
                "source_kind" => expected == &Value::String(event.source_kind.clone()),
                "source_name" => expected == &Value::String(event.source_name.clone()),
                _ => value_path(&event.metadata, key).is_some_and(|actual| actual == expected),
            })
        && trigger.facts.iter().all(|(key, expected)| {
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
        let trigger = catalog_workflow::Trigger {
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
            ..event
        };
        assert!(!workflow_trigger_matches(&trigger, &different));
    }
}
