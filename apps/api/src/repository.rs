use std::collections::HashSet;

use async_recursion::async_recursion;
use serde_json::{Map, Value};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, Blueprint,
        BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint, CreateEntity, Entity,
        EntityPreview, EntityPreviewPage, NewAttributeValue, RelationshipMutation,
        RelationshipTargets,
    },
    projection::PreviewProjectionBuilder,
};

#[derive(Clone)]
pub struct CatalogRepository {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("the context code 'default' is reserved")]
    ReservedContextCode,
    #[error("attribute does not belong to the entity blueprint version")]
    AttributeNotApplicable,
    #[error("provide exactly one of attribute_id or attribute_code")]
    InvalidAttributeSelector,
    #[error("attribute kind does not match the supplied value")]
    AttributeKindMismatch,
    #[error("relationship target does not match the attribute target blueprint")]
    RelationshipTargetTypeMismatch,
    #[error("projections must be a JSON object")]
    InvalidProjections,
    #[error("entity preview must be a JSON object organized by context")]
    InvalidPreview,
    #[error("invalid blueprint definition: {0}")]
    InvalidBlueprintDefinition(String),
    #[error("blueprint code is already owned by another blueprint")]
    BlueprintCodeTaken,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    pub(crate) fn invalid_blueprint_definition(error: impl std::fmt::Display) -> Self {
        Self::InvalidBlueprintDefinition(error.to_string())
    }
}

#[derive(sqlx::FromRow)]
struct LatestBlueprint {
    version: i64,
    code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PreviewRelationship {
    attribute_code: String,
    context_code: Option<String>,
    target_id: Uuid,
    target_code: String,
    target_projections: Value,
    relationship_position: i64,
}

impl CatalogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_blueprint(
        &self,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let compiled = compile_definition(&mut transaction, &input.definition).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(&compiled.code)
            .execute(&mut *transaction)
            .await?;
        let code_exists =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM blueprints WHERE code = $1 LIMIT 1")
                .bind(&compiled.code)
                .fetch_optional(&mut *transaction)
                .await?
                .is_some();
        if code_exists {
            return Err(RepositoryError::BlueprintCodeTaken);
        }

        let result = self
            .insert_blueprint_revision_in_transaction(
                &mut transaction,
                Uuid::new_v4(),
                1,
                input.definition,
                compiled,
            )
            .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn create_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let latest = sqlx::query_as::<_, LatestBlueprint>(
            "SELECT version, code FROM blueprints WHERE id = $1 ORDER BY version DESC LIMIT 1 FOR UPDATE",
        )
        .bind(blueprint_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint"))?;

        let compiled = compile_definition(&mut transaction, &input.definition).await?;
        if latest.code.as_deref() != Some(compiled.code.as_str()) {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "blueprint code cannot change across revisions".to_owned(),
            ));
        }

        let result = self
            .insert_blueprint_revision_in_transaction(
                &mut transaction,
                blueprint_id,
                latest.version + 1,
                input.definition,
                compiled,
            )
            .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn insert_blueprint_revision_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        version: i64,
        definition: String,
        compiled: catalog_blueprint::CompiledBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let includes = serde_json::to_value(&compiled.includes).map_err(|error| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "could not serialize generated includes: {error}"
            ))
        })?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"INSERT INTO blueprints (id, code, name, kind, version, includes, definition, definition_hash)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, code, name, kind, version, includes, created_at, updated_at, deleted_at, definition, definition_hash"#,
        )
        .bind(blueprint_id)
        .bind(compiled.code)
        .bind(compiled.name)
        .bind(compiled.kind.as_str())
        .bind(version)
        .bind(includes)
        .bind(definition)
        .bind(compiled.raw_definition_hash)
        .fetch_one(&mut **transaction)
        .await?;

        let mut attributes = Vec::with_capacity(compiled.attributes.len());
        for attribute in compiled.attributes {
            attributes.push(
                sqlx::query_as::<_, Attribute>(
                    r#"INSERT INTO attributes (id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, position)
                       VALUES ($1, $2, $3, $4, $5, $6, $7)
                       RETURNING id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, position, created_at, updated_at, deleted_at"#,
                )
                .bind(Uuid::new_v4())
                .bind(blueprint_id)
                .bind(version)
                .bind(attribute.code)
                .bind(attribute.value_type)
                .bind(attribute.target_blueprint)
                .bind(attribute.position)
                .fetch_one(&mut **transaction)
                .await?,
            );
        }

        Ok(BlueprintWithAttributes {
            blueprint,
            attributes,
        })
    }

    pub async fn get_current_blueprint(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND deleted_at IS NULL
               ORDER BY version DESC
               LIMIT 1"#,
        )
        .bind(blueprint_id)
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        version: i64,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND version = $2 AND deleted_at IS NULL"#,
        )
        .bind(blueprint_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_by_code(
        &self,
        code: &str,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND deleted_at IS NULL
               ORDER BY version DESC
               LIMIT 1"#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_by_code_and_version(
        &self,
        code: &str,
        version: i64,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND version = $2 AND deleted_at IS NULL"#,
        )
        .bind(code)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn list_attributes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Attribute>(
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, position, created_at, updated_at, deleted_at
               FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND deleted_at IS NULL
               ORDER BY position"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn with_attributes(
        &self,
        blueprint: Option<Blueprint>,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        match blueprint {
            Some(blueprint) => {
                let attributes = self
                    .list_attributes(blueprint.id, blueprint.version)
                    .await?;
                Ok(Some(BlueprintWithAttributes {
                    blueprint,
                    attributes,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn create_context(
        &self,
        input: CreateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if input.code == "default" {
            return Err(RepositoryError::ReservedContextCode);
        }

        Ok(sqlx::query_as::<_, AttributeContext>(
            r#"INSERT INTO attribute_contexts (id, code, data)
               VALUES ($1, $2, $3)
               RETURNING id, code, data"#,
        )
        .bind(Uuid::new_v4())
        .bind(input.code)
        .bind(input.data)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_context_by_code(
        &self,
        code: &str,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        Ok(sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data FROM attribute_contexts WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_entity(&self, input: CreateEntity) -> Result<Entity, RepositoryError> {
        let mut projections = input.projections.unwrap_or_else(empty_projections);
        let projections = projections
            .as_object_mut()
            .ok_or(RepositoryError::InvalidProjections)?;
        projections
            .entry("preview".to_owned())
            .or_insert_with(empty_preview);

        sqlx::query_as::<_, Entity>(
            r#"INSERT INTO entities (id, code, blueprint_id, blueprint_version, projections)
               SELECT $1, $2, b.id, b.version, $3
               FROM blueprints b
               WHERE b.id = $4
                 AND b.version = $5
                 AND b.kind = 'entity'
                 AND b.deleted_at IS NULL
               RETURNING id, code, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(input.code)
        .bind(Value::Object(projections.clone()))
        .bind(input.blueprint_id)
        .bind(input.blueprint_version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"SELECT id, code, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_entity_by_code(
        &self,
        blueprint_id: Uuid,
        code: &str,
    ) -> Result<Option<Entity>, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"SELECT id, code, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities
               WHERE blueprint_id = $1 AND code = $2 AND deleted_at IS NULL"#,
        )
        .bind(blueprint_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn current_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"SELECT id, entity_id, attribute_id, value, relationship_target_entity_id, context_id, active, created_at
               FROM (
                   SELECT DISTINCT ON (entity_id, attribute_id, context_id, relationship_target_entity_id)
                       id, entity_id, attribute_id, value, relationship_target_entity_id, context_id, active, created_at
                   FROM attribute_values
                   WHERE entity_id = $1
                   ORDER BY entity_id, attribute_id, context_id, relationship_target_entity_id, created_at DESC, id DESC
               ) current
               WHERE relationship_target_entity_id IS NULL OR active"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn preview(
        &self,
        entity_id: Uuid,
        relationship_depth: u8,
        relationship_limit: i64,
    ) -> Result<Option<Value>, RepositoryError> {
        let entity = self.get_entity(entity_id).await?;
        let Some(entity) = entity else {
            return Ok(None);
        };
        self.build_preview(
            entity.id,
            &entity.projections,
            relationship_depth,
            relationship_limit,
            &mut HashSet::new(),
        )
        .await
        .map(Some)
    }

    #[async_recursion]
    async fn build_preview(
        &self,
        entity_id: Uuid,
        projections: &Value,
        relationship_depth: u8,
        relationship_limit: i64,
        path: &mut HashSet<Uuid>,
    ) -> Result<Value, RepositoryError> {
        let mut preview = projections
            .get("preview")
            .cloned()
            .ok_or(RepositoryError::InvalidPreview)?;
        if !path.insert(entity_id) {
            return Ok(preview);
        }

        if !preview.is_object() {
            return Err(RepositoryError::InvalidPreview);
        }
        let relationships = sqlx::query_as::<_, PreviewRelationship>(
            r#"WITH current AS (
                   SELECT DISTINCT ON (entity_id, attribute_id, context_id, relationship_target_entity_id)
                        attribute_id, context_id, relationship_target_entity_id, active
                   FROM attribute_values
                   WHERE entity_id = $1 AND relationship_target_entity_id IS NOT NULL
                   ORDER BY entity_id, attribute_id, context_id, relationship_target_entity_id, created_at DESC, id DESC
               ), relationships AS (
                   SELECT a.code AS attribute_code, c.code AS context_code, target.id AS target_id,
                          target.code AS target_code, target.projections AS target_projections,
                          ROW_NUMBER() OVER (PARTITION BY a.id, current.context_id ORDER BY target.id)
                              AS relationship_position
                   FROM current
                   JOIN attributes a ON a.id = current.attribute_id AND a.deleted_at IS NULL
                   JOIN entities target ON target.id = current.relationship_target_entity_id AND target.deleted_at IS NULL
                   LEFT JOIN attribute_contexts c ON c.id = current.context_id
                   WHERE current.active
               )
               SELECT attribute_code, context_code, target_id, target_code, target_projections, relationship_position
               FROM relationships
               WHERE relationship_position <= $2
               ORDER BY attribute_code, context_code NULLS FIRST, relationship_position"#,
        )
        .bind(entity_id)
        .bind(if relationship_depth == 0 { 1 } else { relationship_limit + 1 })
        .fetch_all(&self.pool)
        .await?;

        for relationship in relationships {
            let context_code = relationship
                .context_code
                .unwrap_or_else(|| "default".to_owned());
            let context = preview
                .as_object_mut()
                .expect("preview was validated as an object")
                .entry(context_code.clone())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            let relationship_preview = context
                .entry(relationship.attribute_code)
                .or_insert_with(|| serde_json::json!({ "items": [], "truncated": false }))
                .as_object_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            if relationship_depth == 0 || relationship.relationship_position > relationship_limit {
                relationship_preview.insert("truncated".to_owned(), Value::Bool(true));
                continue;
            }

            let target_preview = self
                .build_preview(
                    relationship.target_id,
                    &relationship.target_projections,
                    relationship_depth - 1,
                    relationship_limit,
                    path,
                )
                .await?;
            let mut target_values = target_preview
                .get(&context_code)
                .or_else(|| target_preview.get("default"))
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));
            let target_values = target_values
                .as_object_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            target_values.insert(
                "id".to_owned(),
                Value::String(relationship.target_id.to_string()),
            );
            target_values.insert("code".to_owned(), Value::String(relationship.target_code));

            let targets = relationship_preview
                .entry("items".to_owned())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .ok_or(RepositoryError::InvalidPreview)?;
            targets.push(Value::Object(target_values.clone()));
        }
        path.remove(&entity_id);
        Ok(preview)
    }

    pub async fn list_previews(
        &self,
        blueprint_code: &str,
        related_from: Uuid,
        relationship: &str,
        limit: i64,
        cursor: Option<Uuid>,
    ) -> Result<EntityPreviewPage, RepositoryError> {
        let mut items = sqlx::query_as::<_, EntityPreview>(
            r#"SELECT target.id, target.code, target.projections -> 'preview' AS preview
               FROM (
                   SELECT DISTINCT ON (av.relationship_target_entity_id)
                       av.relationship_target_entity_id, av.active
                   FROM attribute_values av
                   JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                   WHERE av.entity_id = $1
                     AND a.code = $2
                     AND av.relationship_target_entity_id IS NOT NULL
                     AND ($3::uuid IS NULL OR av.relationship_target_entity_id > $3)
                   ORDER BY av.relationship_target_entity_id, av.created_at DESC, av.id DESC
               ) current
               JOIN entities target ON target.id = current.relationship_target_entity_id
               JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
               WHERE current.active AND target.deleted_at IS NULL AND b.code = $4
               ORDER BY target.id
               LIMIT $5"#,
        )
        .bind(related_from)
        .bind(relationship)
        .bind(cursor)
        .bind(blueprint_code)
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|item| item.id)
        } else {
            None
        };
        Ok(EntityPreviewPage { items, next_cursor })
    }

    pub async fn append_values(
        &self,
        entity_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = sqlx::query_as::<_, Entity>(
            r#"SELECT id, code, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND deleted_at IS NULL
               FOR UPDATE"#,
        )
        .bind(entity_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("entity"))?;

        let mut values = Vec::with_capacity(input.values.len());
        for value in input.values {
            values.push(self.insert_value(&mut transaction, &entity, value).await?);
        }

        let preview = PreviewProjectionBuilder::build(&mut transaction, entity_id).await?;
        sqlx::query(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1"#,
        )
        .bind(entity_id)
        .bind(preview)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
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
            let (attribute_id, target_blueprint_code) = self
                .relationship_attribute(&mut transaction, &entity, &relationship)
                .await?;
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
                .current_relationship_targets(
                    &mut transaction,
                    entity.id,
                    attribute_id,
                    relationship.context_id,
                )
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
                        relationship.context_id,
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
                            relationship.context_id,
                            *target_id,
                            true,
                        )
                        .await?,
                    );
                }
            }
        }
        self.touch_entity(&mut transaction, entity_id).await?;
        transaction.commit().await?;
        Ok(values)
    }

    async fn insert_value(
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

        let (attribute_id, value_type, target_blueprint_code) =
            match (attribute_id, attribute_code) {
                (Some(attribute_id), None) => {
                    sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                        r#"SELECT id, value_type, target_blueprint_code
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
                    sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                        r#"SELECT id, value_type, target_blueprint_code
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

        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (
                    id, entity_id, attribute_id, context_id, value, relationship_target_entity_id, active
                ) VALUES ($1, $2, $3, $4, $5, $6, true)
                RETURNING id, entity_id, attribute_id, value, relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(entity.id)
        .bind(attribute_id)
        .bind(context_id)
        .bind(payload)
        .bind(target_entity_id)
        .fetch_one(&mut **transaction)
        .await?)
    }

    async fn lock_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Entity>(
            r#"SELECT id, code, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities WHERE id = $1 AND deleted_at IS NULL FOR UPDATE"#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("entity"))
    }

    async fn relationship_attribute(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        relationship: &RelationshipTargets,
    ) -> Result<(Uuid, Option<String>), RepositoryError> {
        let attribute = match (relationship.attribute_id, relationship.attribute_code.as_deref()) {
            (Some(id), None) => sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                "SELECT id, value_type, target_blueprint_code FROM attributes WHERE id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL",
            ).bind(id).bind(entity.blueprint_id).bind(entity.blueprint_version).fetch_optional(&mut **transaction).await?,
            (None, Some(code)) => sqlx::query_as::<_, (Uuid, String, Option<String>)>(
                "SELECT id, value_type, target_blueprint_code FROM attributes WHERE code = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL",
            ).bind(code).bind(entity.blueprint_id).bind(entity.blueprint_version).fetch_optional(&mut **transaction).await?,
            _ => return Err(RepositoryError::InvalidAttributeSelector),
        }.ok_or(RepositoryError::AttributeNotApplicable)?;
        if attribute.1 != "relationship" {
            return Err(RepositoryError::AttributeKindMismatch);
        }
        Ok((attribute.0, attribute.2))
    }

    async fn validate_relationship_target(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        target_entity_id: Uuid,
        target_blueprint_code: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let target = sqlx::query_scalar::<_, String>(
            r#"SELECT b.code FROM entities e JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
               WHERE e.id = $1 AND e.deleted_at IS NULL"#,
        )
        .bind(target_entity_id)
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
            r#"SELECT relationship_target_entity_id FROM (
                   SELECT DISTINCT ON (relationship_target_entity_id) relationship_target_entity_id, active
                   FROM attribute_values
                   WHERE entity_id = $1 AND attribute_id = $2 AND context_id IS NOT DISTINCT FROM $3
                     AND relationship_target_entity_id IS NOT NULL
                   ORDER BY relationship_target_entity_id, created_at DESC, id DESC
               ) current WHERE active"#,
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
        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (id, entity_id, attribute_id, context_id, value, relationship_target_entity_id, active)
               VALUES ($1, $2, $3, $4, 'null'::jsonb, $5, $6)
               RETURNING id, entity_id, attribute_id, value, relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4()).bind(entity_id).bind(attribute_id).bind(context_id)
        .bind(target_entity_id).bind(active).fetch_one(&mut **transaction).await?)
    }

    async fn touch_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE entities SET updated_at = now() WHERE id = $1")
            .bind(entity_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }
}

fn empty_preview() -> Value {
    Value::Object(Map::from_iter([(
        String::from("default"),
        Value::Object(Map::new()),
    )]))
}

fn empty_projections() -> Value {
    Value::Object(Map::from_iter([(String::from("preview"), empty_preview())]))
}
