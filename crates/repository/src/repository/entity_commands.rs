use super::entity_search::empty_projections;
use super::extension_catalog_data::{
    ExtensionCatalogBatch, ExtensionCatalogIntent, ExtensionCatalogIntentOutcome,
    ExtensionCatalogIntentStatus, MAX_EXTENSION_BATCH_INTENTS, MAX_EXTENSION_BATCH_KEY_BYTES,
    MAX_EXTENSION_INTENT_KEY_BYTES,
};
use super::record_values::{ContextTree, RecordState, RecordValues, load_record};
use super::values::{NativeValue, ValueType};
use super::write_context::WriteContext;
use super::*;
use crate::domain_events::{
    ATTRIBUTE_VALUE_CHANGED_V1, AttributeValueMutationV1, ENTITY_CREATED_V1, ENTITY_DELETED_V1,
    ENTITY_UPDATED_V1, EntityMutationV1, NewDomainEvent, RELATIONSHIP_CHANGED_V1,
    RelationshipMutationV1,
};
use crate::model::UpdateEntityFormRequest;
use crate::persistence_rows::{Db, IntoDomain};
use catalog_validation::validate_json_schema;
use chrono::Utc;
use serde_json::{Map, Value};
use sha2::Digest;
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(sqlx::FromRow, Clone)]
pub(super) struct AuditValueSnapshot {
    attribute_id: Uuid,
    attribute_code: String,
    context_id: Option<Uuid>,
    context_code: Option<String>,
    relationship_target_entity_id: Option<Uuid>,
    value: Value,
}

/// How [`CatalogRepository::validate_entity_schema_with`] treats status.
/// A value resolved against its [`WriteContext`] and validated in memory.
struct PreparedValue {
    attribute_id: Uuid,
    attribute_code: String,
    context_id: Uuid,
    relationship: Option<PreparedRelationship>,
    native: Option<NativeValue>,
}

struct PreparedRelationship {
    target_entity_id: Uuid,
    target_blueprint_codes: Vec<String>,
    cardinality: String,
    target_cardinality: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Revalidation {
    /// An entity write: status transitions are checked (edges, permissions,
    /// separation of duties, locks) and recorded, and approvals whose covered
    /// content changed are voided, with the writer as actor.
    Write,
    /// A structural change that is not an edit of the entity, such as context
    /// reparenting: effective values may change through inheritance, but no
    /// transition is enforced or recorded and no approval is voided or hold
    /// placed. Schema, principal values, unique keys and entity checks still
    /// apply.
    Structural,
}

pub(super) struct ChosenIdEntityCreate {
    pub entity_id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub values: Vec<NewAttributeValue>,
    pub system_tags: Vec<String>,
    pub system_metadata: Value,
    pub host_sample_marker: bool,
}

struct CardinalityCheck<'a> {
    entity: &'a Entity,
    attribute_id: Uuid,
    attribute_code: &'a str,
    cardinality: &'a str,
    target_cardinality: &'a str,
    context_id: Option<Uuid>,
    target_entity_id: Uuid,
}
use uuid::Uuid;

impl CatalogRepository {
    pub async fn create_entity_with_values(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: Vec<NewAttributeValue>,
        system_tags: Vec<String>,
        system_metadata: Value,
    ) -> Result<Entity, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let (entity, changes, event) = self
            .create_entity_in_transaction(
                &mut transaction,
                ChosenIdEntityCreate {
                    entity_id: Uuid::new_v4(),
                    blueprint_id,
                    blueprint_version,
                    values,
                    system_tags,
                    system_metadata,
                    host_sample_marker: false,
                },
            )
            .await?;
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity)
    }

    /// Chosen-ID, caller-transaction seam for ordinary entity creation.
    ///
    /// The create, update and delete seams share one contract: each takes the
    /// locks it needs itself (the workspace relationship lock when it writes
    /// relationships, then the entity row), which is a no-op when the caller
    /// already holds them, and returns the audit changes and event for the
    /// caller to pass to [`Self::stage_entity_mutation`] (or
    /// [`Self::commit_entity_mutation`]) before committing. A caller that locks
    /// entity rows itself before a relationship write must take the
    /// relationship lock first.
    pub(super) async fn create_entity_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        input: ChosenIdEntityCreate,
    ) -> Result<(Entity, Vec<AuditEventChange>, NewDomainEvent), RepositoryError> {
        let ChosenIdEntityCreate {
            entity_id,
            blueprint_id,
            blueprint_version,
            values,
            system_tags,
            system_metadata,
            host_sample_marker,
        } = input;
        if host_sample_marker {
            let ordinary_tags = system_tags
                .iter()
                .filter(|tag| tag.as_str() != "attricat.sample")
                .cloned()
                .collect::<Vec<_>>();
            if ordinary_tags.len() + 1 != system_tags.len() {
                return Err(RepositoryError::InvalidSystemTags);
            }
            validate_system_annotations(&ordinary_tags, &system_metadata)?;
        } else {
            validate_system_annotations(&system_tags, &system_metadata)?;
        }
        self.ensure_annotation_namespaces_unchanged(
            transaction,
            &[],
            &Value::Object(Map::new()),
            &system_tags,
            &system_metadata,
        )
        .await?;
        // Do not expose an entity before its initial values and derived preview
        // agree; otherwise a concurrent reader can observe a partial create.
        if writes_relationship_values(&values) {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
        }
        let entity = self
            .insert_entity(
                transaction,
                entity_id,
                blueprint_id,
                blueprint_version,
                system_tags,
                system_metadata,
            )
            .await?;
        let write = WriteContext::load(transaction, self.workspace_id.0, &entity).await?;
        let default_context_id = write.default_context_id()?;
        let defaults = write
            .attributes()
            .iter()
            .filter(|attribute| {
                attribute.default_value.is_some()
                    && !values.iter().any(|value| {
                        matches!(
                            value,
                            NewAttributeValue::Scalar {
                                attribute_id,
                                attribute_code,
                                context_id,
                                ..
                            } if (attribute_id == &Some(attribute.id)
                                || attribute_code.as_deref() == Some(attribute.code.as_str()))
                                && context_id.is_none_or(|id| id == default_context_id)
                        )
                    })
            })
            .map(|attribute| NewAttributeValue::Scalar {
                attribute_id: Some(attribute.id),
                attribute_code: None,
                context_id: Some(default_context_id),
                value: attribute.default_value.clone().expect("filtered above"),
            })
            .collect::<Vec<_>>();
        self.insert_values_in(transaction, &write, &entity, defaults)
            .await?;
        self.insert_values_in(transaction, &write, &entity, values)
            .await?;
        self.validate_entity_schema_in(transaction, &write, &entity, Revalidation::Write)
            .await?;
        let preview = Self::build_preview_projection(transaction, entity.id).await?;
        let entity = self.store_preview(transaction, entity.id, preview).await?;
        let after = self.entity_audit_snapshot(transaction, entity.id).await?;
        let changes = Self::audit_changes(entity.id, Vec::new(), after, false);
        let event = self.core_event(
            ENTITY_CREATED_V1,
            "entity",
            entity.id,
            serde_json::to_value(EntityMutationV1 {
                entity_id: entity.id,
                blueprint_id: entity.blueprint_id,
                blueprint_version: entity.blueprint_version,
                facts: Self::affected_facts(&changes),
            })
            .expect("entity-created payload is serializable"),
        );
        Ok((entity, changes, event))
    }

    pub async fn duplicate_entity(&self, entity_id: Uuid) -> Result<Entity, RepositoryError> {
        let source = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let mut file_values = Vec::new();
        let values = self
            .form_values(entity_id)
            .await?
            .into_iter()
            .filter_map(|value| match value {
                FormAttributeValue::Scalar {
                    attribute_code,
                    context_id,
                    value,
                } => Some(NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some(attribute_code),
                    context_id,
                    value,
                }),
                FormAttributeValue::Relationship {
                    attribute_code,
                    context_id,
                    target_entity_id,
                } => Some(NewAttributeValue::Relationship {
                    attribute_id: None,
                    attribute_code: Some(attribute_code),
                    context_id,
                    target_entity_id,
                }),
                FormAttributeValue::File {
                    attribute_code,
                    context_id,
                    files,
                } => {
                    file_values.push((attribute_code, context_id, files));
                    None
                }
            })
            .collect();
        let system_tags = source
            .system_tags
            .into_iter()
            .filter(|tag| tag != "attricat.sample")
            .collect();
        let (system_tags, system_metadata) = self
            .without_claimed_annotations(system_tags, source.system_metadata)
            .await?;
        // The copy, its file references and its audit/event commit together,
        // so a failed file link cannot leave a partial duplicate behind.
        let mut transaction = self.pool.begin().await?;
        let (entity, changes, event) = self
            .create_entity_in_transaction(
                &mut transaction,
                ChosenIdEntityCreate {
                    entity_id: Uuid::new_v4(),
                    blueprint_id: source.blueprint_id,
                    blueprint_version: source.blueprint_version,
                    values,
                    system_tags,
                    system_metadata,
                    host_sample_marker: false,
                },
            )
            .await?;
        let entity = if file_values.is_empty() {
            entity
        } else {
            for (attribute_code, context_id, files) in file_values {
                for file in files {
                    self.link_file_in_transaction(
                        &mut transaction,
                        &entity,
                        &attribute_code,
                        context_id,
                        file.id,
                    )
                    .await?;
                }
            }
            self.revalidate_entity(&mut transaction, &entity).await?
        };
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity)
    }

    pub async fn update_entity_with_values(
        &self,
        entity_id: Uuid,
        values: Vec<NewAttributeValue>,
        relationships: Vec<RelationshipTargets>,
        remove_values: Vec<AttributeValueSelector>,
        system_tags: Option<Vec<String>>,
        system_metadata: Option<Value>,
    ) -> Result<Entity, RepositoryError> {
        self.update_entity_with_values_checked(
            entity_id,
            values,
            relationships,
            remove_values,
            system_tags,
            system_metadata,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_entity_with_values_checked(
        &self,
        entity_id: Uuid,
        values: Vec<NewAttributeValue>,
        relationships: Vec<RelationshipTargets>,
        remove_values: Vec<AttributeValueSelector>,
        system_tags: Option<Vec<String>>,
        system_metadata: Option<Value>,
        expected_updated_at: Option<DateTime<Utc>>,
    ) -> Result<Entity, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let (entity, changes, event) = self
            .update_entity_in_transaction(
                &mut transaction,
                entity_id,
                UpdateEntityFormRequest {
                    expected_updated_at,
                    values,
                    relationships,
                    remove_values,
                    system_tags,
                    system_metadata,
                },
            )
            .await?;
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity)
    }

    /// Caller-transaction seam for an entity update; see
    /// [`Self::create_entity_in_transaction`] for the shared contract.
    /// `system_tags` and `system_metadata` replace the whole field.
    pub(super) async fn update_entity_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        input: UpdateEntityFormRequest,
    ) -> Result<(Entity, Vec<AuditEventChange>, NewDomainEvent), RepositoryError> {
        let UpdateEntityFormRequest {
            expected_updated_at,
            values,
            relationships,
            remove_values,
            system_tags,
            system_metadata,
        } = input;
        if let Some(metadata) = &system_metadata {
            validate_system_metadata(metadata)?;
        }
        if !relationships.is_empty() || writes_relationship_values(&values) {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
        }
        // The row lock serializes writers for an entity. It protects both the
        // one-latest-value invariant and the preview rebuilt from that state.
        let entity = self.lock_entity(transaction, entity_id).await?;
        // Snapshot under the lock so a concurrent writer cannot change the
        // audited "before" state between the read and this mutation.
        let before = self.entity_audit_snapshot(transaction, entity_id).await?;
        let write = WriteContext::load(transaction, self.workspace_id.0, &entity).await?;
        if expected_updated_at.is_some() || Self::has_status_writes(&write, &values, &remove_values)
        {
            Self::check_status_precondition(&write, &entity, expected_updated_at)?;
        }
        if let Some(tags) = &system_tags {
            validate_system_tag_update(&entity.system_tags, tags)?;
        }
        if system_tags.is_some() || system_metadata.is_some() {
            // Whole-field annotation writes may change unrelated tags and keys
            // but never a claimed extension namespace.
            self.ensure_annotation_namespaces_unchanged(
                transaction,
                &entity.system_tags,
                &entity.system_metadata,
                system_tags.as_deref().unwrap_or(&entity.system_tags),
                system_metadata.as_ref().unwrap_or(&entity.system_metadata),
            )
            .await?;
            sqlx::query(
                r#"UPDATE entities
                   SET system_tags = COALESCE($2, system_tags),
                       system_metadata = COALESCE($3, system_metadata),
                       updated_at = now()
                   WHERE id = $1 AND workspace_id = $4"#,
            )
            .bind(entity_id)
            .bind(system_tags)
            .bind(system_metadata)
            .bind(self.workspace_id.0)
            .execute(&mut **transaction)
            .await?;
        }
        self.insert_values_in(transaction, &write, &entity, values)
            .await?;
        for selector in remove_values {
            self.remove_scalar_value(transaction, &write, &entity, selector)
                .await?;
        }
        self.replace_relationship_sets(transaction, &write, &entity, relationships)
            .await?;
        self.validate_entity_schema_in(transaction, &write, &entity, Revalidation::Write)
            .await?;
        let preview = Self::build_preview_projection(transaction, entity.id).await?;
        let entity = self.store_preview(transaction, entity.id, preview).await?;
        let after = self.entity_audit_snapshot(transaction, entity_id).await?;
        let changes = Self::audit_changes(entity_id, before, after, false);
        let event = self.core_event(
            ENTITY_UPDATED_V1,
            "entity",
            entity.id,
            serde_json::to_value(EntityMutationV1 {
                entity_id: entity.id,
                blueprint_id: entity.blueprint_id,
                blueprint_version: entity.blueprint_version,
                facts: Self::affected_facts(&changes),
            })
            .expect("entity-updated payload is serializable"),
        );
        Ok((entity, changes, event))
    }

    pub async fn entity_audit_changes(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityAuditChange>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<EntityAuditChange>>(
            r#"SELECT c.audit_event_id, e.occurred_at, e.actor_user_id,
                      actor.display_name AS actor_display_name, actor.email AS actor_email,
                      actor_avatar.id AS actor_avatar_file_id,
                      e.executor_type, e.agent_run_id, e.approval_decision, e.approved_by_user_id,
                      approver.display_name AS approved_by_display_name,
                      approver_avatar.id AS approved_by_avatar_file_id,
                      c.attribute_id, c.attribute_code, c.context_id, c.context_code,
                      c.change_kind, c.before_value, c.after_value
               FROM audit_event_changes c
               JOIN audit_events e ON e.id = c.audit_event_id
               LEFT JOIN users actor ON actor.id = e.actor_user_id
               LEFT JOIN users approver ON approver.id = e.approved_by_user_id
               LEFT JOIN workspace_memberships actor_m ON actor_m.workspace_id = c.workspace_id AND actor_m.user_id = e.actor_user_id AND actor_m.state = 'active' LEFT JOIN files actor_avatar ON actor_avatar.workspace_id = actor_m.workspace_id AND actor_avatar.id = actor_m.avatar_file_id AND actor_avatar.purpose = 'avatar' AND actor_avatar.status = 'ready' AND actor_avatar.deleted_at IS NULL
               LEFT JOIN workspace_memberships approver_m ON approver_m.workspace_id = c.workspace_id AND approver_m.user_id = e.approved_by_user_id AND approver_m.state = 'active' LEFT JOIN files approver_avatar ON approver_avatar.workspace_id = approver_m.workspace_id AND approver_avatar.id = approver_m.avatar_file_id AND approver_avatar.purpose = 'avatar' AND approver_avatar.status = 'ready' AND approver_avatar.deleted_at IS NULL
               WHERE c.entity_id = $1 AND c.workspace_id = $2
               ORDER BY e.occurred_at DESC, c.id DESC"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn entity_audit_changes_page(
        &self,
        entity_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<EntityAuditChange>, bool), RepositoryError> {
        let mut rows = sqlx::query_as::<_, Db<EntityAuditChange>>(
            r#"SELECT c.audit_event_id, e.occurred_at, e.actor_user_id,
                      actor.display_name AS actor_display_name, actor.email AS actor_email,
                      actor_avatar.id AS actor_avatar_file_id,
                      e.executor_type, e.agent_run_id, e.approval_decision, e.approved_by_user_id,
                      approver.display_name AS approved_by_display_name,
                      approver_avatar.id AS approved_by_avatar_file_id,
                      c.attribute_id, c.attribute_code, c.context_id, c.context_code,
                      c.change_kind, c.before_value, c.after_value
               FROM audit_event_changes c
               JOIN audit_events e ON e.id = c.audit_event_id
               LEFT JOIN users actor ON actor.id = e.actor_user_id
               LEFT JOIN users approver ON approver.id = e.approved_by_user_id
               LEFT JOIN workspace_memberships actor_m ON actor_m.workspace_id = c.workspace_id AND actor_m.user_id = e.actor_user_id AND actor_m.state = 'active' LEFT JOIN files actor_avatar ON actor_avatar.workspace_id = actor_m.workspace_id AND actor_avatar.id = actor_m.avatar_file_id AND actor_avatar.purpose = 'avatar' AND actor_avatar.status = 'ready' AND actor_avatar.deleted_at IS NULL
               LEFT JOIN workspace_memberships approver_m ON approver_m.workspace_id = c.workspace_id AND approver_m.user_id = e.approved_by_user_id AND approver_m.state = 'active' LEFT JOIN files approver_avatar ON approver_avatar.workspace_id = approver_m.workspace_id AND approver_avatar.id = approver_m.avatar_file_id AND approver_avatar.purpose = 'avatar' AND approver_avatar.status = 'ready' AND approver_avatar.deleted_at IS NULL
               WHERE c.entity_id = $1 AND c.workspace_id = $2
               ORDER BY e.occurred_at DESC, c.id DESC
               LIMIT $3 OFFSET $4"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(limit + 1)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows.into_domain(), has_more))
    }

    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<Entity>>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&self.pool)
        .await?
        .into_domain())
    }

    /// Whether a live entity exists, without loading its projections.
    pub async fn entity_exists(&self, entity_id: Uuid) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM entities WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL)",
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let (changes, event) = self
            .delete_entity_in_transaction(&mut transaction, entity_id, None)
            .await?;
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(())
    }

    /// Caller-transaction seam for entity deletion with an optional
    /// optimistic-concurrency precondition; see
    /// [`Self::create_entity_in_transaction`] for the shared contract.
    pub(super) async fn delete_entity_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        expected_updated_at: Option<DateTime<Utc>>,
    ) -> Result<(Vec<AuditEventChange>, NewDomainEvent), RepositoryError> {
        let entity = self.lock_entity(transaction, entity_id).await?;
        let before = self.entity_audit_snapshot(transaction, entity_id).await?;
        if expected_updated_at.is_some_and(|expected| expected != entity.updated_at) {
            return Err(RepositoryError::StaleEntity);
        }
        self.ensure_entity_deletable(transaction, &entity).await?;
        sqlx::query(
            "DELETE FROM entity_channel_publications WHERE workspace_id = $1 AND entity_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .execute(&mut **transaction)
        .await?;
        let result = sqlx::query(
            "UPDATE entities SET deleted_at = now(), updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .execute(&mut **transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("entity"));
        }
        self.delete_entity_unique_keys(transaction, entity_id)
            .await?;
        let changes = Self::audit_changes(entity_id, before, Vec::new(), false);
        let event = self.core_event(
            ENTITY_DELETED_V1,
            "entity",
            entity_id,
            serde_json::to_value(EntityMutationV1 {
                entity_id,
                blueprint_id: entity.blueprint_id,
                blueprint_version: entity.blueprint_version,
                facts: Vec::new(),
            })
            .expect("entity-deleted payload is serializable"),
        );
        Ok((changes, event))
    }

    pub async fn append_values(
        &self,
        entity_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        if input
            .values
            .iter()
            .any(|value| matches!(value, NewAttributeValue::Relationship { .. }))
        {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let entity = sqlx::query_as::<_, Db<Entity>>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
               FOR UPDATE"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut *transaction)
        .await?
        .into_domain()
        .ok_or(RepositoryError::NotFound("entity"))?;
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;

        let write = WriteContext::load(&mut transaction, self.workspace_id.0, &entity).await?;
        if input.expected_updated_at.is_some()
            || Self::has_status_writes(&write, &input.values, &[])
        {
            Self::check_status_precondition(&write, &entity, input.expected_updated_at)?;
        }
        let values = self
            .insert_values_in(&mut transaction, &write, &entity, input.values)
            .await?;

        self.validate_entity_schema_in(&mut transaction, &write, &entity, Revalidation::Write)
            .await?;

        let preview = Self::build_preview_projection(&mut transaction, entity_id).await?;
        sqlx::query(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1 AND workspace_id = $3"#,
        )
        .bind(entity_id)
        .bind(preview)
        .bind(self.workspace_id.0)
        .execute(&mut *transaction)
        .await?;

        let after = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let changes = Self::audit_changes(entity_id, before, after, false);
        let facts = Self::affected_facts(&changes);
        let contains_relationship = values
            .iter()
            .any(|value| value.relationship_target_entity_id.is_some());
        let contains_scalar = values
            .iter()
            .any(|value| value.relationship_target_entity_id.is_none());
        let event = if contains_relationship && !contains_scalar {
            self.core_event(
                RELATIONSHIP_CHANGED_V1,
                "entity",
                entity_id,
                serde_json::to_value(RelationshipMutationV1 { entity_id, facts })
                    .expect("relationship-changed payload is serializable"),
            )
        } else if contains_relationship {
            self.core_event(
                ENTITY_UPDATED_V1,
                "entity",
                entity_id,
                serde_json::to_value(EntityMutationV1 {
                    entity_id,
                    blueprint_id: entity.blueprint_id,
                    blueprint_version: entity.blueprint_version,
                    facts,
                })
                .expect("entity-updated payload is serializable"),
            )
        } else {
            self.core_event(
                ATTRIBUTE_VALUE_CHANGED_V1,
                "entity",
                entity_id,
                serde_json::to_value(AttributeValueMutationV1 { entity_id, facts })
                    .expect("attribute-value-changed payload is serializable"),
            )
        };
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(values)
    }

    pub async fn replace_relationships(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.mutate_relationships(entity_id, input, true).await
    }

    pub async fn remove_relationships(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        self.mutate_relationships(entity_id, input, false).await
    }

    async fn mutate_relationships(
        &self,
        entity_id: Uuid,
        input: RelationshipMutation,
        replace: bool,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        if !input.relationships.is_empty() {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let write = WriteContext::load(&mut transaction, self.workspace_id.0, &entity).await?;
        let mut values = Vec::new();
        for relationship in input.relationships {
            let context_id = Some(write.resolve_context(relationship.context_id)?);
            let (attribute_id, target_blueprint_codes, context_editable) =
                Self::relationship_attribute(&write, &relationship)?;
            write.ensure_editable(context_id, &context_editable)?;
            // Relationship writes are set operations; sorting target IDs gives
            // every concurrent writer the same target-lock order.
            let targets: BTreeSet<_> = relationship.target_entity_ids.into_iter().collect();
            self.validate_relationship_targets(&mut transaction, &targets, &target_blueprint_codes)
                .await?;

            let current = self
                .current_relationship_targets(&mut transaction, entity.id, attribute_id, context_id)
                .await?;
            let removals: Vec<_> = if replace {
                current.difference(&targets).copied().collect()
            } else {
                current.intersection(&targets).copied().collect()
            };
            for target_id in removals {
                values.push(
                    self.insert_relationship_value(
                        &mut transaction,
                        &write,
                        &entity,
                        attribute_id,
                        context_id,
                        target_id,
                        false,
                    )
                    .await?,
                );
            }
            if replace {
                for target_id in targets.difference(&current) {
                    values.push(
                        self.insert_relationship_value(
                            &mut transaction,
                            &write,
                            &entity,
                            attribute_id,
                            context_id,
                            *target_id,
                            true,
                        )
                        .await?,
                    );
                }
            }
        }
        self.validate_entity_schema_in(&mut transaction, &write, &entity, Revalidation::Write)
            .await?;
        self.touch_entity(&mut transaction, entity_id).await?;
        let after = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let changes = Self::audit_changes(entity_id, before, after, false);
        let event = self.core_event(
            RELATIONSHIP_CHANGED_V1,
            "entity",
            entity_id,
            serde_json::to_value(RelationshipMutationV1 {
                entity_id,
                facts: Self::affected_facts(&changes),
            })
            .expect("relationship-changed payload is serializable"),
        );
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(values)
    }

    pub(super) async fn replace_relationship_sets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        relationships: Vec<RelationshipTargets>,
    ) -> Result<(), RepositoryError> {
        for relationship in relationships {
            let context_id = Some(write.resolve_context(relationship.context_id)?);
            let (attribute_id, target_blueprint_codes, context_editable) =
                Self::relationship_attribute(write, &relationship)?;
            write.ensure_editable(context_id, &context_editable)?;
            let targets: BTreeSet<_> = relationship.target_entity_ids.into_iter().collect();
            self.validate_relationship_targets(transaction, &targets, &target_blueprint_codes)
                .await?;
            let current = self
                .current_relationship_targets(transaction, entity.id, attribute_id, context_id)
                .await?;
            for target_id in current.difference(&targets) {
                self.insert_relationship_value(
                    transaction,
                    write,
                    entity,
                    attribute_id,
                    context_id,
                    *target_id,
                    false,
                )
                .await?;
            }
            for target_id in targets.difference(&current) {
                self.insert_relationship_value(
                    transaction,
                    write,
                    entity,
                    attribute_id,
                    context_id,
                    *target_id,
                    true,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Inserts `values` in order. Each value is resolved and validated as it
    /// is reached, so errors keep their input precedence. Relationship values
    /// are written immediately because their cardinality checks read the
    /// values already written; scalar values are written together at the end
    /// with one archive and one insert.
    pub(super) async fn insert_values_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        values: Vec<NewAttributeValue>,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut inserted: Vec<Option<AttributeValue>> = Vec::with_capacity(values.len());
        let mut scalars: Vec<(usize, PreparedValue)> = Vec::new();
        for value in values {
            let prepared = Self::prepare_value(write, value)?;
            if prepared.relationship.is_some() {
                inserted.push(Some(
                    self.write_prepared_value(transaction, write, entity, prepared)
                        .await?,
                ));
            } else {
                scalars.push((inserted.len(), prepared));
                inserted.push(None);
            }
        }
        let distinct_keys = scalars
            .iter()
            .map(|(_, value)| (value.attribute_id, value.context_id))
            .collect::<HashSet<_>>()
            .len();
        if scalars.len() > 1 && distinct_keys == scalars.len() {
            let (positions, prepared): (Vec<_>, Vec<_>) = scalars.into_iter().unzip();
            let written = self
                .write_scalar_values(transaction, entity, prepared)
                .await?;
            for (position, value) in positions.into_iter().zip(written) {
                inserted[position] = Some(value);
            }
        } else {
            // A repeated attribute and context archives the earlier value of
            // the same write, exactly as one-at-a-time writes do.
            for (position, prepared) in scalars {
                inserted[position] = Some(
                    self.write_prepared_value(transaction, write, entity, prepared)
                        .await?,
                );
            }
        }
        Ok(inserted
            .into_iter()
            .map(|value| value.expect("every value was written"))
            .collect())
    }

    /// [`Self::insert_value_in`] for a caller without a write context.
    pub(super) async fn insert_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        value: NewAttributeValue,
    ) -> Result<AttributeValue, RepositoryError> {
        let context = WriteContext::load(transaction, self.workspace_id.0, entity).await?;
        self.insert_value_in(transaction, &context, entity, value)
            .await
    }

    pub(super) async fn insert_value_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        value: NewAttributeValue,
    ) -> Result<AttributeValue, RepositoryError> {
        let prepared = Self::prepare_value(write, value)?;
        self.write_prepared_value(transaction, write, entity, prepared)
            .await
    }

    /// Resolves a value's context and attribute and validates it without a
    /// database read: selector, editability, kind, native type and schema.
    fn prepare_value(
        write: &WriteContext,
        value: NewAttributeValue,
    ) -> Result<PreparedValue, RepositoryError> {
        let (attribute_id, attribute_code, context_id, payload, target_entity_id) = match value {
            NewAttributeValue::Scalar {
                attribute_id,
                attribute_code,
                context_id,
                value,
            } => (attribute_id, attribute_code, context_id, value, None),
            NewAttributeValue::Relationship {
                attribute_id,
                attribute_code,
                context_id,
                target_entity_id,
            } => (
                attribute_id,
                attribute_code,
                context_id,
                Value::Null,
                Some(target_entity_id),
            ),
        };
        let context_id = write.resolve_context(context_id)?;
        let attribute_label = attribute_code.clone();
        let attribute = write.attribute(attribute_id, attribute_code.as_deref())?;
        write.ensure_editable(Some(context_id), &attribute.context_editable)?;
        if (attribute.value_type == "relationship") != target_entity_id.is_some() {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        let attribute_id = attribute.id;
        if let Some(target_entity_id) = target_entity_id {
            return Ok(PreparedValue {
                attribute_id,
                attribute_code: attribute.code.clone(),
                context_id,
                relationship: Some(PreparedRelationship {
                    target_entity_id,
                    target_blueprint_codes: attribute.target_blueprint_codes.clone(),
                    cardinality: attribute
                        .cardinality
                        .clone()
                        .unwrap_or_else(|| "many".to_owned()),
                    target_cardinality: attribute
                        .target_cardinality
                        .clone()
                        .unwrap_or_else(|| "many".to_owned()),
                }),
                native: None,
            });
        }
        let native = NativeValue::parse(ValueType::parse(&attribute.value_type)?, payload)?;
        if let Some(schema) = &attribute.value_schema
            && let Some(error) = validate_json_schema(schema, &native.json())
                .map_err(|message| RepositoryError::AttributeValueSchemaMismatch {
                    attribute: attribute_label
                        .clone()
                        .unwrap_or_else(|| attribute_id.to_string()),
                    instance_path: String::new(),
                    message,
                })?
                .into_iter()
                .next()
        {
            return Err(RepositoryError::AttributeValueSchemaMismatch {
                attribute: attribute_label.unwrap_or_else(|| attribute_id.to_string()),
                instance_path: error.instance_path,
                message: error.message,
            });
        }
        Ok(PreparedValue {
            attribute_id,
            attribute_code: attribute.code.clone(),
            context_id,
            relationship: None,
            native: Some(native),
        })
    }

    /// Writes one prepared value: relationship target and cardinality checks,
    /// then archive the current value and insert the new one.
    async fn write_prepared_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        prepared: PreparedValue,
    ) -> Result<AttributeValue, RepositoryError> {
        let PreparedValue {
            attribute_id,
            attribute_code,
            context_id,
            relationship,
            native,
        } = prepared;
        let target_entity_id = relationship
            .as_ref()
            .map(|relationship| relationship.target_entity_id);
        if let Some(relationship) = &relationship {
            self.validate_relationship_target(
                transaction,
                relationship.target_entity_id,
                &relationship.target_blueprint_codes,
            )
            .await?;
            self.validate_relationship_cardinality(
                transaction,
                write,
                CardinalityCheck {
                    entity,
                    attribute_id,
                    attribute_code: &attribute_code,
                    cardinality: &relationship.cardinality,
                    target_cardinality: &relationship.target_cardinality,
                    context_id: Some(context_id),
                    target_entity_id: relationship.target_entity_id,
                },
            )
            .await?;
        }

        self.archive_current_value(
            transaction,
            entity.id,
            attribute_id,
            Some(context_id),
            target_entity_id,
        )
        .await?;

        let query = sqlx::query_as::<_, Db<AttributeValue>>(
            r#"INSERT INTO attribute_values (
                    id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                    value_time, value_time_zone, value_json
                ) VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                RETURNING id, entity_id, attribute_id,
                    'null'::jsonb AS value,
                    relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .bind(attribute_id)
        .bind(context_id)
        .bind(target_entity_id);
        let query = match native {
            Some(native) => native.bind(query),
            None => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
        };
        Ok(query.fetch_one(&mut **transaction).await?.into_domain())
    }

    /// Writes scalar values with distinct `(attribute, context)` keys: one
    /// statement archives every current value, one inserts every new value.
    /// Returns the inserted values in input order.
    async fn write_scalar_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        prepared: Vec<PreparedValue>,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let attribute_ids: Vec<Uuid> = prepared.iter().map(|value| value.attribute_id).collect();
        let context_ids: Vec<Uuid> = prepared.iter().map(|value| value.context_id).collect();
        self.archive_current_scalar_values(transaction, entity.id, &attribute_ids, &context_ids)
            .await?;
        let ids: Vec<Uuid> = prepared.iter().map(|_| Uuid::new_v4()).collect();
        let mut text = Vec::with_capacity(prepared.len());
        let mut number = Vec::with_capacity(prepared.len());
        let mut integer = Vec::with_capacity(prepared.len());
        let mut boolean = Vec::with_capacity(prepared.len());
        let mut date = Vec::with_capacity(prepared.len());
        let mut datetime = Vec::with_capacity(prepared.len());
        let mut time = Vec::with_capacity(prepared.len());
        let mut time_zone = Vec::with_capacity(prepared.len());
        let mut json = Vec::with_capacity(prepared.len());
        for value in prepared {
            let columns = value
                .native
                .map(NativeValue::into_columns)
                .unwrap_or_default();
            text.push(columns.text);
            number.push(columns.number);
            integer.push(columns.integer);
            boolean.push(columns.boolean);
            date.push(columns.date);
            datetime.push(columns.datetime);
            time.push(columns.time);
            time_zone.push(columns.time_zone);
            json.push(columns.json);
        }
        let rows = sqlx::query_as::<_, Db<AttributeValue>>(
            r#"INSERT INTO attribute_values (
                    id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                    value_time, value_time_zone, value_json
                )
                SELECT v.id, $1, $2, v.attribute_id, v.context_id, NULL, true,
                       v.value_text, v.value_number, v.value_integer, v.value_boolean, v.value_date,
                       v.value_datetime, v.value_time, v.value_time_zone, v.value_json
                FROM UNNEST($3::uuid[], $4::uuid[], $5::uuid[], $6::text[], $7::numeric[],
                            $8::int8[], $9::bool[], $10::date[], $11::timestamptz[], $12::time[],
                            $13::text[], $14::jsonb[])
                     AS v(id, attribute_id, context_id, value_text, value_number, value_integer,
                          value_boolean, value_date, value_datetime, value_time, value_time_zone,
                          value_json)
                RETURNING id, entity_id, attribute_id,
                    'null'::jsonb AS value,
                    relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .bind(&ids)
        .bind(&attribute_ids)
        .bind(&context_ids)
        .bind(text)
        .bind(number)
        .bind(integer)
        .bind(boolean)
        .bind(date)
        .bind(datetime)
        .bind(time)
        .bind(time_zone)
        .bind(json)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .map(|row| {
            let value: AttributeValue = row.into_domain();
            (value.id, value)
        })
        .collect::<HashMap<_, _>>();
        let mut rows = rows;
        Ok(ids
            .iter()
            .map(|id| rows.remove(id).expect("every inserted value is returned"))
            .collect())
    }

    async fn remove_scalar_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        selector: AttributeValueSelector,
    ) -> Result<(), RepositoryError> {
        validate_attribute_selector_code(&selector.attribute_code)?;
        let context_id = Some(write.resolve_context(selector.context_id)?);
        let attribute = write
            .by_code(&selector.attribute_code)
            .ok_or(RepositoryError::AttributeNotApplicable)?;
        if attribute.value_type == "relationship" {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        write.ensure_editable(context_id, &attribute.context_editable)?;
        self.archive_current_value(transaction, entity.id, attribute.id, context_id, None)
            .await?;
        Ok(())
    }

    pub(super) async fn resolve_context_id(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let context_id = match context_id {
            Some(context_id) => context_id,
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
            )
            .bind(workspace_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or(RepositoryError::InvalidContext)?,
        };
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(context_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !exists {
            return Err(RepositoryError::InvalidContext);
        }
        Ok(Some(context_id))
    }

    pub(super) async fn validate_context_editable(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Option<Uuid>,
        context_editable: &str,
    ) -> Result<(), RepositoryError> {
        if context_editable == "default" {
            let is_default = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2 AND code = 'default')",
            )
            .bind(context_id.ok_or(RepositoryError::InvalidContext)?)
            .bind(self.workspace_id.0)
            .fetch_one(&mut **transaction)
            .await?;
            if !is_default {
                return Err(RepositoryError::DefaultContextOnly);
            }
        }
        Ok(())
    }

    pub(super) async fn lock_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Db<Entity>>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at
               FROM entities WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut **transaction)
        .await?
        .into_domain()
        .ok_or(RepositoryError::NotFound("entity"))
    }

    async fn insert_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        blueprint_id: Uuid,
        blueprint_version: i64,
        system_tags: Vec<String>,
        system_metadata: Value,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Db<Entity>>(
            r#"INSERT INTO entities (id, workspace_id, blueprint_id, blueprint_version, projections, system_tags, system_metadata)
               SELECT $1, $2, b.id, b.version, $3, $4, $5
               FROM blueprints b
                WHERE b.id = $6 AND b.version = $7 AND b.workspace_id = $2 AND b.kind = 'entity' AND b.status = 'published' AND b.deleted_at IS NULL
                RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(empty_projections())
        .bind(system_tags)
        .bind(system_metadata)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?
        .into_domain()
        .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    /// The shared validation step of every entity write; see [`Revalidation`].
    pub(super) async fn validate_entity_schema(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        self.validate_entity_schema_with(transaction, entity, Revalidation::Write)
            .await
    }

    pub(super) async fn validate_entity_schema_with(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        mode: Revalidation,
    ) -> Result<(), RepositoryError> {
        let write = WriteContext::load(transaction, self.workspace_id.0, entity).await?;
        self.validate_entity_schema_in(transaction, &write, entity, mode)
            .await
    }

    pub(super) async fn validate_entity_schema_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        mode: Revalidation,
    ) -> Result<(), RepositoryError> {
        let tree = &write.tree;
        // The write's own status changes, validated before system effects
        // (approval voids) add transitions of their own.
        let changes = match mode {
            Revalidation::Write => {
                self.checked_status_changes(transaction, entity, write)
                    .await?
            }
            Revalidation::Structural => Vec::new(),
        };
        self.validate_principal_values(transaction, entity, write)
            .await?;
        if mode == Revalidation::Write {
            self.apply_status_effects_in(transaction, entity, write)
                .await?;
        }
        // Every value write validates here, so unique keys stay current.
        self.sync_entity_unique_keys(transaction, entity).await?;
        let entity_schema = sqlx::query_scalar::<_, Option<Value>>(
            "SELECT entity_schema FROM blueprints WHERE id = $1 AND version = $2 AND deleted_at IS NULL",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .fetch_one(&mut **transaction)
        .await?;
        let record = load_record(
            transaction,
            self.workspace_id.0,
            entity.id,
            RecordState::After,
        )
        .await?
        .unwrap_or_else(|| {
            RecordValues::empty(entity.id, entity.blueprint_id, entity.blueprint_version)
        });
        if let Some(entity_schema) = &entity_schema {
            validate_json_entity_schema(tree, &record, entity_schema)?;
        }
        self.enforce_declarative_checks(
            transaction,
            entity_schema.as_ref(),
            tree,
            &record,
            &changes,
        )
        .await
    }

    /// The validation tail for writes that change stored values without the
    /// update seam (file references, reusable attribute attachment):
    /// [`Self::validate_entity_schema`] with its status side effects, then the
    /// rebuilt preview. Returns the stored entity.
    pub(super) async fn revalidate_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<Entity, RepositoryError> {
        self.validate_entity_schema(transaction, entity).await?;
        let preview = Self::build_preview_projection(transaction, entity.id).await?;
        self.store_preview(transaction, entity.id, preview).await
    }

    fn relationship_attribute(
        write: &WriteContext,
        relationship: &RelationshipTargets,
    ) -> Result<(Uuid, Vec<String>, String), RepositoryError> {
        let attribute = write.attribute(
            relationship.attribute_id,
            relationship.attribute_code.as_deref(),
        )?;
        if attribute.value_type != "relationship" {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        Ok((
            attribute.id,
            attribute.target_blueprint_codes.clone(),
            attribute.context_editable.clone(),
        ))
    }

    async fn validate_relationship_target(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        target_entity_id: Uuid,
        allowed_target_blueprints: &[String],
    ) -> Result<(), RepositoryError> {
        let target = sqlx::query_scalar::<_, String>(
            r#"SELECT b.code FROM entities e JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
               WHERE e.id = $1 AND e.workspace_id = $2 AND e.deleted_at IS NULL"#,
        )
        .bind(target_entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("relationship target entity"))?;
        // An empty set accepts any entity blueprint.
        if !allowed_target_blueprints.is_empty() && !allowed_target_blueprints.contains(&target) {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
        }
        Ok(())
    }

    /// [`Self::validate_relationship_target`] for a set of targets in one
    /// query, reporting the first failing target in set order.
    async fn validate_relationship_targets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        targets: &BTreeSet<Uuid>,
        allowed_target_blueprints: &[String],
    ) -> Result<(), RepositoryError> {
        if targets.is_empty() {
            return Ok(());
        }
        let ids: Vec<Uuid> = targets.iter().copied().collect();
        let codes: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>(
            r#"SELECT e.id, b.code FROM entities e JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
               WHERE e.id = ANY($1) AND e.workspace_id = $2 AND e.deleted_at IS NULL"#,
        )
        .bind(&ids)
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .collect();
        for target in &ids {
            let code = codes
                .get(target)
                .ok_or(RepositoryError::NotFound("relationship target entity"))?;
            // An empty set accepts any entity blueprint.
            if !allowed_target_blueprints.is_empty() && !allowed_target_blueprints.contains(code) {
                return Err(RepositoryError::RelationshipTargetTypeMismatch);
            }
        }
        Ok(())
    }

    async fn current_relationship_targets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
    ) -> Result<BTreeSet<Uuid>, RepositoryError> {
        Ok(sqlx::query_scalar::<_, Uuid>(
            r#"SELECT relationship_target_entity_id
               FROM attribute_values
               WHERE entity_id = $1
                 AND attribute_id = $2
                 AND context_id IS NOT DISTINCT FROM $3
                 AND relationship_target_entity_id IS NOT NULL
                  AND active"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .collect())
    }

    pub(super) async fn lock_relationship_cardinality_writes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        // All relationship writers in a workspace acquire the same lock before
        // entity rows, cardinality validation, or relationship mutations. A
        // single canonical key avoids opposite-order deadlocks across payloads
        // that claim more than one target while keeping workspaces independent.
        let lock_key = format!("relationship-cardinality:{}", self.workspace_id.0);
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    async fn validate_relationship_cardinality(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        check: CardinalityCheck<'_>,
    ) -> Result<(), RepositoryError> {
        let CardinalityCheck {
            entity,
            attribute_id,
            attribute_code,
            cardinality,
            target_cardinality,
            context_id,
            target_entity_id,
        } = check;
        self.validate_relationship_hierarchy(
            transaction,
            write,
            entity,
            attribute_id,
            attribute_code,
            context_id,
            target_entity_id,
        )
        .await?;
        let target_is_one = target_cardinality == "one"
            || write
                .family_constraints(transaction, self.workspace_id.0, entity.blueprint_id)
                .await?
                .target_one_codes
                .contains(attribute_code);
        if cardinality != "one" && !target_is_one {
            return Ok(());
        }

        let source_conflict = if cardinality == "one" {
            sqlx::query_scalar::<_, Uuid>(
                r#"SELECT relationship_target_entity_id
               FROM attribute_values
               WHERE entity_id = $1 AND attribute_id = $2
                 AND context_id IS NOT DISTINCT FROM $3
                 AND relationship_target_entity_id IS NOT NULL AND active
                 AND relationship_target_entity_id <> $4
               FOR UPDATE"#,
            )
            .bind(entity.id)
            .bind(attribute_id)
            .bind(context_id)
            .bind(target_entity_id)
            .fetch_optional(&mut **transaction)
            .await?
        } else {
            None
        };
        if let Some(existing_target) = source_conflict {
            return Err(RepositoryError::RelationshipCardinalityConflict {
                attribute: attribute_code.to_owned(),
                context_id,
                source_entity_id: entity.id,
                target_entity_id: existing_target,
                conflicting_source_entity_id: None,
            });
        }

        // Attribute IDs are revision-local. Field identity is the blueprint
        // family plus code, so revisions with a target limit cannot claim the
        // same target merely because their attribute IDs differ.
        let target_conflict = if target_is_one {
            sqlx::query_scalar::<_, Uuid>(
                r#"SELECT av.entity_id
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               WHERE ((a.blueprint_id = $1 AND a.code = $2)
                    OR (a.reusable_attribute_revision_id = (SELECT reusable_attribute_revision_id FROM attributes WHERE id = $7) AND a.code = $2))
                 AND a.workspace_id = $6 AND av.workspace_id = $6
                 AND av.context_id IS NOT DISTINCT FROM $3
                 AND av.relationship_target_entity_id = $4 AND av.active
                 AND av.entity_id <> $5
               FOR UPDATE OF av"#,
            )
            .bind(entity.blueprint_id)
            .bind(attribute_code)
            .bind(context_id)
            .bind(target_entity_id)
            .bind(entity.id)
            .bind(self.workspace_id.0)
            .bind(attribute_id)
            .fetch_optional(&mut **transaction)
            .await?
        } else {
            None
        };
        if let Some(conflicting_source_entity_id) = target_conflict {
            return Err(RepositoryError::RelationshipCardinalityConflict {
                attribute: attribute_code.to_owned(),
                context_id,
                source_entity_id: entity.id,
                target_entity_id,
                conflicting_source_entity_id: Some(conflicting_source_entity_id),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_relationship_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        entity: &Entity,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        target_entity_id: Uuid,
        active: bool,
    ) -> Result<AttributeValue, RepositoryError> {
        // The caller holds the entity lock and resolved the attribute from
        // the same write context.
        let entity_id = entity.id;
        if active {
            let attribute = write
                .by_id(attribute_id)
                .filter(|attribute| attribute.value_type == "relationship")
                .ok_or(RepositoryError::AttributeNotApplicable)?;
            self.validate_relationship_cardinality(
                transaction,
                write,
                CardinalityCheck {
                    entity,
                    attribute_id,
                    attribute_code: &attribute.code,
                    cardinality: attribute.cardinality.as_deref().unwrap_or("many"),
                    target_cardinality: attribute.target_cardinality.as_deref().unwrap_or("many"),
                    context_id,
                    target_entity_id,
                },
            )
            .await?;
        }
        let archived = self
            .archive_current_value(
                transaction,
                entity_id,
                attribute_id,
                context_id,
                Some(target_entity_id),
            )
            .await?;

        if !active {
            return Ok(archived
                .map(|mut value| {
                    value.active = false;
                    value
                })
                .unwrap_or(AttributeValue {
                    id: Uuid::new_v4(),
                    entity_id,
                    attribute_id,
                    value: Value::Null,
                    relationship_target_entity_id: Some(target_entity_id),
                    active: false,
                    context_id,
                    created_at: Utc::now(),
                }));
        }

        Ok(sqlx::query_as::<_, Db<AttributeValue>>(
            r#"INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, entity_id, attribute_id, 'null'::jsonb AS value, relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4()).bind(self.workspace_id.0).bind(entity_id).bind(attribute_id).bind(context_id)
        .bind(target_entity_id).bind(active).fetch_one(&mut **transaction).await?
        .into_domain())
    }

    pub(super) async fn entity_audit_snapshot(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Vec<AuditValueSnapshot>, RepositoryError> {
        sqlx::query_as::<_, AuditValueSnapshot>(
            r#"SELECT av.attribute_id, a.code AS attribute_code, av.context_id, c.code AS context_code,
                      av.relationship_target_entity_id,
                      CASE a.value_type
                        WHEN 'string' THEN to_jsonb(av.value_text)
                        WHEN 'number' THEN to_jsonb(av.value_number)
                        WHEN 'integer' THEN to_jsonb(av.value_integer)
                        WHEN 'boolean' THEN to_jsonb(av.value_boolean)
                        WHEN 'date' THEN to_jsonb(av.value_date)
                        WHEN 'datetime' THEN to_jsonb(av.value_datetime)
                        WHEN 'time' THEN jsonb_build_object('time', av.value_time::text, 'time_zone', av.value_time_zone)
                        WHEN 'json' THEN av.value_json
                        WHEN 'relationship' THEN to_jsonb(av.relationship_target_entity_id::text)
                      END AS value
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               LEFT JOIN attribute_contexts c ON c.id = av.context_id
               WHERE av.entity_id = $1 AND av.workspace_id = $2 AND (av.relationship_target_entity_id IS NULL OR av.active)
                 AND a.value_type <> 'file'
               ORDER BY a.code, c.code, av.relationship_target_entity_id"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await
        .map_err(Into::into)
    }

    pub(super) fn audit_changes(
        entity_id: Uuid,
        before: Vec<AuditValueSnapshot>,
        after: Vec<AuditValueSnapshot>,
        restored: bool,
    ) -> Vec<AuditEventChange> {
        let index = |values: Vec<AuditValueSnapshot>| {
            values
                .into_iter()
                .map(|value| {
                    (
                        (
                            value.attribute_id,
                            value.context_id,
                            value.relationship_target_entity_id,
                        ),
                        value,
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        let before = index(before);
        let after = index(after);
        let keys: std::collections::BTreeSet<_> =
            before.keys().chain(after.keys()).copied().collect();
        keys.into_iter()
            .filter_map(|key| {
                let old = before.get(&key);
                let new = after.get(&key);
                if old.map(|value| &value.value) == new.map(|value| &value.value) {
                    return None;
                }
                let source = new.or(old)?;
                let relationship = source.relationship_target_entity_id.is_some();
                let change_kind = if restored {
                    "restore"
                } else if relationship && old.is_none() {
                    "relationship_add"
                } else if relationship && new.is_none() {
                    "relationship_remove"
                } else if old.is_none() {
                    "set"
                } else if new.is_none() {
                    "remove"
                } else {
                    "replace"
                };
                Some(AuditEventChange {
                    entity_id,
                    attribute_id: source.attribute_id,
                    attribute_code: source.attribute_code.clone(),
                    context_id: source.context_id,
                    context_code: source.context_code.clone(),
                    relationship_target_entity_id: source.relationship_target_entity_id,
                    change_kind,
                    before_value: old.map(|value| value.value.clone()),
                    after_value: new.map(|value| value.value.clone()),
                })
            })
            .collect()
    }

    /// [`Self::archive_current_value`] for many scalar `(attribute, context)`
    /// keys of one entity in one statement.
    async fn archive_current_scalar_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_ids: &[Uuid],
        context_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"WITH keys AS (
                    SELECT * FROM UNNEST($2::uuid[], $3::uuid[]) AS k(attribute_id, context_id)
                ), file_references AS MATERIALIZED (
                    SELECT r.attribute_value_id, r.workspace_id, r.file_id, r.position
                    FROM attribute_file_references r
                    JOIN attribute_values av ON av.id = r.attribute_value_id
                    JOIN keys ON keys.attribute_id = av.attribute_id AND keys.context_id = av.context_id
                    WHERE av.entity_id = $1
                      AND av.workspace_id = $4
                      AND av.relationship_target_entity_id IS NULL
                ), archived AS (
                    DELETE FROM attribute_values av
                    USING keys
                    WHERE av.entity_id = $1
                      AND av.workspace_id = $4
                      AND av.attribute_id = keys.attribute_id
                      AND av.context_id = keys.context_id
                      AND av.relationship_target_entity_id IS NULL
                    RETURNING av.*
                ), stored AS (
                    INSERT INTO attribute_value_history (
                        id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                        value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                        value_time, value_time_zone, value_json, created_at
                    )
                    SELECT id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                           value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                           value_time, value_time_zone, value_json, created_at
                    FROM archived
                    RETURNING id, workspace_id, archived_at
                )
                INSERT INTO attribute_file_reference_history (
                    attribute_value_history_id, attribute_value_history_archived_at, workspace_id, file_id, position
                )
                SELECT stored.id, stored.archived_at, file_references.workspace_id, file_references.file_id, file_references.position
                FROM file_references JOIN stored
                  ON stored.id = file_references.attribute_value_id
                 AND stored.workspace_id = file_references.workspace_id"#,
        )
        .bind(entity_id)
        .bind(attribute_ids)
        .bind(context_ids)
        .bind(self.workspace_id.0)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    pub(super) async fn archive_current_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        relationship_target_entity_id: Option<Uuid>,
    ) -> Result<Option<AttributeValue>, RepositoryError> {
        sqlx::query_as::<_, Db<AttributeValue>>(
            r#"WITH file_references AS MATERIALIZED (
                    SELECT r.attribute_value_id, r.workspace_id, r.file_id, r.position
                    FROM attribute_file_references r
                    JOIN attribute_values av ON av.id = r.attribute_value_id
                    WHERE av.entity_id = $1
                      AND av.workspace_id = $5
                      AND av.attribute_id = $2
                      AND av.context_id IS NOT DISTINCT FROM $3
                      AND av.relationship_target_entity_id IS NOT DISTINCT FROM $4
                ), archived AS (
                    DELETE FROM attribute_values
                    WHERE entity_id = $1
                      AND workspace_id = $5
                      AND attribute_id = $2
                      AND context_id IS NOT DISTINCT FROM $3
                      AND relationship_target_entity_id IS NOT DISTINCT FROM $4
                    RETURNING *
                ), stored AS (
                    INSERT INTO attribute_value_history (
                        id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                        value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                        value_time, value_time_zone, value_json, created_at
                    )
                    SELECT id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                           value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                           value_time, value_time_zone, value_json, created_at
                    FROM archived
                    RETURNING id, workspace_id, archived_at
                ), copied_references AS (
                    INSERT INTO attribute_file_reference_history (
                        attribute_value_history_id, attribute_value_history_archived_at, workspace_id, file_id, position
                    )
                    SELECT stored.id, stored.archived_at, file_references.workspace_id, file_references.file_id, file_references.position
                    FROM file_references JOIN stored
                      ON stored.id = file_references.attribute_value_id
                     AND stored.workspace_id = file_references.workspace_id
                )
                SELECT id, entity_id, attribute_id, 'null'::jsonb AS value,
                       relationship_target_entity_id, context_id, active, created_at
                FROM archived"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .bind(relationship_target_entity_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut **transaction)
        .await
        .map(IntoDomain::into_domain)
        .map_err(Into::into)
    }

    async fn touch_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE entities SET updated_at = now() WHERE id = $1 AND workspace_id = $2")
            .bind(entity_id)
            .bind(self.workspace_id.0)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }
    /// Applies one bounded extension batch. Each intent receives its own
    /// transaction so a retry may return durable per-intent outcomes without
    /// repeating a catalog mutation. The marker, audit rows, outbox event and
    /// task-fence check share that transaction.
    pub async fn execute_extension_catalog_batch(
        &self,
        batch: ExtensionCatalogBatch,
    ) -> Result<Vec<ExtensionCatalogIntentOutcome>, RepositoryError> {
        if batch.batch_key.is_empty()
            || batch.batch_key.len() > MAX_EXTENSION_BATCH_KEY_BYTES
            || !batch.batch_key.is_ascii()
        {
            return Err(RepositoryError::InvalidExtension(
                "batch key must be 1-256 ASCII bytes".into(),
            ));
        }
        if batch.intents.is_empty() || batch.intents.len() > MAX_EXTENSION_BATCH_INTENTS {
            return Err(RepositoryError::InvalidExtension(
                "batch must contain 1-100 intents".into(),
            ));
        }
        let extension_id = self.extension_id.as_deref().ok_or_else(|| {
            RepositoryError::InvalidExtension(
                "extension batch requires extension provenance".into(),
            )
        })?;
        let mut outcomes = Vec::with_capacity(batch.intents.len());
        let mut keys = HashSet::new();
        for intent in batch.intents {
            let key = extension_intent_key(&intent).to_owned();
            if key.is_empty()
                || key.len() > MAX_EXTENSION_INTENT_KEY_BYTES
                || !key.is_ascii()
                || !keys.insert(key.clone())
            {
                return Err(RepositoryError::InvalidExtension(
                    "intent keys must be unique 1-128 ASCII bytes".into(),
                ));
            }
            match self
                .execute_extension_catalog_intent(
                    extension_id,
                    &batch.batch_key,
                    batch.dry_run,
                    intent,
                )
                .await
            {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => outcomes.push(ExtensionCatalogIntentOutcome {
                    intent_key: key,
                    status: ExtensionCatalogIntentStatus::Rejected,
                    entity_id: None,
                    error: Some(error.to_string()),
                    annotation_revision: None,
                }),
            }
        }
        Ok(outcomes)
    }

    async fn execute_extension_catalog_intent(
        &self,
        extension_id: &str,
        batch_key: &str,
        dry_run: bool,
        intent: ExtensionCatalogIntent,
    ) -> Result<ExtensionCatalogIntentOutcome, RepositoryError> {
        let key = extension_intent_key(&intent).to_owned();
        let serialized = serde_json::to_vec(&intent).expect("extension intent serializes");
        let input_hash = format!("{:x}", sha2::Sha256::digest(serialized));
        let ws = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        if !dry_run {
            let existing: Option<(String, Value)> = sqlx::query_as("SELECT input_hash,outcome FROM extension_catalog_batch_intents WHERE workspace_id=$1 AND extension_id=$2 AND batch_key=$3 AND intent_key=$4 FOR UPDATE")
                .bind(ws).bind(extension_id).bind(batch_key).bind(&key).fetch_optional(&mut *transaction).await?;
            if let Some((existing_hash, outcome)) = existing {
                if existing_hash != input_hash {
                    return Err(RepositoryError::InvalidExtension(
                        "intent key was reused with different input".into(),
                    ));
                }
                let mut outcome: ExtensionCatalogIntentOutcome = serde_json::from_value(outcome)
                    .map_err(|_| {
                        RepositoryError::InvalidExtension("stored batch outcome is invalid".into())
                    })?;
                outcome.status = ExtensionCatalogIntentStatus::AlreadyApplied;
                transaction.commit().await?;
                return Ok(outcome);
            }
        }
        let result = match intent {
            ExtensionCatalogIntent::Annotate {
                entity_id,
                add_tags,
                remove_tags,
                set_metadata,
                remove_metadata,
                expected_revision,
                ..
            } => self
                .apply_extension_annotation_patch(
                    &mut transaction,
                    extension_id,
                    entity_id,
                    &super::ExtensionAnnotationPatch {
                        add_tags,
                        remove_tags,
                        set_metadata,
                        remove_metadata,
                        expected_revision,
                    },
                    super::extension_annotations::AnnotationPatchAuthority::Extension,
                )
                .await
                .map(|revision| (entity_id, Some(revision))),
            intent => self
                .apply_extension_catalog_intent(&mut transaction, intent)
                .await
                .map(|entity_id| (entity_id, None)),
        };
        match result {
            Ok((entity_id, annotation_revision)) => {
                let status = if dry_run {
                    ExtensionCatalogIntentStatus::Validated
                } else {
                    ExtensionCatalogIntentStatus::Applied
                };
                let outcome = ExtensionCatalogIntentOutcome {
                    intent_key: key.clone(),
                    status,
                    entity_id: Some(entity_id),
                    error: None,
                    annotation_revision,
                };
                if dry_run {
                    transaction.rollback().await?;
                    return Ok(outcome);
                }
                self.ensure_task_fence(&mut transaction).await?;
                sqlx::query("INSERT INTO extension_catalog_batch_intents(workspace_id,extension_id,batch_key,intent_key,input_hash,outcome) VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(ws).bind(extension_id).bind(batch_key).bind(&key).bind(input_hash).bind(serde_json::to_value(&outcome).expect("outcome serializes")).execute(&mut *transaction).await?;
                transaction.commit().await?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await?;
                if dry_run {
                    Ok(ExtensionCatalogIntentOutcome {
                        intent_key: key,
                        status: ExtensionCatalogIntentStatus::Rejected,
                        entity_id: None,
                        error: Some(error.to_string()),
                        annotation_revision: None,
                    })
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn apply_extension_catalog_intent(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        intent: ExtensionCatalogIntent,
    ) -> Result<Uuid, RepositoryError> {
        // An interactive run is bounded by its frozen selection; it cannot
        // create catalog entities outside that selection.
        if self.authorization_actor().is_some()
            && matches!(
                intent,
                ExtensionCatalogIntent::Create { .. } | ExtensionCatalogIntent::Upsert { .. }
            )
        {
            return Err(RepositoryError::InvalidExtension(
                "interactive runs cannot create or upsert entities".into(),
            ));
        }
        match intent {
            ExtensionCatalogIntent::Create {
                blueprint_id,
                blueprint_version,
                values,
                system_tags,
                system_metadata,
                ..
            } => {
                self.apply_extension_catalog_create(
                    transaction,
                    blueprint_id,
                    blueprint_version,
                    values,
                    system_tags,
                    system_metadata,
                )
                .await
            }
            ExtensionCatalogIntent::Update {
                entity_id,
                values,
                relationships,
                ..
            } => {
                self.apply_extension_catalog_update(transaction, entity_id, values, relationships)
                    .await
            }
            ExtensionCatalogIntent::Relationships {
                entity_id,
                relationships,
                ..
            } => {
                self.apply_extension_catalog_update(
                    transaction,
                    entity_id,
                    Vec::new(),
                    relationships,
                )
                .await
            }
            ExtensionCatalogIntent::Annotate { .. } => Err(RepositoryError::InvalidExtension(
                "annotation intents use the namespace patch path".into(),
            )),
            ExtensionCatalogIntent::Upsert {
                blueprint_id,
                blueprint_version,
                lookup_attribute_id,
                lookup_value,
                values,
                relationships,
                system_tags,
                system_metadata,
                ..
            } => {
                if lookup_value.is_empty()
                    || lookup_value.len()
                        > super::extension_catalog_data::MAX_EXTENSION_LOOKUP_VALUE_BYTES
                {
                    return Err(RepositoryError::InvalidExtension(
                        "upsert lookup value must be 1-512 bytes".into(),
                    ));
                }
                // The lookup locks the matched entity row, so the workspace
                // relationship lock must be taken before it.
                if !relationships.is_empty() || writes_relationship_values(&values) {
                    self.lock_relationship_cardinality_writes(transaction)
                        .await?;
                }
                let existing = self
                    .extension_upsert_lookup(
                        transaction,
                        blueprint_id,
                        blueprint_version,
                        lookup_attribute_id,
                        &lookup_value,
                    )
                    .await?;
                match existing {
                    Some(entity_id) => {
                        self.apply_extension_catalog_update(
                            transaction,
                            entity_id,
                            values,
                            relationships,
                        )
                        .await
                    }
                    None => {
                        self.apply_extension_catalog_create(
                            transaction,
                            blueprint_id,
                            blueprint_version,
                            values,
                            system_tags,
                            system_metadata,
                        )
                        .await
                    }
                }
            }
        }
    }

    /// Finds the entity a legacy upsert updates, locking its row. When the
    /// lookup attribute alone forms a declared unique key, the key index
    /// resolves it across every revision of the blueprint family, with the
    /// key's normalization. Otherwise the lookup is advisory: an exact text
    /// match among entities pinned to the requested revision.
    async fn extension_upsert_lookup(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        lookup_attribute_id: Uuid,
        lookup_value: &str,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let attribute: Option<(String, String)> = sqlx::query_as(
            "SELECT code, value_type FROM attributes WHERE id = $1 AND workspace_id = $2 AND blueprint_id = $3 AND blueprint_version = $4 AND deleted_at IS NULL",
        )
        .bind(lookup_attribute_id)
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?;
        let unique_key = match &attribute {
            Some((code, value_type)) if value_type == "string" => {
                super::structural_constraints::enforced_unique_keys(
                    transaction,
                    workspace_id,
                    blueprint_id,
                )
                .await?
                .into_iter()
                .find(|key| key.attributes.len() == 1 && &key.attributes[0] == code)
            }
            _ => None,
        };
        let matches: Vec<Uuid> = if let Some(key) = unique_key {
            let Some(component) = catalog_validation::unique_key::normalize_key_component(
                "string",
                &Value::String(lookup_value.to_owned()),
                key.case_sensitive,
            ) else {
                return Ok(None);
            };
            let key_hash = super::structural_constraints::key_hash(&Value::Array(vec![component]));
            // Serializes concurrent absent-key upserts of the same key value,
            // so the second one updates rather than conflicting on the index.
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!(
                    "extension-upsert-key:{workspace_id}:{blueprint_id}:{}:{key_hash}",
                    key.code
                ))
                .execute(&mut **transaction)
                .await?;
            // Workspace keys are indexed in the default context only; for a
            // context key, the default context's value identifies the record.
            let default_context_id = self
                .resolve_context_id(transaction, None)
                .await?
                .ok_or(RepositoryError::InvalidContext)?;
            sqlx::query_scalar(
                "SELECT e.id FROM entity_unique_key_values k JOIN entities e ON e.id = k.entity_id AND e.workspace_id = k.workspace_id \
                 WHERE k.workspace_id = $1 AND k.blueprint_id = $2 AND k.key_code = $3 AND k.context_id = $4 AND k.key_hash = $5 AND e.deleted_at IS NULL \
                 ORDER BY e.id FOR UPDATE OF e LIMIT 2",
            )
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(&key.code)
            .bind(default_context_id)
            .bind(&key_hash)
            .fetch_all(&mut **transaction)
            .await?
        } else {
            // Without a declared single-attribute unique key nothing prevents
            // duplicates of this value. Serialize the lookup so concurrent
            // absent-key upserts cannot both take the create branch.
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!("extension-upsert:{workspace_id}:{blueprint_id}:{blueprint_version}:{lookup_attribute_id}:{lookup_value}"))
                .execute(&mut **transaction)
                .await?;
            sqlx::query_scalar(
                "SELECT e.id FROM entities e JOIN attribute_values v ON v.entity_id=e.id AND v.workspace_id=e.workspace_id AND v.active \
                 WHERE e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=$2 AND e.blueprint_version=$3 \
                   AND v.attribute_id=$4 AND v.relationship_target_entity_id IS NULL AND v.value_text=$5 \
                 ORDER BY e.id FOR UPDATE OF e LIMIT 2",
            )
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(lookup_attribute_id)
            .bind(lookup_value)
            .fetch_all(&mut **transaction)
            .await?
        };
        match matches.as_slice() {
            [] => Ok(None),
            [entity_id] => Ok(Some(*entity_id)),
            _ => Err(RepositoryError::InvalidExtension(
                "upsert lookup matched multiple entities".into(),
            )),
        }
    }

    /// A legacy extension create: the ordinary create seam. Legacy
    /// create/upsert annotation fields predate namespace ownership and cannot
    /// write a claimed namespace, including the caller's own.
    async fn apply_extension_catalog_create(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: Vec<NewAttributeValue>,
        system_tags: Vec<String>,
        system_metadata: Value,
    ) -> Result<Uuid, RepositoryError> {
        let (entity, changes, event) = self
            .create_entity_in_transaction(
                transaction,
                ChosenIdEntityCreate {
                    entity_id: Uuid::new_v4(),
                    blueprint_id,
                    blueprint_version,
                    values,
                    system_tags,
                    system_metadata,
                    host_sample_marker: false,
                },
            )
            .await?;
        self.stage_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity.id)
    }

    /// A legacy extension update: the ordinary update seam, bounded for an
    /// interactive run by its initiator's grants. The legacy intent ABI has
    /// no caller-supplied version token, so it cannot change a status.
    async fn apply_extension_catalog_update(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        values: Vec<NewAttributeValue>,
        relationships: Vec<RelationshipTargets>,
    ) -> Result<Uuid, RepositoryError> {
        // The seam takes this lock too; the actor checks below come first.
        if !relationships.is_empty() || writes_relationship_values(&values) {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
        }
        self.ensure_actor_may(transaction, "entities.write", entity_id)
            .await?;
        // A run bound to a user may link only to entities that user can read,
        // whether or not they are in the run's selection.
        let targets: Vec<Uuid> = values
            .iter()
            .filter_map(|value| match value {
                NewAttributeValue::Relationship {
                    target_entity_id, ..
                } => Some(*target_entity_id),
                NewAttributeValue::Scalar { .. } => None,
            })
            .chain(
                relationships
                    .iter()
                    .flat_map(|set| set.target_entity_ids.iter().copied()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.ensure_actor_may_all(transaction, "entities.read", &targets)
            .await?;
        let (entity, changes, event) = self
            .update_entity_in_transaction(
                transaction,
                entity_id,
                UpdateEntityFormRequest {
                    expected_updated_at: None,
                    values,
                    relationships,
                    remove_values: Vec::new(),
                    system_tags: None,
                    system_metadata: None,
                },
            )
            .await?;
        self.stage_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity.id)
    }
}

fn writes_relationship_values(values: &[NewAttributeValue]) -> bool {
    values
        .iter()
        .any(|value| matches!(value, NewAttributeValue::Relationship { .. }))
}

/// Validates the blueprint's JSON entity schema in every context against the
/// resolved values of the blueprint's fields (see
/// [`RecordValues::schema_document`]); the first violation is returned.
pub(super) fn validate_json_entity_schema(
    tree: &ContextTree,
    record: &RecordValues,
    entity_schema: &Value,
) -> Result<(), RepositoryError> {
    for context in tree.nodes() {
        let document = record.schema_document(&tree.path(context.id, true)?);
        if let Some(error) = validate_json_schema(entity_schema, &Value::Object(document))
            .map_err(|message| RepositoryError::EntitySchemaMismatch {
                context: context.code.clone(),
                instance_path: String::new(),
                message,
            })?
            .into_iter()
            .next()
        {
            return Err(RepositoryError::EntitySchemaMismatch {
                context: context.code.clone(),
                instance_path: error.instance_path,
                message: error.message,
            });
        }
    }
    Ok(())
}

fn extension_intent_key(intent: &ExtensionCatalogIntent) -> &str {
    match intent {
        ExtensionCatalogIntent::Create { intent_key, .. }
        | ExtensionCatalogIntent::Update { intent_key, .. }
        | ExtensionCatalogIntent::Relationships { intent_key, .. }
        | ExtensionCatalogIntent::Upsert { intent_key, .. }
        | ExtensionCatalogIntent::Annotate { intent_key, .. } => intent_key,
    }
}

fn validate_system_annotations(tags: &[String], metadata: &Value) -> Result<(), RepositoryError> {
    validate_system_tags(tags)?;
    validate_system_metadata(metadata)
}

pub(super) fn validate_system_tag_update(
    current: &[String],
    requested: &[String],
) -> Result<(), RepositoryError> {
    let marker = "attricat.sample";
    if requested.iter().any(|tag| tag == marker) && !current.iter().any(|tag| tag == marker) {
        return Err(RepositoryError::InvalidSystemTags);
    }
    validate_system_tags(
        &requested
            .iter()
            .filter(|tag| tag.as_str() != marker)
            .cloned()
            .collect::<Vec<_>>(),
    )
}

/// A tag and metadata patch, applied by [`apply_tag_metadata_patch`].
#[derive(Default)]
pub(super) struct TagMetadataPatch {
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub set_metadata: BTreeMap<String, Value>,
    pub remove_metadata: Vec<String>,
}

/// The effective changes of one [`TagMetadataPatch`].
pub(super) struct TagMetadataChanges {
    pub added_tags: Vec<String>,
    pub removed_tags: Vec<String>,
    pub set_keys: Vec<String>,
    pub removed_keys: Vec<String>,
}

impl TagMetadataChanges {
    pub fn is_empty(&self) -> bool {
        self.added_tags.is_empty()
            && self.removed_tags.is_empty()
            && self.set_keys.is_empty()
            && self.removed_keys.is_empty()
    }
}

/// Applies `patch` in place: tag removals, then additions (kept in order,
/// without duplicates), then metadata sets and key removals. Reports only
/// what actually changed. Callers validate the result.
pub(super) fn apply_tag_metadata_patch(
    tags: &mut Vec<String>,
    metadata: &mut Map<String, Value>,
    patch: &TagMetadataPatch,
) -> TagMetadataChanges {
    let removed_tags = patch
        .remove_tags
        .iter()
        .filter(|tag| tags.contains(tag))
        .cloned()
        .collect();
    tags.retain(|tag| !patch.remove_tags.contains(tag));
    let mut added_tags = Vec::new();
    for tag in &patch.add_tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
            added_tags.push(tag.clone());
        }
    }
    let mut set_keys = Vec::new();
    for (key, value) in &patch.set_metadata {
        if metadata.get(key) != Some(value) {
            set_keys.push(key.clone());
        }
        metadata.insert(key.clone(), value.clone());
    }
    let removed_keys = patch
        .remove_metadata
        .iter()
        .filter(|key| metadata.remove(*key).is_some())
        .cloned()
        .collect();
    TagMetadataChanges {
        added_tags,
        removed_tags,
        set_keys,
        removed_keys,
    }
}

/// Longest stored system tag, including an extension namespace prefix.
pub(super) const MAX_SYSTEM_TAG_BYTES: usize = 128;

pub(super) fn validate_system_tags(tags: &[String]) -> Result<(), RepositoryError> {
    if tags.len() > 100
        || tags.iter().any(|tag| {
            tag.trim().is_empty() || tag.len() > MAX_SYSTEM_TAG_BYTES || tag == "attricat.sample"
        })
        || tags.iter().collect::<HashSet<_>>().len() != tags.len()
    {
        return Err(RepositoryError::InvalidSystemTags);
    }
    Ok(())
}

pub(super) fn validate_system_metadata(metadata: &Value) -> Result<(), RepositoryError> {
    if !metadata.is_object()
        || serde_json::to_vec(metadata).map_or(true, |value| value.len() > 64 * 1024)
    {
        return Err(RepositoryError::InvalidSystemMetadata);
    }
    Ok(())
}
