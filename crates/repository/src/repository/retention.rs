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

    /// Succeeded and cancelled task envelopes older than `days`, and dead
    /// letters a replay has superseded with a newer generation. Queued and
    /// leased envelopes, and each subject's latest dead letter, stay: workers
    /// and replay need them.
    /// Event-delivery envelopes are linked to their delivery and go with it in
    /// [`Self::purge_domain_events_batch`]; they are outside this scan so they
    /// never sit in front of rows it can delete.
    pub async fn purge_tasks_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT t.id FROM tasks t WHERE t.status IN ('succeeded', 'cancelled', 'dead_letter') AND t.kind <> 'event_delivery.v1' AND t.created_at < now() - ($2 * interval '1 day') AND COALESCE(t.completed_at, t.cancelled_at, t.failed_at, t.updated_at) < now() - ($2 * interval '1 day') AND (t.status <> 'dead_letter' OR EXISTS (SELECT 1 FROM tasks newer WHERE newer.workspace_id = t.workspace_id AND newer.kind = t.kind AND newer.subject_id = t.subject_id AND newer.generation > t.generation)) ORDER BY t.created_at, t.id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM tasks t USING expired e WHERE t.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Terminal rule runs: record-scoped completed or cancelled runs after
    /// `record_days`, and full runs and dead letters after `run_days`. Each
    /// rule version keeps its latest run, and each definition its latest
    /// completed full dry run, which the dry-run gate reads. Findings keep their resolution
    /// (`resolved_by_run_id` becomes null), and a dry run's named failures go
    /// with it (`ON DELETE CASCADE`).
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
             AND NOT (r.dry_run AND r.scope_record_id IS NULL AND r.status = 'completed' AND NOT EXISTS (SELECT 1 FROM rule_runs n WHERE n.workspace_id = r.workspace_id AND n.rule_id = r.rule_id AND n.definition_hash = r.definition_hash AND n.dry_run AND n.scope_record_id IS NULL AND n.status = 'completed' AND n.completed_at > r.completed_at)) \
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

    /// Completed and failed metadata jobs older than `days`. Purge jobs stay: a
    /// completed one stops the file from being queued for purging again, and a
    /// failed one is the operator's evidence of a purge that cannot finish.
    pub async fn purge_file_processing_jobs_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM file_processing_jobs WHERE kind <> 'purge' AND status IN ('completed', 'failed') AND created_at < now() - ($2 * interval '1 day') AND updated_at < now() - ($2 * interval '1 day') ORDER BY created_at, id LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM file_processing_jobs j USING expired e WHERE j.id = e.id",
            Some(days),
            limit,
        )
        .await
    }

    /// Domain events older than `days` that every consumer in the workspace
    /// has passed and delivered, with their completed deliveries and every
    /// envelope generation of those deliveries. An event stays while any
    /// delivery is pending, leased or dead-lettered, or still has a queued or
    /// leased envelope, and so does each workspace's newest event, which
    /// activation boundaries and new consumers read. Workflow runs keep their
    /// copy of the trigger event (`trigger_event_id` becomes null); extension
    /// handler outcomes of the event go with it.
    ///
    /// Each workspace scans its own sequence range, from where its last batch
    /// stopped up to a ceiling: the slowest consumer's watermark, the event
    /// before its newest, and the first event inside the retention window. A
    /// batch examines at most `limit` events per workspace, so events held by
    /// a dead letter cost one pass per cycle, not one per batch. The scan wraps
    /// to the start once it reaches the ceiling. The highest purged sequence
    /// is recorded so change-feed cursors issued before a purge are rejected.
    pub async fn purge_domain_events_batch(
        &self,
        days: RetentionDays,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.begin_purge().await?;
        let examined: Vec<(Uuid, Uuid, i64, bool)> = sqlx::query_as(&format!(
            "WITH window_start AS (SELECT COALESCE((SELECT sequence FROM domain_events WHERE occurred_at >= now() - ($2 * interval '1 day') ORDER BY occurred_at, id LIMIT 1), 9223372036854775807) AS sequence) \
             SELECT w.id, x.id, x.sequence, x.deletable FROM workspaces w \
             LEFT JOIN domain_event_retention r ON r.workspace_id = w.id \
             CROSS JOIN LATERAL (SELECT LEAST( \
                 COALESCE((SELECT min(c.watermark) FROM event_consumers c WHERE c.workspace_id = w.id), 9223372036854775807), \
                 (SELECT max(m.sequence) FROM domain_events m WHERE m.workspace_id = w.id) - 1, \
                 (SELECT sequence FROM window_start) - 1) AS sequence) ceiling \
             CROSS JOIN LATERAL (SELECT e.id, e.sequence, \
                     e.occurred_at < now() - ($2 * interval '1 day') \
                     AND {UNHELD_EVENT} AS deletable \
                 FROM domain_events e \
                 WHERE e.workspace_id = w.id AND e.sequence > COALESCE(r.scan_after_sequence, 0) AND e.sequence <= ceiling.sequence \
                 ORDER BY e.sequence LIMIT $1) x"
        ))
        .bind(limit)
        .bind(days.get())
        .fetch_all(&mut *tx)
        .await?;
        let candidates: Vec<Uuid> = examined
            .iter()
            .filter(|(_, _, _, deletable)| *deletable)
            .map(|(_, id, _, _)| *id)
            .collect();
        // A delivery may have been added since the scan; check again under the lock.
        let locked: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(&format!(
            "SELECT e.workspace_id, e.id, e.sequence FROM domain_events e WHERE e.id = ANY($1) AND {UNHELD_EVENT} FOR UPDATE OF e SKIP LOCKED"
        ))
        .bind(&candidates)
        .fetch_all(&mut *tx)
        .await?;
        let ids: Vec<Uuid> = locked.iter().map(|(_, id, _)| *id).collect();
        let deliveries: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "DELETE FROM event_deliveries d USING domain_events e WHERE d.event_id = e.id AND e.id = ANY($1) AND d.status = 'completed' RETURNING e.workspace_id, d.id",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
        let (delivery_workspaces, delivery_ids): (Vec<Uuid>, Vec<Uuid>) = deliveries
            .into_iter()
            .filter_map(|(workspace, id)| id.map(|id| (workspace, id)))
            .unzip();
        sqlx::query("DELETE FROM tasks t USING unnest($1::uuid[], $2::uuid[]) AS d(workspace_id, id) WHERE t.workspace_id = d.workspace_id AND t.kind = 'event_delivery.v1' AND t.subject_id = d.id")
            .bind(&delivery_workspaces)
            .bind(&delivery_ids)
            .execute(&mut *tx)
            .await?;
        // Retrying and debouncing handlers keep their delivery open, so only
        // settled outcomes are left here, and they name the event.
        sqlx::query("DELETE FROM extension_handler_attempts WHERE event_id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        let deleted = sqlx::query("DELETE FROM domain_events WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        // Per workspace: where the next batch resumes, and the purge boundary.
        let mut progress: std::collections::HashMap<Uuid, (i64, i64, i64)> =
            std::collections::HashMap::new();
        for (workspace, _, sequence, _) in &examined {
            let entry = progress.entry(*workspace).or_insert((0, 0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(*sequence);
        }
        for (workspace, _, sequence) in &locked {
            let entry = progress.entry(*workspace).or_insert((0, 0, 0));
            entry.2 = entry.2.max(*sequence);
        }
        let mut workspaces = Vec::new();
        let mut scan_after = Vec::new();
        let mut purged_through = Vec::new();
        for (workspace, (count, last, purged)) in progress {
            workspaces.push(workspace);
            scan_after.push(if count < limit { 0 } else { last });
            purged_through.push(purged);
        }
        sqlx::query("INSERT INTO domain_event_retention (workspace_id, scan_after_sequence, purged_through_sequence) SELECT * FROM unnest($1::uuid[], $2::bigint[], $3::bigint[]) ON CONFLICT (workspace_id) DO UPDATE SET scan_after_sequence = EXCLUDED.scan_after_sequence, purged_through_sequence = GREATEST(domain_event_retention.purged_through_sequence, EXCLUDED.purged_through_sequence)")
            .bind(&workspaces)
            .bind(&scan_after)
            .bind(&purged_through)
            .execute(&mut *tx)
            .await?;
        // A workspace with nothing left to examine starts its next cycle over.
        sqlx::query("UPDATE domain_event_retention SET scan_after_sequence = 0 WHERE scan_after_sequence > 0 AND workspace_id <> ALL($1)")
            .bind(&workspaces)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(deleted)
    }

    /// Extension handler outcomes last updated 30 days ago: rejected and
    /// exhausted ones, which the extension's page lists, and those of a
    /// delivery that ended without settling (dead-lettered). Outcomes of a
    /// delivery that is still open stay, so a late retry never runs a
    /// completed handler again.
    pub async fn purge_extension_handler_outcomes_batch(
        &self,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT a.workspace_id, a.event_id, a.extension_id, a.handler_id FROM extension_handler_attempts a WHERE a.updated_at < clock_timestamp() - interval '30 days' AND (a.status IN ('rejected', 'exhausted') OR NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.event_id = a.event_id AND d.status IN ('pending', 'leased'))) ORDER BY a.updated_at LIMIT $1 FOR UPDATE OF a SKIP LOCKED) DELETE FROM extension_handler_attempts a USING expired e WHERE a.workspace_id = e.workspace_id AND a.event_id = e.event_id AND a.extension_id = e.extension_id AND a.handler_id = e.handler_id",
            None,
            limit,
        )
        .await
    }

    /// Public extension file links that expired more than a day ago.
    pub async fn purge_extension_file_links_batch(
        &self,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM extension_file_links WHERE expires_at < clock_timestamp() - interval '1 day' ORDER BY expires_at LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM extension_file_links l USING expired e WHERE l.id = e.id",
            None,
            limit,
        )
        .await
    }

    /// Browser authorizations of extension connections that expired without
    /// completing: the state digest, verifier and session binding are
    /// cleared, and the connection itself stays.
    pub async fn purge_extension_authorization_requests_batch(
        &self,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT id FROM extension_connections WHERE authorization_expires_at < clock_timestamp() ORDER BY authorization_expires_at LIMIT $1 FOR UPDATE SKIP LOCKED) UPDATE extension_connections c SET authorization_state_sha256 = NULL, authorization_verifier_ciphertext = NULL, authorization_session_sha256 = NULL, authorization_expires_at = NULL FROM expired e WHERE c.id = e.id",
            None,
            limit,
        )
        .await
    }

    /// Keyed export retry requests after a week: a client resends a timed
    /// out request within minutes, not days.
    pub async fn purge_extension_export_retry_requests_batch(
        &self,
        limit: i64,
    ) -> Result<u64, RepositoryError> {
        self.purge_with(
            "WITH expired AS (SELECT workspace_id, extension_id, request_key FROM extension_export_retry_requests WHERE created_at < now() - interval '7 days' ORDER BY created_at LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM extension_export_retry_requests r USING expired e WHERE r.workspace_id = e.workspace_id AND r.extension_id = e.extension_id AND r.request_key = e.request_key",
            None,
            limit,
        )
        .await
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
        // Sign-ins abandoned at an identity provider are as short-lived.
        deleted += self
            .purge_with(
                "WITH expired AS (SELECT state_digest FROM oidc_login_requests WHERE expires_at <= clock_timestamp() LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM oidc_login_requests WHERE state_digest IN (SELECT state_digest FROM expired)",
                None,
                limit,
            )
            .await?;
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
            "WITH expired AS (SELECT id FROM browser_sessions WHERE expires_at < now() - ($2 * interval '1 day') OR revoked_at < now() - ($2 * interval '1 day') LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM browser_sessions s USING expired e WHERE s.id = e.id",
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
            "WITH expired AS (SELECT id FROM user_lifecycle_action_tokens WHERE expires_at < now() - ($2 * interval '1 day') OR consumed_at < now() - ($2 * interval '1 day') OR revoked_at < now() - ($2 * interval '1 day') LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM user_lifecycle_action_tokens t USING expired e WHERE t.id = e.id",
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
            "WITH expired AS (SELECT id FROM workspace_invitations WHERE accepted_at IS NULL AND (expires_at < now() - ($2 * interval '1 day') OR revoked_at < now() - ($2 * interval '1 day')) LIMIT $1 FOR UPDATE SKIP LOCKED) DELETE FROM workspace_invitations i USING expired e WHERE i.id = e.id",
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

/// An event none of whose deliveries is unfinished, dead-lettered, or has a
/// queued or leased envelope. Expects the event aliased as `e`.
const UNHELD_EVENT: &str = "NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.event_id = e.id AND (d.status IN ('pending', 'leased', 'dead_letter') OR EXISTS (SELECT 1 FROM tasks t WHERE t.workspace_id = e.workspace_id AND t.kind = 'event_delivery.v1' AND t.subject_id = d.id AND t.status IN ('queued', 'leased'))))";

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
