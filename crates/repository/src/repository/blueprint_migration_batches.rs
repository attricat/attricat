use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use crate::{
    model::{BlueprintMigrationBatch, BlueprintMigrationBatchStatus, MigrateEntityRequest},
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, Db<BlueprintMigrationBatchStatus>>(
            r#"WITH stats AS (
                   SELECT m.batch_id,
                          COUNT(DISTINCT m.entity_id) AS processed_entities,
                          COUNT(DISTINCT m.entity_id) FILTER (WHERE m.status = 'migrated') AS migrated_entities,
                          COUNT(DISTINCT m.entity_id) FILTER (WHERE m.status IN ('needs_input', 'blocked')) AS needs_input_entities,
                          COUNT(DISTINCT m.entity_id) FILTER (WHERE m.status = 'failed') AS failed_entities
                   FROM entity_blueprint_migrations m
                   JOIN blueprint_migration_batches scoped ON scoped.id = m.batch_id
                   WHERE m.workspace_id = $1
                     AND scoped.workspace_id = $1
                     AND scoped.blueprint_id = $2
                   GROUP BY m.batch_id
               )
               SELECT b.id, b.blueprint_id, b.target_version, b.status,
                      b.created_at, b.started_at, b.completed_at,
                      COALESCE(s.processed_entities, 0) + (
                          SELECT COUNT(*) FROM entities e
                          WHERE e.workspace_id = b.workspace_id
                            AND e.blueprint_id = b.blueprint_id
                            AND e.blueprint_version < b.target_version
                            AND e.deleted_at IS NULL
                            AND NOT EXISTS (
                                SELECT 1 FROM entity_blueprint_migrations m
                                WHERE m.batch_id = b.id AND m.entity_id = e.id
                            )
                      ) AS total_entities,
                      COALESCE(s.processed_entities, 0) AS processed_entities,
                      COALESCE(s.migrated_entities, 0) AS migrated_entities,
                      COALESCE(s.needs_input_entities, 0) AS needs_input_entities,
                      COALESCE(s.failed_entities, 0) AS failed_entities
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        if let Some(existing) = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "SELECT id, blueprint_id, target_version, status, created_at, started_at, completed_at FROM blueprint_migration_batches WHERE workspace_id = $1 AND blueprint_id = $2 AND target_version = $3 AND status IN ('queued', 'running')",
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
        if target.blueprint.version != target_version || target.blueprint.kind != "entity" {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        }
        let revisions = self.list_blueprint_revisions(blueprint_id).await?;
        let Some(source_revision) = revisions.iter().find(|revision| {
            revision.version == target_version - 1 && revision.status == "published"
        }) else {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        };
        let source = self
            .get_blueprint_revision(blueprint_id, source_revision.version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint version"))?;
        if !safe_automatic_migration(&source, &target) {
            return Err(RepositoryError::BlueprintMigrationNotSafe);
        }

        let mut transaction = self.pool.begin().await?;
        let batch = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "INSERT INTO blueprint_migration_batches (id, workspace_id, blueprint_id, target_version, status) VALUES ($1, $2, $3, $4, 'queued') ON CONFLICT (workspace_id, blueprint_id, target_version) WHERE status IN ('queued', 'running') DO UPDATE SET workspace_id = EXCLUDED.workspace_id RETURNING id, blueprint_id, target_version, status, created_at, started_at, completed_at",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
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
        transaction.commit().await?;
        Ok(batch.into_domain())
    }

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

    /// Runs a batch under the shared task lease. Each entity is reserved before
    /// preview/migration, making `(batch_id, entity_id)` its durable retry
    /// identity. Every progress checkpoint commits through the task fence.
    pub async fn run_safe_blueprint_migration_batch_task(
        &self,
        batch_id: Uuid,
        page_size: usize,
        concurrency: usize,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        let batch = sqlx::query_as::<_, Db<BlueprintMigrationBatch>>(
            "UPDATE blueprint_migration_batches SET status = 'running', started_at = COALESCE(started_at, now()) WHERE id = $1 AND workspace_id = $2 AND status IN ('queued', 'running') RETURNING id, blueprint_id, target_version, status, created_at, started_at, completed_at",
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
        loop {
            let candidates = match cursor {
                Some((created_at, id)) => {
                    sqlx::query_as::<_, BatchCandidate>(
                        r#"SELECT e.id, e.created_at
                           FROM entities e
                           WHERE e.blueprint_id = $1
                             AND e.blueprint_version < $2
                             AND e.workspace_id = $3
                             AND e.deleted_at IS NULL
                             AND (e.created_at, e.id) < ($4, $5)
                             AND NOT EXISTS (
                                 SELECT 1 FROM entity_blueprint_migrations m
                                 WHERE m.batch_id = $6 AND m.entity_id = e.id
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
                           FROM entities e
                           WHERE e.blueprint_id = $1
                             AND e.blueprint_version < $2
                             AND e.workspace_id = $3
                             AND e.deleted_at IS NULL
                             AND NOT EXISTS (
                                 SELECT 1 FROM entity_blueprint_migrations m
                                 WHERE m.batch_id = $4 AND m.entity_id = e.id
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
            let results = stream::iter(candidate_ids)
                .map(|entity_id| self.process_batch_candidate(&batch, entity_id))
                .buffer_unordered(concurrency)
                .collect::<Vec<_>>()
                .await;
            for result in results {
                let outcome = result?;
                metrics::counter!("catalog_blueprint_migration_entities_total", "outcome" => outcome)
                    .increment(1);
            }
            let elapsed = page_started.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                metrics::histogram!("catalog_blueprint_migration_entities_per_second")
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
        entity_id: Uuid,
    ) -> Result<&'static str, RepositoryError> {
        let migration = self.reserve_batch_migration(batch, entity_id).await?;
        match migration.status.as_str() {
            "migrated" | "needs_input" | "blocked" | "failed" | "skipped" | "superseded" => {
                return Ok("already_terminal");
            }
            "pending" => match self
                .preview_entity_migration_into(entity_id, Some(migration.id))
                .await
            {
                Ok(preview) if preview.status == "ready" => {}
                Ok(_) => return Ok("needs_input"),
                Err(error) => {
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
            .migrate_entity_to_latest(
                entity_id,
                MigrateEntityRequest {
                    migration_id: migration.id,
                    expected_target_version: batch.target_version,
                    values: Vec::new(),
                    relationships: Vec::new(),
                    discard_attributes: Vec::new(),
                },
            )
            .await
        {
            Ok(_) => Ok("migrated"),
            Err(error) => {
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
        entity_id: Uuid,
    ) -> Result<BatchMigration, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as::<_, BatchMigration>(
            "INSERT INTO entity_blueprint_migrations (id, batch_id, workspace_id, entity_id, blueprint_id, source_version, target_version, status, issues, task_owned) SELECT $1, $2, $3, e.id, $4, e.blueprint_version, $5, 'pending', '[]'::jsonb, true FROM entities e WHERE e.id = $6 AND e.workspace_id = $3 ON CONFLICT (batch_id, entity_id) WHERE batch_id IS NOT NULL AND task_owned DO NOTHING RETURNING id, status",
        )
        .bind(Uuid::new_v4())
        .bind(batch.id)
        .bind(workspace_id)
        .bind(batch.blueprint_id)
        .bind(batch.target_version)
        .bind(entity_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let migration = match inserted {
            Some(migration) => migration,
            None => sqlx::query_as::<_, BatchMigration>(
                "SELECT id, status FROM entity_blueprint_migrations WHERE batch_id = $1 AND entity_id = $2 AND workspace_id = $3 AND task_owned FOR UPDATE",
            )
            .bind(batch.id)
            .bind(entity_id)
            .bind(workspace_id)
            .fetch_one(&mut *transaction)
            .await?,
        };
        self.commit_mutation(transaction).await?;
        Ok(migration)
    }

    async fn begin_batch_migration(&self, migration_id: Uuid) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE entity_blueprint_migrations SET status = 'migrating', started_at = COALESCE(started_at, now()) WHERE id = $1 AND workspace_id = $2 AND status IN ('ready', 'migrating')",
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE entity_blueprint_migrations SET status = 'failed', issues = $2, completed_at = now() WHERE id = $1 AND workspace_id = $3 AND status IN ('pending', 'ready', 'migrating')",
        )
        .bind(migration_id)
        .bind(json!([{"kind": "migration_failed", "message": message}]))
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await
    }
}

fn safe_automatic_migration(
    source: &crate::model::BlueprintWithAttributes,
    target: &crate::model::BlueprintWithAttributes,
) -> bool {
    if source.blueprint.entity_schema != target.blueprint.entity_schema {
        return false;
    }
    let target_attributes: HashMap<_, _> = target
        .attributes
        .iter()
        .map(|attribute| (attribute.code.as_str(), attribute))
        .collect();
    source.attributes.iter().all(|source_attribute| {
        target_attributes
            .get(source_attribute.code.as_str())
            .is_some_and(|target_attribute| {
                source_attribute.value_type == target_attribute.value_type
                    && source_attribute.value_schema == target_attribute.value_schema
                    && source_attribute.default_value == target_attribute.default_value
                    && source_attribute.file_policy == target_attribute.file_policy
                    && source_attribute.target_blueprint_code
                        == target_attribute.target_blueprint_code
                    && source_attribute.cardinality == target_attribute.cardinality
                    && source_attribute.target_cardinality == target_attribute.target_cardinality
                    && source_attribute.context_fallback == target_attribute.context_fallback
                    && source_attribute.context_editable == target_attribute.context_editable
                    && source_attribute.readonly == target_attribute.readonly
            })
    })
}
