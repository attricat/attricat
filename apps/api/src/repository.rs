use serde_json::{Map, Value};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, Blueprint,
        BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint, CreateEntity, Entity,
        NewAttributeValue,
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
    #[error("attribute kind does not match the supplied value")]
    AttributeKindMismatch,
    #[error("projections must be a JSON object")]
    InvalidProjections,
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
                    r#"INSERT INTO attributes (id, blueprint_id, blueprint_version, code, value_type, position)
                       VALUES ($1, $2, $3, $4, $5, $6)
                       RETURNING id, blueprint_id, blueprint_version, code, value_type, position, created_at, updated_at, deleted_at"#,
                )
                .bind(Uuid::new_v4())
                .bind(blueprint_id)
                .bind(version)
                .bind(attribute.code)
                .bind(attribute.value_type)
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

    pub async fn list_attributes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Attribute>(
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, position, created_at, updated_at, deleted_at
               FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND deleted_at IS NULL
               ORDER BY position"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&self.pool)
        .await?)
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

    pub async fn create_entity(&self, input: CreateEntity) -> Result<Entity, RepositoryError> {
        let mut projections = input.projections.unwrap_or_else(empty_projections);
        let projections = projections
            .as_object_mut()
            .ok_or(RepositoryError::InvalidProjections)?;
        projections
            .entry("preview".to_owned())
            .or_insert_with(empty_preview);

        Ok(sqlx::query_as::<_, Entity>(
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
        .ok_or(RepositoryError::NotFound("blueprint version"))?)
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

    async fn insert_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        value: NewAttributeValue,
    ) -> Result<AttributeValue, RepositoryError> {
        let (attribute_id, context_id, payload, target_entity_id, is_relationship) = match value {
            NewAttributeValue::Scalar {
                attribute_id,
                context_id,
                value,
            } => (attribute_id, context_id, value, None, false),
            NewAttributeValue::Relationship {
                attribute_id,
                context_id,
                target_entity_id,
            } => (
                attribute_id,
                context_id,
                Value::Null,
                Some(target_entity_id),
                true,
            ),
        };

        let value_type = sqlx::query_scalar::<_, String>(
            r#"SELECT value_type
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
        .ok_or(RepositoryError::AttributeNotApplicable)?;

        if (value_type == "relationship") != is_relationship {
            return Err(RepositoryError::AttributeKindMismatch);
        }

        if let Some(target_entity_id) = target_entity_id {
            let target_exists = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM entities WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(target_entity_id)
            .fetch_optional(&mut **transaction)
            .await?
            .is_some();
            if !target_exists {
                return Err(RepositoryError::NotFound("relationship target entity"));
            }
        }

        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (
                    id, entity_id, attribute_id, context_id, value, relationship_target_entity_id
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, entity_id, attribute_id, value, relationship_target_entity_id, context_id, created_at"#,
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
