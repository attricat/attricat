//! Shared application service for catalogue reads exposed through HTTP and agents.

use uuid::Uuid;

use crate::{
    model::{FormAttributeValue, Record},
    repository::{AttricatRepository, RepositoryError},
};

pub struct AttricatReadService<'a> {
    repository: &'a AttricatRepository,
}

impl<'a> AttricatReadService<'a> {
    pub fn new(repository: &'a AttricatRepository) -> Self {
        Self { repository }
    }

    /// Loads the record and its current form values as one application-level
    /// read model. Callers may add presentation-specific data such as the
    /// blueprint definition.
    pub async fn record_with_values(
        &self,
        record_id: Uuid,
    ) -> Result<(Record, Vec<FormAttributeValue>), RepositoryError> {
        let record = self
            .repository
            .get_record(record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        let values = self.repository.form_values(record_id).await?;
        Ok((record, values))
    }
}
