use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use crate::{
    model::{
        BlueprintMigrationBatch, BlueprintMigrationBatchStatus, BlueprintMigrationImpact,
        BlueprintMigrationRemovalPolicy, MigrateRecordRequest,
    },
    task_queue::{TaskInsert, TaskKind},
};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct BatchMigration {
    id: Uuid,
    status: String,
}

#[derive(Clone, Copy, sqlx::FromRow)]
struct BatchCandidate {
    id: Uuid,
    created_at: DateTime<Utc>,
}

impl CatalogRepository {
    pub async fn list_blueprint_migration_batches(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<BlueprintMigrationBatchStatus>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<BlueprintMigrationBatchStatus>>(
            r#"WITH stats AS (
                   SELECT m.batch_id,
                          COUNT(DISTINCT m.record_id) AS processed_records,
                          COUNT(DISTINCT m.record_id) FILTER (WHERE m.status = 'migrated') AS migrated_records,
                          COUNT(DISTINCT m.record_id) FILTER (WHERE m.status IN ('needs_input', 'blocked')) AS needs_input_records,
                          COUNT(DISTINCT m.record_id) FILTER (WHERE m.status = 'failed') AS failed_records
                   FROM record_blueprint_migrations m
                   JOIN blueprint_migration_batches scoped ON scoped.id = m.batch_id
                   WHERE m.workspace_id = $1
                     AND scoped.workspace_id = $1
                     AND scoped.blueprint_id = $2
                   GROUP BY m.batch_id
               )
               SELECT b.id, b.blueprint_id, b.target_version, b.status, b.removal_policy,
                      b.created_at, b.started_at, b.completed_at,
                      COALESCE(s.processed_records, 0) + (
                          SELECT COUNT(*) FROM records e
                          WHERE e.workspace_id = b.workspace_id
                            AND e.blueprint_id = b.blueprint_id
                            AND e.blueprint_version < b.target_version
                            AND e.deleted_at IS NULL
                            AND NOT EXISTS (
                                SELECT 1 FROM record_blueprint_migrations m
                                WHERE m.batch_id = b.id AND m.record_id = e.id
                            )
                      ) AS total_records,
                      COALESCE(s.processed_records, 0) AS processed_records,
                      COALESCE(s.migrated_records, 0) AS migrated_records,
                      COALESCE(s.needs_input_records, 0) AS needs_input_records,
                      COALESCE(s.failed_records, 0) AS failed_records
               FROM blueprint_migration_batches b
               LEFT JOIN stats s ON s.batch_id = b.id
               WHERE b.workspace_id = $1 AND b.blueprint_id = $2
               ORDER BY b.created_at DESC, b.id DESC"#,
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    /// Starts the safe bulk path and atomically adds its durable delivery
    /// envelope. The active-target index makes retries return the same batch;
    /// task uniqueness makes the matching delivery idempotent as well.
    pub async fn start_safe_blueprint_migration_batch(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
    ) -> Result<BlueprintMigrationBatch, RepositoryError> {
        self.start_safe_blueprint_migration_batch_with_removal_disposition(
            blueprint_id,
            target_version,
            None,
        )
        .await
    }

    pub async fn start_safe_blueprint_migration_batch_with_removal_disposition(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
        removal_disposition: Option<&str>,
    ) -> Result<BlueprintMigrationBatch, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        if let Some(existing) = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "SELECT id, blueprint_id, target_version, status, removal_policy, created_at, started_at, completed_at FROM blueprint_migration_batches WHERE workspace_id = $1 AND blueprint_id = $2 AND target_version = $3 AND status IN ('queued', 'running')",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(existing.into_domain());
        }
        let target = self
            .get_current_blueprint(blueprint_id)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))?;
        if target.blueprint.version != target_version || target.blueprint.kind != "record" {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        }
        let removed_attribute_codes = self
            .safe_removed_attribute_codes(blueprint_id, target_version, &target)
            .await?;
        let impact = self
            .safe_blueprint_migration_impact_for(
                blueprint_id,
                target_version,
                &removed_attribute_codes,
            )
            .await?;
        let removal_policy = match (impact.requires_removal_disposition, removal_disposition) {
            (true, Some("archive")) => serde_json::to_value(BlueprintMigrationRemovalPolicy {
                disposition: "archive".to_owned(),
                attribute_codes: removed_attribute_codes,
            })
            .expect("removal policy serializes"),
            (true, _) => return Err(RepositoryError::BlueprintMigrationNotSafe),
            (false, Some(disposition)) if disposition != "archive" => {
                return Err(RepositoryError::BlueprintMigrationNotSafe);
            }
            (false, _) => json!({}),
        };

        let mut transaction = self.pool.begin().await?;
        let batch = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "INSERT INTO blueprint_migration_batches (id, workspace_id, blueprint_id, target_version, status, removal_policy) VALUES ($1, $2, $3, $4, 'queued', $5) ON CONFLICT (workspace_id, blueprint_id, target_version) WHERE status IN ('queued', 'running') DO UPDATE SET workspace_id = EXCLUDED.workspace_id RETURNING id, blueprint_id, target_version, status, removal_policy, created_at, started_at, completed_at",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
        .bind(removal_policy)
        .fetch_one(&mut *transaction)
        .await?;
        self.enqueue_task(
            &mut transaction,
            TaskInsert {
                workspace_id,
                kind: TaskKind::BlueprintMigrationBatchV1,
                subject_id: batch.id,
                generation: 0,
                payload: json!({"batch_id": batch.id.to_string()}),
                correlation_id: None,
                causation_id: None,
            },
        )
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(batch.into_domain())
    }

    pub async fn safe_blueprint_migration_impact(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
    ) -> Result<BlueprintMigrationImpact, RepositoryError> {
        let target = self
            .get_current_blueprint(blueprint_id)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))?;
        if target.blueprint.version != target_version || target.blueprint.kind != "record" {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        }
        let removed_attribute_codes = self
            .safe_removed_attribute_codes(blueprint_id, target_version, &target)
            .await?;
        self.safe_blueprint_migration_impact_for(
            blueprint_id,
            target_version,
            &removed_attribute_codes,
        )
        .await
    }

    async fn safe_removed_attribute_codes(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
        target: &crate::model::BlueprintWithAttributes,
    ) -> Result<Vec<String>, RepositoryError> {
        let revisions = self.list_blueprint_revisions(blueprint_id).await?;
        if !revisions
            .iter()
            .any(|revision| revision.version < target_version && revision.status == "published")
        {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        }
        // Draft revisions cannot have records, so use the nearest published
        // ancestor and every earlier published revision as migration sources.
        // Aggregate removal requirements across all possible sources.
        let mut removed = std::collections::BTreeSet::new();
        for revision in revisions
            .iter()
            .filter(|revision| revision.version < target_version && revision.status == "published")
        {
            let source = self
                .get_blueprint_revision(blueprint_id, revision.version)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint version"))?;
            removed.extend(
                safe_automatic_migration(&source, target)
                    .ok_or(RepositoryError::BlueprintMigrationNotSafe)?,
            );
        }
        Ok(removed.into_iter().collect())
    }

    async fn safe_blueprint_migration_impact_for(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
        removed_attribute_codes: &[String],
    ) -> Result<BlueprintMigrationImpact, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let eligible_records = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM records WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version < $3 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
        .fetch_one(&self.pool)
        .await?;
        let (records_with_removed_values, removed_values) = if removed_attribute_codes.is_empty() {
            (0, 0)
        } else {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT count(DISTINCT e.id), count(av.id) FROM records e JOIN attribute_values av ON av.record_id = e.id AND av.workspace_id = e.workspace_id JOIN attributes a ON a.id = av.attribute_id AND a.workspace_id = e.workspace_id WHERE e.workspace_id = $1 AND e.blueprint_id = $2 AND e.blueprint_version < $3 AND e.deleted_at IS NULL AND a.blueprint_version = e.blueprint_version AND a.code = ANY($4)",
            )
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(target_version)
            .bind(removed_attribute_codes)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(BlueprintMigrationImpact {
            eligible_records,
            removed_attribute_codes: removed_attribute_codes.to_vec(),
            records_with_removed_values,
            removed_values,
            requires_removal_disposition: removed_values > 0,
        })
    }

    /// Runs a batch under the shared task lease. Each record is reserved before
    /// preview/migration, making `(batch_id, record_id)` its durable retry
    /// identity. Every progress checkpoint commits through the task fence.
    pub async fn run_safe_blueprint_migration_batch_task(
        &self,
        batch_id: Uuid,
        page_size: usize,
        concurrency: usize,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        let batch = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "UPDATE blueprint_migration_batches SET status = 'running', started_at = COALESCE(started_at, now()) WHERE id = $1 AND workspace_id = $2 AND status IN ('queued', 'running') RETURNING id, blueprint_id, target_version, status, removal_policy, created_at, started_at, completed_at",
        )
        .bind(batch_id)
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        let Some(batch) = batch else {
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM blueprint_migration_batches WHERE id = $1 AND workspace_id = $2",
            )
            .bind(batch_id)
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await?;
            return match status.as_deref() {
                Some("completed") => Ok(()),
                Some("failed") => Err(RepositoryError::InvalidBlueprintDefinition(
                    "safe blueprint migration batch is dead-lettered".into(),
                )),
                _ => Err(RepositoryError::NotFound("blueprint migration batch")),
            };
        };
        let mut cursor: Option<(DateTime<Utc>, Uuid)> = None;
        // Read on the first candidate page; see `preview_record_migration_against`.
        let mut target = None;
        loop {
            let candidates = match cursor {
                Some((created_at, id)) => {
                    sqlx::query_as::<_, BatchCandidate>(
                        r#"SELECT e.id, e.created_at
                           FROM records e
                           WHERE e.blueprint_id = $1
                             AND e.blueprint_version < $2
                             AND e.workspace_id = $3
                             AND e.deleted_at IS NULL
                             AND (e.created_at, e.id) < ($4, $5)
                             AND NOT EXISTS (
                                 SELECT 1 FROM record_blueprint_migrations m
                                 WHERE m.batch_id = $6 AND m.record_id = e.id
                                   AND m.workspace_id = $3
                                   AND m.status IN ('migrated', 'needs_input', 'blocked', 'failed', 'skipped', 'superseded')
                             )
                           ORDER BY e.created_at DESC, e.id DESC
                           LIMIT $7"#,
                    )
                    .bind(batch.blueprint_id)
                    .bind(batch.target_version)
                    .bind(workspace_id)
                    .bind(created_at)
                    .bind(id)
                    .bind(batch.id)
                    .bind(page_size as i64)
                    .fetch_all(&self.pool)
                    .await?
                }
                None => {
                    sqlx::query_as::<_, BatchCandidate>(
                        r#"SELECT e.id, e.created_at
                           FROM records e
                           WHERE e.blueprint_id = $1
                             AND e.blueprint_version < $2
                             AND e.workspace_id = $3
                             AND e.deleted_at IS NULL
                             AND NOT EXISTS (
                                 SELECT 1 FROM record_blueprint_migrations m
                                 WHERE m.batch_id = $4 AND m.record_id = e.id
                                   AND m.workspace_id = $3
                                   AND m.status IN ('migrated', 'needs_input', 'blocked', 'failed', 'skipped', 'superseded')
                             )
                           ORDER BY e.created_at DESC, e.id DESC
                           LIMIT $5"#,
                    )
                    .bind(batch.blueprint_id)
                    .bind(batch.target_version)
                    .bind(workspace_id)
                    .bind(batch.id)
                    .bind(page_size as i64)
                    .fetch_all(&self.pool)
                    .await?
                }
            };
            if candidates.is_empty() {
                if cursor.take().is_some() {
                    // The eligible set shrinks while rows migrate. Rechecking
                    // from the newest edge also catches candidates inserted
                    // after the first page was read.
                    continue;
                }
                break;
            }
            metrics::counter!("catalog_blueprint_migration_candidates_scanned_total")
                .increment(candidates.len() as u64);
            let page_started = std::time::Instant::now();
            let candidate_ids: Vec<_> = candidates.iter().map(|candidate| candidate.id).collect();
            if target.is_none() {
                target = Some(self.migration_target(batch.blueprint_id).await?);
            }
            let preloaded = target.as_ref().and_then(Option::as_ref);
            let results = stream::iter(candidate_ids)
                .map(|record_id| self.process_batch_candidate(&batch, preloaded, record_id))
                .buffer_unordered(concurrency)
                .collect::<Vec<_>>()
                .await;
            for result in results {
                let outcome = result?;
                metrics::counter!("catalog_blueprint_migration_records_total", "outcome" => outcome)
                    .increment(1);
            }
            let elapsed = page_started.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                metrics::histogram!("catalog_blueprint_migration_records_per_second")
                    .record(candidates.len() as f64 / elapsed);
            }
            let last = candidates.last().expect("non-empty candidate page");
            cursor = Some((last.created_at, last.id));
        }
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE blueprint_migration_batches SET status = 'completed', completed_at = now() WHERE id = $1 AND workspace_id = $2 AND status = 'running'",
        )
        .bind(batch_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        self.commit_mutation(transaction).await?;
        if updated == 1 {
            Ok(())
        } else {
            Err(RepositoryError::InvalidBlueprintDefinition(
                "safe blueprint migration batch checkpoint was lost".into(),
            ))
        }
    }

    async fn process_batch_candidate(
        &self,
        batch: &BlueprintMigrationBatch,
        target: Option<&super::record_migration::MigrationTarget>,
        record_id: Uuid,
    ) -> Result<&'static str, RepositoryError> {
        let migration = self.reserve_batch_migration(batch, record_id).await?;
        match migration.status.as_str() {
            "migrated" | "needs_input" | "blocked" | "failed" | "skipped" | "superseded" => {
                return Ok("already_terminal");
            }
            "pending" => match self
                .preview_record_migration_against(record_id, Some(migration.id), target)
                .await
            {
                Ok(preview)
                    if preview.status == "ready"
                        || removal_policy_covers_preview(
                            &batch.removal_policy,
                            &preview.issues,
                        ) => {}
                Ok(_) => return Ok("needs_input"),
                Err(error) => {
                    if retryable_batch_error(&error) {
                        // Preserve this reservation so the task queue can retry
                        // after transient database failures.
                        return Err(error);
                    }
                    self.record_batch_failure(migration.id, &error.to_string())
                        .await?;
                    return Ok("failed");
                }
            },
            "ready" | "migrating" => {}
            status => {
                return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                    "unknown batch migration status '{status}'"
                )));
            }
        }
        self.begin_batch_migration(migration.id).await?;
        match self
            .migrate_record_to_latest(
                record_id,
                MigrateRecordRequest {
                    migration_id: migration.id,
                    expected_target_version: batch.target_version,
                    values: Vec::new(),
                    relationships: Vec::new(),
                    discard_attributes: batch
                        .removal_policy
                        .get("attribute_codes")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                    removal_policy: serde_json::from_value(batch.removal_policy.clone()).ok(),
                },
            )
            .await
        {
            Ok(_) => Ok("migrated"),
            Err(error) => {
                if retryable_batch_error(&error) {
                    return Err(error);
                }
                self.record_batch_failure(migration.id, &error.to_string())
                    .await?;
                Ok("failed")
            }
        }
    }

    pub async fn dead_letter_safe_blueprint_migration_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE blueprint_migration_batches SET status = 'failed', completed_at = now() WHERE id = $1 AND workspace_id = $2 AND status IN ('queued', 'running')",
        )
        .bind(batch_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await
    }

    async fn reserve_batch_migration(
        &self,
        batch: &BlueprintMigrationBatch,
        record_id: Uuid,
    ) -> Result<BatchMigration, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as::<_, BatchMigration>(
            "INSERT INTO record_blueprint_migrations (id, batch_id, workspace_id, record_id, blueprint_id, source_version, target_version, status, issues, task_owned) SELECT $1, $2, $3, e.id, $4, e.blueprint_version, $5, 'pending', '[]'::jsonb, true FROM records e WHERE e.id = $6 AND e.workspace_id = $3 ON CONFLICT (batch_id, record_id) WHERE batch_id IS NOT NULL AND task_owned DO NOTHING RETURNING id, status",
        )
        .bind(Uuid::new_v4())
        .bind(batch.id)
        .bind(workspace_id)
        .bind(batch.blueprint_id)
        .bind(batch.target_version)
        .bind(record_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let migration = match inserted {
            Some(migration) => migration,
            None => sqlx::query_as::<_, BatchMigration>(
                "SELECT id, status FROM record_blueprint_migrations WHERE batch_id = $1 AND record_id = $2 AND workspace_id = $3 AND task_owned FOR UPDATE",
            )
            .bind(batch.id)
            .bind(record_id)
            .bind(workspace_id)
            .fetch_one(&mut *transaction)
            .await?,
        };
        self.commit_mutation(transaction).await?;
        Ok(migration)
    }

    async fn begin_batch_migration(&self, migration_id: Uuid) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE record_blueprint_migrations SET status = 'migrating', started_at = COALESCE(started_at, now()) WHERE id = $1 AND workspace_id = $2 AND status IN ('ready', 'migrating')",
        )
        .bind(migration_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await
    }

    async fn record_batch_failure(
        &self,
        migration_id: Uuid,
        message: &str,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE record_blueprint_migrations SET status = 'failed', issues = $2, completed_at = now() WHERE id = $1 AND workspace_id = $3 AND status IN ('pending', 'ready', 'migrating')",
        )
        .bind(migration_id)
        .bind(json!([{"kind": "migration_failed", "message": message}]))
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await
    }
}

fn retryable_batch_error(error: &RepositoryError) -> bool {
    matches!(error, RepositoryError::Database(_))
}

fn removal_policy_covers_preview(
    removal_policy: &serde_json::Value,
    issues: &[crate::model::MigrationIssue],
) -> bool {
    let Some(policy) =
        serde_json::from_value::<BlueprintMigrationRemovalPolicy>(removal_policy.clone()).ok()
    else {
        return false;
    };
    policy.disposition == "archive"
        && !issues.is_empty()
        && issues.iter().all(|issue| {
            issue.kind == "removed"
                && issue
                    .attribute_code
                    .as_ref()
                    .is_some_and(|code| policy.attribute_codes.contains(code))
        })
}

fn safe_automatic_migration(
    source: &crate::model::BlueprintWithAttributes,
    target: &crate::model::BlueprintWithAttributes,
) -> Option<Vec<String>> {
    if source.blueprint.record_schema != target.blueprint.record_schema {
        return None;
    }
    let target_attributes: HashMap<_, _> = target
        .attributes
        .iter()
        .map(|attribute| (attribute.code.as_str(), attribute))
        .collect();
    let mut removed = Vec::new();
    for source_attribute in &source.attributes {
        let Some(target_attribute) = target_attributes.get(source_attribute.code.as_str()) else {
            removed.push(source_attribute.code.clone());
            continue;
        };
        if source_attribute.value_type != target_attribute.value_type
            || source_attribute.value_schema != target_attribute.value_schema
            || source_attribute.default_value != target_attribute.default_value
            || source_attribute.file_policy != target_attribute.file_policy
            || source_attribute.target_blueprint_codes != target_attribute.target_blueprint_codes
            || source_attribute.cardinality != target_attribute.cardinality
            || source_attribute.target_cardinality != target_attribute.target_cardinality
            || source_attribute.context_fallback != target_attribute.context_fallback
            || source_attribute.context_editable != target_attribute.context_editable
            || source_attribute.readonly != target_attribute.readonly
        {
            return None;
        }
    }
    Some(removed)
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    /// Transitional reconciliation for batches committed by an API version
    /// before task delivery owned this kind. It is safe to run repeatedly and
    /// is deliberately not a process-local execution/recovery loop.
    pub async fn backfill_safe_blueprint_migration_tasks(&self) -> Result<(), RepositoryError> {
        let batches = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT workspace_id, id FROM blueprint_migration_batches WHERE status IN ('queued', 'running')",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut transaction = self.pool.begin().await?;
        for (workspace_id, batch_id) in batches {
            self.enqueue_task(
                &mut transaction,
                TaskInsert {
                    workspace_id,
                    kind: TaskKind::BlueprintMigrationBatchV1,
                    subject_id: batch_id,
                    generation: 0,
                    payload: json!({"batch_id": batch_id.to_string()}),
                    correlation_id: None,
                    causation_id: None,
                },
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod batch_error_tests {
    use super::*;

    #[test]
    fn database_failures_remain_retryable_but_validation_failures_are_terminal() {
        assert!(retryable_batch_error(&RepositoryError::Database(
            sqlx::Error::PoolClosed
        )));
        assert!(!retryable_batch_error(
            &RepositoryError::InvalidBlueprintDefinition("invalid".into())
        ));
    }
}
