use super::*;
use crate::model::{BlueprintMigrationBatch, MigrateEntityRequest};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

impl CatalogRepository {
    /// Starts the deliberately narrow bulk path: the immediately preceding
    /// published revision must preserve every existing attribute's storage
    /// semantics and entity schema. New optional attributes are allowed.
    pub async fn start_safe_blueprint_migration_batch(
        &self,
        blueprint_id: Uuid,
        target_version: i64,
    ) -> Result<BlueprintMigrationBatch, RepositoryError> {
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let batch = sqlx::query_as::<_, BlueprintMigrationBatch>(
            "INSERT INTO blueprint_migration_batches (id, workspace_id, blueprint_id, target_version, status, started_at) VALUES ($1, $2, $3, $4, 'running', now()) RETURNING id, blueprint_id, target_version, status, created_at, started_at, completed_at",
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let batch = sqlx::query_as::<_, BlueprintMigrationBatch>(
            "SELECT id, blueprint_id, target_version, status, created_at, started_at, completed_at FROM blueprint_migration_batches WHERE id = $1 AND workspace_id = $2",
        )
        .bind(batch_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("migration batch"))?;
        if batch.status != "running" {
            return Ok(());
        }
        let entity_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM entities WHERE blueprint_id = $1 AND blueprint_version = $2 AND workspace_id = $3 AND deleted_at IS NULL ORDER BY id",
        )
        .bind(batch.blueprint_id)
        .bind(batch.target_version - 1)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        for entity_id in entity_ids {
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
        sqlx::query("UPDATE blueprint_migration_batches SET status = 'completed', completed_at = now() WHERE id = $1 AND workspace_id = $2")
            .bind(batch_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        Ok(())
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
