use super::*;
use crate::model::{BlueprintMigrationBatch, BlueprintMigrationBatchStatus, MigrateEntityRequest};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

impl CatalogRepository {
    pub async fn list_blueprint_migration_batches(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<BlueprintMigrationBatchStatus>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, BlueprintMigrationBatchStatus>(
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
        .await?)
    }

    /// Starts the safe bulk path. The immediately preceding published revision
    /// must preserve every existing attribute's storage semantics and entity
    /// schema. Once admitted, the batch previews every older entity revision
    /// and automatically migrates only entities that need no input.
    pub async fn start_safe_blueprint_migration_batch(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
    ) -> Result<BlueprintMigrationBatch, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        if let Some(existing) = sqlx::query_as::<_, BlueprintMigrationBatch>(
            "SELECT id, blueprint_id, target_version, status, created_at, started_at, completed_at FROM blueprint_migration_batches WHERE workspace_id = $1 AND blueprint_id = $2 AND target_version = $3 AND status IN ('queued', 'running')",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(existing);
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

        let id = Uuid::new_v4();
        // The partial unique index makes an active batch a durable singleton,
        // even when two API processes receive the same request concurrently.
        // Returning the existing row also makes callers safely retryable.
        let batch = sqlx::query_as::<_, BlueprintMigrationBatch>(
            "INSERT INTO blueprint_migration_batches (id, workspace_id, blueprint_id, target_version, status) VALUES ($1, $2, $3, $4, 'queued') ON CONFLICT (workspace_id, blueprint_id, target_version) WHERE status IN ('queued', 'running') DO UPDATE SET workspace_id = EXCLUDED.workspace_id RETURNING id, blueprint_id, target_version, status, created_at, started_at, completed_at",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(target_version)
        .fetch_one(&self.pool)
        .await?;
        Ok(batch)
    }

    /// Runs one safe batch. Each entity still uses the ordinary transactional
    /// migration path, so an unexpected data problem is isolated and recorded.
    pub async fn run_safe_blueprint_migration_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<(), RepositoryError> {
        self.run_safe_blueprint_migration_batch_with_lease(batch_id, Uuid::new_v4())
            .await
    }

    pub(crate) async fn run_safe_blueprint_migration_batch_with_lease(
        &self,
        batch_id: Uuid,
        lease_owner: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        // Only one task may claim a queued or abandoned batch. The lease keeps
        // startup recovery from resetting work still owned by another API
        // process, while allowing a crashed process to be recovered.
        let batch = sqlx::query_as::<_, BlueprintMigrationBatch>(
            "UPDATE blueprint_migration_batches SET status = 'running', started_at = COALESCE(started_at, now()), lease_owner = $3, lease_until = now() + interval '15 minutes' WHERE id = $1 AND workspace_id = $2 AND (status = 'queued' OR (status = 'running' AND (lease_until IS NULL OR lease_until <= now()))) RETURNING id, blueprint_id, target_version, status, created_at, started_at, completed_at",
        )
        .bind(batch_id)
        .bind(workspace_id)
        .bind(lease_owner)
        .fetch_optional(&self.pool)
        .await?;
        let Some(batch) = batch else {
            return Ok(());
        };
        let entity_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM entities WHERE blueprint_id = $1 AND blueprint_version < $2 AND workspace_id = $3 AND deleted_at IS NULL ORDER BY id",
        )
        .bind(batch.blueprint_id)
        .bind(batch.target_version)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        for entity_id in entity_ids {
            self.renew_batch_lease(batch_id, workspace_id, lease_owner)
                .await?;
            match self.preview_entity_migration(entity_id).await {
                Ok(preview) => {
                    sqlx::query("UPDATE entity_blueprint_migrations SET batch_id = $2 WHERE id = $1 AND workspace_id = $3")
                        .bind(preview.migration_id)
                        .bind(batch_id)
                        .bind(workspace_id)
                        .execute(&self.pool)
                        .await?;
                    if preview.status != "ready" {
                        continue;
                    }
                    let result = self
                        .migrate_entity_to_latest(
                            entity_id,
                            MigrateEntityRequest {
                                migration_id: preview.migration_id,
                                expected_target_version: batch.target_version,
                                values: Vec::new(),
                                relationships: Vec::new(),
                                discard_attributes: Vec::new(),
                            },
                        )
                        .await;
                    if let Err(error) = result {
                        self.record_batch_failure(preview.migration_id, &error.to_string())
                            .await?;
                    }
                }
                Err(error) => {
                    let migration_id = Uuid::new_v4();
                    sqlx::query(
                        "INSERT INTO entity_blueprint_migrations (id, batch_id, workspace_id, entity_id, blueprint_id, source_version, target_version, status, issues, completed_at) VALUES ($1, $2, $3, $4, $5, $6, $7, 'failed', $8, now())",
                    )
                    .bind(migration_id)
                    .bind(batch_id)
                    .bind(workspace_id)
                    .bind(entity_id)
                    .bind(batch.blueprint_id)
                    .bind(batch.target_version - 1)
                    .bind(batch.target_version)
                    .bind(json!([{"kind": "migration_failed", "message": error.to_string()}]))
                    .execute(&self.pool)
                    .await?;
                }
            }
        }
        self.renew_batch_lease(batch_id, workspace_id, lease_owner)
            .await?;
        let result = sqlx::query("UPDATE blueprint_migration_batches SET status = 'completed', completed_at = now(), lease_owner = NULL, lease_until = NULL WHERE id = $1 AND workspace_id = $2 AND status = 'running' AND lease_owner = $3")
            .bind(batch_id)
            .bind(workspace_id)
            .bind(lease_owner)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(RepositoryError::InvalidBlueprintDefinition(
                "safe blueprint migration batch lease was lost".into(),
            ))
        }
    }

    /// Returns a claimed batch to the durable queue during graceful shutdown.
    /// The lease owner predicate prevents a stale task from releasing another
    /// process's work.
    pub(crate) async fn release_safe_blueprint_migration_batch_lease(
        &self,
        batch_id: Uuid,
        lease_owner: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        sqlx::query("UPDATE blueprint_migration_batches SET status = 'queued', started_at = NULL, lease_owner = NULL, lease_until = NULL WHERE id = $1 AND workspace_id = $2 AND status = 'running' AND lease_owner = $3")
            .bind(batch_id)
            .bind(workspace_id)
            .bind(lease_owner)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Requeue batches interrupted by a previous API process and return all
    /// queued work. Claiming in `run_safe_blueprint_migration_batch` keeps this
    /// safe when multiple processes recover at once.
    pub async fn recover_safe_blueprint_migration_batches(
        &self,
    ) -> Result<Vec<(Uuid, Uuid)>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("UPDATE blueprint_migration_batches SET status = 'queued', started_at = NULL, lease_owner = NULL, lease_until = NULL WHERE status = 'running' AND (lease_until IS NULL OR lease_until <= now())")
            .execute(&mut *transaction)
            .await?;
        let batches = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT workspace_id, id FROM blueprint_migration_batches WHERE status = 'queued' ORDER BY created_at, id",
        )
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(batches)
    }

    async fn renew_batch_lease(
        &self,
        batch_id: Uuid,
        workspace_id: Uuid,
        lease_owner: Uuid,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE blueprint_migration_batches SET lease_until = now() + interval '15 minutes' WHERE id = $1 AND workspace_id = $2 AND status = 'running' AND lease_owner = $3")
            .bind(batch_id)
            .bind(workspace_id)
            .bind(lease_owner)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(RepositoryError::InvalidBlueprintDefinition(
                "safe blueprint migration batch lease was lost".into(),
            ))
        }
    }

    async fn record_batch_failure(
        &self,
        migration_id: Uuid,
        message: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE entity_blueprint_migrations SET status = 'failed', issues = $2, completed_at = now() WHERE id = $1 AND workspace_id = $3")
            .bind(migration_id)
            .bind(json!([{"kind": "migration_failed", "message": message}]))
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .execute(&self.pool)
            .await?;
        Ok(())
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
