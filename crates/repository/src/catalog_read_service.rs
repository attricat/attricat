//! Shared application service for catalogue reads exposed through HTTP and agents.

use uuid::Uuid;

use crate::{
    model::{Entity, FormAttributeValue},
    repository::{CatalogRepository, RepositoryError},
};

pub struct CatalogReadService<'a> {
    repository: &'a CatalogRepository,
}

impl<'a> CatalogReadService<'a> {
    pub fn new(repository: &'a CatalogRepository) -> Self {
        Self { repository }
    }

    /// Loads the entity and its current form values as one application-level
    /// read model. Callers may add presentation-specific data such as the
    /// blueprint definition.
    pub async fn entity_with_values(
        &self,
        entity_id: Uuid,
    ) -> Result<(Entity, Vec<FormAttributeValue>), RepositoryError> {
        let entity = self
            .repository
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let values = self.repository.form_values(entity_id).await?;
        Ok((entity, values))
    }
}
