use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};
use crate::task_queue::{
    ParseTaskKindError, TaskInsert, TaskKind, TaskStatus, TaskValidationError,
};

/// Workspace-scoped aggregate diagnostics; never includes task payloads or errors.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct BackgroundProcessingStatus {
    pub kind: String,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
    pub expired_leases: i64,
    pub oldest_due_seconds: Option<f64>,
}

const MAX_LEASE_OWNER_BYTES: usize = 128;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_ERROR_MESSAGE_BYTES: usize = 1024;

fn lease_seconds(duration: Duration) -> Result<i64, TaskError> {
    if duration.is_zero() {
        return Err(TaskError::ValueTooLong);
    }
    let seconds = duration
        .as_secs()
        .checked_add(u64::from(duration.subsec_nanos() != 0))
        .ok_or(TaskError::ValueTooLong)?;
    seconds.try_into().map_err(|_| TaskError::ValueTooLong)
}

#[derive(Debug, Error)]
pub enum TaskError {
    #[error(transparent)]
    Validation(#[from] TaskValidationError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    UnknownKind(#[from] ParseTaskKindError),
    #[error("task lease was lost or task is no longer leased")]
    LeaseLost,
    #[error("task value is too long")]
    ValueTooLong,
    #[error("task kind does not permit this operation")]
    OperationNotPermitted,
}

#[derive(Debug, Clone)]
pub struct ClaimedTask {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: TaskKind,
    pub subject_id: Uuid,
    pub generation: i32,
    pub payload: Value,
    pub attempts: i32,
    pub failures: i32,
    pub max_failures: i32,
    pub lease_owner: String,
    pub lease_token: Uuid,
    pub lease_until: DateTime<Utc>,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
}

/// Payload-free diagnostic record. Operator surfaces must use this instead of
/// `ClaimedTask`, so task envelopes cannot accidentally expose domain data.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskSummary {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub subject_id: Uuid,
    pub generation: i32,
    pub status: String,
    pub attempts: i32,
    pub failures: i32,
    pub max_failures: i32,
    pub available_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReplayableTaskRow {
    workspace_id: Uuid,
    kind: String,
    subject_id: Uuid,
    generation: i32,
    payload: Value,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct ClaimedTaskRow {
    id: Uuid,
    workspace_id: Uuid,
    kind: String,
    subject_id: Uuid,
    generation: i32,
    payload: Value,
    attempts: i32,
    failures: i32,
    max_failures: i32,
    lease_owner: String,
    lease_token: Uuid,
    lease_until: DateTime<Utc>,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
}

impl TryFrom<ClaimedTaskRow> for ClaimedTask {
    type Error = TaskError;

    fn try_from(row: ClaimedTaskRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            workspace_id: row.workspace_id,
            kind: TaskKind::from_str(&row.kind)?,
            subject_id: row.subject_id,
            generation: row.generation,
            payload: row.payload,
            attempts: row.attempts,
            failures: row.failures,
            max_failures: row.max_failures,
            lease_owner: row.lease_owner,
            lease_token: row.lease_token,
            lease_until: row.lease_until,
            correlation_id: row.correlation_id,
            causation_id: row.causation_id,
        })
    }
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    /// Inserts a delivery envelope in the caller's domain transaction. A
    /// conflict means that exact subject generation was already committed.
    pub async fn enqueue_task(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        task: TaskInsert,
    ) -> Result<Option<Uuid>, TaskError> {
        task.validate()?;
        let id = Uuid::new_v4();
        let policy = task.kind.policy();
        let inserted = sqlx::query_scalar(
            "INSERT INTO tasks (id, workspace_id, kind, envelope_version, subject_id, generation, payload, max_failures, correlation_id, causation_id) VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9) ON CONFLICT (workspace_id, kind, subject_id, generation) DO NOTHING RETURNING id",
        )
        .bind(id)
        .bind(task.workspace_id)
        .bind(task.kind.as_str())
        .bind(task.subject_id)
        .bind(task.generation)
        .bind(task.payload)
        .bind(policy.max_failures)
        .bind(task.correlation_id)
        .bind(task.causation_id)
        .fetch_optional(&mut **transaction)
        .await?;
        Ok(inserted)
    }

    /// Claims one due envelope with an exclusive, fresh token. The service row
    /// is locked with the task, so parallel workers select least-recently-
    /// served workspaces rather than merely globally-oldest tasks.
    pub async fn claim_task(
        &self,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<Option<ClaimedTask>, TaskError> {
        self.claim_task_for_kinds(worker_id, lease_duration, &TaskKind::ALL)
            .await
    }

    /// Claims only registered kinds. A runtime with no active handler must not
    /// lease work it cannot safely execute during a phased cutover.
    pub async fn claim_task_for_kinds(
        &self,
        worker_id: &str,
        lease_duration: Duration,
        kinds: &[TaskKind],
    ) -> Result<Option<ClaimedTask>, TaskError> {
        if kinds.is_empty() {
            return Ok(None);
        }
        if worker_id.is_empty() || worker_id.len() > MAX_LEASE_OWNER_BYTES {
            return Err(TaskError::ValueTooLong);
        }
        let lease_seconds = lease_seconds(lease_duration)?;
        let kinds: Vec<String> = kinds.iter().map(|kind| kind.to_string()).collect();
        let mut transaction = self.pool.begin().await?;
        // Materializing service rows is safe to repeat and happens before the
        // fairness lock. It includes expired leases because they are eligible
        // for reclaim in the same claim operation.
        sqlx::query("INSERT INTO task_workspace_service (workspace_id) SELECT DISTINCT workspace_id FROM tasks WHERE kind = ANY($1) AND ((status = 'queued' AND available_at <= now()) OR (status = 'leased' AND lease_until <= now())) ON CONFLICT (workspace_id) DO NOTHING")
            .bind(&kinds)
            .execute(&mut *transaction)
            .await?;
        let row = sqlx::query_as::<_, ClaimedTaskRow>(
            "WITH candidate AS (SELECT t.id, t.workspace_id FROM tasks t JOIN task_workspace_service s ON s.workspace_id = t.workspace_id WHERE t.kind = ANY($4) AND ((t.status = 'queued' AND t.available_at <= now()) OR (t.status = 'leased' AND t.lease_until <= now())) ORDER BY s.last_served_at ASC, t.available_at ASC, t.created_at ASC, t.id ASC FOR UPDATE OF t, s SKIP LOCKED LIMIT 1), service AS (UPDATE task_workspace_service s SET last_served_at = now(), updated_at = now() FROM candidate c WHERE s.workspace_id = c.workspace_id RETURNING s.workspace_id) UPDATE tasks t SET status = 'leased', attempts = t.attempts + 1, lease_owner = $1, lease_token = $2, lease_until = now() + ($3 * interval '1 second'), started_at = COALESCE(t.started_at, now()), updated_at = now() FROM candidate c JOIN service s ON s.workspace_id = c.workspace_id WHERE t.id = c.id RETURNING t.id, t.workspace_id, t.kind, t.subject_id, t.generation, t.payload, t.attempts, t.failures, t.max_failures, t.lease_owner, t.lease_token, t.lease_until, t.correlation_id, t.causation_id",
        )
        .bind(worker_id)
        .bind(Uuid::new_v4())
        .bind(lease_seconds)
        .bind(&kinds)
        .fetch_optional(&mut *transaction)
        .await?;
        transaction.commit().await?;
        row.map(TryInto::try_into).transpose()
    }

    pub async fn heartbeat_task(
        &self,
        task_id: Uuid,
        lease_owner: &str,
        lease_token: Uuid,
        lease_duration: Duration,
    ) -> Result<(), TaskError> {
        let lease_seconds = lease_seconds(lease_duration)?;
        let changed = sqlx::query(
            "UPDATE tasks SET lease_until = now() + ($4 * interval '1 second'), updated_at = now() WHERE id = $1 AND status = 'leased' AND lease_until > now() AND lease_owner = $2 AND lease_token = $3",
        )
        .bind(task_id)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(lease_seconds)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(TaskError::LeaseLost)
        }
    }

    pub async fn complete_task(
        &self,
        task_id: Uuid,
        lease_owner: &str,
        lease_token: Uuid,
    ) -> Result<(), TaskError> {
        let changed = sqlx::query("UPDATE tasks SET status = 'succeeded', lease_owner = NULL, lease_token = NULL, lease_until = NULL, completed_at = now(), updated_at = now() WHERE id = $1 AND status = 'leased' AND lease_until > now() AND lease_owner = $2 AND lease_token = $3")
            .bind(task_id).bind(lease_owner).bind(lease_token).execute(&self.pool).await?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(TaskError::LeaseLost)
        }
    }

    /// Returns this task to the queue without increasing its failure budget;
    /// paged handlers use this after atomically checkpointing domain progress.
    pub async fn reschedule_task_at(
        &self,
        task_id: Uuid,
        lease_owner: &str,
        lease_token: Uuid,
        available_at: DateTime<Utc>,
    ) -> Result<(), TaskError> {
        let changed = sqlx::query("UPDATE tasks SET status = 'queued', available_at = $4, lease_owner = NULL, lease_token = NULL, lease_until = NULL, updated_at = now() WHERE id = $1 AND status = 'leased' AND lease_until > now() AND lease_owner = $2 AND lease_token = $3")
            .bind(task_id).bind(lease_owner).bind(lease_token).bind(available_at).execute(&self.pool).await?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(TaskError::LeaseLost)
        }
    }

    /// Records one failed attempt. Reaching the per-kind budget is terminal.
    pub async fn retry_task_at(
        &self,
        task_id: Uuid,
        lease_owner: &str,
        lease_token: Uuid,
        available_at: DateTime<Utc>,
        error_code: &str,
        error_message: &str,
    ) -> Result<TaskStatus, TaskError> {
        if error_code.len() > MAX_ERROR_CODE_BYTES || error_message.len() > MAX_ERROR_MESSAGE_BYTES
        {
            return Err(TaskError::ValueTooLong);
        }
        let status: Option<String> = sqlx::query_scalar("UPDATE tasks SET status = CASE WHEN failures + 1 >= max_failures THEN 'dead_letter' ELSE 'queued' END, failures = failures + 1, available_at = CASE WHEN failures + 1 >= max_failures THEN available_at ELSE $4 END, lease_owner = NULL, lease_token = NULL, lease_until = NULL, last_error_code = $5, last_error_message = $6, failed_at = CASE WHEN failures + 1 >= max_failures THEN now() ELSE failed_at END, updated_at = now() WHERE id = $1 AND status = 'leased' AND lease_until > now() AND lease_owner = $2 AND lease_token = $3 RETURNING status")
            .bind(task_id).bind(lease_owner).bind(lease_token).bind(available_at).bind(error_code).bind(error_message).fetch_optional(&self.pool).await?;
        match status.as_deref() {
            Some("queued") => Ok(TaskStatus::Queued),
            Some("dead_letter") => Ok(TaskStatus::DeadLetter),
            _ => Err(TaskError::LeaseLost),
        }
    }

    /// Marks a currently owned envelope terminally failed. Domain handlers use
    /// this only when their domain terminal transition has already committed
    /// in the same transaction, or when repairing an older split transition.
    pub async fn dead_letter_task(
        &self,
        task: &ClaimedTask,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), TaskError> {
        if error_code.len() > MAX_ERROR_CODE_BYTES || error_message.len() > MAX_ERROR_MESSAGE_BYTES
        {
            return Err(TaskError::ValueTooLong);
        }
        let changed = sqlx::query("UPDATE tasks SET status='dead_letter',failures=max_failures,lease_owner=NULL,lease_token=NULL,lease_until=NULL,last_error_code=$4,last_error_message=$5,failed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>clock_timestamp()")
            .bind(task.id).bind(&task.lease_owner).bind(task.lease_token).bind(error_code).bind(error_message).execute(&self.pool).await?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(TaskError::LeaseLost)
        }
    }

    /// Cancels queued work in the same transaction as its domain record.
    /// Leased work must be cancelled through a token-fenced handler boundary.
    pub async fn cancel_queued_task(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        task_id: Uuid,
    ) -> Result<(), TaskError> {
        let kind: Option<String> = sqlx::query_scalar(
            "SELECT kind FROM tasks WHERE id = $1 AND status = 'queued' FOR UPDATE",
        )
        .bind(task_id)
        .fetch_optional(&mut **transaction)
        .await?;
        let kind = kind.ok_or(TaskError::LeaseLost)?.parse::<TaskKind>()?;
        if !kind.policy().cancel_allowed {
            return Err(TaskError::OperationNotPermitted);
        }
        sqlx::query("UPDATE tasks SET status = 'cancelled', cancelled_at = now(), updated_at = now() WHERE id = $1")
            .bind(task_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    /// Creates the next generation of a dead-lettered task. It is intentionally
    /// transactional so an authorized domain replay can use the same commit.
    pub async fn replay_task(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        task_id: Uuid,
    ) -> Result<Option<Uuid>, TaskError> {
        let row = sqlx::query_as::<_, ReplayableTaskRow>(
            "SELECT workspace_id, kind, subject_id, generation, payload, correlation_id, causation_id FROM tasks WHERE id = $1 AND status = 'dead_letter' FOR UPDATE",
        )
        .bind(task_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(TaskError::LeaseLost)?;
        let kind = row.kind.parse::<TaskKind>()?;
        if !kind.policy().replay_allowed {
            return Err(TaskError::OperationNotPermitted);
        }
        self.enqueue_task(
            transaction,
            TaskInsert {
                workspace_id: row.workspace_id,
                kind,
                subject_id: row.subject_id,
                generation: row
                    .generation
                    .checked_add(1)
                    .ok_or(TaskError::ValueTooLong)?,
                payload: row.payload,
                correlation_id: row.correlation_id,
                causation_id: row.causation_id,
            },
        )
        .await
    }

    /// Returns payload-free diagnostics scoped to a trusted workspace. Generic
    /// operator APIs must use this form rather than exposing task envelopes.
    pub async fn task_summary_for_workspace(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<TaskSummary>, TaskError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, kind, subject_id, generation, status, attempts, failures, max_failures, available_at, created_at, completed_at, failed_at, cancelled_at, last_error_code FROM tasks WHERE workspace_id = $1 AND id = $2")
            .bind(workspace_id)
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn task_summary(&self, task_id: Uuid) -> Result<Option<TaskSummary>, TaskError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, kind, subject_id, generation, status, attempts, failures, max_failures, available_at, created_at, completed_at, failed_at, cancelled_at, last_error_code FROM tasks WHERE id = $1").bind(task_id).fetch_optional(&self.pool).await?)
    }

    /// Read-only diagnostics for a trusted request workspace, not global worker metrics.
    /// Expired leases are a subset of running tasks, not an additional task count.
    pub async fn background_processing_status_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<BackgroundProcessingStatus>, RepositoryError> {
        Ok(sqlx::query_as(
            r#"SELECT kind,
                count(*) FILTER (WHERE status = 'queued') AS queued,
                count(*) FILTER (WHERE status = 'leased') AS running,
                count(*) FILTER (WHERE status = 'dead_letter') AS failed,
                count(*) FILTER (WHERE status = 'leased' AND lease_until < now()) AS expired_leases,
                extract(epoch FROM (now() - min(available_at) FILTER (
                    WHERE status = 'queued' AND available_at <= now()
                )))::float8 AS oldest_due_seconds
            FROM tasks
            WHERE workspace_id = $1 AND status IN ('queued', 'leased', 'dead_letter')
            GROUP BY kind ORDER BY kind"#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Aggregate, payload-free process metrics for the registered worker kinds.
    pub async fn task_queue_health(
        &self,
        kinds: &[TaskKind],
    ) -> Result<Vec<(String, String, i64, f64, i64)>, TaskError> {
        let kinds: Vec<String> = kinds.iter().map(ToString::to_string).collect();
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        Ok(sqlx::query_as(
            "SELECT kind,status,count(*)::bigint,COALESCE(extract(epoch FROM (clock_timestamp()-min(created_at))),0)::float8,COALESCE(sum(failures),0)::bigint FROM tasks WHERE kind=ANY($1) AND status IN ('queued','leased','dead_letter') GROUP BY kind,status",
        )
        .bind(kinds)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Extension-operation domain health complements the generic envelope
    /// metrics with run status and lag visible to operators.
    pub async fn extension_operation_health(
        &self,
    ) -> Result<Vec<(String, i64, f64, i64)>, TaskError> {
        Ok(sqlx::query_as(
            "SELECT status,count(*)::bigint,COALESCE(extract(epoch FROM (clock_timestamp()-min(created_at))),0)::float8,COALESCE(sum(GREATEST(attempts-1,0)),0)::bigint FROM extension_operation_runs WHERE status IN ('pending','leased','dead_letter') GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_queue::TaskInsert;

    #[test]
    fn task_insert_rejects_non_reference_payloads() {
        let task = TaskInsert {
            workspace_id: Uuid::new_v4(),
            kind: TaskKind::RuleRunV1,
            subject_id: Uuid::new_v4(),
            generation: 0,
            payload: serde_json::json!({"id": Uuid::new_v4().to_string(), "version": 1}),
            correlation_id: None,
            causation_id: None,
        };
        assert!(task.validate().is_ok());
        let nested = TaskInsert {
            payload: serde_json::json!({"domain_snapshot": {"secret": "no"}}),
            ..task
        };
        assert!(matches!(
            nested.validate(),
            Err(TaskValidationError::PayloadValueNotScalar)
        ));
    }
}
