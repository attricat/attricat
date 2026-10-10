use super::record_commands::Revalidation;
use super::write_context::WriteContext;
use super::*;
use crate::domain_events::{
    EventSource, EventSourceKind, MAX_RELEASED_RELATIONSHIP_TARGETS,
    MAX_RELEASED_RELATIONSHIPS_BYTES, MAX_RELEASED_TARGETS_PER_EVENT, NewDomainEvent,
    RECORD_MIGRATED_V1, RecordMigratedV1, ReleasedRelationshipV1,
};
use crate::persistence_rows::Db;
use catalog_validation::validate_json_schema;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

type AttributeContextKey = (String, Option<Uuid>);

/// The relationship targets a migration's audit changes removed, grouped by
/// attribute code; see [`RecordMigratedV1::released_relationships`].
fn released_relationships(changes: &[AuditEventChange]) -> Vec<ReleasedRelationshipV1> {
    let mut released: std::collections::BTreeMap<&str, Vec<Uuid>> = Default::default();
    for change in changes {
        if let (Some(target), "relationship_remove") =
            (change.relationship_target_record_id, change.change_kind)
        {
            released
                .entry(change.attribute_code.as_str())
                .or_default()
                .push(target);
        }
    }
    // Serialized sizes: an entry's fixed JSON plus its code, and a quoted
    // UUID with its comma.
    const ENTRY_BYTES: usize = 48;
    const TARGET_BYTES: usize = 39;
    let mut remaining = MAX_RELEASED_TARGETS_PER_EVENT;
    let mut remaining_bytes = MAX_RELEASED_RELATIONSHIPS_BYTES;
    released
        .into_iter()
        .map_while(|(attribute_code, mut target_record_ids)| {
            let entry_bytes = ENTRY_BYTES + attribute_code.len();
            if remaining == 0 || remaining_bytes < entry_bytes + TARGET_BYTES {
                return None;
            }
            let fits = (remaining_bytes - entry_bytes) / TARGET_BYTES;
            target_record_ids.sort_unstable();
            target_record_ids.dedup();
            target_record_ids.truncate(MAX_RELEASED_RELATIONSHIP_TARGETS.min(remaining).min(fits));
            remaining -= target_record_ids.len();
            remaining_bytes -= entry_bytes + target_record_ids.len() * TARGET_BYTES;
            Some(ReleasedRelationshipV1 {
                attribute_code: attribute_code.to_owned(),
                target_record_ids,
            })
        })
        .collect()
}

/// The target revision and default context a migration batch previews every
/// record against, read once per batch.
pub(super) struct MigrationTarget {
    pub target: BlueprintWithAttributes,
    pub default_context_id: Uuid,
}

impl CatalogRepository {
    pub async fn preview_record_migration(
        &self,
        record_id: Uuid,
    ) -> Result<RecordMigrationPreview, RepositoryError> {
        self.preview_record_migration_into(record_id, None).await
    }

    /// Populates a batch-reserved migration row. Reserving the row before this
    /// preview makes retries reuse one stable migration identity per record.
    pub async fn preview_record_migration_into(
        &self,
        record_id: Uuid,
        migration_id: Option<Uuid>,
    ) -> Result<RecordMigrationPreview, RepositoryError> {
        self.preview_record_migration_against(record_id, migration_id, None)
            .await
    }

    /// The current published revision of `blueprint_id` and the default
    /// context, for [`Self::preview_record_migration_against`].
    pub(super) async fn migration_target(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Option<MigrationTarget>, RepositoryError> {
        let Some(target) = self.get_current_blueprint(blueprint_id).await? else {
            return Ok(None);
        };
        let default_context_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
        )
        .bind(self.workspace_id.0)
        .fetch_one(&self.pool)
        .await?;
        Ok(Some(MigrationTarget {
            target,
            default_context_id,
        }))
    }

    /// [`Self::preview_record_migration_into`] against a preloaded target. A
    /// target that changes afterwards supersedes the migration when it runs.
    pub(super) async fn preview_record_migration_against(
        &self,
        record_id: Uuid,
        migration_id: Option<Uuid>,
        preloaded: Option<&MigrationTarget>,
    ) -> Result<RecordMigrationPreview, RepositoryError> {
        let record = self
            .get_record(record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        let target = match preloaded {
            Some(preloaded) => preloaded.target.clone(),
            None => self
                .get_current_blueprint(record.blueprint_id)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint"))?,
        };
        if target.blueprint.version == record.blueprint_version {
            return Err(RepositoryError::RecordBlueprintCurrent);
        }
        let source_attributes = self
            .list_attributes(record.blueprint_id, record.blueprint_version)
            .await?;
        let values = self.form_values(record_id).await?;
        let default_context_id = match preloaded {
            Some(preloaded) => preloaded.default_context_id,
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
            )
            .bind(self.workspace_id.0)
            .fetch_one(&self.pool)
            .await?,
        };
        let target_attributes: HashMap<_, _> = target
            .attributes
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect();
        let mut issues = Vec::new();
        for value in &values {
            let (attribute_code, context_id) = match value {
                FormAttributeValue::Scalar {
                    attribute_code,
                    context_id,
                    ..
                }
                | FormAttributeValue::Relationship {
                    attribute_code,
                    context_id,
                    ..
                }
                | FormAttributeValue::File {
                    attribute_code,
                    context_id,
                    ..
                } => (attribute_code, context_id),
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
                        && target.target_blueprint_codes != source.target_blueprint_codes =>
                {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "relationship_target_changed".to_owned(),
                        message: "The relationship target blueprint changed in the target revision"
                            .to_owned(),
                    });
                }
                Some(target)
                    if source.value_type == "relationship"
                        && (target.cardinality != source.cardinality
                            || target.target_cardinality != source.target_cardinality) =>
                {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "relationship_cardinality_changed".to_owned(),
                        message: "The relationship cardinality changed in the target revision"
                            .to_owned(),
                    });
                }
                Some(target)
                    if source.value_type == "file"
                        && (target.file_policy != source.file_policy
                            || target.cardinality != source.cardinality) =>
                {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "file_contract_changed".to_owned(),
                        message: "The file policy or cardinality changed in the target revision"
                            .to_owned(),
                    });
                }
                Some(target)
                    if target.context_editable == "default"
                        && *context_id != Some(default_context_id) =>
                {
                    issues.push(MigrationIssue {
                        attribute_code: Some(source.code.clone()),
                        kind: "context_not_editable".to_owned(),
                        message: "The stored value context is not editable in the target revision"
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
        let mut relationship_targets: HashMap<(String, Option<Uuid>), HashSet<Uuid>> =
            HashMap::new();
        for value in &values {
            if let FormAttributeValue::Relationship {
                attribute_code,
                context_id,
                target_record_id,
            } = value
                && target_attributes
                    .get(attribute_code.as_str())
                    .is_some_and(|attribute| attribute.cardinality.as_deref() == Some("one"))
            {
                relationship_targets
                    .entry((attribute_code.clone(), *context_id))
                    .or_default()
                    .insert(*target_record_id);
            }
        }
        for ((attribute_code, _), targets) in relationship_targets {
            if targets.len() > 1 {
                issues.push(MigrationIssue {
                    attribute_code: Some(attribute_code),
                    kind: "relationship_cardinality_conflict".to_owned(),
                    message:
                        "The target revision permits only one relationship target per source in each context"
                            .to_owned(),
                });
            }
        }
        if let Some(schema) = &target.blueprint.record_schema {
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
                        target_record_id,
                        ..
                    } => {
                        document
                            .entry(attribute_code.clone())
                            .or_insert_with(|| Value::Array(Vec::new()))
                            .as_array_mut()
                            .expect("relationship values are arrays")
                            .push(Value::String(target_record_id.to_string()));
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
                            message: "A value is required by the target record schema".to_owned(),
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
        let (migration_id, reserved_for_batch) = match migration_id {
            Some(migration_id) => (migration_id, true),
            None => (Uuid::new_v4(), false),
        };
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        if reserved_for_batch {
            let updated = sqlx::query(
                "UPDATE record_blueprint_migrations SET source_version = $2, target_version = $3, status = $4, issues = $5 WHERE id = $1 AND workspace_id = $6 AND status = 'pending'",
            )
            .bind(migration_id)
            .bind(record.blueprint_version)
            .bind(target.blueprint.version)
            .bind(status)
            .bind(serde_json::to_value(&issues).expect("migration issues serialize"))
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if updated != 1 {
                return Err(RepositoryError::MigrationNotApplicable);
            }
        } else {
            sqlx::query(
                r#"INSERT INTO record_blueprint_migrations
                       (id, workspace_id, record_id, blueprint_id, source_version, target_version, status, issues)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(migration_id)
            .bind(workspace_id)
            .bind(record.id)
            .bind(record.blueprint_id)
            .bind(record.blueprint_version)
            .bind(target.blueprint.version)
            .bind(status)
            .bind(serde_json::to_value(&issues).expect("migration issues serialize"))
            .execute(&mut *transaction)
            .await?;
        }
        self.commit_mutation(transaction).await?;
        Ok(RecordMigrationPreview {
            source_updated_at: record.updated_at,
            migration_id,
            source_version: record.blueprint_version,
            target,
            values,
            status: status.to_owned(),
            issues,
        })
    }

    pub async fn migrate_record_to_latest(
        &self,
        record_id: Uuid,
        input: MigrateRecordRequest,
    ) -> Result<Record, RepositoryError> {
        self.migrate_record_to_latest_checked(record_id, input, None)
            .await
    }

    pub async fn migrate_record_to_latest_checked(
        &self,
        record_id: Uuid,
        input: MigrateRecordRequest,
        expected_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Record, RepositoryError> {
        let migration_input = serde_json::to_value(&input).expect("migration input serializes");
        let mut input = input;
        let mut transaction = self.pool.begin().await?;
        let transaction_started = std::time::Instant::now();
        let input_has_relationships = !input.relationships.is_empty()
            || input
                .values
                .iter()
                .any(|value| matches!(value, NewAttributeValue::Relationship { .. }));
        // Published blueprint definitions are immutable. If no revision of
        // this record's blueprint defines relationships and the payload has no
        // relationship writes, the migration cannot read or mutate protected
        // relationship rows. Looking across all revisions is intentionally
        // more conservative than inspecting only the mutable migration row.
        let blueprint_has_relationships = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS (
                   SELECT 1
                   FROM records e
                   JOIN attributes a ON a.blueprint_id = e.blueprint_id
                   WHERE e.id = $1 AND e.workspace_id = $2
                     AND a.workspace_id = $2 AND a.deleted_at IS NULL
                     AND a.value_type = 'relationship'
               )"#,
        )
        .bind(record_id)
        .bind(self.workspace_id.0)
        .fetch_one(&mut *transaction)
        .await?;
        if input_has_relationships || blueprint_has_relationships {
            let lock_started = std::time::Instant::now();
            self.lock_relationship_cardinality_writes(&mut transaction)
                .await?;
            metrics::histogram!("catalog_record_migration_relationship_lock_wait_seconds")
                .record(lock_started.elapsed().as_secs_f64());
        }
        let record = self.lock_record(&mut transaction, record_id).await?;
        // Snapshot under the lock so a concurrent writer cannot change the
        // audited "before" state between the read and this migration.
        let before = self
            .record_audit_snapshot(&mut transaction, record_id)
            .await?;
        let source_write =
            WriteContext::load(&mut transaction, self.workspace_id.0, &record).await?;
        if expected_updated_at.is_some()
            || Self::has_status_writes(&source_write, &input.values, &[])
        {
            Self::check_status_precondition(&source_write, &record, expected_updated_at)?;
        }
        let migration = sqlx::query_as::<_, (Uuid, i64, i64, String)>(
            "SELECT record_id, source_version, target_version, status FROM record_blueprint_migrations WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(input.migration_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::MigrationNotApplicable)?;
        if migration.0 != record.id || migration.1 != record.blueprint_version {
            return Err(RepositoryError::MigrationNotApplicable);
        }
        let target_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM blueprints WHERE id = $1 AND workspace_id = $2 AND status = 'published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(record.blueprint_id)
        .bind(self.workspace_id.0)
        .fetch_one(&mut *transaction)
        .await?;
        if target_version != input.expected_target_version || target_version != migration.2 {
            sqlx::query("UPDATE record_blueprint_migrations SET status = 'superseded', completed_at = now() WHERE id = $1 AND workspace_id = $2")
                .bind(input.migration_id)
                .bind(self.workspace_id.0)
                .execute(&mut *transaction)
                .await?;
            self.commit_mutation(transaction).await?;
            metrics::histogram!("catalog_record_migration_transaction_duration_seconds")
                .record(transaction_started.elapsed().as_secs_f64());
            return Err(RepositoryError::MigrationTargetChanged);
        }
        if migration.3 == "blocked" {
            return Err(RepositoryError::MigrationNotApplicable);
        }
        let source_values = self
            .current_blueprint_values_in_transaction(
                &mut transaction,
                record.id,
                record.blueprint_id,
                record.blueprint_version,
            )
            .await?;
        let source_attributes = self
            .list_attributes_in_transaction(
                &mut transaction,
                record.blueprint_id,
                record.blueprint_version,
            )
            .await?;
        let target_attributes = self
            .list_attributes_in_transaction(&mut transaction, record.blueprint_id, target_version)
            .await?;
        let source_by_id: HashMap<_, _> = source_attributes
            .iter()
            .map(|attribute| (attribute.id, attribute))
            .collect();
        let target_by_code: HashMap<_, _> = target_attributes
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect();
        let target_by_id: HashMap<_, _> = target_attributes
            .iter()
            .map(|attribute| (attribute.id, attribute))
            .collect();
        let default_context_id = self.resolve_context_id(&mut transaction, None).await?;

        // The migration form submits all populated target fields. Treat values
        // and relationship sets that are identical to compatible source rows as
        // confirmations, not replacements, so the normal HTTP path preserves
        // their row identity just like an empty automatic-migration payload.
        let mut effective_values = Vec::with_capacity(input.values.len());
        for supplied in std::mem::take(&mut input.values) {
            let (attribute_id, attribute_code, supplied_context_id) = match &supplied {
                NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code,
                    context_id,
                    ..
                }
                | NewAttributeValue::Relationship {
                    attribute_id,
                    attribute_code,
                    context_id,
                    ..
                } => (*attribute_id, attribute_code.as_deref(), *context_id),
            };
            let code = attribute_code.or_else(|| {
                attribute_id.and_then(|id| target_by_id.get(&id).map(|a| a.code.as_str()))
            });
            let context_id = self
                .resolve_context_id(&mut transaction, supplied_context_id)
                .await?;
            let unchanged = code.is_some_and(|code| {
                let target = target_by_code.get(code).copied();
                source_values.iter().any(|current| {
                    let Some(source) = source_by_id.get(&current.attribute_id).copied() else {
                        return false;
                    };
                    source.code == code
                        && current.context_id == context_id
                        && target.is_some_and(|target| {
                            migration_value_compatible(source, target, current, default_context_id)
                        })
                        && match &supplied {
                            NewAttributeValue::Scalar { value, .. } => {
                                current.relationship_target_record_id.is_none()
                                    && migration_scalar_values_equal(
                                        &source.value_type,
                                        &current.value,
                                        value,
                                    )
                            }
                            NewAttributeValue::Relationship {
                                target_record_id, ..
                            } => {
                                current.active
                                    && current.relationship_target_record_id
                                        == Some(*target_record_id)
                            }
                        }
                })
            });
            if !unchanged {
                effective_values.push(supplied);
            }
        }
        input.values = effective_values;

        let mut effective_relationships = Vec::with_capacity(input.relationships.len());
        for relationship in std::mem::take(&mut input.relationships) {
            let code = relationship.attribute_code.as_deref().or_else(|| {
                relationship
                    .attribute_id
                    .and_then(|id| target_by_id.get(&id).map(|a| a.code.as_str()))
            });
            let context_id = self
                .resolve_context_id(&mut transaction, relationship.context_id)
                .await?;
            let unchanged = code.is_some_and(|code| {
                let source = source_attributes.iter().find(|source| source.code == code);
                let target = target_by_code.get(code).copied();
                source.is_some_and(|source| {
                    target.is_some_and(|target| {
                        migration_contract_compatible(
                            source,
                            target,
                            context_id,
                            default_context_id,
                        )
                    })
                }) && source_values
                    .iter()
                    .filter(|current| {
                        current.active
                            && current.context_id == context_id
                            && source_by_id
                                .get(&current.attribute_id)
                                .is_some_and(|source| source.code == code)
                    })
                    .filter_map(|current| current.relationship_target_record_id)
                    .collect::<HashSet<_>>()
                    == relationship
                        .target_record_ids
                        .iter()
                        .copied()
                        .collect::<HashSet<_>>()
            });
            if !unchanged {
                effective_relationships.push(relationship);
            }
        }
        input.relationships = effective_relationships;

        let mut supplied_keys: HashSet<AttributeContextKey> = HashSet::new();
        let mut supplied_scalar_keys: HashSet<AttributeContextKey> = HashSet::new();
        let mut supplied_relationship_sets: HashSet<AttributeContextKey> = HashSet::new();
        let mut supplied_relationship_values = HashSet::new();
        for value in &input.values {
            let (attribute_id, attribute_code, context_id, relationship_target) = match value {
                NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code,
                    context_id,
                    ..
                } => (*attribute_id, attribute_code.as_deref(), *context_id, None),
                NewAttributeValue::Relationship {
                    attribute_id,
                    attribute_code,
                    context_id,
                    target_record_id,
                } => (
                    *attribute_id,
                    attribute_code.as_deref(),
                    *context_id,
                    Some(*target_record_id),
                ),
            };
            let code = attribute_code.or_else(|| {
                attribute_id.and_then(|id| target_by_id.get(&id).map(|a| a.code.as_str()))
            });
            if let Some(code) = code {
                let context_id = self
                    .resolve_context_id(&mut transaction, context_id)
                    .await?;
                supplied_keys.insert((code.to_owned(), context_id));
                if let Some(target) = relationship_target {
                    supplied_relationship_values.insert((code.to_owned(), context_id, target));
                } else {
                    supplied_scalar_keys.insert((code.to_owned(), context_id));
                }
            }
        }
        for relationship in &input.relationships {
            let code = relationship.attribute_code.as_deref().or_else(|| {
                relationship
                    .attribute_id
                    .and_then(|id| target_by_id.get(&id).map(|a| a.code.as_str()))
            });
            if let Some(code) = code {
                let context_id = self
                    .resolve_context_id(&mut transaction, relationship.context_id)
                    .await?;
                supplied_keys.insert((code.to_owned(), context_id));
                supplied_relationship_sets.insert((code.to_owned(), context_id));
            }
        }
        let discarded_attributes: HashSet<_> = input
            .discard_attributes
            .iter()
            .map(String::as_str)
            .collect();
        let mut archive_ids = Vec::new();
        let mut remap_ids: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut unresolved = HashSet::new();

        for value in source_values
            .iter()
            .filter(|value| source_by_id.contains_key(&value.attribute_id))
        {
            let source = source_by_id
                .get(&value.attribute_id)
                .expect("filtered source attribute exists");
            let target = target_by_code.get(source.code.as_str()).copied();
            let compatible = target.is_some_and(|target| {
                migration_value_compatible(source, target, value, default_context_id)
            });
            let discarded = discarded_attributes.contains(source.code.as_str());
            let key = (source.code.clone(), value.context_id);
            // A replacement in another context does not resolve this value;
            // silently archiving it would lose data in the current context.
            if !compatible && !discarded && !supplied_keys.contains(&key) {
                unresolved.insert(source.code.clone());
                continue;
            }
            let replaced = if source.value_type == "relationship" {
                supplied_relationship_sets.contains(&key)
                    || value.relationship_target_record_id.is_some_and(|target| {
                        supplied_relationship_values.contains(&(
                            source.code.clone(),
                            value.context_id,
                            target,
                        ))
                    })
            } else {
                supplied_scalar_keys.contains(&key)
            };
            if discarded || !compatible || replaced {
                archive_ids.push(value.id);
            } else {
                remap_ids
                    .entry(target.expect("compatible target exists").id)
                    .or_default()
                    .push(value.id);
            }
        }
        if !unresolved.is_empty() {
            let unresolved: Vec<_> = unresolved.into_iter().collect();
            sqlx::query(
                "UPDATE record_blueprint_migrations SET status = 'needs_input', input = $2 WHERE id = $1 AND workspace_id = $3",
            )
            .bind(input.migration_id)
            .bind(migration_input)
            .bind(self.workspace_id.0)
            .execute(&mut *transaction)
            .await?;
            self.commit_mutation(transaction).await?;
            metrics::histogram!("catalog_record_migration_transaction_duration_seconds")
                .record(transaction_started.elapsed().as_secs_f64());
            return Err(RepositoryError::MigrationNeedsResolution(unresolved));
        }
        let archived_count = self
            .archive_current_value_ids(&mut transaction, record.id, &archive_ids)
            .await?;
        let (remap_value_ids, remap_attribute_ids): (Vec<Uuid>, Vec<Uuid>) = remap_ids
            .into_iter()
            .flat_map(|(target_attribute_id, value_ids)| {
                value_ids
                    .into_iter()
                    .map(move |value_id| (value_id, target_attribute_id))
            })
            .unzip();
        let preserved_count = if remap_value_ids.is_empty() {
            0
        } else {
            sqlx::query(
                "UPDATE attribute_values av SET attribute_id = remap.attribute_id FROM unnest($1::uuid[], $2::uuid[]) AS remap(id, attribute_id) WHERE av.id = remap.id AND av.record_id = $3 AND av.workspace_id = $4",
            )
            .bind(&remap_value_ids)
            .bind(&remap_attribute_ids)
            .bind(record.id)
            .bind(self.workspace_id.0)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
        };
        let target_record = sqlx::query_as::<_, Db<Record>>(
            r#"UPDATE records SET blueprint_version = $2, updated_at = now()
               WHERE id = $1 AND workspace_id = $3
               RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at"#,
        )
        .bind(record.id)
        .bind(target_version)
        .bind(self.workspace_id.0)
        .fetch_one(&mut *transaction)
        .await?;
        let replaced_count = input.values.len() as u64
            + input
                .relationships
                .iter()
                .map(|relationship| {
                    relationship
                        .target_record_ids
                        .iter()
                        .copied()
                        .collect::<HashSet<_>>()
                        .len() as u64
                })
                .sum::<u64>();
        let write =
            WriteContext::load(&mut transaction, self.workspace_id.0, &target_record).await?;
        if expected_updated_at.is_none() && Self::has_status_writes(&write, &input.values, &[]) {
            return Err(RepositoryError::StatusPreconditionRequired);
        }
        self.insert_values_in(&mut transaction, &write, &target_record, input.values)
            .await?;
        self.replace_relationship_sets(
            &mut transaction,
            &write,
            &target_record,
            input.relationships,
        )
        .await?;
        // A migration is not an implicit escape hatch from the source status policy.
        self.validate_status_values(&mut transaction, &record)
            .await?;
        self.validate_record_schema_in(
            &mut transaction,
            &write,
            &target_record,
            Revalidation::Write,
        )
        .await?;
        let preview = write.preview(&mut transaction, record.id).await?;
        let target_record = self
            .store_preview(&mut transaction, record.id, preview)
            .await?;
        sqlx::query(
            "UPDATE record_blueprint_migrations SET status = 'migrated', input = $2, started_at = COALESCE(started_at, now()), completed_at = now() WHERE id = $1 AND workspace_id = $3",
        )
        .bind(input.migration_id)
        .bind(migration_input)
        .bind(self.workspace_id.0)
        .execute(&mut *transaction)
        .await?;
        let after = self
            .record_audit_snapshot(&mut transaction, record.id)
            .await?;
        let changes = Self::migration_audit_changes(record.id, before, after);
        let released_relationships = released_relationships(&changes);
        // A migration changes stored values like any record write: field-level
        // audit and publication reconciliation go through the shared seam.
        self.commit_record_mutation(
            transaction,
            changes,
            NewDomainEvent {
                event_type: RECORD_MIGRATED_V1.to_owned(),
                aggregate_kind: "record".to_owned(),
                aggregate_id: target_record.id,
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
                metadata: input
                    .removal_policy
                    .as_ref()
                    .map(|policy| serde_json::json!({"removal_policy": policy}))
                    .unwrap_or_else(|| serde_json::json!({})),
                payload: serde_json::to_value(RecordMigratedV1 {
                    record_id: target_record.id,
                    blueprint_id: target_record.blueprint_id,
                    source_version: record.blueprint_version,
                    target_version: target_record.blueprint_version,
                    migration_id: input.migration_id,
                    released_relationships,
                })
                .expect("record-migrated payload is serializable"),
            },
        )
        .await?;
        metrics::counter!("catalog_record_migration_values_total", "action" => "preserved")
            .increment(preserved_count);
        metrics::counter!("catalog_record_migration_values_total", "action" => "archived")
            .increment(archived_count);
        metrics::counter!("catalog_record_migration_values_total", "action" => "replaced")
            .increment(replaced_count);
        metrics::histogram!("catalog_record_migration_transaction_duration_seconds")
            .record(transaction_started.elapsed().as_secs_f64());
        Ok(target_record)
    }

    async fn archive_current_value_ids(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        record_id: Uuid,
        value_ids: &[Uuid],
    ) -> Result<u64, RepositoryError> {
        if value_ids.is_empty() {
            return Ok(0);
        }
        let (archived, _copied_references) = sqlx::query_as::<_, (i64, i64)>(
            r#"WITH file_references AS MATERIALIZED (
                    SELECT r.attribute_value_id, r.workspace_id, r.file_id, r.position
                    FROM attribute_file_references r
                    JOIN attribute_values av ON av.id = r.attribute_value_id
                    WHERE av.record_id = $1 AND av.workspace_id = $2 AND av.id = ANY($3)
                ), archived AS (
                    DELETE FROM attribute_values
                    WHERE record_id = $1 AND workspace_id = $2 AND id = ANY($3)
                    RETURNING *
                ), stored AS (
                    INSERT INTO attribute_value_history (
                        id, workspace_id, record_id, attribute_id, context_id, relationship_target_record_id, active,
                        value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                        value_time, value_time_zone, value_json, created_at
                    )
                    SELECT id, workspace_id, record_id, attribute_id, context_id, relationship_target_record_id, active,
                           value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                           value_time, value_time_zone, value_json, created_at
                    FROM archived
                    RETURNING id, workspace_id, archived_at
                ), copied_references AS (
                    INSERT INTO attribute_file_reference_history (
                        attribute_value_history_id, attribute_value_history_archived_at, workspace_id, file_id, position
                    )
                    SELECT stored.id, stored.archived_at, file_references.workspace_id,
                           file_references.file_id, file_references.position
                    FROM file_references JOIN stored
                      ON stored.id = file_references.attribute_value_id
                     AND stored.workspace_id = file_references.workspace_id
                    RETURNING 1
                )
                SELECT (SELECT count(*) FROM stored), (SELECT count(*) FROM copied_references)"#,
        )
        .bind(record_id)
        .bind(self.workspace_id.0)
        .bind(value_ids)
        .fetch_one(&mut **transaction)
        .await?;
        Ok(archived as u64)
    }
}

fn migration_contract_compatible(
    source: &Attribute,
    target: &Attribute,
    context_id: Option<Uuid>,
    default_context_id: Option<Uuid>,
) -> bool {
    if target.value_type != source.value_type
        || target.context_editable == "default" && context_id != default_context_id
    {
        return false;
    }
    match source.value_type.as_str() {
        "relationship" => {
            target.target_blueprint_codes == source.target_blueprint_codes
                && target.cardinality == source.cardinality
                && target.target_cardinality == source.target_cardinality
        }
        "file" => {
            target.file_policy == source.file_policy && target.cardinality == source.cardinality
        }
        _ => true,
    }
}

fn migration_value_compatible(
    source: &Attribute,
    target: &Attribute,
    value: &AttributeValue,
    default_context_id: Option<Uuid>,
) -> bool {
    migration_contract_compatible(source, target, value.context_id, default_context_id)
        && (source.value_type == "relationship"
            || source.value_type == "file"
            || target.value_schema.as_ref().is_none_or(|schema| {
                validate_json_schema(schema, &value.value)
                    .expect("compiled blueprint schema is valid")
                    .is_empty()
            }))
}

fn migration_scalar_values_equal(value_type: &str, current: &Value, supplied: &Value) -> bool {
    current == supplied
        || super::values::ValueType::parse(value_type)
            .and_then(|value_type| super::values::NativeValue::parse(value_type, supplied.clone()))
            .is_ok_and(|value| value.json() == *current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn removal(attribute_code: &str) -> AuditEventChange {
        AuditEventChange {
            record_id: Uuid::nil(),
            attribute_id: Uuid::nil(),
            attribute_code: attribute_code.to_owned(),
            context_id: None,
            context_code: None,
            relationship_target_record_id: Some(Uuid::new_v4()),
            change_kind: "relationship_remove",
            before_value: None,
            after_value: None,
        }
    }

    #[test]
    fn released_targets_are_capped_per_relationship_and_per_event() {
        let changes: Vec<_> = (0..17)
            .flat_map(|attribute| {
                let code = format!("rel_{attribute:02}");
                (0..150).map(move |_| removal(&code))
            })
            .collect();
        let released = released_relationships(&changes);
        assert!(
            released
                .iter()
                .all(|entry| entry.target_record_ids.len() <= MAX_RELEASED_RELATIONSHIP_TARGETS)
        );
        let total: usize = released
            .iter()
            .map(|entry| entry.target_record_ids.len())
            .sum();
        assert_eq!(total, MAX_RELEASED_TARGETS_PER_EVENT);
        assert_eq!(released.len(), 10);
    }

    #[test]
    fn released_relationships_stay_within_the_byte_budget() {
        let long = "r".repeat(120);
        let changes: Vec<_> = (0..1_000)
            .map(|attribute| removal(&format!("{long}{attribute:04}")))
            .collect();
        let released = released_relationships(&changes);
        let bytes = serde_json::to_vec(&released).unwrap().len();
        assert!(bytes <= MAX_RELEASED_RELATIONSHIPS_BYTES, "{bytes} bytes");
        assert!(!released.is_empty());
    }

    #[test]
    fn the_per_event_cap_can_end_partway_through_a_relationship() {
        let changes: Vec<_> = (0..11)
            .flat_map(|attribute| {
                let code = format!("rel_{attribute:02}");
                let count = if attribute == 10 { 100 } else { 95 };
                (0..count).map(move |_| removal(&code))
            })
            .collect();
        let released = released_relationships(&changes);
        assert_eq!(released.len(), 11);
        assert_eq!(released[10].target_record_ids.len(), 50);
        let total: usize = released
            .iter()
            .map(|entry| entry.target_record_ids.len())
            .sum();
        assert_eq!(total, MAX_RELEASED_TARGETS_PER_EVENT);
    }
}
