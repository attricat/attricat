//! Atomic changes across several records.
//!
//! A batch runs ordinary create, update and delete operations, in order, in
//! one transaction. Every operation applies the same validation, locking,
//! status-transition, unique-key and hierarchy rules as its single-record
//! endpoint, and stages its own audit evidence and domain event. Nothing is
//! committed, audited or published unless every operation succeeds.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{Value, json};
use uuid::Uuid;

use super::record_commands::ChosenIdRecordCreate;
use super::{AttricatRepository, AuthorizationActor, RepositoryError, validate_code};
use crate::model::{
    MAX_RECORD_BATCH_OPERATIONS, MAX_RECORD_BATCH_VALUES, RecordBatchOperation,
    RecordBatchOperationResult, RecordBatchRequest, RecordBatchResponse, SearchBlueprint,
    UpdateRecordFormRequest,
};

impl AttricatRepository {
    /// Checks every operation's permission for one principal, and for a
    /// personal API token its live permissions, before a batch runs. Existing
    /// records are checked in one query per permission.
    pub async fn is_authorized_for_record_batch(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        token_id: Option<Uuid>,
        request: &RecordBatchRequest,
    ) -> Result<bool, RepositoryError> {
        let actor = AuthorizationActor { user_id, token_id };
        let mut targets: BTreeMap<&str, Vec<Uuid>> = BTreeMap::new();
        let mut untargeted = BTreeSet::new();
        for operation in &request.operations {
            match operation.permission() {
                (permission, Some(record_id)) => {
                    targets.entry(permission).or_default().push(record_id);
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
        for (permission, record_ids) in targets {
            let permitted = Self::principal_record_ids_on(
                &mut connection,
                actor,
                workspace_id,
                permission,
                &record_ids,
            )
            .await?;
            if !record_ids.iter().all(|id| permitted.contains(id)) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Applies every operation or none. A failing operation aborts the batch
    /// with [`RepositoryError::RecordBatchOperationFailed`], which names the
    /// operation and keeps its original error.
    pub async fn apply_record_batch(
        &self,
        request: RecordBatchRequest,
    ) -> Result<RecordBatchResponse, RepositoryError> {
        validate_record_batch(&request)?;
        let operation_count = request.operations.len();
        let mut transaction = self.pool.begin().await?;
        // Lock order matches single-record writes: the workspace relationship
        // lock first, then record rows in ID order, so concurrent batches that
        // touch the same records cannot deadlock.
        if request
            .operations
            .iter()
            .any(RecordBatchOperation::writes_relationships)
        {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let mut existing: Vec<_> = request
            .operations
            .iter()
            .filter(|operation| !matches!(operation, RecordBatchOperation::Create { .. }))
            .filter_map(RecordBatchOperation::record_id)
            .collect();
        existing.sort();
        super::record_commands::lock_record_writes(&mut transaction, self.workspace_id.0, false)
            .await?;
        sqlx::query(
            "SELECT id FROM records WHERE workspace_id = $1 AND id = ANY($2) ORDER BY id FOR UPDATE",
        )
        .bind(self.workspace_id.0)
        .bind(&existing)
        .execute(&mut *transaction)
        .await?;

        let mut results = Vec::with_capacity(operation_count);
        for (index, operation) in request.operations.into_iter().enumerate() {
            let record_id = operation.record_id();
            let failed = |source: RepositoryError| RepositoryError::RecordBatchOperationFailed {
                index,
                record_id,
                source: Box::new(source),
            };
            let result = self
                .apply_record_batch_operation(&mut transaction, index, operation_count, operation)
                .await
                .map_err(failed)?;
            results.push(result);
        }
        transaction.commit().await?;
        Ok(RecordBatchResponse {
            operations: results,
        })
    }

    async fn apply_record_batch_operation(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        index: usize,
        operation_count: usize,
        operation: RecordBatchOperation,
    ) -> Result<RecordBatchOperationResult, RepositoryError> {
        Ok(match operation {
            RecordBatchOperation::Create {
                record_id,
                blueprint,
                values,
                system_tags,
                system_metadata,
            } => {
                let record_id = record_id.unwrap_or_else(Uuid::new_v4);
                let taken = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM records WHERE id = $1)",
                )
                .bind(record_id)
                .fetch_one(&mut **transaction)
                .await?;
                if taken {
                    return Err(RepositoryError::RecordIdTaken(record_id));
                }
                let (blueprint_id, blueprint_version) = self
                    .batch_blueprint_revision(transaction, &blueprint)
                    .await?;
                let repository = self.for_batch_operation(index, operation_count, record_id);
                let (record, changes, event) = repository
                    .create_record_in_transaction(
                        transaction,
                        ChosenIdRecordCreate {
                            record_id,
                            blueprint_id,
                            blueprint_version,
                            values,
                            files: super::record_commands::CreateFileValues::None,
                            system_tags,
                            system_metadata,
                            host_sample_marker: false,
                        },
                    )
                    .await?;
                repository
                    .stage_record_mutation(transaction, changes, event)
                    .await?;
                RecordBatchOperationResult::Create { record }
            }
            RecordBatchOperation::Update {
                record_id,
                expected_updated_at,
                values,
                relationships,
                remove_values,
                system_tags,
                system_metadata,
            } => {
                let repository = self.for_batch_operation(index, operation_count, record_id);
                let (record, changes, event) = repository
                    .update_record_in_transaction(
                        transaction,
                        record_id,
                        UpdateRecordFormRequest {
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
                    .stage_record_mutation(transaction, changes, event)
                    .await?;
                RecordBatchOperationResult::Update { record }
            }
            RecordBatchOperation::Delete {
                record_id,
                expected_updated_at,
            } => {
                let repository = self.for_batch_operation(index, operation_count, record_id);
                let (changes, event) = repository
                    .delete_record_in_transaction(transaction, record_id, expected_updated_at)
                    .await?;
                repository
                    .stage_record_mutation(transaction, changes, event)
                    .await?;
                RecordBatchOperationResult::Delete { record_id }
            }
        })
    }

    /// Resolves a published record blueprint revision inside the batch
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

    /// Each operation is audited against its own record, with the batch
    /// position, while sharing the request and correlation IDs.
    fn for_batch_operation(&self, index: usize, operation_count: usize, record_id: Uuid) -> Self {
        let mut repository = self.clone();
        if let Some(audit) = repository.audit_context.as_mut() {
            audit.target = json!({ "type": "record", "id": record_id });
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

fn validate_record_batch(request: &RecordBatchRequest) -> Result<(), RepositoryError> {
    let operations = &request.operations;
    if operations.is_empty() || operations.len() > MAX_RECORD_BATCH_OPERATIONS {
        return Err(RepositoryError::InvalidRecordBatch(format!(
            "a batch holds 1 to {MAX_RECORD_BATCH_OPERATIONS} operations"
        )));
    }
    if operations
        .iter()
        .map(RecordBatchOperation::value_count)
        .sum::<usize>()
        > MAX_RECORD_BATCH_VALUES
    {
        return Err(RepositoryError::InvalidRecordBatch(format!(
            "a batch writes at most {MAX_RECORD_BATCH_VALUES} values, relationship targets and removals"
        )));
    }
    let mut records = HashSet::new();
    for (index, operation) in operations.iter().enumerate() {
        if let Some(record_id) = operation.record_id()
            && !records.insert(record_id)
        {
            return Err(RepositoryError::InvalidRecordBatch(format!(
                "operation {index} repeats record {record_id}; combine its changes into one operation"
            )));
        }
    }
    Ok(())
}
