//! Atomic changes across several entities.
//!
//! A batch runs ordinary create, update and delete operations, in order, in
//! one transaction. Every operation applies the same validation, locking,
//! status-transition, unique-key and hierarchy rules as its single-entity
//! endpoint, and stages its own audit evidence and domain event. Nothing is
//! committed, audited or published unless every operation succeeds.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{Value, json};
use uuid::Uuid;

use super::entity_commands::ChosenIdEntityCreate;
use super::{AuthorizationActor, CatalogRepository, RepositoryError, validate_code};
use crate::model::{
    EntityBatchOperation, EntityBatchOperationResult, EntityBatchRequest, EntityBatchResponse,
    MAX_ENTITY_BATCH_OPERATIONS, MAX_ENTITY_BATCH_VALUES, SearchBlueprint, UpdateEntityFormRequest,
};

impl CatalogRepository {
    /// Checks every operation's permission for one principal, and for a
    /// personal API token its live permissions, before a batch runs. Existing
    /// entities are checked in one query per permission.
    pub async fn is_authorized_for_entity_batch(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        token_id: Option<Uuid>,
        request: &EntityBatchRequest,
    ) -> Result<bool, RepositoryError> {
        let actor = AuthorizationActor { user_id, token_id };
        let mut targets: BTreeMap<&str, Vec<Uuid>> = BTreeMap::new();
        let mut untargeted = BTreeSet::new();
        for operation in &request.operations {
            match operation.permission() {
                (permission, Some(entity_id)) => {
                    targets.entry(permission).or_default().push(entity_id);
                }
                (permission, None) => {
                    untargeted.insert(permission);
                }
            }
        }
        let mut connection = self.pool.acquire().await?;
        for permission in untargeted {
            if !Self::principal_may_on(&mut connection, actor, workspace_id, permission, None, None)
                .await?
            {
                return Ok(false);
            }
        }
        for (permission, entity_ids) in targets {
            let permitted = Self::principal_entity_ids_on(
                &mut connection,
                actor,
                workspace_id,
                permission,
                &entity_ids,
            )
            .await?;
            if !entity_ids.iter().all(|id| permitted.contains(id)) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Applies every operation or none. A failing operation aborts the batch
    /// with [`RepositoryError::EntityBatchOperationFailed`], which names the
    /// operation and keeps its original error.
    pub async fn apply_entity_batch(
        &self,
        request: EntityBatchRequest,
    ) -> Result<EntityBatchResponse, RepositoryError> {
        validate_entity_batch(&request)?;
        let operation_count = request.operations.len();
        let mut transaction = self.pool.begin().await?;
        // Lock order matches single-entity writes: the workspace relationship
        // lock first, then entity rows in ID order, so concurrent batches that
        // touch the same entities cannot deadlock.
        if request
            .operations
            .iter()
            .any(EntityBatchOperation::writes_relationships)
        {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let mut existing: Vec<_> = request
            .operations
            .iter()
            .filter(|operation| !matches!(operation, EntityBatchOperation::Create { .. }))
            .filter_map(EntityBatchOperation::entity_id)
            .collect();
        existing.sort();
        super::entity_commands::lock_entity_writes(&mut transaction, self.workspace_id.0, false)
            .await?;
        sqlx::query(
            "SELECT id FROM entities WHERE workspace_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        )
        .bind(self.workspace_id.0)
        .bind(&existing)
        .execute(&mut *transaction)
        .await?;

        let mut results = Vec::with_capacity(operation_count);
        for (index, operation) in request.operations.into_iter().enumerate() {
            let entity_id = operation.entity_id();
            let failed = |source: RepositoryError| RepositoryError::EntityBatchOperationFailed {
                index,
                entity_id,
                source: Box::new(source),
            };
            let result = self
                .apply_entity_batch_operation(&mut transaction, index, operation_count, operation)
                .await
                .map_err(failed)?;
            results.push(result);
        }
        transaction.commit().await?;
        Ok(EntityBatchResponse {
            operations: results,
        })
    }

    async fn apply_entity_batch_operation(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        index: usize,
        operation_count: usize,
        operation: EntityBatchOperation,
    ) -> Result<EntityBatchOperationResult, RepositoryError> {
        Ok(match operation {
            EntityBatchOperation::Create {
                entity_id,
                blueprint,
                values,
                system_tags,
                system_metadata,
            } => {
                let entity_id = entity_id.unwrap_or_else(Uuid::new_v4);
                let taken = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM entities WHERE id = $1)",
                )
                .bind(entity_id)
                .fetch_one(&mut **transaction)
                .await?;
                if taken {
                    return Err(RepositoryError::EntityIdTaken(entity_id));
                }
                let (blueprint_id, blueprint_version) = self
                    .batch_blueprint_revision(transaction, &blueprint)
                    .await?;
                let repository = self.for_batch_operation(index, operation_count, entity_id);
                let (entity, changes, event) = repository
                    .create_entity_in_transaction(
                        transaction,
                        ChosenIdEntityCreate {
                            entity_id,
                            blueprint_id,
                            blueprint_version,
                            values,
                            files: super::entity_commands::CreateFileValues::None,
                            system_tags,
                            system_metadata,
                            host_sample_marker: false,
                        },
                    )
                    .await?;
                repository
                    .stage_entity_mutation(transaction, changes, event)
                    .await?;
                EntityBatchOperationResult::Create { entity }
            }
            EntityBatchOperation::Update {
                entity_id,
                expected_updated_at,
                values,
                relationships,
                remove_values,
                system_tags,
                system_metadata,
            } => {
                let repository = self.for_batch_operation(index, operation_count, entity_id);
                let (entity, changes, event) = repository
                    .update_entity_in_transaction(
                        transaction,
                        entity_id,
                        UpdateEntityFormRequest {
                            expected_updated_at,
                            values,
                            relationships,
                            remove_values,
                            system_tags,
                            system_metadata,
                        },
                    )
                    .await?;
                repository
                    .stage_entity_mutation(transaction, changes, event)
                    .await?;
                EntityBatchOperationResult::Update { entity }
            }
            EntityBatchOperation::Delete {
                entity_id,
                expected_updated_at,
            } => {
                let repository = self.for_batch_operation(index, operation_count, entity_id);
                let (changes, event) = repository
                    .delete_entity_in_transaction(transaction, entity_id, expected_updated_at)
                    .await?;
                repository
                    .stage_entity_mutation(transaction, changes, event)
                    .await?;
                EntityBatchOperationResult::Delete { entity_id }
            }
        })
    }

    /// Resolves a published entity blueprint revision inside the batch
    /// transaction, so the batch sees one consistent catalog state.
    async fn batch_blueprint_revision(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        blueprint: &SearchBlueprint,
    ) -> Result<(Uuid, i64), RepositoryError> {
        validate_code(&blueprint.code)?;
        sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT id, version FROM blueprints WHERE workspace_id = $1 AND code = $2 AND ($3::bigint IS NULL OR version = $3) AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(self.workspace_id.0)
        .bind(&blueprint.code)
        .bind(blueprint.version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint"))
    }

    /// Each operation is audited against its own entity, with the batch
    /// position, while sharing the request and correlation IDs.
    fn for_batch_operation(&self, index: usize, operation_count: usize, entity_id: Uuid) -> Self {
        let mut repository = self.clone();
        if let Some(audit) = repository.audit_context.as_mut() {
            audit.target = json!({ "type": "entity", "id": entity_id });
            if let Value::Object(metadata) = &mut audit.metadata {
                metadata.insert(
                    "batch".to_owned(),
                    json!({ "operation_index": index, "operation_count": operation_count }),
                );
            }
        }
        repository
    }
}

fn validate_entity_batch(request: &EntityBatchRequest) -> Result<(), RepositoryError> {
    let operations = &request.operations;
    if operations.is_empty() || operations.len() > MAX_ENTITY_BATCH_OPERATIONS {
        return Err(RepositoryError::InvalidEntityBatch(format!(
            "a batch holds 1 to {MAX_ENTITY_BATCH_OPERATIONS} operations"
        )));
    }
    if operations
        .iter()
        .map(EntityBatchOperation::value_count)
        .sum::<usize>()
        > MAX_ENTITY_BATCH_VALUES
    {
        return Err(RepositoryError::InvalidEntityBatch(format!(
            "a batch writes at most {MAX_ENTITY_BATCH_VALUES} values, relationship targets and removals"
        )));
    }
    let mut entities = HashSet::new();
    for (index, operation) in operations.iter().enumerate() {
        if let Some(entity_id) = operation.entity_id()
            && !entities.insert(entity_id)
        {
            return Err(RepositoryError::InvalidEntityBatch(format!(
                "operation {index} repeats entity {entity_id}; combine its changes into one operation"
            )));
        }
    }
    Ok(())
}
