//! Shared application service for catalogue mutations.
//!
//! HTTP handlers and agent tools use this boundary rather than invoking
//! repository write methods directly, so catalogue write semantics stay
//! consistent across entry points.

use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, AttributeContext, AttributeValue, BlueprintWithAttributes,
        CreateAttributeContext, CreateBlueprint, CreateEntityFormRequest, Entity,
        EntityPublicationStatus, MigrateEntityRequest, RelationshipMutation, SearchBlueprint,
        UpdateAttributeContext, UpdateEntityFormRequest,
    },
    repository::{CatalogRepository, FileMetadata, RepositoryError},
};

pub struct CatalogMutationService<'a> {
    repository: &'a CatalogRepository,
}

impl<'a> CatalogMutationService<'a> {
    pub fn new(repository: &'a CatalogRepository) -> Self {
        Self { repository }
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

    pub async fn update_entity(
        &self,
        entity_id: Uuid,
        input: UpdateEntityFormRequest,
    ) -> Result<Entity, RepositoryError> {
        self.repository
            .update_entity_with_values(
                entity_id,
                input.values,
                input.relationships,
                input.remove_values,
                input.system_tags,
                input.system_metadata,
            )
            .await
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

    pub async fn link_file(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
    ) -> Result<FileMetadata, RepositoryError> {
        self.repository
            .link_file_to_attribute(entity_id, attribute_code, context_id, file_id)
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
