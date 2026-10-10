//! Shared application service for catalogue mutations.
//!
//! HTTP handlers and agent tools use this boundary rather than invoking
//! repository write methods directly, so catalogue write semantics stay
//! consistent across entry points.

use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, AttachReusableAttribute, AttributeContext, AttributeValue,
        BlueprintRecordPublicationSummary, BlueprintWithAttributes, CreateAttributeContext,
        CreateBlueprint, CreateRecordFormRequest, CreateReusableAttribute,
        CreateReusableAttributeGroup, MigrateRecordRequest, Record, RecordBatchRequest,
        RecordBatchResponse, RecordPublicationStatus, RecordReusableAttribute,
        RelationshipMutation, ReusableAttribute, ReusableAttributeGroup, SearchBlueprint,
        UpdateAttributeContext, UpdateRecordFormRequest,
    },
    repository::{
        AttricatRepository, ExtensionAttricatBatch, ExtensionAttricatIntentOutcome, FileMetadata,
        RepositoryError,
    },
};

pub struct AttricatMutationService<'a> {
    repository: &'a AttricatRepository,
}

impl<'a> AttricatMutationService<'a> {
    pub fn new(repository: &'a AttricatRepository) -> Self {
        Self { repository }
    }

    /// Applies extension batch intents through the same transaction-aware
    /// mutation path as other catalog writers.
    pub async fn execute_extension_attricat_batch(
        &self,
        batch: ExtensionAttricatBatch,
    ) -> Result<Vec<ExtensionAttricatIntentOutcome>, RepositoryError> {
        self.repository
            .execute_extension_attricat_batch(batch)
            .await
    }

    pub async fn create_blueprint(
        &self,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        self.repository.create_blueprint(input).await
    }

    pub async fn create_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        self.repository
            .create_blueprint_revision(blueprint_id, input)
            .await
    }

    pub async fn publish_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        version: i64,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        self.repository
            .publish_blueprint_revision(blueprint_id, version)
            .await
    }

    pub async fn create_reusable_attribute(
        &self,
        input: CreateReusableAttribute,
    ) -> Result<ReusableAttribute, RepositoryError> {
        self.repository.create_reusable_attribute(input).await
    }

    pub async fn create_reusable_attribute_revision(
        &self,
        definition_id: Uuid,
        input: CreateReusableAttribute,
    ) -> Result<ReusableAttribute, RepositoryError> {
        self.repository
            .create_reusable_attribute_revision(definition_id, input)
            .await
    }

    pub async fn publish_reusable_attribute_revision(
        &self,
        revision_id: Uuid,
    ) -> Result<ReusableAttribute, RepositoryError> {
        self.repository
            .publish_reusable_attribute_revision(revision_id)
            .await
    }

    pub async fn create_reusable_attribute_group(
        &self,
        input: CreateReusableAttributeGroup,
    ) -> Result<ReusableAttributeGroup, RepositoryError> {
        self.repository.create_reusable_attribute_group(input).await
    }

    pub async fn attach_reusable_attribute(
        &self,
        record_id: Uuid,
        input: AttachReusableAttribute,
    ) -> Result<RecordReusableAttribute, RepositoryError> {
        self.repository
            .attach_reusable_attribute(record_id, input)
            .await
    }

    pub async fn attach_reusable_attribute_group(
        &self,
        record_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<RecordReusableAttribute>, RepositoryError> {
        self.repository
            .attach_reusable_attribute_group(record_id, group_id)
            .await
    }

    /// Creates a record. `input.files` must be files that `uploaded_by`
    /// staged for the blueprint; without an uploader none may be given.
    pub async fn create_record(
        &self,
        input: CreateRecordFormRequest,
        uploaded_by: Option<Uuid>,
    ) -> Result<Record, RepositoryError> {
        let blueprint = self.resolve_published_blueprint(&input.blueprint).await?;
        self.repository
            .create_record_with_staged_files(
                blueprint.blueprint.id,
                blueprint.blueprint.version,
                input.values,
                input.files,
                uploaded_by,
                input.system_tags,
                input.system_metadata,
            )
            .await
    }

    pub async fn delete_record(&self, record_id: Uuid) -> Result<(), RepositoryError> {
        self.repository.delete_record(record_id).await
    }

    pub async fn delete_record_checked(
        &self,
        record_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), RepositoryError> {
        self.repository
            .delete_record_checked(record_id, expected_updated_at)
            .await
    }

    pub async fn duplicate_record(&self, record_id: Uuid) -> Result<Record, RepositoryError> {
        self.repository.duplicate_record(record_id).await
    }

    pub async fn update_record(
        &self,
        record_id: Uuid,
        input: UpdateRecordFormRequest,
    ) -> Result<Record, RepositoryError> {
        self.repository
            .update_record_with_values_checked(
                record_id,
                input.values,
                input.relationships,
                input.remove_values,
                input.system_tags,
                input.system_metadata,
                input.expected_updated_at,
            )
            .await
    }

    /// Applies several record writes atomically. Callers authorize every
    /// operation first with `AttricatRepository::is_authorized_for_record_batch`.
    pub async fn apply_record_batch(
        &self,
        request: RecordBatchRequest,
    ) -> Result<RecordBatchResponse, RepositoryError> {
        self.repository.apply_record_batch(request).await
    }

    pub async fn publish_record(
        &self,
        record_id: Uuid,
        context_id: Uuid,
    ) -> Result<RecordPublicationStatus, RepositoryError> {
        self.repository.publish_record(record_id, context_id).await
    }

    pub async fn publish_record_all_channels(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<RecordPublicationStatus>, RepositoryError> {
        self.repository.publish_record_all_channels(record_id).await
    }

    pub async fn publish_blueprint_records(
        &self,
        blueprint_id: Uuid,
        version: i64,
        context_id: Option<Uuid>,
    ) -> Result<BlueprintRecordPublicationSummary, RepositoryError> {
        self.repository
            .publish_blueprint_records(blueprint_id, version, context_id)
            .await
    }

    pub async fn unpublish_record(
        &self,
        record_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        self.repository
            .unpublish_record(record_id, context_id)
            .await
    }

    pub async fn migrate_record(
        &self,
        record_id: Uuid,
        input: MigrateRecordRequest,
    ) -> Result<Record, RepositoryError> {
        self.repository
            .migrate_record_to_latest(record_id, input)
            .await
    }

    pub async fn migrate_record_checked(
        &self,
        record_id: Uuid,
        input: MigrateRecordRequest,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Record, RepositoryError> {
        self.repository
            .migrate_record_to_latest_checked(record_id, input, expected_updated_at)
            .await
    }

    pub async fn append_values(
        &self,
        record_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository.append_values(record_id, input).await
    }

    pub async fn restore_value(
        &self,
        record_id: Uuid,
        history_id: Uuid,
    ) -> Result<AttributeValue, RepositoryError> {
        self.repository.restore_value(record_id, history_id).await
    }

    pub async fn restore_value_checked(
        &self,
        record_id: Uuid,
        history_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<AttributeValue, RepositoryError> {
        self.repository
            .restore_value_checked(record_id, history_id, expected_updated_at)
            .await
    }

    pub async fn replace_relationships(
        &self,
        record_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository
            .replace_relationships(record_id, input)
            .await
    }

    pub async fn remove_relationships(
        &self,
        record_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository.remove_relationships(record_id, input).await
    }

    /// Replaces (`replace`) or removes relationship targets with an optional
    /// optimistic-concurrency precondition.
    pub async fn mutate_relationships_checked(
        &self,
        record_id: Uuid,
        input: RelationshipMutation,
        replace: bool,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository
            .mutate_relationships_checked(record_id, input, replace, expected_updated_at)
            .await
    }

    pub async fn link_file(
        &self,
        record_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
    ) -> Result<FileMetadata, RepositoryError> {
        self.link_file_checked(record_id, attribute_code, context_id, file_id, None)
            .await
    }

    pub async fn link_file_checked(
        &self,
        record_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<FileMetadata, RepositoryError> {
        self.repository
            .link_file_to_attribute_checked(
                record_id,
                attribute_code,
                context_id,
                file_id,
                expected_updated_at,
            )
            .await
    }

    pub async fn create_context(
        &self,
        input: CreateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        self.repository.create_context(input).await
    }

    pub async fn update_context(
        &self,
        id: Uuid,
        input: UpdateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        self.repository.update_context(id, input).await
    }

    pub async fn delete_context(&self, id: Uuid) -> Result<(), RepositoryError> {
        self.repository.delete_context(id).await
    }

    async fn resolve_published_blueprint(
        &self,
        blueprint: &SearchBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        match blueprint.version {
            Some(version) => {
                self.repository
                    .get_published_blueprint_by_code_and_version(&blueprint.code, version)
                    .await?
            }
            None => {
                self.repository
                    .get_blueprint_by_code(&blueprint.code)
                    .await?
            }
        }
        .ok_or(RepositoryError::NotFound("blueprint"))
    }
}
