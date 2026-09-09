use super::*;
use crate::domain_events::{
    ENTITY_MIGRATED_V1, EntityMigratedV1, EventSource, EventSourceKind, NewDomainEvent,
};
use catalog_validation::validate_json_schema;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

type FileReferenceKey = (String, Option<Uuid>);
type FileReferencesByAttribute = HashMap<FileReferenceKey, Vec<(Uuid, i32)>>;

impl CatalogRepository {
    pub async fn preview_entity_migration(
        &self,
        entity_id: Uuid,
    ) -> Result<EntityMigrationPreview, RepositoryError> {
        let entity = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let target = self
            .get_current_blueprint(entity.blueprint_id)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))?;
        if target.blueprint.version == entity.blueprint_version {
            return Err(RepositoryError::EntityBlueprintCurrent);
        }
        let source_attributes = self
            .list_attributes(entity.blueprint_id, entity.blueprint_version)
            .await?;
        let values = self.form_values(entity_id).await?;
        let target_attributes: HashMap<_, _> = target
            .attributes
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect();
        let mut issues = Vec::new();
        for value in &values {
            let attribute_code = match value {
                FormAttributeValue::Scalar { attribute_code, .. }
                | FormAttributeValue::Relationship { attribute_code, .. }
                | FormAttributeValue::File { attribute_code, .. } => attribute_code,
            };
            let source = source_attributes
                .iter()
                .find(|attribute| attribute.code == *attribute_code)
                .expect("form values belong to the source blueprint");
            match target_attributes.get(source.code.as_str()) {
                None => issues.push(MigrationIssue {
                    attribute_code: Some(source.code.clone()),
                    kind: "removed".to_owned(),
                    message: "The attribute was removed in the target revision".to_owned(),
                }),
                Some(target) if target.value_type != source.value_type => {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "value_type_changed".to_owned(),
                        message: "The attribute value type changed in the target revision"
                            .to_owned(),
                    });
                }
                Some(target)
                    if source.value_type == "relationship"
                        && target.target_blueprint_code != source.target_blueprint_code =>
                {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "relationship_target_changed".to_owned(),
                        message: "The relationship target blueprint changed in the target revision"
                            .to_owned(),
                    });
                }
                Some(target) => {
                    if let (Some(schema), FormAttributeValue::Scalar { value, .. }) =
                        (&target.value_schema, value)
                        && !validate_json_schema(schema, value)
                            .map_err(RepositoryError::invalid_blueprint_definition)?
                            .is_empty()
                    {
                        issues.push(MigrationIssue {
                            attribute_code: Some(source.code.clone()),
                            kind: "attribute_schema_mismatch".to_owned(),
                            message: "The stored value does not meet the target attribute schema"
                                .to_owned(),
                        });
                    }
                }
            }
        }
        if let Some(schema) = &target.blueprint.entity_schema {
            let mut document = Map::new();
            for value in &values {
                match value {
                    FormAttributeValue::Scalar {
                        attribute_code,
                        value,
                        ..
                    } => {
                        document.insert(attribute_code.clone(), value.clone());
                    }
                    FormAttributeValue::Relationship {
                        attribute_code,
                        target_entity_id,
                        ..
                    } => {
                        document
                            .entry(attribute_code.clone())
                            .or_insert_with(|| Value::Array(Vec::new()))
                            .as_array_mut()
                            .expect("relationship values are arrays")
                            .push(Value::String(target_entity_id.to_string()));
                    }
                    FormAttributeValue::File { .. } => {}
                }
            }
            let target_codes: HashSet<_> = target_attributes.keys().copied().collect();
            for violation in validate_json_schema(schema, &Value::Object(document))
                .map_err(RepositoryError::invalid_blueprint_definition)?
            {
                for field in missing_required_fields(&violation.message, &target_codes) {
                    if !issues.iter().any(|issue| {
                        issue.kind == "missing_required"
                            && issue.attribute_code.as_deref() == Some(field.as_str())
                    }) {
                        issues.push(MigrationIssue {
                            attribute_code: Some(field),
                            kind: "missing_required".to_owned(),
                            message: "A value is required by the target entity schema".to_owned(),
                        });
                    }
                }
            }
        }
        let status = if issues.is_empty() {
            "ready"
        } else {
            "needs_input"
        };
        let migration_id = Uuid::new_v4();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO entity_blueprint_migrations
                   (id, workspace_id, entity_id, blueprint_id, source_version, target_version, status, issues)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(migration_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .bind(entity.id)
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(target.blueprint.version)
        .bind(status)
        .bind(serde_json::to_value(&issues).expect("migration issues serialize"))
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(EntityMigrationPreview {
            migration_id,
            source_version: entity.blueprint_version,
            target,
            values,
            status: status.to_owned(),
            issues,
        })
    }

    pub async fn migrate_entity_to_latest(
        &self,
        entity_id: Uuid,
        input: MigrateEntityRequest,
    ) -> Result<Entity, RepositoryError> {
        let migration_input = serde_json::to_value(&input).expect("migration input serializes");
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let migration = sqlx::query_as::<_, (Uuid, i64, i64, String)>(
            "SELECT entity_id, source_version, target_version, status FROM entity_blueprint_migrations WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(input.migration_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::MigrationNotApplicable)?;
        if migration.0 != entity.id || migration.1 != entity.blueprint_version {
            return Err(RepositoryError::MigrationNotApplicable);
        }
        let target_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM blueprints WHERE id = $1 AND workspace_id = $2 AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(entity.blueprint_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_one(&mut *transaction)
        .await?;
        if target_version != input.expected_target_version || target_version != migration.2 {
            sqlx::query("UPDATE entity_blueprint_migrations SET status = 'superseded', completed_at = now() WHERE id = $1 AND workspace_id = $2")
                .bind(input.migration_id)
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .execute(&mut *transaction)
                .await?;
            self.commit_mutation(transaction).await?;
            return Err(RepositoryError::MigrationTargetChanged);
        }
        if migration.3 == "blocked" {
            return Err(RepositoryError::MigrationNotApplicable);
        }
        let source_values = self
            .form_values_in_transaction(&mut transaction, entity.id)
            .await?;
        let source_file_references = sqlx::query_as::<_, (String, Option<Uuid>, Uuid, i32)>(
            r#"SELECT a.code, av.context_id, r.file_id, r.position
               FROM attribute_file_references r
               JOIN attribute_values av ON av.id = r.attribute_value_id
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1
                 AND av.workspace_id = $2
                 AND a.value_type = 'file'
               ORDER BY a.code, av.context_id, r.position"#,
        )
        .bind(entity.id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&mut *transaction)
        .await?;
        let source_attributes = self
            .list_attributes_in_transaction(
                &mut transaction,
                entity.blueprint_id,
                entity.blueprint_version,
            )
            .await?;
        let target_attributes = self
            .list_attributes_in_transaction(&mut transaction, entity.blueprint_id, target_version)
            .await?;
        let source_by_code: HashMap<_, _> = source_attributes
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect();
        let target_by_code: HashMap<_, _> = target_attributes
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect();
        let supplied_scalar_attributes: HashSet<_> = input
            .values
            .iter()
            .filter_map(|value| match value {
                NewAttributeValue::Scalar {
                    attribute_code: Some(attribute_code),
                    ..
                } => Some(attribute_code.as_str()),
                _ => None,
            })
            .collect();
        let supplied_relationship_attributes: HashSet<_> = input
            .relationships
            .iter()
            .filter_map(|relationship| relationship.attribute_code.as_deref())
            .collect();
        let discarded_attributes: HashSet<_> = input
            .discard_attributes
            .iter()
            .map(String::as_str)
            .collect();
        let mut carried_values = Vec::new();
        let mut carried_relationships: HashMap<(String, Option<Uuid>), Vec<Uuid>> = HashMap::new();
        let mut carried_file_references: FileReferencesByAttribute = HashMap::new();
        let mut unresolved = HashSet::new();
        for value in source_values {
            match value {
                FormAttributeValue::Scalar {
                    attribute_code,
                    context_id,
                    value,
                } => {
                    let source = source_by_code
                        .get(attribute_code.as_str())
                        .expect("form values belong to the source blueprint");
                    let incompatible = match target_by_code.get(attribute_code.as_str()) {
                        None => true,
                        Some(target) if target.value_type != source.value_type => true,
                        Some(target) => target.value_schema.as_ref().is_some_and(|schema| {
                            !validate_json_schema(schema, &value)
                                .expect("compiled blueprint schema is valid")
                                .is_empty()
                        }),
                    };
                    if discarded_attributes.contains(attribute_code.as_str()) {
                        continue;
                    }
                    if incompatible {
                        if !supplied_scalar_attributes.contains(attribute_code.as_str()) {
                            unresolved.insert(attribute_code);
                        }
                    } else {
                        carried_values.push(NewAttributeValue::Scalar {
                            attribute_id: None,
                            attribute_code: Some(attribute_code),
                            context_id,
                            value,
                        });
                    }
                }
                FormAttributeValue::Relationship {
                    attribute_code,
                    context_id,
                    target_entity_id,
                } => {
                    let source = source_by_code
                        .get(attribute_code.as_str())
                        .expect("form values belong to the source blueprint");
                    let incompatible = match target_by_code.get(attribute_code.as_str()) {
                        None => true,
                        Some(target) => {
                            target.value_type != source.value_type
                                || target.target_blueprint_code != source.target_blueprint_code
                        }
                    };
                    if discarded_attributes.contains(attribute_code.as_str()) {
                        continue;
                    }
                    if incompatible {
                        if !supplied_relationship_attributes.contains(attribute_code.as_str()) {
                            unresolved.insert(attribute_code);
                        }
                    } else {
                        carried_relationships
                            .entry((attribute_code, context_id))
                            .or_default()
                            .push(target_entity_id);
                    }
                }
                // File references are carried below so their IDs and ordering are retained.
                FormAttributeValue::File { .. } => {}
            }
        }
        for (attribute_code, context_id, file_id, position) in source_file_references {
            let source = source_by_code
                .get(attribute_code.as_str())
                .expect("file references belong to the source blueprint");
            let incompatible = match target_by_code.get(attribute_code.as_str()) {
                None => true,
                Some(target) => target.value_type != source.value_type,
            };
            if discarded_attributes.contains(attribute_code.as_str()) {
                continue;
            }
            if incompatible {
                unresolved.insert(attribute_code);
            } else {
                carried_file_references
                    .entry((attribute_code, context_id))
                    .or_default()
                    .push((file_id, position));
            }
        }
        if !unresolved.is_empty() {
            let unresolved: Vec<_> = unresolved.into_iter().collect();
            sqlx::query(
                "UPDATE entity_blueprint_migrations SET status = 'needs_input', input = $2 WHERE id = $1 AND workspace_id = $3",
            )
            .bind(input.migration_id)
            .bind(migration_input)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .execute(&mut *transaction)
            .await?;
            self.commit_mutation(transaction).await?;
            return Err(RepositoryError::MigrationNeedsResolution(unresolved));
        }
        self.archive_all_current_values(&mut transaction, entity.id)
            .await?;
        let target_entity = sqlx::query_as::<_, Entity>(
            r#"UPDATE entities SET blueprint_version = $2, updated_at = now()
               WHERE id = $1 AND workspace_id = $3
               RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at"#,
        )
        .bind(entity.id)
        .bind(target_version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_one(&mut *transaction)
        .await?;
        for value in carried_values.into_iter().chain(input.values) {
            self.insert_value(&mut transaction, &target_entity, value)
                .await?;
        }
        for ((attribute_code, context_id), files) in carried_file_references {
            let attribute_id = target_by_code
                .get(attribute_code.as_str())
                .expect("compatible file attribute belongs to the target blueprint")
                .id;
            let value_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .bind(target_entity.id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut *transaction)
            .await?;
            for (file_id, position) in files {
                sqlx::query(
                    "INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1, $2, $3, $4)",
                )
                .bind(value_id)
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .bind(file_id)
                .bind(position)
                .execute(&mut *transaction)
                .await?;
            }
        }
        let carried_relationships = carried_relationships
            .into_iter()
            .map(
                |((attribute_code, context_id), target_entity_ids)| RelationshipTargets {
                    attribute_id: None,
                    attribute_code: Some(attribute_code),
                    context_id,
                    target_entity_ids,
                },
            )
            .chain(input.relationships)
            .collect();
        self.replace_relationship_sets(&mut transaction, &target_entity, carried_relationships)
            .await?;
        self.validate_entity_schema(&mut transaction, &target_entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let target_entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        sqlx::query(
            "UPDATE entity_blueprint_migrations SET status = 'migrated', input = $2, started_at = COALESCE(started_at, now()), completed_at = now() WHERE id = $1 AND workspace_id = $3",
        )
        .bind(input.migration_id)
        .bind(migration_input)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .execute(&mut *transaction)
        .await?;
        self.commit_mutation_with_event(
            transaction,
            NewDomainEvent {
                event_type: ENTITY_MIGRATED_V1.to_owned(),
                aggregate_kind: "entity".to_owned(),
                aggregate_id: target_entity.id,
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
                payload: serde_json::to_value(EntityMigratedV1 {
                    entity_id: target_entity.id,
                    blueprint_id: target_entity.blueprint_id,
                    source_version: entity.blueprint_version,
                    target_version: target_entity.blueprint_version,
                    migration_id: input.migration_id,
                })
                .expect("entity-migrated payload is serializable"),
            },
        )
        .await?;
        Ok(target_entity)
    }
}
