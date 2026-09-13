use super::*;
use crate::domain_events::{ENTITY_PUBLISHED_V1, ENTITY_UNPUBLISHED_V1, EntityPublicationV1};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

impl CatalogRepository {
    /// Installs publication permissions in application code, preserving the
    /// declarative-only migration contract. Owners and administrators receive
    /// publication authority; editors deliberately do not.
    pub async fn ensure_entity_publication_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('entities.publish', 'Publish catalog entities to channel contexts') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'entities.publish') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_publication_channels(
        &self,
    ) -> Result<Vec<PublicationChannel>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, PublicationChannel>(
            "SELECT c.context_id, a.code AS context_code, c.enabled FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 ORDER BY a.code",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn set_publication_channel(
        &self,
        context_id: Uuid,
        enabled: bool,
    ) -> Result<PublicationChannel, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let context = sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 AND id = $2",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::InvalidContext)?;
        sqlx::query("INSERT INTO publication_channels (workspace_id, context_id, enabled) VALUES ($1, $2, $3) ON CONFLICT (workspace_id, context_id) DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now()")
            .bind(workspace_id).bind(context_id).bind(enabled).execute(&mut *tx).await?;
        self.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(PublicationChannel {
            context_id,
            context_code: context.code,
            enabled,
        })
    }

    pub async fn publication_statuses(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        if self.get_entity(entity_id).await?.is_none() {
            return Err(RepositoryError::NotFound("entity"));
        }
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, EntityPublicationStatus>(
            "SELECT c.context_id, a.code AS context_code, CASE WHEN p.published_at IS NULL THEN 'not_published' ELSE 'published' END AS status, p.published_at, p.published_by_user_id FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id LEFT JOIN entity_channel_publications p ON p.workspace_id = c.workspace_id AND p.entity_id = $2 AND p.context_id = c.context_id WHERE c.workspace_id = $1 AND c.enabled ORDER BY a.code",
        )
        .bind(workspace_id)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn publish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let status = self
            .publish_to_channel(&mut tx, entity_id, context_id)
            .await?;
        self.commit_publication_mutation(
            tx,
            ENTITY_PUBLISHED_V1,
            entity_id,
            context_id,
            status.published_at,
            status.published_by_user_id,
            None,
        )
        .await?;
        Ok(status)
    }

    pub async fn publish_entity_all_channels(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let channels: Vec<Uuid> = sqlx::query_scalar(
            "SELECT context_id FROM publication_channels WHERE workspace_id = $1 AND enabled ORDER BY context_id FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut statuses = Vec::with_capacity(channels.len());
        for context_id in channels {
            statuses.push(
                self.publish_to_channel(&mut tx, entity_id, context_id)
                    .await?,
            );
        }
        self.write_audit_event(&mut tx).await?;
        for status in &statuses {
            self.enqueue_event(
                &mut tx,
                self.publication_event(
                    ENTITY_PUBLISHED_V1,
                    entity_id,
                    status.context_id,
                    status.published_at,
                    status.published_by_user_id,
                    None,
                ),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(statuses)
    }

    pub async fn publish_blueprint_entities(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        context_id: Option<Uuid>,
    ) -> Result<BlueprintEntityPublicationSummary, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let blueprint_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3 AND kind = 'entity' AND deleted_at IS NULL)",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_one(&mut *tx)
        .await?;
        if !blueprint_exists {
            return Err(RepositoryError::NotFound("entity blueprint revision"));
        }
        let entity_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM entities WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL ORDER BY id FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut *tx)
        .await?;
        let channel_ids: Vec<Uuid> = match context_id {
            Some(context_id) => vec![context_id],
            None => sqlx::query_scalar(
                "SELECT context_id FROM publication_channels WHERE workspace_id = $1 AND enabled ORDER BY context_id FOR UPDATE",
            )
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await?,
        };
        if let Some(context_id) = context_id {
            let enabled: bool = sqlx::query_scalar(
                "SELECT enabled FROM publication_channels WHERE workspace_id = $1 AND context_id = $2 FOR UPDATE",
            )
            .bind(workspace_id)
            .bind(context_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
            if !enabled {
                return Err(RepositoryError::PublicationChannelDisabled);
            }
        }
        let actor = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
            .ok_or(RepositoryError::PublicationActorRequired)?;
        let published_at = chrono::Utc::now();
        for entity_id in &entity_ids {
            for context_id in &channel_ids {
                sqlx::query("INSERT INTO entity_channel_publications (workspace_id, entity_id, context_id, published_at, published_by_user_id) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, entity_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
                    .bind(workspace_id)
                    .bind(entity_id)
                    .bind(context_id)
                    .bind(published_at)
                    .bind(actor)
                    .execute(&mut *tx)
                    .await?;
                self.enqueue_event(
                    &mut tx,
                    self.publication_event(
                        ENTITY_PUBLISHED_V1,
                        *entity_id,
                        *context_id,
                        Some(published_at),
                        Some(actor),
                        Some("blueprint_bulk"),
                    ),
                )
                .await?;
            }
        }
        self.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(BlueprintEntityPublicationSummary {
            entity_count: entity_ids.len() as i64,
            channel_count: channel_ids.len() as i64,
            publication_count: (entity_ids.len() * channel_ids.len()) as i64,
        })
    }

    pub async fn unpublish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let updated = sqlx::query("UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND entity_id = $2 AND context_id = $3 AND published_at IS NOT NULL")
            .bind(workspace_id).bind(entity_id).bind(context_id).execute(&mut *tx).await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("active entity publication"));
        }
        self.commit_publication_mutation(
            tx,
            ENTITY_UNPUBLISHED_V1,
            entity_id,
            context_id,
            None,
            None,
            Some("manual"),
        )
        .await
    }

    pub(in crate::repository) async fn clear_entity_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        reason: &str,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let contexts: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND entity_id = $2 AND published_at IS NOT NULL RETURNING context_id",
        )
        .bind(workspace_id)
        .bind(entity_id)
        .fetch_all(&mut **tx)
        .await?;
        for context_id in contexts {
            self.enqueue_event(
                tx,
                self.publication_event(
                    ENTITY_UNPUBLISHED_V1,
                    entity_id,
                    context_id,
                    None,
                    None,
                    Some(reason),
                ),
            )
            .await?;
        }
        Ok(())
    }

    pub(in crate::repository) async fn clear_context_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let entities: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND context_id = $2 AND published_at IS NOT NULL RETURNING entity_id",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_all(&mut **tx)
        .await?;
        for entity_id in entities {
            self.enqueue_event(
                tx,
                self.publication_event(
                    ENTITY_UNPUBLISHED_V1,
                    entity_id,
                    context_id,
                    None,
                    None,
                    Some("context_changed"),
                ),
            )
            .await?;
        }
        Ok(())
    }

    async fn publish_to_channel(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let channel = sqlx::query_as::<_, PublicationChannel>(
            "SELECT c.context_id, a.code AS context_code, c.enabled FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 AND c.context_id = $2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?
        .filter(|channel| channel.enabled)
        .ok_or(RepositoryError::PublicationChannelDisabled)?;
        let actor = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
            .ok_or(RepositoryError::PublicationActorRequired)?;
        let published_at = chrono::Utc::now();
        sqlx::query("INSERT INTO entity_channel_publications (workspace_id, entity_id, context_id, published_at, published_by_user_id) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, entity_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
            .bind(workspace_id).bind(entity_id).bind(context_id).bind(published_at).bind(actor).execute(&mut **tx).await?;
        Ok(EntityPublicationStatus {
            context_id,
            context_code: channel.context_code,
            status: "published".to_owned(),
            published_at: Some(published_at),
            published_by_user_id: Some(actor),
        })
    }

    async fn commit_publication_mutation(
        &self,
        mut tx: Transaction<'_, Postgres>,
        event_type: &str,
        entity_id: Uuid,
        context_id: Uuid,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
        published_by_user_id: Option<Uuid>,
        reason: Option<&str>,
    ) -> Result<(), RepositoryError> {
        self.write_audit_event(&mut tx).await?;
        self.enqueue_event(
            &mut tx,
            self.publication_event(
                event_type,
                entity_id,
                context_id,
                published_at,
                published_by_user_id,
                reason,
            ),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    fn publication_event(
        &self,
        event_type: &str,
        entity_id: Uuid,
        context_id: Uuid,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
        published_by_user_id: Option<Uuid>,
        reason: Option<&str>,
    ) -> crate::domain_events::NewDomainEvent {
        self.core_event(
            event_type,
            "entity",
            entity_id,
            serde_json::to_value(EntityPublicationV1 {
                entity_id,
                context_id,
                published_at,
                published_by_user_id,
                reason: reason.map(str::to_owned),
            })
            .expect("publication event serializable"),
        )
    }
}
