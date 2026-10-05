//! Shared application service for catalogue mutations.
//!
//! HTTP handlers and agent tools use this boundary rather than invoking
//! repository write methods directly, so catalogue write semantics stay
//! consistent across entry points.

use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, AttachReusableAttribute, AttributeContext, AttributeValue,
        BlueprintEntityPublicationSummary, BlueprintWithAttributes, CreateAttributeContext,
        CreateBlueprint, CreateEntityFormRequest, CreateReusableAttribute,
        CreateReusableAttributeGroup, Entity, EntityBatchRequest, EntityBatchResponse,
        EntityPublicationStatus, EntityReusableAttribute, MigrateEntityRequest,
        RelationshipMutation, ReusableAttribute, ReusableAttributeGroup, SearchBlueprint,
        UpdateAttributeContext, UpdateEntityFormRequest,
    },
    repository::{
        CatalogRepository, ExtensionCatalogBatch, ExtensionCatalogIntentOutcome, FileMetadata,
        RepositoryError,
    },
};

pub struct CatalogMutationService<'a> {
    repository: &'a CatalogRepository,
}

impl<'a> CatalogMutationService<'a> {
    pub fn new(repository: &'a CatalogRepository) -> Self {
        Self { repository }
    }

    /// Applies extension batch intents through the same transaction-aware
    /// mutation path as other catalog writers.
    pub async fn execute_extension_catalog_batch(
        &self,
        batch: ExtensionCatalogBatch,
    ) -> Result<Vec<ExtensionCatalogIntentOutcome>, RepositoryError> {
        self.repository.execute_extension_catalog_batch(batch).await
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
        entity_id: Uuid,
        input: AttachReusableAttribute,
    ) -> Result<EntityReusableAttribute, RepositoryError> {
        self.repository
            .attach_reusable_attribute(entity_id, input)
            .await
    }

    pub async fn attach_reusable_attribute_group(
        &self,
        entity_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<EntityReusableAttribute>, RepositoryError> {
        self.repository
            .attach_reusable_attribute_group(entity_id, group_id)
            .await
    }

    pub async fn create_entity(
        &self,
        input: CreateEntityFormRequest,
    ) -> Result<Entity, RepositoryError> {
        let blueprint = self.resolve_published_blueprint(&input.blueprint).await?;
        self.repository
            .create_entity_with_values(
                blueprint.blueprint.id,
                blueprint.blueprint.version,
                input.values,
                input.system_tags,
                input.system_metadata,
            )
            .await
    }

    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<(), RepositoryError> {
        self.repository.delete_entity(entity_id).await
    }

    pub async fn delete_entity_checked(
        &self,
        entity_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), RepositoryError> {
        self.repository
            .delete_entity_checked(entity_id, expected_updated_at)
            .await
    }

    pub async fn duplicate_entity(&self, entity_id: Uuid) -> Result<Entity, RepositoryError> {
        self.repository.duplicate_entity(entity_id).await
    }

    pub async fn update_entity(
        &self,
        entity_id: Uuid,
        input: UpdateEntityFormRequest,
    ) -> Result<Entity, RepositoryError> {
        self.repository
            .update_entity_with_values_checked(
                entity_id,
                input.values,
                input.relationships,
                input.remove_values,
                input.system_tags,
                input.system_metadata,
                input.expected_updated_at,
            )
            .await
    }

    /// Applies several entity writes atomically. Callers authorize every
    /// operation first with `CatalogRepository::is_authorized_for_entity_batch`.
    pub async fn apply_entity_batch(
        &self,
        request: EntityBatchRequest,
    ) -> Result<EntityBatchResponse, RepositoryError> {
        self.repository.apply_entity_batch(request).await
    }

    pub async fn publish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        self.repository.publish_entity(entity_id, context_id).await
    }

    pub async fn publish_entity_all_channels(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        self.repository.publish_entity_all_channels(entity_id).await
    }

    pub async fn publish_blueprint_entities(
        &self,
        blueprint_id: Uuid,
        version: i64,
        context_id: Option<Uuid>,
    ) -> Result<BlueprintEntityPublicationSummary, RepositoryError> {
        self.repository
            .publish_blueprint_entities(blueprint_id, version, context_id)
            .await
    }

    pub async fn unpublish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        self.repository
            .unpublish_entity(entity_id, context_id)
            .await
    }

    pub async fn migrate_entity(
        &self,
        entity_id: Uuid,
        input: MigrateEntityRequest,
    ) -> Result<Entity, RepositoryError> {
        self.repository
            .migrate_entity_to_latest(entity_id, input)
            .await
    }

    pub async fn migrate_entity_checked(
        &self,
        entity_id: Uuid,
        input: MigrateEntityRequest,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Entity, RepositoryError> {
        self.repository
            .migrate_entity_to_latest_checked(entity_id, input, expected_updated_at)
            .await
    }

    pub async fn append_values(
        &self,
        entity_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository.append_values(entity_id, input).await
    }

    pub async fn restore_value(
        &self,
        entity_id: Uuid,
        history_id: Uuid,
    ) -> Result<AttributeValue, RepositoryError> {
        self.repository.restore_value(entity_id, history_id).await
    }

    pub async fn restore_value_checked(
        &self,
        entity_id: Uuid,
        history_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<AttributeValue, RepositoryError> {
        self.repository
            .restore_value_checked(entity_id, history_id, expected_updated_at)
            .await
    }

    pub async fn replace_relationships(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository
            .replace_relationships(entity_id, input)
            .await
    }

    pub async fn remove_relationships(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository.remove_relationships(entity_id, input).await
    }

    /// Replaces (`replace`) or removes relationship targets with an optional
    /// optimistic-concurrency precondition.
    pub async fn mutate_relationships_checked(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
        replace: bool,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.repository
            .mutate_relationships_checked(entity_id, input, replace, expected_updated_at)
            .await
    }

    pub async fn link_file(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
    ) -> Result<FileMetadata, RepositoryError> {
        self.link_file_checked(entity_id, attribute_code, context_id, file_id, None)
            .await
    }

    pub async fn link_file_checked(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<FileMetadata, RepositoryError> {
        self.repository
            .link_file_to_attribute_checked(
                entity_id,
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
