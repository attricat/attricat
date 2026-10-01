use super::*;
use catalog_validation::status::{STATUS_KEY, validate_status_transition};
use chrono::Utc;

impl CatalogRepository {
    pub(super) async fn has_status_writes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        values: &[NewAttributeValue],
        removed: &[AttributeValueSelector],
    ) -> Result<bool, RepositoryError> {
        let statuses = sqlx::query_as::<_, (Uuid, String)>("SELECT id, code FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_schema ? 'x-attricat-status'")
            .bind(entity.blueprint_id).bind(entity.blueprint_version).bind(entity.id)
            .fetch_all(&mut **transaction).await?;
        Ok(statuses.iter().any(|(id, code)| {
            removed.iter().any(|selector| &selector.attribute_code == code)
                || values.iter().any(|value| matches!(value, NewAttributeValue::Scalar { attribute_id, attribute_code, .. } if attribute_id.as_ref() == Some(id) || attribute_code.as_ref() == Some(code)))
        }))
    }

    pub(super) async fn check_status_precondition(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        expected_updated_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        if expected_updated_at.is_some_and(|expected| expected != entity.updated_at) {
            return Err(RepositoryError::StaleEntity);
        }
        if expected_updated_at.is_none() {
            let has_status: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_schema ? 'x-attricat-status')")
                .bind(entity.blueprint_id).bind(entity.blueprint_version).bind(entity.id)
                .fetch_one(&mut **transaction).await?;
            if has_status {
                return Err(RepositoryError::StatusPreconditionRequired);
            }
        }
        Ok(())
    }

    /// Run once on the final transaction state, using the locked entity's saved
    /// projection as the baseline. Inheritance is resolved on both sides.
    pub(super) async fn validate_status_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let attributes = sqlx::query_as::<_, (String, Value, String)>(
            "SELECT code, value_schema, context_fallback FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_schema ? $4",
        ).bind(entity.blueprint_id).bind(entity.blueprint_version).bind(entity.id).bind(STATUS_KEY)
            .fetch_all(&mut **transaction).await?;
        if attributes.is_empty() {
            return Ok(());
        }
        let contexts = sqlx::query_as::<_, (Uuid, String, Option<Uuid>)>(
            "SELECT id, code, parent_id FROM attribute_contexts WHERE workspace_id = $1",
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&mut **transaction)
        .await?;
        let after = Self::build_preview_projection(transaction, entity.id).await?;
        let before = entity.projections.get("preview").unwrap_or(&Value::Null);
        for (code, schema, fallback) in attributes {
            for context in &contexts {
                let mut path = vec![context.1.as_str()];
                let mut parent = if fallback == "none" { None } else { context.2 };
                let mut visited = std::collections::HashSet::from([context.0]);
                while let Some(item) =
                    parent.and_then(|id| contexts.iter().find(|item| item.0 == id))
                {
                    if !visited.insert(item.0) {
                        return Err(RepositoryError::InvalidContext);
                    }
                    path.push(item.1.as_str());
                    parent = item.2;
                }
                let effective = |projection: &Value| {
                    path.iter()
                        .find_map(|context| {
                            projection
                                .get(*context)
                                .and_then(|values| values.get(&code))
                        })
                        .cloned()
                        .unwrap_or(Value::Null)
                };
                validate_status_transition(&schema, &effective(before), &effective(&after))
                    .map_err(|message| RepositoryError::AttributeValueSchemaMismatch {
                        attribute: code.clone(),
                        instance_path: String::new(),
                        message: format!("{message} (context: {})", context.1),
                    })?;
            }
        }
        Ok(())
    }
}
