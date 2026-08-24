use sqlx::query_as;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError, validate_code};
use crate::model::{AttributeContext, CreateAttributeContext, Entity, UpdateAttributeContext};

impl CatalogRepository {
    pub async fn create_context(
        &self,
        input: CreateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if input.code == "default" {
            return Err(RepositoryError::ReservedContextCode);
        }
        validate_code(&input.code)?;
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(input.parent_id)
        .fetch_one(&self.pool)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        Ok(query_as::<_, AttributeContext>(
            r#"INSERT INTO attribute_contexts (id, code, data, parent_id)
            VALUES ($1, $2, $3, $4) RETURNING id, code, data, parent_id"#,
        )
        .bind(Uuid::new_v4())
        .bind(input.code)
        .bind(input.data)
        .bind(input.parent_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_context_by_code(
        &self,
        code: &str,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        validate_code(code)?;
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_contexts(&self) -> Result<Vec<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_authorized_contexts(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM catalog_authorized_contexts($1, $2)",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_context(
        &self,
        id: Uuid,
        input: UpdateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if id == Self::DEFAULT_CONTEXT_ID {
            return Err(RepositoryError::DefaultContextProtected);
        }
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }
        let mut transaction = self.pool.begin().await?;
        // Reparenting changes resolved values for every entity, so it must serialize with writes.
        sqlx::query("LOCK TABLE entities IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *transaction)
            .await?;
        let context_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if !context_exists {
            return Err(RepositoryError::NotFound("context"));
        }
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(input.parent_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        let result = query_as::<_, AttributeContext>(r#"WITH RECURSIVE descendants AS (
                SELECT id FROM attribute_contexts WHERE id = $1
                UNION ALL SELECT c.id FROM attribute_contexts c JOIN descendants d ON c.parent_id = d.id
            ) UPDATE attribute_contexts SET parent_id = $2, data = $3
              WHERE id = $1 AND $2 NOT IN (SELECT id FROM descendants)
              RETURNING id, code, data, parent_id"#)
            .bind(id).bind(input.parent_id).bind(input.data).fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::ContextCycle)?;
        let entities = query_as::<_, Entity>("SELECT id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at FROM entities WHERE deleted_at IS NULL")
            .fetch_all(&mut *transaction).await?;
        for entity in &entities {
            self.validate_entity_schema(&mut transaction, entity)
                .await?;
        }
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn delete_context(&self, id: Uuid) -> Result<(), RepositoryError> {
        if id == Self::DEFAULT_CONTEXT_ID {
            return Err(RepositoryError::DefaultContextProtected);
        }
        let context_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        if !context_exists {
            return Err(RepositoryError::NotFound("context"));
        }
        let result = sqlx::query("DELETE FROM attribute_contexts c WHERE c.id = $1 AND NOT EXISTS (SELECT 1 FROM attribute_contexts child WHERE child.parent_id = c.id) AND NOT EXISTS (SELECT 1 FROM attribute_values value WHERE value.context_id = c.id)")
            .bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::ContextInUse);
        }
        Ok(())
    }

    pub(super) async fn get_context_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
