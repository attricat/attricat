use crate::persistence_rows::{Db, IntoDomain};
use sqlx::query_as;
use uuid::Uuid;

use crate::domain_events::{
    CONTEXT_CREATED_V1, CONTEXT_DELETED_V1, CONTEXT_UPDATED_V1, ContextCreatedV1, NewDomainEvent,
};

use super::entity_commands::lock_entity_writes;
use super::generations::{Generation, advance_generation};
use super::{CatalogRepository, RepositoryError, validate_code};
use crate::model::{AttributeContext, CreateAttributeContext, Entity, UpdateAttributeContext};

impl CatalogRepository {
    pub async fn create_context(
        &self,
        input: CreateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let context = self
            .create_context_in_transaction(&mut transaction, Uuid::new_v4(), input)
            .await?;
        let payload = serde_json::to_value(ContextCreatedV1 {
            context_id: context.id,
            code: context.code.clone(),
            parent_id: context.parent_id,
        })
        .expect("context-created payload is serializable");
        self.commit_mutation_with_event(
            transaction,
            self.core_event(CONTEXT_CREATED_V1, "context", context.id, payload),
        )
        .await?;
        Ok(context)
    }

    /// Shared mutation seam for callers that must atomically persist additional
    /// evidence with an ordinary context creation.
    pub(super) async fn create_context_in_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        context_id: Uuid,
        input: CreateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if input.code == "default" {
            return Err(RepositoryError::ReservedContextCode);
        }
        validate_code(&input.code)?;
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }
        let workspace_id = self.workspace_id.0;
        super::lock_workspace_resource_code(transaction, workspace_id, &input.code).await?;
        if !super::workspace_resource_code_matches(transaction, workspace_id, &input.code)
            .await?
            .is_empty()
        {
            return Err(RepositoryError::CatalogCodeTaken);
        }
        let parent_id = match input.parent_id {
            Some(parent_id) => parent_id,
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
            )
            .bind(workspace_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or(RepositoryError::InvalidContext)?,
        };
        let parent_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(parent_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        // Lock order (see the module docs in `mod.rs`): the workspace row,
        // as blueprint publication takes it first, then this workspace's
        // entity-writer lock, which waits for its in-flight entity writers,
        // then the family key locks. The new context is indexed from
        // committed values and later writes see it.
        advance_generation(transaction, workspace_id, Generation::Contexts).await?;
        lock_entity_writes(transaction, workspace_id, true).await?;
        let key_families = self.lock_context_unique_keys(transaction).await?;
        let context = query_as::<_, Db<AttributeContext>>(
            r#"INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id)
            VALUES ($1, $2, $3, $4, $5) RETURNING id, code, data, parent_id"#,
        )
        .bind(context_id)
        .bind(workspace_id)
        .bind(input.code)
        .bind(input.data)
        .bind(parent_id)
        .fetch_one(&mut **transaction)
        .await?
        .into_domain();
        self.seed_context_unique_keys(transaction, &key_families, context_id)
            .await?;
        Ok(context)
    }

    pub async fn get_context_by_code(
        &self,
        code: &str,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        validate_code(code)?;
        Ok(query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE code = $1 AND workspace_id = $2",
        )
        .bind(code)
        .bind(self.workspace_id.0)
        .fetch_optional(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn list_contexts(&self) -> Result<Vec<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn list_authorized_contexts(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<AttributeContext>, RepositoryError> {
        if !self.is_active_principal(user_id, workspace_id).await? {
            return Ok(Vec::new());
        }
        let contexts = query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain();
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
        // Lock order (see the module docs in `mod.rs`): the workspace row,
        // as blueprint publication takes it first; the relationship lock,
        // which relationship writers take before entity rows; this
        // workspace's entity-writer lock, which waits for its in-flight
        // entity writers; then the family key locks. Reparenting changes
        // resolved values for every entity, so it must serialize with writes.
        advance_generation(&mut transaction, self.workspace_id.0, Generation::Contexts).await?;
        self.lock_relationship_cardinality_writes(&mut transaction)
            .await?;
        lock_entity_writes(&mut transaction, self.workspace_id.0, true).await?;
        let key_families = self.lock_context_unique_keys(&mut transaction).await?;
        let context_code = sqlx::query_scalar::<_, String>(
            "SELECT code FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.0)
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
        .bind(self.workspace_id.0)
        .fetch_one(&mut *transaction)
        .await?;
        if !parent_exists {
            return Err(RepositoryError::InvalidContext);
        }
        let result = query_as::<_, Db<AttributeContext>>(r#"WITH RECURSIVE descendants AS (
                SELECT id FROM attribute_contexts WHERE id = $1 AND workspace_id = $4
                UNION ALL SELECT c.id FROM attribute_contexts c JOIN descendants d ON c.parent_id = d.id AND c.workspace_id = $4
            ) UPDATE attribute_contexts SET parent_id = $2, data = $3
              WHERE id = $1 AND workspace_id = $4 AND $2 NOT IN (SELECT id FROM descendants)
              RETURNING id, code, data, parent_id"#)
            .bind(id).bind(input.parent_id).bind(input.data).bind(self.workspace_id.0).fetch_optional(&mut *transaction).await?
            .ok_or(RepositoryError::ContextCycle)?
            .into_domain();
        let entities = query_as::<_, Db<Entity>>("SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at FROM entities WHERE workspace_id = $1 AND deleted_at IS NULL")
            .bind(self.workspace_id.0)
            .fetch_all(&mut *transaction).await?
            .into_domain();
        // Re-index whole families first: syncing entities one at a time
        // could collide with another entity's still-stale row.
        self.rebuild_context_unique_keys(&mut transaction, &key_families)
            .await?;
        // Reparenting is not an edit of any entity: it is revalidated
        // structurally, without transition enforcement or status effects.
        for entity in &entities {
            self.validate_entity_schema_with(
                &mut transaction,
                entity,
                super::entity_commands::Revalidation::Structural,
            )
            .await?;
        }
        self.validate_workspace_hierarchies(&mut transaction)
            .await?;
        let affected_contexts: Vec<Uuid> = sqlx::query_scalar(
            "WITH RECURSIVE descendants AS (SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND id = $2 UNION ALL SELECT child.id FROM attribute_contexts child JOIN descendants parent ON child.parent_id = parent.id WHERE child.workspace_id = $1) SELECT id FROM descendants",
        )
        .bind(self.workspace_id.0)
        .bind(result.id)
        .fetch_all(&mut *transaction)
        .await?;
        for context_id in affected_contexts {
            self.clear_context_publications(&mut transaction, context_id)
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
        let context = query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.0)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("context"))?;
        if context.code == "default" {
            return Err(RepositoryError::DefaultContextProtected);
        }
        let mut transaction = self.pool.begin().await?;
        // The workspace row comes first, as in context creation (see the lock
        // order in `mod.rs`).
        advance_generation(&mut transaction, self.workspace_id.0, Generation::Contexts).await?;
        // Lock first so the active-run check below sees every run that
        // started against this context; run creation holds a share lock.
        sqlx::query(
            "SELECT id FROM attribute_contexts WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(id)
        .bind(self.workspace_id.0)
        .execute(&mut *transaction)
        .await?;
        let read_by_active_run: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM extension_operation_runs WHERE workspace_id = $1 AND selection_context_id = $2 AND status IN ('pending','leased'))",
        )
        .bind(self.workspace_id.0)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if read_by_active_run {
            return Err(RepositoryError::ContextInUse);
        }
        // Withdraw live publications through the shared helper so consumers
        // receive `entity.unpublished`, then drop the channel's rows.
        self.clear_context_publications(&mut transaction, id)
            .await?;
        sqlx::query(
            "DELETE FROM entity_channel_publications WHERE workspace_id = $1 AND context_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM publication_channels WHERE workspace_id = $1 AND context_id = $2")
            .bind(self.workspace_id.0)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query("DELETE FROM attribute_contexts c WHERE c.id = $1 AND c.workspace_id = $2 AND NOT EXISTS (SELECT 1 FROM attribute_contexts child WHERE child.parent_id = c.id AND child.workspace_id = c.workspace_id) AND NOT EXISTS (SELECT 1 FROM attribute_values value WHERE value.context_id = c.id AND value.workspace_id = c.workspace_id)")
            .bind(id).bind(self.workspace_id.0).execute(&mut *transaction).await?;
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

    pub async fn get_context_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        Ok(query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(id)
        .bind(self.workspace_id.0)
        .fetch_optional(&self.pool)
        .await?
        .into_domain())
    }
}

pub(super) fn context_event(
    repository: &CatalogRepository,
    event_type: &str,
    context: &AttributeContext,
) -> NewDomainEvent {
    repository.core_event(
        event_type,
        "context",
        context.id,
        serde_json::to_value(ContextCreatedV1 {
            context_id: context.id,
            code: context.code.clone(),
            parent_id: context.parent_id,
        })
        .expect("context event payload is serializable"),
    )
}
