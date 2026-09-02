use super::entity_search::empty_projections;
use super::values::{NativeValue, ValueType};
use super::*;
use catalog_validation::validate_json_schema;
use chrono::Utc;
use serde_json::{Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
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
        let entity = self
            .insert_entity(
                &mut transaction,
                blueprint_id,
                blueprint_version,
                system_tags,
                system_metadata,
            )
            .await?;
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        self.validate_entity_schema(&mut transaction, &entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        self.commit_mutation(transaction).await?;
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
        self.commit_mutation(transaction).await?;
        Ok(entity)
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
        self.commit_mutation(transaction).await?;
        Ok(())
    }

    pub async fn append_values(
        &self,
        entity_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
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

        self.commit_mutation(transaction).await?;
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
            // Relationship writes are set operations; collapsing duplicate IDs
            // makes a retried or malformed client payload idempotent.
            let targets: HashSet<_> = relationship.target_entity_ids.into_iter().collect();
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
        self.commit_mutation(transaction).await?;
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
            let targets: HashSet<_> = relationship.target_entity_ids.into_iter().collect();
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
        let (attribute_id, value_type, value_schema, target_blueprint_code, context_editable) =
            match (attribute_id, attribute_code.as_deref()) {
                (Some(attribute_id), None) => {
                    sqlx::query_as::<_, (Uuid, String, Option<Value>, Option<String>, String)>(
                        r#"SELECT id, value_type, value_schema, target_blueprint_code, context_editable
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
                    validate_code(&attribute_code)?;
                    sqlx::query_as::<_, (Uuid, String, Option<Value>, Option<String>, String)>(
                        r#"SELECT id, value_type, value_schema, target_blueprint_code, context_editable
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
        }

        let native = if is_relationship {
            None
        } else {
            Some(NativeValue::parse(ValueType::parse(&value_type)?, payload)?)
        };
        if let (Some(schema), Some(native)) = (&value_schema, &native) {
            if let Some(error) = validate_json_schema(schema, &native.json())
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
    ) -> Result<HashSet<Uuid>, RepositoryError> {
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

    async fn insert_relationship_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        target_entity_id: Uuid,
        active: bool,
    ) -> Result<AttributeValue, RepositoryError> {
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
