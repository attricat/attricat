//! Bounded retention batches for the maintenance worker.
//!
//! Each `purge_*_batch` method is one short transaction: it selects at most
//! `limit` expired rows with `FOR UPDATE SKIP LOCKED`, deletes them and their
//! dependent rows, and returns how many primary rows it deleted. A result
//! below `limit` means nothing expired remains. Concurrent maintenance
//! processes skip each other's rows, and the local timeouts keep a batch from
//! queueing behind, or holding up, request traffic.
//!
//! No batch touches a row that is pending, leased or running, or one that a
//! live row still needs.

use std::{num::NonZeroI64, str::FromStr};

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError, RepositoryScope};

/// A retention interval that is safe to pass to destructive cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionDays(NonZeroI64);

impl RetentionDays {
    pub const fn get(self) -> i64 {
        self.0.get()
    }
}

impl FromStr for RetentionDays {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<i64>()
            .ok()
            .and_then(NonZeroI64::new)
            .filter(|days| days.get() > 0)
            .map(Self)
            .ok_or("must be a positive integer")
    }
}

impl<S: RepositoryScope> AttricatRepository<S> {
    async fn begin_purge(&self) -> Result<Transaction<'static, Postgres>, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout = '2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL lock_timeout = '1s'")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    /// Runs one single-statement purge in its own bounded transaction.
    async fn purge_with(
        &self,
        sql: &str,
        days: Option<RetentionDays>,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let query = sqlx::query(sql).bind(limit);
        let query = match days {
            Some(days) => query.bind(days.get()),
            None => query,
        };
        let deleted = query.execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(deleted)
    }

    /// Archived attribute values older than the retention window. Current
    /// values live in `attribute_values` and are never touched.
    pub async fn purge_value_history_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT workspace_id, id, archived_at FROM attribute_value_history WHERE archived_at < now() - ($2 * interval '1 day') ORDER BY archived_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM attribute_value_history h USING expired e WHERE h.workspace_id = e.workspace_id AND h.id = e.id AND h.archived_at = e.archived_at",
            Some(days),
            limit,
        )
        .await
    }

    /// Succeeded and cancelled task envelopes older than `days`. Queued,
    /// leased and dead-lettered envelopes stay: workers and replay need them.
    /// Event-delivery envelopes go with their delivery in
    /// [`Self::purge_domain_events_batch`].
    pub async fn purge_tasks_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT t.id FROM tasks t WHERE t.status IN ('succeeded', 'cancelled') AND t.created_at < now() - ($2 * interval '1 day') AND COALESCE(t.completed_at, t.cancelled_at, t.updated_at) < now() - ($2 * interval '1 day') AND NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.task_id = t.id) ORDER BY t.created_at, t.id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM tasks t USING expired e WHERE t.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Terminal rule runs: record-scoped completed or cancelled runs after
    /// `record_days`, and full runs and dead letters after `run_days`. Each
    /// rule version keeps its latest run and its latest completed full dry run,
    /// which the enable gate reads. Findings keep their resolution
    /// (`resolved_by_run_id` becomes null).
    pub async fn purge_rule_runs_batch(
        &self,
        record_days: RetentionDays,
        run_days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let expired: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT r.workspace_id, r.id FROM rule_runs r \
             WHERE r.status IN ('completed', 'cancelled', 'dead_letter') \
             AND r.created_at < now() - (LEAST($2, $3) * interval '1 day') \
             AND COALESCE(r.completed_at, r.cancelled_at, r.updated_at) < now() - ((CASE WHEN r.scope_record_id IS NOT NULL AND r.status <> 'dead_letter' THEN $2 ELSE $3 END) * interval '1 day') \
             AND EXISTS (SELECT 1 FROM rule_runs n WHERE n.workspace_id = r.workspace_id AND n.rule_id = r.rule_id AND n.rule_version = r.rule_version AND n.created_at > r.created_at) \
             AND NOT (r.dry_run AND r.scope_record_id IS NULL AND r.status = 'completed' AND NOT EXISTS (SELECT 1 FROM rule_runs n WHERE n.workspace_id = r.workspace_id AND n.rule_id = r.rule_id AND n.rule_version = r.rule_version AND n.dry_run AND n.scope_record_id IS NULL AND n.status = 'completed' AND n.completed_at > r.completed_at)) \
             AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.workspace_id = r.workspace_id AND t.kind = 'rule_run.v1' AND t.subject_id = r.id AND t.status IN ('queued', 'leased')) \
             ORDER BY r.created_at, r.id LIMIT $1 FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(limit)
        .bind(record_days.get())
        .bind(run_days.get())
        .fetch_all(&mut *tx)
        .await?;
        let deleted = delete_runs_with_tasks(&mut tx, "rule_runs", "rule_run.v1", expired).await?;
        tx.commit().await?;
        Ok(deleted)
    }

    /// Terminal workflow runs older than `days`, with their action progress
    /// (`ON DELETE CASCADE`) and task envelopes.
    pub async fn purge_workflow_runs_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let expired: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT r.workspace_id, r.id FROM workflow_runs r \
             WHERE r.status IN ('completed', 'cancelled', 'dead_letter') \
             AND r.created_at < now() - ($2 * interval '1 day') \
             AND COALESCE(r.completed_at, r.cancelled_at, r.failed_at, r.updated_at) < now() - ($2 * interval '1 day') \
             AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.workspace_id = r.workspace_id AND t.kind = 'workflow_run.v1' AND t.subject_id = r.id AND t.status IN ('queued', 'leased')) \
             ORDER BY r.created_at, r.id LIMIT $1 FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        let deleted =
            delete_runs_with_tasks(&mut tx, "workflow_runs", "workflow_run.v1", expired).await?;
        tx.commit().await?;
        Ok(deleted)
    }

    /// Terminal extension operation runs older than `days` whose stored
    /// outputs have already expired, so no object is left behind. Runs with an
    /// unfinished artifact upload or an unexpired output chunk wait.
    pub async fn purge_extension_operation_runs_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let expired: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT r.workspace_id, r.id FROM extension_operation_runs r \
             WHERE r.outputs_expired AND r.status IN ('completed', 'cancelled', 'dead_letter') \
             AND r.created_at < now() - ($2 * interval '1 day') \
             AND COALESCE(r.completed_at, r.cancelled_at, r.updated_at) < now() - ($2 * interval '1 day') \
             AND NOT EXISTS (SELECT 1 FROM extension_operation_artifacts a WHERE a.workspace_id = r.workspace_id AND a.operation_run_id = r.id AND a.state = 'incomplete') \
             AND NOT EXISTS (SELECT 1 FROM extension_operation_output_chunks c WHERE c.workspace_id = r.workspace_id AND c.operation_run_id = r.id) \
             AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.workspace_id = r.workspace_id AND t.kind = 'extension_operation_run.v1' AND t.subject_id = r.id AND t.status IN ('queued', 'leased')) \
             ORDER BY r.created_at, r.id LIMIT $1 FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        let (workspaces, ids): (Vec<Uuid>, Vec<Uuid>) = expired.iter().copied().unzip();
        // Children first: none of these foreign keys cascade.
        for table in [
            "extension_operation_http_deliveries",
            "extension_operation_http_inputs",
            "extension_operation_output_staging",
            "extension_operation_artifacts",
            "extension_operation_run_records",
        ] {
            sqlx::query(&format!("DELETE FROM {table} c USING unnest($1::uuid[], $2::uuid[]) AS e(workspace_id, id) WHERE c.workspace_id = e.workspace_id AND c.operation_run_id = e.id"))
                .bind(&workspaces)
                .bind(&ids)
                .execute(&mut *tx)
                .await?;
        }
        let deleted = delete_runs_with_tasks(
            &mut tx,
            "extension_operation_runs",
            "extension_operation_run.v1",
            expired,
        )
        .await?;
        tx.commit().await?;
        Ok(deleted)
    }

    /// Stream events of agent runs that finished more than `days` ago. The
    /// run, its tool calls and the conversation stay: they are approval and
    /// audit evidence and go only with their conversation.
    pub async fn purge_agent_run_events_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT e.id FROM agent_run_events e JOIN agent_runs r ON r.id = e.run_id WHERE e.created_at < now() - ($2 * interval '1 day') AND r.finished_at < now() - ($2 * interval '1 day') ORDER BY e.created_at, e.id LIMIT $1 FOR UPDATE OF e SKIP LOCKED) DELETE FROM agent_run_events e USING expired x WHERE e.id = x.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Failed file jobs and completed metadata jobs older than `days`. A
    /// completed purge job stays: it is what stops the file from being queued
    /// for purging again.
    pub async fn purge_file_processing_jobs_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM file_processing_jobs WHERE (status = 'failed' OR (status = 'completed' AND kind <> 'purge')) AND created_at < now() - ($2 * interval '1 day') AND updated_at < now() - ($2 * interval '1 day') ORDER BY created_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM file_processing_jobs j USING expired e WHERE j.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Domain events older than `days` that every consumer in the workspace
    /// has passed and delivered, with their completed deliveries and delivery
    /// envelopes. An event with a pending, leased or dead-lettered delivery
    /// stays, and so does each workspace's newest event, which activation
    /// boundaries and new consumers read. Workflow runs keep their copy of the
    /// trigger event (`trigger_event_id` becomes null).
    pub async fn purge_domain_events_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT e.id FROM domain_events e \
             WHERE e.occurred_at < now() - ($2 * interval '1 day') \
             AND e.sequence <= COALESCE((SELECT min(c.watermark) FROM event_consumers c WHERE c.workspace_id = e.workspace_id), e.sequence) \
             AND e.sequence < (SELECT max(m.sequence) FROM domain_events m WHERE m.workspace_id = e.workspace_id) \
             AND NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.event_id = e.id AND d.status IN ('pending', 'leased', 'dead_letter')) \
             ORDER BY e.occurred_at, e.id LIMIT $1 FOR UPDATE OF e SKIP LOCKED",
        )
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        let tasks: Vec<Uuid> = sqlx::query_scalar::<_, Option<Uuid>>(
            "DELETE FROM event_deliveries WHERE event_id = ANY($1) AND status = 'completed' RETURNING task_id",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .flatten()
        .collect();
        sqlx::query(
            "DELETE FROM tasks WHERE id = ANY($1) AND status IN ('succeeded', 'cancelled')",
        )
        .bind(&tasks)
        .execute(&mut *tx)
        .await?;
        let deleted = sqlx::query("DELETE FROM domain_events WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(deleted)
    }

    /// Rate-limit rows whose 15-minute window has ended. The limiters reset
    /// such a row on its next use, so deleting it changes nothing.
    pub async fn purge_rate_limit_windows_batch(&self, limit: i64) -> Result<u64, RepositoryError> {
        let mut deleted = 0;
        for table in [
            "browser_login_rate_limits",
            "password_reset_rate_limits",
            "workspace_discovery_rate_limits",
        ] {
            deleted += self
                .purge_with(
                    &format!("WITH expired AS (SELECT key_digest FROM {table} WHERE window_started_at <= clock_timestamp() - interval '15 minutes' ORDER BY window_started_at LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM {table} t USING expired e WHERE t.key_digest = e.key_digest"),
                    None,
                    limit,
                )
                .await?;
        }
        Ok(deleted)
    }

    /// Browser sessions that expired or were revoked (including rotated
    /// predecessors) more than `days` ago. Authentication already ignores them.
    pub async fn purge_browser_sessions_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM browser_sessions WHERE expires_at < now() - ($2 * interval '1 day') UNION SELECT id FROM browser_sessions WHERE revoked_at < now() - ($2 * interval '1 day') LIMIT $1), locked AS (SELECT s.id FROM browser_sessions s JOIN expired e ON e.id = s.id FOR UPDATE OF s SKIP LOCKED) DELETE FROM browser_sessions s USING locked l WHERE s.id = l.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Email-verification, password-setup and password-reset tokens that were
    /// consumed, revoked or expired more than `days` ago. None can be redeemed.
    pub async fn purge_user_action_tokens_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM user_lifecycle_action_tokens WHERE expires_at < now() - ($2 * interval '1 day') OR consumed_at < now() - ($2 * interval '1 day') OR revoked_at < now() - ($2 * interval '1 day') ORDER BY expires_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM user_lifecycle_action_tokens t USING expired e WHERE t.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Unaccepted invitations that expired or were revoked more than `days`
    /// ago, with their onboarding rows (`ON DELETE CASCADE`). Accepted
    /// invitations stay: they record who invited a member.
    pub async fn purge_workspace_invitations_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM workspace_invitations WHERE accepted_at IS NULL AND (expires_at < now() - ($2 * interval '1 day') OR revoked_at < now() - ($2 * interval '1 day')) ORDER BY expires_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM workspace_invitations i USING expired e WHERE i.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Manual solution-pack check runs older than `days`, with their results.
    /// Every application keeps its post-apply run and its latest run.
    pub async fn purge_solution_pack_check_runs_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let expired: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT r.workspace_id, r.id FROM solution_pack_check_runs r WHERE r.trigger <> 'post_apply' AND r.completed_at < now() - ($2 * interval '1 day') AND EXISTS (SELECT 1 FROM solution_pack_check_runs newer WHERE newer.workspace_id = r.workspace_id AND newer.application_id = r.application_id AND (newer.completed_at, newer.id) > (r.completed_at, r.id)) ORDER BY r.completed_at, r.id LIMIT $1 FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        let (workspaces, ids): (Vec<_>, Vec<_>) = expired.into_iter().unzip();
        sqlx::query("DELETE FROM solution_pack_check_results r USING unnest($1::uuid[], $2::uuid[]) AS e(workspace_id, id) WHERE r.workspace_id = e.workspace_id AND r.run_id = e.id")
            .bind(&workspaces)
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        let deleted = sqlx::query("DELETE FROM solution_pack_check_runs r USING unnest($1::uuid[], $2::uuid[]) AS e(workspace_id, id) WHERE r.workspace_id = e.workspace_id AND r.id = e.id")
            .bind(&workspaces)
            .bind(&ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(deleted)
    }

    /// Read notifications older than `days`. Unread ones are never deleted.
    pub async fn purge_read_notifications_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM user_notifications WHERE read_at < now() - ($2 * interval '1 day') ORDER BY read_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM user_notifications n USING expired e WHERE n.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Audit events older than `days`, with their value changes. Only runs
    /// when an operator configures audit retention.
    pub async fn purge_audit_events_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM audit_events WHERE occurred_at < now() - ($2 * interval '1 day') ORDER BY occurred_at, id LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM audit_event_changes WHERE audit_event_id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        let deleted = sqlx::query("DELETE FROM audit_events WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(deleted)
    }
}

/// Deletes locked terminal runs and every envelope generation of each run.
async fn delete_runs_with_tasks(
    tx: &mut Transaction<'_, Postgres>,
    table: &str,
    task_kind: &str,
    runs: Vec<(Uuid, Uuid)>,
) -> Result<u64, RepositoryError> {
    let (workspaces, ids): (Vec<Uuid>, Vec<Uuid>) = runs.into_iter().unzip();
    sqlx::query("DELETE FROM tasks t USING unnest($1::uuid[], $2::uuid[]) AS e(workspace_id, id) WHERE t.workspace_id = e.workspace_id AND t.kind = $3 AND t.subject_id = e.id")
        .bind(&workspaces)
        .bind(&ids)
        .bind(task_kind)
        .execute(&mut **tx)
        .await?;
    Ok(sqlx::query(&format!("DELETE FROM {table} r USING unnest($1::uuid[], $2::uuid[]) AS e(workspace_id, id) WHERE r.workspace_id = e.workspace_id AND r.id = e.id"))
        .bind(&workspaces)
        .bind(&ids)
        .execute(&mut **tx)
        .await?
        .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_days_rejects_unsafe_values() {
        assert_eq!("1".parse::<RetentionDays>().unwrap().get(), 1);
        for value in ["0", "-1", "not-a-number", "9223372036854775808"] {
            assert_eq!(
                value.parse::<RetentionDays>().unwrap_err(),
                "must be a positive integer"
            );
        }
    }
}
