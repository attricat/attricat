use super::entity_search::empty_projections;
use super::values::{NativeValue, ValueType};
use super::*;
use crate::domain_events::{
    ATTRIBUTE_VALUE_CHANGED_V1, AttributeValueMutationV1, ENTITY_CREATED_V1, ENTITY_DELETED_V1,
    ENTITY_UPDATED_V1, EntityMutationV1, RELATIONSHIP_CHANGED_V1, RelationshipMutationV1,
};
use catalog_validation::validate_json_schema;
use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(sqlx::FromRow, Clone)]
pub(super) struct AuditValueSnapshot {
    attribute_id: Uuid,
    attribute_code: String,
    context_id: Option<Uuid>,
    context_code: Option<String>,
    relationship_target_entity_id: Option<Uuid>,
    value: Value,
}

fn workflow_event_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| match value {
        Value::Object(_) => value.get(key),
        Value::Array(values) => key
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index)),
        _ => None,
    })
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
        validate_system_annotations(&system_tags, &system_metadata)?;
        // Do not expose an entity before its initial values and derived preview
        // agree; otherwise a concurrent reader can observe a partial create.
        let mut transaction = self.pool.begin().await?;
        if values
            .iter()
            .any(|value| matches!(value, NewAttributeValue::Relationship { .. }))
        {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let entity = self
            .insert_entity(
                &mut transaction,
                blueprint_id,
                blueprint_version,
                system_tags,
                system_metadata,
            )
            .await?;
        let default_context_id = self
            .resolve_context_id(&mut transaction, None)
            .await?
            .expect("the default context is required");
        let attributes = self
            .list_attributes_in_transaction(
                &mut transaction,
                entity.blueprint_id,
                entity.blueprint_version,
            )
            .await?;
        for attribute in attributes.iter().filter(|attribute| {
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
        }) {
            self.insert_value(
                &mut transaction,
                &entity,
                NewAttributeValue::Scalar {
                    attribute_id: Some(attribute.id),
                    attribute_code: None,
                    context_id: Some(default_context_id),
                    value: attribute.default_value.clone().expect("filtered above"),
                },
            )
            .await?;
        }
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        self.validate_entity_schema(&mut transaction, &entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        let after = self
            .entity_audit_snapshot(&mut transaction, entity.id)
            .await?;
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
        if let Some(tags) = &system_tags {
            validate_system_tags(tags)?;
        }
        if let Some(metadata) = &system_metadata {
            validate_system_metadata(metadata)?;
        }
        let mut transaction = self.pool.begin().await?;
        if !relationships.is_empty()
            || values
                .iter()
                .any(|value| matches!(value, NewAttributeValue::Relationship { .. }))
        {
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
        }
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        // The row lock serializes writers for an entity. It protects both the
        // one-latest-value invariant and the preview rebuilt from that state.
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        if system_tags.is_some() || system_metadata.is_some() {
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
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .execute(&mut *transaction)
            .await?;
        }
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        for selector in remove_values {
            self.remove_scalar_value(&mut transaction, &entity, selector)
                .await?;
        }
        self.replace_relationship_sets(&mut transaction, &entity, relationships)
            .await?;
        self.validate_entity_schema(&mut transaction, &entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        let after = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
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
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(entity)
    }

    /// Executes one local workflow action and records its idempotency key in the
    /// same transaction as the locked entity mutation, audit, and outbox event.
    /// A reclaimed run therefore observes the key and cannot repeat catalog effects.
    pub(crate) async fn execute_workflow_action(
        &self,
        run: &super::ClaimedWorkflowRun,
        action_index: i32,
        action: &catalog_workflow::Action,
        event: &crate::domain_events::DomainEvent,
    ) -> Result<super::WorkflowActionResult, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        // Every path that changes workflow state takes this lock before a run
        // lock. Keeping that order aligned with disable avoids a run/lifecycle
        // deadlock while making disable a durable execution fence.
        let enabled_version: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT enabled_version FROM workflow_lifecycles WHERE workflow_id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.run.workflow_id)
        .bind(ws)
        .fetch_optional(&mut *transaction)
        .await?
        .flatten();
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM workflow_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.run.id)
        .bind(ws)
        .fetch_optional(&mut *transaction)
        .await?;
        match status.as_deref() {
            Some("cancelled") => {
                transaction.commit().await?;
                return Ok(super::WorkflowActionResult::Cancelled);
            }
            Some("leased") => {}
            _ => {
                return Err(RepositoryError::InvalidWorkflowDefinition(
                    "workflow run is no longer leased".into(),
                ));
            }
        }
        let owner: Option<String> =
            sqlx::query_scalar("SELECT lease_owner FROM workflow_runs WHERE id=$1")
                .bind(run.run.id)
                .fetch_one(&mut *transaction)
                .await?;
        if owner.as_deref() != Some(run.lease_owner.as_str()) {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "workflow run lease was lost".into(),
            ));
        }
        if enabled_version != Some(run.run.workflow_version) {
            sqlx::query("UPDATE workflow_runs SET status='cancelled',cancelled_at=clock_timestamp(),lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2")
                .bind(run.run.id)
                .bind(&run.lease_owner)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::Cancelled);
        }
        let marker = sqlx::query("INSERT INTO workflow_run_actions(run_id,action_index) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(run.run.id).bind(action_index).execute(&mut *transaction).await?;
        if marker.rows_affected() == 0 {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::AlreadyCompleted);
        }

        // Lock first, then derive desired tags/metadata from current state: event
        // payloads intentionally contain only immutable facts and are unordered.
        let entity = self
            .lock_entity(&mut transaction, event.aggregate_id)
            .await?;
        let before = self
            .entity_audit_snapshot(&mut transaction, event.aggregate_id)
            .await?;
        let (values, system_tags, system_metadata) = match action {
            catalog_workflow::Action::SystemTagsAdd { tags } => {
                let mut result = entity.system_tags.clone();
                for tag in tags {
                    if !result.contains(tag) {
                        result.push(tag.clone());
                    }
                }
                validate_system_tags(&result)?;
                (Vec::new(), Some(result), None)
            }
            catalog_workflow::Action::SystemTagsRemove { tags } => (
                Vec::new(),
                Some(
                    entity
                        .system_tags
                        .iter()
                        .filter(|tag| !tags.contains(*tag))
                        .cloned()
                        .collect(),
                ),
                None,
            ),
            catalog_workflow::Action::SystemMetadataMerge { values } => {
                let mut metadata = entity
                    .system_metadata
                    .as_object()
                    .cloned()
                    .ok_or(RepositoryError::InvalidSystemMetadata)?;
                for (key, value) in values {
                    metadata.insert(key.clone(), value.clone());
                }
                let metadata = Value::Object(metadata);
                validate_system_metadata(&metadata)?;
                (Vec::new(), None, Some(metadata))
            }
            catalog_workflow::Action::SystemMetadataDelete { keys } => {
                let mut metadata = entity
                    .system_metadata
                    .as_object()
                    .cloned()
                    .ok_or(RepositoryError::InvalidSystemMetadata)?;
                for key in keys {
                    metadata.remove(key);
                }
                (Vec::new(), None, Some(Value::Object(metadata)))
            }
            catalog_workflow::Action::AttributeWrite {
                attribute_code,
                value,
            } => {
                let value = match value {
                    catalog_workflow::ScalarSource::Fixed { fixed } => fixed.clone(),
                    catalog_workflow::ScalarSource::Event { event_field } => {
                        workflow_event_path(&event.payload, event_field)
                            .cloned()
                            .ok_or_else(|| {
                                RepositoryError::InvalidWorkflowDefinition(
                                    "event field missing".into(),
                                )
                            })?
                    }
                };
                if !(value.is_string()
                    || value.is_number()
                    || value.is_boolean()
                    || value.is_null())
                {
                    return Err(RepositoryError::InvalidWorkflowDefinition(
                        "event field must resolve to a scalar".into(),
                    ));
                }
                (
                    vec![NewAttributeValue::Scalar {
                        attribute_id: None,
                        attribute_code: Some(attribute_code.clone()),
                        context_id: None,
                        value,
                    }],
                    None,
                    None,
                )
            }
        };
        if system_tags.is_some() || system_metadata.is_some() {
            sqlx::query("UPDATE entities SET system_tags=COALESCE($2,system_tags),system_metadata=COALESCE($3,system_metadata),updated_at=now() WHERE id=$1 AND workspace_id=$4")
                .bind(entity.id).bind(system_tags).bind(system_metadata).bind(ws).execute(&mut *transaction).await?;
        }
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        self.validate_entity_schema(&mut transaction, &entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        let after = self
            .entity_audit_snapshot(&mut transaction, entity.id)
            .await?;
        let changes = Self::audit_changes(entity.id, before, after, false);
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
            .expect("entity-updated payload serializes"),
        );
        if let Some(audit_event_id) = self.write_audit_event(&mut transaction).await? {
            for change in changes {
                sqlx::query("INSERT INTO audit_event_changes (id,audit_event_id,workspace_id,entity_id,attribute_id,attribute_code,context_id,context_code,change_kind,before_value,after_value) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                    .bind(Uuid::new_v4()).bind(audit_event_id).bind(ws).bind(change.entity_id).bind(change.attribute_id).bind(change.attribute_code).bind(change.context_id).bind(change.context_code).bind(change.change_kind).bind(change.before_value).bind(change.after_value).execute(&mut *transaction).await?;
            }
        }
        self.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(super::WorkflowActionResult::Executed)
    }

    pub async fn entity_audit_changes(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityAuditChange>, RepositoryError> {
        Ok(sqlx::query_as::<_, EntityAuditChange>(
            r#"SELECT c.audit_event_id, e.occurred_at, e.actor_user_id,
                      actor.display_name AS actor_display_name, actor.email AS actor_email,
                      e.executor_type, e.agent_run_id, e.approval_decision, e.approved_by_user_id,
                      approver.display_name AS approved_by_display_name,
                      c.attribute_id, c.attribute_code, c.context_id, c.context_code,
                      c.change_kind, c.before_value, c.after_value
               FROM audit_event_changes c
               JOIN audit_events e ON e.id = c.audit_event_id
               LEFT JOIN users actor ON actor.id = e.actor_user_id
               LEFT JOIN users approver ON approver.id = e.approved_by_user_id
               WHERE c.entity_id = $1 AND c.workspace_id = $2
               ORDER BY e.occurred_at DESC, c.id DESC"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        sqlx::query(
            "DELETE FROM entity_channel_publications WHERE workspace_id = $1 AND entity_id = $2",
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .bind(entity_id)
        .execute(&mut *transaction)
        .await?;
        let result = sqlx::query(
            "UPDATE entities SET deleted_at = now(), updated_at = now() WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("entity"));
        }
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
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(())
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
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let entity = sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
               FOR UPDATE"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("entity"))?;

        let mut values = Vec::with_capacity(input.values.len());
        for value in input.values {
            values.push(self.insert_value(&mut transaction, &entity, value).await?);
        }

        self.validate_entity_schema(&mut transaction, &entity)
            .await?;

        let preview = Self::build_preview_projection(&mut transaction, entity_id).await?;
        sqlx::query(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1 AND workspace_id = $3"#,
        )
        .bind(entity_id)
        .bind(preview)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let mut values = Vec::new();
        for relationship in input.relationships {
            let context_id = self
                .resolve_context_id(&mut transaction, relationship.context_id)
                .await?;
            let (attribute_id, target_blueprint_code, context_editable) = self
                .relationship_attribute(&mut transaction, &entity, &relationship)
                .await?;
            self.validate_context_editable(&mut transaction, context_id, &context_editable)
                .await?;
            // Relationship writes are set operations; sorting target IDs gives
            // every concurrent writer the same target-lock order.
            let targets: BTreeSet<_> = relationship.target_entity_ids.into_iter().collect();
            for target_id in &targets {
                self.validate_relationship_target(
                    &mut transaction,
                    *target_id,
                    target_blueprint_code.as_deref(),
                )
                .await?;
            }

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
                        entity.id,
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
                            entity.id,
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
        self.validate_entity_schema(&mut transaction, &entity)
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
        entity: &Entity,
        relationships: Vec<RelationshipTargets>,
    ) -> Result<(), RepositoryError> {
        for relationship in relationships {
            let context_id = self
                .resolve_context_id(transaction, relationship.context_id)
                .await?;
            let (attribute_id, target_blueprint_code, context_editable) = self
                .relationship_attribute(transaction, entity, &relationship)
                .await?;
            self.validate_context_editable(transaction, context_id, &context_editable)
                .await?;
            let targets: BTreeSet<_> = relationship.target_entity_ids.into_iter().collect();
            for target_id in &targets {
                self.validate_relationship_target(
                    transaction,
                    *target_id,
                    target_blueprint_code.as_deref(),
                )
                .await?;
            }
            let current = self
                .current_relationship_targets(transaction, entity.id, attribute_id, context_id)
                .await?;
            for target_id in current.difference(&targets) {
                self.insert_relationship_value(
                    transaction,
                    entity.id,
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
                    entity.id,
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

    pub(super) async fn insert_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        value: NewAttributeValue,
    ) -> Result<AttributeValue, RepositoryError> {
        let (attribute_id, attribute_code, context_id, payload, target_entity_id, is_relationship) =
            match value {
                NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code,
                    context_id,
                    value,
                } => (attribute_id, attribute_code, context_id, value, None, false),
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
                    true,
                ),
            };
        let context_id = self.resolve_context_id(transaction, context_id).await?;

        let attribute_label = attribute_code.clone();
        let (attribute_id, attribute_code, value_type, value_schema, target_blueprint_code, cardinality, target_cardinality, context_editable) =
            match (attribute_id, attribute_code.as_deref()) {
                (Some(attribute_id), None) => {
                    sqlx::query_as::<_, (Uuid, String, String, Option<Value>, Option<String>, Option<String>, Option<String>, String)>(
                        r#"SELECT id, code, value_type, value_schema, target_blueprint_code, cardinality, target_cardinality, context_editable
                   FROM attributes
                   WHERE id = $1
                     AND blueprint_id = $2
                     AND blueprint_version = $3
                     AND deleted_at IS NULL"#,
                    )
                    .bind(attribute_id)
                    .bind(entity.blueprint_id)
                    .bind(entity.blueprint_version)
                    .fetch_optional(&mut **transaction)
                    .await?
                }
                (None, Some(attribute_code)) => {
                    validate_code(attribute_code)?;
                    sqlx::query_as::<_, (Uuid, String, String, Option<Value>, Option<String>, Option<String>, Option<String>, String)>(
                        r#"SELECT id, code, value_type, value_schema, target_blueprint_code, cardinality, target_cardinality, context_editable
                   FROM attributes
                   WHERE code = $1
                     AND blueprint_id = $2
                     AND blueprint_version = $3
                     AND deleted_at IS NULL"#,
                    )
                    .bind(attribute_code)
                    .bind(entity.blueprint_id)
                    .bind(entity.blueprint_version)
                    .fetch_optional(&mut **transaction)
                    .await?
                }
                _ => return Err(RepositoryError::InvalidAttributeSelector),
            }
            .ok_or(RepositoryError::AttributeNotApplicable)?;

        self.validate_context_editable(transaction, context_id, &context_editable)
            .await?;

        if (value_type == "relationship") != is_relationship {
            return Err(RepositoryError::AttributeKindMismatch);
        }

        if let Some(target_entity_id) = target_entity_id {
            self.validate_relationship_target(
                transaction,
                target_entity_id,
                target_blueprint_code.as_deref(),
            )
            .await?;
            self.validate_relationship_cardinality(
                transaction,
                CardinalityCheck {
                    entity,
                    attribute_id,
                    attribute_code: &attribute_code,
                    cardinality: cardinality.as_deref().unwrap_or("many"),
                    target_cardinality: target_cardinality.as_deref().unwrap_or("many"),
                    context_id,
                    target_entity_id,
                },
            )
            .await?;
        }

        let native = if is_relationship {
            None
        } else {
            Some(NativeValue::parse(ValueType::parse(&value_type)?, payload)?)
        };
        if let (Some(schema), Some(native)) = (&value_schema, &native)
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

        self.archive_current_value(
            transaction,
            entity.id,
            attribute_id,
            context_id,
            target_entity_id,
        )
        .await?;

        let query = sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (
                    id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                    value_time, value_time_zone
                ) VALUES ($1, $2, $3, $4, $5, $6, true, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING id, entity_id, attribute_id,
                    'null'::jsonb AS value,
                    relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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
                .bind(Option::<String>::None),
        };
        Ok(query.fetch_one(&mut **transaction).await?)
    }

    async fn remove_scalar_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        selector: AttributeValueSelector,
    ) -> Result<(), RepositoryError> {
        validate_code(&selector.attribute_code)?;
        let context_id = self
            .resolve_context_id(transaction, selector.context_id)
            .await?;
        let attribute = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id, value_type, context_editable FROM attributes WHERE code = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL",
        )
        .bind(selector.attribute_code)
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::AttributeNotApplicable)?;
        if attribute.1 == "relationship" {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        self.validate_context_editable(transaction, context_id, &attribute.2)
            .await?;
        self.archive_current_value(transaction, entity.id, attribute.0, context_id, None)
            .await?;
        Ok(())
    }

    async fn resolve_context_id(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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
        sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at
               FROM entities WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("entity"))
    }

    async fn insert_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        system_tags: Vec<String>,
        system_metadata: Value,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Entity>(
            r#"INSERT INTO entities (id, workspace_id, blueprint_id, blueprint_version, projections, system_tags, system_metadata)
               SELECT $1, $2, b.id, b.version, $3, $4, $5
               FROM blueprints b
                WHERE b.id = $6 AND b.version = $7 AND b.workspace_id = $2 AND b.kind = 'entity' AND b.status = 'published' AND b.deleted_at IS NULL
                RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .bind(empty_projections())
        .bind(system_tags)
        .bind(system_metadata)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    pub(super) async fn validate_entity_schema(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let entity_schema = sqlx::query_scalar::<_, Option<Value>>(
            "SELECT entity_schema FROM blueprints WHERE id = $1 AND version = $2 AND deleted_at IS NULL",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .fetch_one(&mut **transaction)
        .await?;
        let Some(entity_schema) = entity_schema else {
            return Ok(());
        };

        let attributes = self
            .list_attributes(entity.blueprint_id, entity.blueprint_version)
            .await?;
        let contexts = sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&mut **transaction)
        .await?;
        let context_by_id: std::collections::HashMap<_, _> = contexts
            .iter()
            .map(|context| (context.id, context))
            .collect();
        let scalar_projection = Self::build_preview_projection(transaction, entity.id).await?;
        let mut direct_values = scalar_projection
            .as_object()
            .cloned()
            .ok_or(RepositoryError::InvalidPreview)?;
        let relationships = sqlx::query_as::<_, (String, String, Uuid)>(
            r#"SELECT a.code, c.code, av.relationship_target_entity_id
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               JOIN attribute_contexts c ON c.id = av.context_id
               WHERE av.entity_id = $1
                 AND a.blueprint_id = $2
                 AND a.blueprint_version = $3
                 AND av.relationship_target_entity_id IS NOT NULL
                  AND av.active"#,
        )
        .bind(entity.id)
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .fetch_all(&mut **transaction)
        .await?;
        for (attribute_code, context_code, target_id) in relationships {
            let context = direct_values
                .entry(context_code)
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            let targets = context
                .entry(attribute_code)
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            targets.push(Value::String(target_id.to_string()));
        }

        for context in &contexts {
            let mut path = Vec::new();
            let mut current = Some(context);
            while let Some(item) = current {
                current = item
                    .parent_id
                    .and_then(|parent_id| context_by_id.get(&parent_id).copied());
                path.push(item);
            }
            let mut document = Map::new();
            for attribute in &attributes {
                for (index, source_context) in path.iter().enumerate() {
                    if index > 0 && attribute.context_fallback == "none" {
                        break;
                    }
                    if let Some(value) = direct_values
                        .get(&source_context.code)
                        .and_then(Value::as_object)
                        .and_then(|values| values.get(&attribute.code))
                    {
                        document.insert(attribute.code.clone(), value.clone());
                        break;
                    }
                }
            }
            if let Some(error) = validate_json_schema(&entity_schema, &Value::Object(document))
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

    async fn relationship_attribute(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        relationship: &RelationshipTargets,
    ) -> Result<(Uuid, Option<String>, String), RepositoryError> {
        let attribute = match (relationship.attribute_id, relationship.attribute_code.as_deref()) {
            (Some(id), None) => sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
                "SELECT id, value_type, target_blueprint_code, context_editable FROM attributes WHERE id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL",
            ).bind(id).bind(entity.blueprint_id).bind(entity.blueprint_version).fetch_optional(&mut **transaction).await?,
            (None, Some(code)) => {
                validate_code(code)?;
                sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
                    "SELECT id, value_type, target_blueprint_code, context_editable FROM attributes WHERE code = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL",
                )
                .bind(code)
                .bind(entity.blueprint_id)
                .bind(entity.blueprint_version)
                .fetch_optional(&mut **transaction)
                .await?
            }
            _ => return Err(RepositoryError::InvalidAttributeSelector),
        }.ok_or(RepositoryError::AttributeNotApplicable)?;
        if attribute.1 != "relationship" {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        Ok((attribute.0, attribute.2, attribute.3))
    }

    async fn validate_relationship_target(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        target_entity_id: Uuid,
        target_blueprint_code: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let target = sqlx::query_scalar::<_, String>(
            r#"SELECT b.code FROM entities e JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
               WHERE e.id = $1 AND e.workspace_id = $2 AND e.deleted_at IS NULL"#,
        )
        .bind(target_entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("relationship target entity"))?;
        if target_blueprint_code.is_some_and(|expected| expected != target) {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
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
        let lock_key = format!(
            "relationship-cardinality:{}",
            self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)
        );
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    async fn validate_relationship_cardinality(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
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
        let target_is_one = if target_cardinality == "one" {
            true
        } else {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM attributes a JOIN blueprints b ON b.id = a.blueprint_id AND b.version = a.blueprint_version WHERE a.blueprint_id = $1 AND a.code = $2 AND a.target_cardinality = 'one' AND a.workspace_id = $3 AND a.deleted_at IS NULL AND b.status = 'published' AND b.deleted_at IS NULL)",
            )
            .bind(entity.blueprint_id)
            .bind(attribute_code)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_one(&mut **transaction)
            .await?
        };
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
               WHERE a.blueprint_id = $1 AND a.code = $2
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
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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

    async fn insert_relationship_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        target_entity_id: Uuid,
        active: bool,
    ) -> Result<AttributeValue, RepositoryError> {
        if active {
            let entity = self.lock_entity(transaction, entity_id).await?;
            let (attribute_code, cardinality, target_cardinality) = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
                "SELECT code, cardinality, target_cardinality FROM attributes WHERE id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND value_type = 'relationship'",
            )
            .bind(attribute_id)
            .bind(entity.blueprint_id)
            .bind(entity.blueprint_version)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or(RepositoryError::AttributeNotApplicable)?;
            self.validate_relationship_cardinality(
                transaction,
                CardinalityCheck {
                    entity: &entity,
                    attribute_id,
                    attribute_code: &attribute_code,
                    cardinality: cardinality.as_deref().unwrap_or("many"),
                    target_cardinality: target_cardinality.as_deref().unwrap_or("many"),
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

        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, relationship_target_entity_id, active)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, entity_id, attribute_id, 'null'::jsonb AS value, relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4()).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(entity_id).bind(attribute_id).bind(context_id)
        .bind(target_entity_id).bind(active).fetch_one(&mut **transaction).await?)
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
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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

    pub(super) async fn archive_current_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        relationship_target_entity_id: Option<Uuid>,
    ) -> Result<Option<AttributeValue>, RepositoryError> {
        sqlx::query_as::<_, AttributeValue>(
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
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut **transaction)
        .await
        .map_err(Into::into)
    }

    pub(super) async fn archive_all_current_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"WITH file_references AS MATERIALIZED (
                    SELECT r.attribute_value_id, r.workspace_id, r.file_id, r.position
                    FROM attribute_file_references r
                    JOIN attribute_values av ON av.id = r.attribute_value_id
                    WHERE av.entity_id = $1 AND av.workspace_id = $2
                ), archived AS (
                    DELETE FROM attribute_values WHERE entity_id = $1 AND workspace_id = $2 RETURNING *
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
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    async fn touch_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE entities SET updated_at = now() WHERE id = $1 AND workspace_id = $2")
            .bind(entity_id)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }
}

fn validate_system_annotations(tags: &[String], metadata: &Value) -> Result<(), RepositoryError> {
    validate_system_tags(tags)?;
    validate_system_metadata(metadata)
}

pub(super) fn validate_system_tags(tags: &[String]) -> Result<(), RepositoryError> {
    if tags.len() > 100
        || tags
            .iter()
            .any(|tag| tag.trim().is_empty() || tag.len() > 128)
        || tags.iter().collect::<HashSet<_>>().len() != tags.len()
    {
        return Err(RepositoryError::InvalidSystemTags);
    }
    Ok(())
}

fn validate_system_metadata(metadata: &Value) -> Result<(), RepositoryError> {
    if !metadata.is_object()
        || serde_json::to_vec(metadata).map_or(true, |value| value.len() > 64 * 1024)
    {
        return Err(RepositoryError::InvalidSystemMetadata);
    }
    Ok(())
}

#[cfg(test)]
mod workflow_event_path_tests {
    use super::workflow_event_path;
    use serde_json::json;

    #[test]
    fn resolves_the_compiled_facts_array_path_exactly() {
        let event = json!({"facts": [{"attribute_code": "title"}]});
        assert_eq!(
            workflow_event_path(&event, "facts.0.attribute_code"),
            Some(&json!("title"))
        );
        assert_eq!(workflow_event_path(&event, "facts.title"), None);
        assert_eq!(workflow_event_path(&event, "facts.1.attribute_code"), None);
    }
}
