use sqlx::query_as;
use uuid::Uuid;

use crate::domain_events::{
    CONTEXT_CREATED_V1, CONTEXT_DELETED_V1, CONTEXT_UPDATED_V1, ContextCreatedV1, EventSource,
    EventSourceKind, NewDomainEvent,
};

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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let parent_id = match input.parent_id {
            Some(parent_id) => parent_id,
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
            )
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(RepositoryError::InvalidContext)?,
        };
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(parent_id)
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        let mut transaction = self.pool.begin().await?;
        let context = query_as::<_, AttributeContext>(
            r#"INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id)
            VALUES ($1, $2, $3, $4, $5) RETURNING id, code, data, parent_id"#,
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(input.code)
        .bind(input.data)
        .bind(parent_id)
        .fetch_one(&mut *transaction)
        .await?;
        let payload = serde_json::to_value(ContextCreatedV1 {
            context_id: context.id,
            code: context.code.clone(),
            parent_id: context.parent_id,
        })
        .expect("context-created payload is serializable");
        self.commit_mutation_with_event(
            transaction,
            NewDomainEvent {
                event_type: CONTEXT_CREATED_V1.to_owned(),
                aggregate_kind: "context".to_owned(),
                aggregate_id: context.id,
                correlation_id: self
                    .audit_context
                    .as_ref()
                    .map(|audit| audit.correlation_id)
                    .unwrap_or_else(Uuid::new_v4),
                causation_id: None,
                source: EventSource {
                    kind: EventSourceKind::Api,
                    name: "catalog_api".to_owned(),
                },
                metadata: serde_json::json!({}),
                payload,
            },
        )
        .await?;
        Ok(context)
    }

    pub async fn get_context_by_code(
        &self,
        code: &str,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        validate_code(code)?;
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE code = $1 AND workspace_id = $2",
        )
        .bind(code)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_contexts(&self) -> Result<Vec<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_authorized_contexts(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<AttributeContext>, RepositoryError> {
        if !self.is_active_principal(user_id, workspace_id).await? {
            return Ok(Vec::new());
        }
        let contexts = query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let mut authorized = Vec::new();
        for context in contexts {
            if self
                .is_authorized(
                    user_id,
                    workspace_id,
                    "contexts.read",
                    Some(context.id),
                    None,
                )
                .await?
            {
                authorized.push(context);
            }
        }
        Ok(authorized)
    }

    pub async fn update_context(
        &self,
        id: Uuid,
        input: UpdateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }
        let mut transaction = self.pool.begin().await?;
        // Reparenting changes resolved values for every entity, so it must serialize with writes.
        sqlx::query("LOCK TABLE entities IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *transaction)
            .await?;
        let context_code = sqlx::query_scalar::<_, String>(
            "SELECT code FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("context"))?;
        if context_code == "default" {
            return Err(RepositoryError::DefaultContextProtected);
        }
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(input.parent_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_one(&mut *transaction)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        let result = query_as::<_, AttributeContext>(r#"WITH RECURSIVE descendants AS (
                SELECT id FROM attribute_contexts WHERE id = $1 AND workspace_id = $4
                UNION ALL SELECT c.id FROM attribute_contexts c JOIN descendants d ON c.parent_id = d.id AND c.workspace_id = $4
            ) UPDATE attribute_contexts SET parent_id = $2, data = $3
              WHERE id = $1 AND workspace_id = $4 AND $2 NOT IN (SELECT id FROM descendants)
              RETURNING id, code, data, parent_id"#)
            .bind(id).bind(input.parent_id).bind(input.data).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::ContextCycle)?;
        let entities = query_as::<_, Entity>("SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at FROM entities WHERE workspace_id = $1 AND deleted_at IS NULL")
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&mut *transaction).await?;
        for entity in &entities {
            self.validate_entity_schema(&mut transaction, entity)
                .await?;
        }
        self.commit_mutation_with_event(
            transaction,
            context_event(self, CONTEXT_UPDATED_V1, &result),
        )
        .await?;
        Ok(result)
    }

    pub async fn delete_context(&self, id: Uuid) -> Result<(), RepositoryError> {
        let context = query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("context"))?;
        if context.code == "default" {
            return Err(RepositoryError::DefaultContextProtected);
        }
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM attribute_contexts c WHERE c.id = $1 AND c.workspace_id = $2 AND NOT EXISTS (SELECT 1 FROM attribute_contexts child WHERE child.parent_id = c.id AND child.workspace_id = c.workspace_id) AND NOT EXISTS (SELECT 1 FROM attribute_values value WHERE value.context_id = c.id AND value.workspace_id = c.workspace_id)")
            .bind(id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).execute(&mut *transaction).await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::ContextInUse);
        }
        self.commit_mutation_with_event(
            transaction,
            context_event(self, CONTEXT_DELETED_V1, &context),
        )
        .await?;
        Ok(())
    }

    pub(super) async fn get_context_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?)
    }
}

fn context_event(
    repository: &CatalogRepository,
    event_type: &str,
    context: &AttributeContext,
) -> NewDomainEvent {
    NewDomainEvent {
        event_type: event_type.to_owned(),
        aggregate_kind: "context".to_owned(),
        aggregate_id: context.id,
        correlation_id: repository
            .audit_context
            .as_ref()
            .map(|audit| audit.correlation_id)
            .unwrap_or_else(Uuid::new_v4),
        causation_id: None,
        source: EventSource {
            kind: EventSourceKind::Api,
            name: "catalog_api".to_owned(),
        },
        metadata: serde_json::json!({}),
        payload: serde_json::to_value(ContextCreatedV1 {
            context_id: context.id,
            code: context.code.clone(),
            parent_id: context.parent_id,
        })
        .expect("context event payload is serializable"),
    }
}
