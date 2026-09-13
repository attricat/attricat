use super::*;
use crate::domain_events::{ENTITY_PUBLISHED_V1, ENTITY_UNPUBLISHED_V1, EntityPublicationV1};
use sha2::{Digest, Sha256};
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
        #[derive(sqlx::FromRow)]
        struct Active {
            context_id: Uuid,
            context_code: String,
            revision: Option<i64>,
            published_at: Option<chrono::DateTime<chrono::Utc>>,
            payload_hash: Option<String>,
        }
        let active = sqlx::query_as::<_, Active>(
            "SELECT c.context_id, a.code AS context_code, s.revision, s.published_at, s.payload_hash FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id LEFT JOIN entity_channel_publications p ON p.workspace_id = c.workspace_id AND p.entity_id = $2 AND p.context_id = c.context_id LEFT JOIN entity_publication_snapshots s ON s.id = p.active_snapshot_id WHERE c.workspace_id = $1 AND c.enabled ORDER BY a.code"
        ).bind(workspace_id).bind(entity_id).fetch_all(&self.pool).await?;
        let mut statuses = Vec::with_capacity(active.len());
        for item in active {
            let status = match item.payload_hash {
                None => "not_published".to_owned(),
                Some(hash) => {
                    if self
                        .publication_payload(entity_id, item.context_id)
                        .await?
                        .1
                        == hash
                    {
                        "published".to_owned()
                    } else {
                        "changes_pending".to_owned()
                    }
                }
            };
            statuses.push(EntityPublicationStatus {
                context_id: item.context_id,
                context_code: item.context_code,
                status,
                revision: item.revision,
                published_at: item.published_at,
            });
        }
        Ok(statuses)
    }

    pub async fn publication_snapshot(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<Option<EntityPublicationSnapshot>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, EntityPublicationSnapshot>(
            "SELECT s.id, s.context_id, s.revision, s.payload, s.payload_hash, s.published_at FROM entity_channel_publications p JOIN entity_publication_snapshots s ON s.id = p.active_snapshot_id WHERE p.workspace_id = $1 AND p.entity_id = $2 AND p.context_id = $3"
        ).bind(workspace_id).bind(entity_id).bind(context_id).fetch_optional(&self.pool).await?)
    }

    pub async fn publish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        // Every entity writer takes this row lock. Build the publication only
        // after it is held, so the resolved payload cannot race a value write.
        self.lock_entity(&mut tx, entity_id).await?;
        let prepared = self
            .prepare_publication(&mut tx, entity_id, context_id)
            .await?;
        let status = self.publish_prepared(&mut tx, prepared).await?;
        let event = self.core_event(
            ENTITY_PUBLISHED_V1,
            "entity",
            entity_id,
            serde_json::to_value(EntityPublicationV1 {
                entity_id,
                context_id,
                revision: status.revision,
                blueprint_id: None,
                blueprint_version: None,
            })
            .expect("publication event serializable"),
        );
        self.commit_entity_mutation(tx, Vec::new(), event).await?;
        Ok(status)
    }

    pub async fn publish_entity_all_channels(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        // Serialize every bulk snapshot against ordinary entity writes before
        // resolving any channel-specific values.
        self.lock_entity(&mut tx, entity_id).await?;
        // Lock every enabled channel in context UUID order. A concurrent
        // enable/disable waits until this batch commits or rolls back.
        let channels = sqlx::query_as::<_, PublicationChannel>(
            "SELECT c.context_id, a.code AS context_code, c.enabled \
             FROM publication_channels c \
             JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id \
             WHERE c.workspace_id = $1 AND c.enabled \
             ORDER BY c.context_id FOR UPDATE",
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&mut *tx)
        .await?;
        let mut prepared = Vec::with_capacity(channels.len());
        for channel in channels {
            prepared.push(
                self.prepare_locked_publication(entity_id, channel.context_id)
                    .await?,
            );
        }
        let mut statuses = Vec::with_capacity(prepared.len());
        for item in prepared {
            statuses.push(self.publish_prepared(&mut tx, item).await?);
        }
        self.write_audit_event(&mut tx).await?;
        for status in &statuses {
            let event = self.core_event(
                ENTITY_PUBLISHED_V1,
                "entity",
                entity_id,
                serde_json::to_value(EntityPublicationV1 {
                    entity_id,
                    context_id: status.context_id,
                    revision: status.revision,
                    blueprint_id: None,
                    blueprint_version: None,
                })
                .expect("publication event serializable"),
            );
            self.enqueue_event(&mut tx, event).await?;
        }
        tx.commit().await?;
        Ok(statuses)
    }

    pub async fn unpublish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        // Publish locks the source and every target entity. Taking the target
        // lock here makes the dependency check and clearing active publication
        // mutually exclusive with a concurrent source publication.
        self.lock_entity(&mut tx, entity_id).await?;
        let dependent_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT p.entity_id FROM entity_channel_publications p JOIN entity_publication_dependencies d ON d.snapshot_id = p.active_snapshot_id WHERE p.workspace_id = $1 AND d.target_entity_id = $2 AND d.context_id = $3 AND p.active_snapshot_id IS NOT NULL"
        ).bind(workspace_id).bind(entity_id).bind(context_id).fetch_all(&mut *tx).await?;
        if !dependent_ids.is_empty() {
            return Err(RepositoryError::PublicationHasDependents(
                dependent_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        let updated = sqlx::query("UPDATE entity_channel_publications SET active_snapshot_id = NULL, unpublished_at = now(), unpublished_by_user_id = $4 WHERE workspace_id = $1 AND entity_id = $2 AND context_id = $3 AND active_snapshot_id IS NOT NULL")
            .bind(workspace_id).bind(entity_id).bind(context_id).bind(self.audit_context.as_ref().and_then(|audit| audit.actor_user_id)).execute(&mut *tx).await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("active entity publication"));
        }
        let event = self.core_event(
            ENTITY_UNPUBLISHED_V1,
            "entity",
            entity_id,
            serde_json::to_value(EntityPublicationV1 {
                entity_id,
                context_id,
                revision: None,
                blueprint_id: None,
                blueprint_version: None,
            })
            .expect("publication event serializable"),
        );
        self.commit_entity_mutation(tx, Vec::new(), event).await
    }

    async fn prepare_publication(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<PreparedPublication, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        // The channel lock is held through the snapshot insert/switch below;
        // channel configuration cannot race publication eligibility.
        let enabled: bool = sqlx::query_scalar(
            "SELECT enabled FROM publication_channels WHERE workspace_id = $1 AND context_id = $2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(false);
        if !enabled {
            return Err(RepositoryError::PublicationChannelDisabled);
        }
        self.prepare_locked_publication(entity_id, context_id).await
    }

    async fn prepare_locked_publication(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<PreparedPublication, RepositoryError> {
        let entity = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let (payload, hash, dependencies) = self.publication_payload(entity_id, context_id).await?;
        Ok(PreparedPublication {
            entity,
            context_id,
            payload,
            hash,
            dependencies,
        })
    }

    async fn publication_payload(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<(Value, String, Vec<Uuid>), RepositoryError> {
        let entity = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let resolved = self
            .resolved_preview(entity_id, context_id, 1)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let mut values = resolved.values.as_object().cloned().unwrap_or_default();
        let mut dependencies = Vec::new();
        for entry in values.values_mut() {
            let targets = entry
                .get("value")
                .and_then(|value| value.get("items"))
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get("id")
                                .and_then(Value::as_str)
                                .and_then(|value| value.parse().ok())
                        })
                        .collect::<Vec<Uuid>>()
                });
            if let Some(targets) = targets {
                dependencies.extend(targets.iter().copied());
                if let Some(object) = entry.as_object_mut() {
                    object.insert("value".to_owned(), serde_json::json!(targets));
                }
            }
        }
        dependencies.sort();
        dependencies.dedup();
        let payload = serde_json::json!({
            "entity_id": entity.id,
            "blueprint_id": entity.blueprint_id,
            "blueprint_version": entity.blueprint_version,
            "context_id": context_id,
            "values": values,
            "system_tags": entity.system_tags,
            "system_metadata": entity.system_metadata,
        });
        let hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&payload).expect("publication payload serializable"))
        );
        Ok((payload, hash, dependencies))
    }

    async fn publish_prepared(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        prepared: PreparedPublication,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        sqlx::query("SELECT id FROM entities WHERE workspace_id = $1 AND id = $2 FOR UPDATE")
            .bind(workspace_id)
            .bind(prepared.entity.id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        // Lock targets in UUID order. Unpublish takes the same target lock,
        // so no active source snapshot can commit beside an unpublished target.
        let locked_target_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM entities WHERE workspace_id = $1 AND id = ANY($2::uuid[]) AND deleted_at IS NULL ORDER BY id FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(&prepared.dependencies)
        .fetch_all(&mut **tx)
        .await?;
        if locked_target_ids.len() != prepared.dependencies.len() {
            return Err(RepositoryError::PublicationDependenciesMissing(
                "one or more relationship targets no longer exist".to_owned(),
            ));
        }
        let missing: Vec<Uuid> = sqlx::query_scalar("SELECT target_id FROM unnest($1::uuid[]) AS target_id WHERE NOT EXISTS (SELECT 1 FROM entity_channel_publications p WHERE p.workspace_id = $2 AND p.entity_id = target_id AND p.context_id = $3 AND p.active_snapshot_id IS NOT NULL)")
            .bind(&prepared.dependencies).bind(workspace_id).bind(prepared.context_id).fetch_all(&mut **tx).await?;
        if !missing.is_empty() {
            return Err(RepositoryError::PublicationDependenciesMissing(
                missing
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        let revision: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(revision), 0) + 1 FROM entity_publication_snapshots WHERE workspace_id = $1 AND entity_id = $2 AND context_id = $3")
            .bind(workspace_id).bind(prepared.entity.id).bind(prepared.context_id).fetch_one(&mut **tx).await?;
        let snapshot_id = Uuid::new_v4();
        sqlx::query("INSERT INTO entity_publication_snapshots (id, workspace_id, entity_id, context_id, revision, blueprint_id, blueprint_version, payload, payload_hash, published_by_user_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(snapshot_id).bind(workspace_id).bind(prepared.entity.id).bind(prepared.context_id).bind(revision).bind(prepared.entity.blueprint_id).bind(prepared.entity.blueprint_version).bind(prepared.payload).bind(prepared.hash).bind(self.audit_context.as_ref().and_then(|audit| audit.actor_user_id)).execute(&mut **tx).await?;
        for target in prepared.dependencies {
            sqlx::query("INSERT INTO entity_publication_dependencies (snapshot_id, workspace_id, target_entity_id, context_id) VALUES ($1, $2, $3, $4)")
                .bind(snapshot_id)
                .bind(workspace_id)
                .bind(target)
                .bind(prepared.context_id)
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("INSERT INTO entity_channel_publications (workspace_id, entity_id, context_id, active_snapshot_id, unpublished_at, unpublished_by_user_id) VALUES ($1,$2,$3,$4,NULL,NULL) ON CONFLICT (workspace_id, entity_id, context_id) DO UPDATE SET active_snapshot_id = EXCLUDED.active_snapshot_id, unpublished_at = NULL, unpublished_by_user_id = NULL")
            .bind(workspace_id).bind(prepared.entity.id).bind(prepared.context_id).bind(snapshot_id).execute(&mut **tx).await?;
        let context = self
            .get_context_by_id(prepared.context_id)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        Ok(EntityPublicationStatus {
            context_id: prepared.context_id,
            context_code: context.code,
            status: "published".to_owned(),
            revision: Some(revision),
            published_at: Some(chrono::Utc::now()),
        })
    }
}

struct PreparedPublication {
    entity: Entity,
    context_id: Uuid,
    payload: Value,
    hash: String,
    dependencies: Vec<Uuid>,
}
