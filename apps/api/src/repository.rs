use std::collections::HashSet;

use async_recursion::async_recursion;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use catalog_validation::is_valid_code;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, AttributeValueSelector,
        Blueprint, BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint, CreateEntity,
        Entity, EntityPreview, EntityPreviewPage, FormAttributeValue, NewAttributeValue,
        RelationshipMutation, RelationshipTargets, ResolvedEntityPreviewResponse,
        UpdateAttributeContext,
    },
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
    #[error("code must contain only ASCII letters, numbers, hyphens, and underscores")]
    InvalidCode,
    #[error("context data must be a JSON object")]
    InvalidContextData,
    #[error("context was not found")]
    InvalidContext,
    #[error("the default context cannot be changed or deleted")]
    DefaultContextProtected,
    #[error("a context cannot be its own descendant")]
    ContextCycle,
    #[error("a context with descendants or active values cannot be deleted")]
    ContextInUse,
    #[error("attribute can only be edited in the default context")]
    DefaultContextOnly,
    #[error("attribute does not belong to the entity blueprint version")]
    AttributeNotApplicable,
    #[error("provide exactly one of attribute_id or attribute_code")]
    InvalidAttributeSelector,
    #[error("attribute kind does not match the supplied value")]
    AttributeKindMismatch,
    #[error("value does not match the attribute type")]
    AttributeValueTypeMismatch,
    #[error("stored attribute value does not match its attribute type")]
    InvalidStoredAttributeValue,
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
    target_projections: Value,
    target_display: Value,
    target_context_fallback: Value,
    relationship_position: i64,
}

#[derive(sqlx::FromRow)]
struct EntityPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_display: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct FormNativeValueRow {
    attribute_code: String,
    context_id: Option<Uuid>,
    relationship_target_entity_id: Option<Uuid>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

#[derive(sqlx::FromRow)]
struct CurrentNativeValueRow {
    id: Uuid,
    entity_id: Uuid,
    attribute_id: Uuid,
    relationship_target_entity_id: Option<Uuid>,
    active: bool,
    context_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

#[derive(sqlx::FromRow)]
struct ProjectionNativeValueRow {
    attribute_code: String,
    context_code: String,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

enum NativeValue {
    Text(String),
    Number(Decimal),
    Integer(i64),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
    Time(NaiveTime, String),
}

#[derive(Clone, Copy)]
enum ValueType {
    String,
    Number,
    Integer,
    Boolean,
    Date,
    Datetime,
    Time,
}

#[derive(sqlx::FromRow)]
pub(crate) struct NativeValueRow {
    pub(crate) value_type: String,
    pub(crate) value_text: Option<String>,
    pub(crate) value_number: Option<Decimal>,
    pub(crate) value_integer: Option<i64>,
    pub(crate) value_boolean: Option<bool>,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) value_datetime: Option<DateTime<Utc>>,
    pub(crate) value_time: Option<NaiveTime>,
    pub(crate) value_time_zone: Option<String>,
}

impl ValueType {
    fn parse(value_type: &str) -> Result<Self, RepositoryError> {
        match value_type {
            "string" => Ok(Self::String),
            "number" => Ok(Self::Number),
            "integer" => Ok(Self::Integer),
            "boolean" => Ok(Self::Boolean),
            "date" => Ok(Self::Date),
            "datetime" => Ok(Self::Datetime),
            "time" => Ok(Self::Time),
            _ => Err(RepositoryError::AttributeValueTypeMismatch),
        }
    }
}

impl NativeValue {
    fn parse(value_type: ValueType, value: Value) -> Result<Self, RepositoryError> {
        let invalid = || RepositoryError::AttributeValueTypeMismatch;
        match value_type {
            ValueType::String => value
                .as_str()
                .map(|value| Self::Text(value.to_owned()))
                .ok_or_else(invalid),
            ValueType::Number => value
                .as_number()
                .and_then(|value| value.to_string().parse().ok())
                .map(Self::Number)
                .ok_or_else(invalid),
            ValueType::Integer => value.as_i64().map(Self::Integer).ok_or_else(invalid),
            ValueType::Boolean => value.as_bool().map(Self::Boolean).ok_or_else(invalid),
            ValueType::Date => value
                .as_str()
                .and_then(|value| value.parse().ok())
                .map(Self::Date)
                .ok_or_else(invalid),
            ValueType::Datetime => value
                .as_str()
                .and_then(|value| value.parse().ok())
                .map(Self::Datetime)
                .ok_or_else(invalid),
            ValueType::Time => {
                let time = value
                    .get("time")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(invalid)?;
                let time_zone = value
                    .get("time_zone")
                    .and_then(Value::as_str)
                    .filter(|value| value.parse::<Tz>().is_ok())
                    .ok_or_else(invalid)?;
                Ok(Self::Time(time, time_zone.to_owned()))
            }
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::Text(value) => Value::String(value.clone()),
            Self::Number(value) => {
                Value::Number(value.to_string().parse().expect("decimal is a JSON number"))
            }
            Self::Integer(value) => Value::from(*value),
            Self::Boolean(value) => Value::Bool(*value),
            Self::Date(value) => Value::String(value.to_string()),
            Self::Datetime(value) => Value::String(value.to_rfc3339()),
            Self::Time(time, time_zone) => {
                serde_json::json!({ "time": time.to_string(), "time_zone": time_zone })
            }
        }
    }

    fn bind<'q, O>(
        self,
        query: sqlx::query::QueryAs<'q, Postgres, O, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, Postgres, O, sqlx::postgres::PgArguments>
    where
        O: Send + Unpin,
    {
        match self {
            Self::Text(value) => query
                .bind(Some(value))
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Number(value) => query
                .bind(Option::<String>::None)
                .bind(Some(value))
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Integer(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Some(value))
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Boolean(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Some(value))
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Date(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Some(value))
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Datetime(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Some(value))
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None),
            Self::Time(time, time_zone) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Some(time))
                .bind(Some(time_zone)),
        }
    }
}

pub(crate) fn native_value_json(row: NativeValueRow) -> Result<Value, RepositoryError> {
    let value_type = ValueType::parse(&row.value_type)
        .map_err(|_| RepositoryError::InvalidStoredAttributeValue)?;
    let populated = [
        row.value_text.is_some(),
        row.value_number.is_some(),
        row.value_integer.is_some(),
        row.value_boolean.is_some(),
        row.value_date.is_some(),
        row.value_datetime.is_some(),
        row.value_time.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();
    let value = match value_type {
        ValueType::String => row.value_text.map(NativeValue::Text),
        ValueType::Number => row.value_number.map(NativeValue::Number),
        ValueType::Integer => row.value_integer.map(NativeValue::Integer),
        ValueType::Boolean => row.value_boolean.map(NativeValue::Boolean),
        ValueType::Date => row.value_date.map(NativeValue::Date),
        ValueType::Datetime => row.value_datetime.map(NativeValue::Datetime),
        ValueType::Time => row
            .value_time
            .zip(row.value_time_zone)
            .filter(|(_, zone)| zone.parse::<Tz>().is_ok())
            .map(|(time, zone)| NativeValue::Time(time, zone)),
    }
    .ok_or(RepositoryError::InvalidStoredAttributeValue)?;
    if populated != 1 {
        return Err(RepositoryError::InvalidStoredAttributeValue);
    }
    Ok(value.json())
}

impl CatalogRepository {
    const DEFAULT_CONTEXT_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_entity_blueprints(&self) -> Result<Vec<Blueprint>, RepositoryError> {
        Ok(sqlx::query_as::<_, Blueprint>(
            r#"SELECT DISTINCT ON (id)
                    id, code, name, kind, version, display, views, includes, created_at, updated_at,
                    deleted_at, definition, definition_hash
               FROM blueprints
               WHERE kind = 'entity' AND deleted_at IS NULL
               ORDER BY id, version DESC"#,
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_blueprint(
        &self,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let compiled = compile_definition(&mut transaction, &input.definition).await?;
        // PostgreSQL cannot express uniqueness across all revisions with the
        // versioned primary key. Serialize writers for this code so the
        // existence check and first-revision insert are one logical operation.
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
        let display = serde_json::to_value(&compiled.display).map_err(|error| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "could not serialize generated display definitions: {error}"
            ))
        })?;
        let views = serde_json::to_value(&compiled.views).map_err(|error| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "could not serialize generated views: {error}"
            ))
        })?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"INSERT INTO blueprints (id, code, name, kind, version, includes, display, views, definition, definition_hash)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id, code, name, kind, version, includes, display, views, created_at, updated_at, deleted_at, definition, definition_hash"#,
        )
        .bind(blueprint_id)
        .bind(compiled.code)
        .bind(compiled.name)
        .bind(compiled.kind.as_str())
        .bind(version)
        .bind(includes)
        .bind(display)
        .bind(views)
        .bind(definition)
        .bind(compiled.raw_definition_hash)
        .fetch_one(&mut **transaction)
        .await?;

        let mut attributes = Vec::with_capacity(compiled.attributes.len());
        for attribute in compiled.attributes {
            attributes.push(
                sqlx::query_as::<_, Attribute>(
                    r#"INSERT INTO attributes (id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, tags, context_fallback, context_editable, position)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                       RETURNING id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, tags, context_fallback, context_editable, position, created_at, updated_at, deleted_at"#,
                )
                .bind(Uuid::new_v4())
                .bind(blueprint_id)
                .bind(version)
                .bind(attribute.code)
                .bind(attribute.value_type)
                .bind(attribute.target_blueprint)
                .bind(serde_json::to_value(attribute.tags).expect("attribute tags serialize"))
                .bind(attribute.context_fallback)
                .bind(attribute.context_editable)
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
            r#"SELECT id, code, name, kind, version, includes, display, views, created_at, updated_at, deleted_at, definition, definition_hash
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
            r#"SELECT id, code, name, kind, version, includes, display, views, created_at, updated_at, deleted_at, definition, definition_hash
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
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, display, views, created_at, updated_at, deleted_at, definition, definition_hash
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
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, display, views, created_at, updated_at, deleted_at, definition, definition_hash
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
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, target_blueprint_code, tags, context_fallback, context_editable, position, created_at, updated_at, deleted_at
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
        validate_code(&input.code)?;
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }

        if !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(input.parent_id)
        .fetch_one(&self.pool)
        .await?
        {
            return Err(RepositoryError::InvalidContext);
        }
        Ok(sqlx::query_as::<_, AttributeContext>(
            r#"INSERT INTO attribute_contexts (id, code, data, parent_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id, code, data, parent_id"#,
        )
        .bind(Uuid::new_v4())
        .bind(input.code)
        .bind(input.data)
        .bind(input.parent_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_context_by_code(
        &self,
        code: &str,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        validate_code(code)?;
        Ok(sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_contexts(&self) -> Result<Vec<AttributeContext>, RepositoryError> {
        Ok(sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_context(
        &self,
        id: Uuid,
        input: UpdateAttributeContext,
    ) -> Result<AttributeContext, RepositoryError> {
        if id == Self::DEFAULT_CONTEXT_ID {
            return Err(RepositoryError::DefaultContextProtected);
        }
        if !input.data.is_object() {
            return Err(RepositoryError::InvalidContextData);
        }
        if !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(input.parent_id)
        .fetch_one(&self.pool)
        .await?
        {
            return Err(RepositoryError::InvalidContext);
        }
        let result = sqlx::query_as::<_, AttributeContext>(
            r#"WITH RECURSIVE descendants AS (
                    SELECT id FROM attribute_contexts WHERE id = $1
                    UNION ALL
                    SELECT c.id FROM attribute_contexts c JOIN descendants d ON c.parent_id = d.id
                )
                UPDATE attribute_contexts SET parent_id = $2, data = $3
                WHERE id = $1 AND $2 NOT IN (SELECT id FROM descendants)
                RETURNING id, code, data, parent_id"#,
        )
        .bind(id)
        .bind(input.parent_id)
        .bind(input.data)
        .fetch_optional(&self.pool)
        .await?;
        result.ok_or(RepositoryError::ContextCycle)
    }

    pub async fn delete_context(&self, id: Uuid) -> Result<(), RepositoryError> {
        if id == Self::DEFAULT_CONTEXT_ID {
            return Err(RepositoryError::DefaultContextProtected);
        }
        let result = sqlx::query(
            "DELETE FROM attribute_contexts c WHERE c.id = $1 AND NOT EXISTS (SELECT 1 FROM attribute_contexts child WHERE child.parent_id = c.id) AND NOT EXISTS (SELECT 1 FROM attribute_values value WHERE value.context_id = c.id AND value.latest)",
        ).bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::ContextInUse);
        }
        Ok(())
    }

    pub async fn resolved_preview(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<Option<ResolvedEntityPreviewResponse>, RepositoryError> {
        let entity = match self.get_entity(entity_id).await? {
            Some(entity) => entity,
            None => return Ok(None),
        };
        let requested_context = self
            .get_context_by_id(context_id)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        let attributes = self
            .list_attributes(entity.blueprint_id, entity.blueprint_version)
            .await?;
        let preview = entity
            .projections
            .get("preview")
            .and_then(Value::as_object)
            .ok_or(RepositoryError::InvalidPreview)?;
        let contexts = self.list_contexts().await?;
        let context_by_id: std::collections::HashMap<_, _> = contexts
            .into_iter()
            .map(|context| (context.id, context))
            .collect();
        let mut path = Vec::new();
        let mut current = Some(requested_context.clone());
        while let Some(context) = current {
            current = context
                .parent_id
                .and_then(|parent_id| context_by_id.get(&parent_id).cloned());
            path.push(context);
        }
        let mut values = attributes.iter().filter_map(|attribute| {
            path.iter().enumerate().find_map(|(index, context)| {
                if index > 0 && attribute.context_fallback == "none" { return None; }
                preview.get(&context.code).and_then(Value::as_object).and_then(|values| values.get(&attribute.code)).map(|value| {
                    (attribute.code.clone(), serde_json::json!({ "value": value, "source_context": { "id": context.id, "code": context.code } }))
                })
            })
        }).collect::<Map<_, _>>();
        let enriched_preview = self
            .build_preview(entity.id, &entity.projections, 1, 10, &mut HashSet::new())
            .await?;
        let relationships = attributes
            .iter()
            .filter(|attribute| attribute.value_type == "relationship")
            .filter_map(|attribute| {
                path.iter().enumerate().find_map(|(index, context)| {
                    if index > 0 && attribute.context_fallback == "none" {
                        return None;
                    }
                    enriched_preview
                        .get(&context.code)
                        .and_then(Value::as_object)
                        .and_then(|values| values.get(&attribute.code))
                        .filter(|value| value.get("items").is_some())
                        .map(|value| {
                            (
                                attribute.code.clone(),
                                serde_json::json!({
                                    "value": value,
                                    "source_context": { "id": context.id, "code": context.code },
                                }),
                            )
                        })
                })
            })
            .collect::<Map<_, _>>();
        values.extend(relationships);
        Ok(Some(ResolvedEntityPreviewResponse {
            requested_context,
            values: Value::Object(values),
        }))
    }

    async fn get_context_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AttributeContext>, RepositoryError> {
        Ok(sqlx::query_as::<_, AttributeContext>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE id = $1",
        )
        .bind(id)
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
            r#"INSERT INTO entities (id, blueprint_id, blueprint_version, projections)
               SELECT $1, b.id, b.version, $2
               FROM blueprints b
                WHERE b.id = $3
                  AND b.version = $4
                 AND b.kind = 'entity'
                 AND b.deleted_at IS NULL
                RETURNING id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(Value::Object(projections.clone()))
        .bind(input.blueprint_id)
        .bind(input.blueprint_version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    pub async fn create_entity_with_values(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: Vec<NewAttributeValue>,
    ) -> Result<Entity, RepositoryError> {
        // Do not expose an entity before its initial values and derived preview
        // agree; otherwise a concurrent reader can observe a partial create.
        let mut transaction = self.pool.begin().await?;
        let entity = self
            .insert_entity(&mut transaction, blueprint_id, blueprint_version)
            .await?;
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        transaction.commit().await?;
        Ok(entity)
    }

    pub async fn update_entity_with_values(
        &self,
        entity_id: Uuid,
        values: Vec<NewAttributeValue>,
        relationships: Vec<RelationshipTargets>,
        remove_values: Vec<AttributeValueSelector>,
    ) -> Result<Entity, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        // The row lock serializes writers for an entity. It protects both the
        // one-latest-value invariant and the preview rebuilt from that state.
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        for value in values {
            self.insert_value(&mut transaction, &entity, value).await?;
        }
        for selector in remove_values {
            self.remove_scalar_value(&mut transaction, &entity, selector)
                .await?;
        }
        self.replace_relationship_sets(&mut transaction, &entity, relationships)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        let entity = self
            .store_preview(&mut transaction, entity.id, preview)
            .await?;
        transaction.commit().await?;
        Ok(entity)
    }

    pub async fn form_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FormNativeValueRow>(
            r#"SELECT a.code AS attribute_code, av.context_id, av.relationship_target_entity_id,
                      a.value_type, av.value_text, av.value_number, av.value_integer,
                      av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                      av.value_time_zone
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1
                 AND av.latest
                 AND (av.relationship_target_entity_id IS NULL OR av.active)
               ORDER BY a.position, av.relationship_target_entity_id"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                Ok(match row.relationship_target_entity_id {
                    Some(target_entity_id) => FormAttributeValue::Relationship {
                        attribute_code: row.attribute_code,
                        context_id: row.context_id,
                        target_entity_id,
                    },
                    None => FormAttributeValue::Scalar {
                        attribute_code: row.attribute_code,
                        context_id: row.context_id,
                        value: native_value_json(row.native)?,
                    },
                })
            })
            .collect::<Result<_, RepositoryError>>()?)
    }

    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities
               WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "UPDATE entities SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(entity_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("entity"));
        }
        Ok(())
    }

    pub async fn current_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, CurrentNativeValueRow>(
            r#"SELECT av.id, av.entity_id, av.attribute_id, av.relationship_target_entity_id,
                      av.context_id, av.active, av.created_at, a.value_type, av.value_text,
                      av.value_number, av.value_integer, av.value_boolean, av.value_date,
                      av.value_datetime, av.value_time, av.value_time_zone
                FROM attribute_values av
                JOIN attributes a ON a.id = av.attribute_id
                WHERE av.entity_id = $1
                  AND av.latest
                  AND (av.relationship_target_entity_id IS NULL OR av.active)"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AttributeValue {
                    id: row.id,
                    entity_id: row.entity_id,
                    attribute_id: row.attribute_id,
                    value: if row.relationship_target_entity_id.is_some() {
                        Value::Null
                    } else {
                        native_value_json(row.native)?
                    },
                    relationship_target_entity_id: row.relationship_target_entity_id,
                    active: row.active,
                    context_id: row.context_id,
                    created_at: row.created_at,
                })
            })
            .collect()
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
            r#"WITH relationships AS (
                    SELECT a.code AS attribute_code, c.code AS context_code, target.id AS target_id,
                            target.projections AS target_projections, b.display AS target_display,
                            (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                               FROM attributes attribute
                              WHERE attribute.blueprint_id = target.blueprint_id
                                AND attribute.blueprint_version = target.blueprint_version
                                AND attribute.deleted_at IS NULL) AS target_context_fallback,
                           ROW_NUMBER() OVER (PARTITION BY a.id, av.context_id ORDER BY target.id)
                               AS relationship_position
                    FROM attribute_values av
                    JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                    JOIN entities target ON target.id = av.relationship_target_entity_id AND target.deleted_at IS NULL
                    JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
                    LEFT JOIN attribute_contexts c ON c.id = av.context_id
                    WHERE av.entity_id = $1
                      AND av.relationship_target_entity_id IS NOT NULL
                      AND av.latest
                      AND av.active
                )
               SELECT attribute_code, context_code, target_id, target_projections, target_display, target_context_fallback, relationship_position
               FROM relationships
               WHERE relationship_position <= $2
               ORDER BY attribute_code, context_code NULLS FIRST, relationship_position"#,
        )
        .bind(entity_id)
        // Fetch one extra edge to report truncation without a separate count.
        // At depth zero one edge is still enough to indicate that data exists.
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
            target_values.retain(|_, value| value.get("items").is_some());
            target_values.insert(
                "id".to_owned(),
                Value::String(relationship.target_id.to_string()),
            );
            target_values.insert(
                "display".to_owned(),
                display_label(
                    &target_preview,
                    &relationship.target_display,
                    &relationship.target_context_fallback,
                    &context_code,
                ),
            );

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
        validate_code(blueprint_code)?;
        validate_code(relationship)?;
        let rows = sqlx::query_as::<_, EntityPreviewRow>(
            r#"SELECT target.id, target.blueprint_version, target.created_at, target.projections -> 'preview' AS preview,
                      b.display AS blueprint_display,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.blueprint_id = target.blueprint_id
                          AND attribute.blueprint_version = target.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
               JOIN entities target ON target.id = av.relationship_target_entity_id
               JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
               WHERE av.entity_id = $1
                 AND a.code = $2
                 AND av.relationship_target_entity_id IS NOT NULL
                 AND av.latest
                 AND av.active
                 AND ($3::uuid IS NULL OR av.relationship_target_entity_id > $3)
                 AND target.deleted_at IS NULL
                 AND b.code = $4
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
        let mut items: Vec<_> = rows.into_iter().map(entity_preview).collect();
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|item| item.id)
        } else {
            None
        };
        Ok(EntityPreviewPage { items, next_cursor })
    }

    pub async fn search_entity_previews(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        query: Option<&str>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        let sql = r#"SELECT e.id, e.blueprint_version, e.created_at, e.projections -> 'preview' AS preview,
                      b.display AS blueprint_display,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.blueprint_id = e.blueprint_id
                          AND attribute.blueprint_version = e.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
                FROM entities e
                JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                WHERE e.blueprint_id = $1
                  AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                 AND e.deleted_at IS NULL
                 AND (
                    $3::text IS NULL
                     OR EXISTS (
                        SELECT 1
                        FROM attribute_values av
                        WHERE av.entity_id = e.id
                          AND av.relationship_target_entity_id IS NULL
                          AND av.latest
                           AND COALESCE(
                             av.value_text,
                             av.value_number::text,
                             av.value_integer::text,
                             av.value_boolean::text,
                             av.value_date::text,
                             av.value_datetime::text,
                             av.value_time::text
                           ) ILIKE '%' || $3 || '%'
                    )
                 )
                 AND (
                     $4::timestamptz IS NULL
                     OR (e.created_at, e.id) > ($4, $5)
                  )
                ORDER BY e.created_at, e.id
                LIMIT $6"#;
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = sqlx::query_as::<_, EntityPreviewRow>(sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(query)
            .bind(cursor_created_at)
            .bind(cursor_id)
            .bind(limit + 1)
            .fetch_all(&self.pool)
            .await?;
        let mut items: Vec<_> = rows.into_iter().map(entity_preview).collect();
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|item| encode_search_cursor(item.created_at, item.id))
        } else {
            None
        };
        Ok((items, next_cursor))
    }

    pub async fn append_values(
        &self,
        entity_id: Uuid,
        input: AppendAttributeValues,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
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

        let preview = Self::build_preview_projection(&mut transaction, entity_id).await?;
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
            let (attribute_id, target_blueprint_code, context_editable) = self
                .relationship_attribute(&mut transaction, &entity, &relationship)
                .await?;
            self.validate_context_id(&mut transaction, relationship.context_id)
                .await?;
            self.validate_context_editable(relationship.context_id, &context_editable)?;
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

    async fn replace_relationship_sets(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        relationships: Vec<RelationshipTargets>,
    ) -> Result<(), RepositoryError> {
        for relationship in relationships {
            self.validate_context_id(transaction, relationship.context_id)
                .await?;
            let (attribute_id, target_blueprint_code, context_editable) = self
                .relationship_attribute(transaction, entity, &relationship)
                .await?;
            self.validate_context_editable(relationship.context_id, &context_editable)?;
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
                .current_relationship_targets(
                    transaction,
                    entity.id,
                    attribute_id,
                    relationship.context_id,
                )
                .await?;
            for target_id in current.difference(&targets) {
                self.insert_relationship_value(
                    transaction,
                    entity.id,
                    attribute_id,
                    relationship.context_id,
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
                    relationship.context_id,
                    *target_id,
                    true,
                )
                .await?;
            }
        }
        Ok(())
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

        let (attribute_id, value_type, target_blueprint_code, context_editable) =
            match (attribute_id, attribute_code) {
                (Some(attribute_id), None) => {
                    sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
                        r#"SELECT id, value_type, target_blueprint_code, context_editable
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
                    sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
                        r#"SELECT id, value_type, target_blueprint_code, context_editable
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

        self.validate_context_id(transaction, context_id).await?;
        self.validate_context_editable(context_id, &context_editable)?;

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

        self.supersede_latest_value(
            transaction,
            entity.id,
            attribute_id,
            context_id,
            target_entity_id,
        )
        .await?;

        let query = sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (
                    id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
                    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
                    value_time, value_time_zone
                ) VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8, $9, $10, $11, $12, $13)
                RETURNING id, entity_id, attribute_id,
                    'null'::jsonb AS value,
                    relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4())
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
        self.validate_context_id(transaction, selector.context_id)
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
        self.validate_context_editable(selector.context_id, &attribute.2)?;
        sqlx::query(
            "UPDATE attribute_values SET latest = false WHERE entity_id = $1 AND attribute_id = $2 AND context_id IS NOT DISTINCT FROM $3 AND relationship_target_entity_id IS NULL AND latest",
        )
        .bind(entity.id)
        .bind(attribute.0)
        .bind(selector.context_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    async fn validate_context_id(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        let context_id = context_id.ok_or(RepositoryError::InvalidContext)?;
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1)",
        )
        .bind(context_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !exists {
            return Err(RepositoryError::InvalidContext);
        }
        Ok(())
    }

    fn validate_context_editable(
        &self,
        context_id: Option<Uuid>,
        context_editable: &str,
    ) -> Result<(), RepositoryError> {
        if context_id != Some(Self::DEFAULT_CONTEXT_ID) && context_editable == "default" {
            return Err(RepositoryError::DefaultContextOnly);
        }
        Ok(())
    }

    async fn lock_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Entity>(
            r#"SELECT id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at
               FROM entities WHERE id = $1 AND deleted_at IS NULL FOR UPDATE"#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("entity"))
    }

    async fn insert_entity(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Entity, RepositoryError> {
        sqlx::query_as::<_, Entity>(
            r#"INSERT INTO entities (id, blueprint_id, blueprint_version, projections)
               SELECT $1, b.id, b.version, $2
               FROM blueprints b
               WHERE b.id = $3 AND b.version = $4 AND b.kind = 'entity' AND b.deleted_at IS NULL
                RETURNING id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at"#,
        )
        .bind(Uuid::new_v4())
        .bind(empty_projections())
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    async fn store_preview(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        preview: Value,
    ) -> Result<Entity, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1
                RETURNING id, blueprint_id, blueprint_version, projections, created_at, updated_at, deleted_at"#,
        )
        .bind(entity_id)
        .bind(preview)
        .fetch_one(&mut **transaction)
        .await?)
    }

    async fn build_preview_projection(
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Value, RepositoryError> {
        let values = sqlx::query_as::<_, ProjectionNativeValueRow>(
            r#"SELECT a.code AS attribute_code, c.code AS context_code,
                      a.value_type, av.value_text, av.value_number, av.value_integer,
                      av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                      av.value_time_zone
                FROM attribute_values av
                JOIN entities e ON e.id = av.entity_id
                JOIN attributes a ON a.id = av.attribute_id
                 AND a.blueprint_id = e.blueprint_id
                 AND a.blueprint_version = e.blueprint_version
                JOIN attribute_contexts c ON c.id = av.context_id
                WHERE av.entity_id = $1
                  AND av.relationship_target_entity_id IS NULL
                  AND av.latest
                  AND a.deleted_at IS NULL
                ORDER BY a.position, c.code"#,
        )
        .bind(entity_id)
        .fetch_all(&mut **transaction)
        .await?;
        let mut default = Map::new();
        let mut contexts = Map::new();
        for value in values {
            let values = if value.context_code == "default" {
                &mut default
            } else {
                contexts
                    .entry(value.context_code)
                    .or_insert_with(|| Value::Object(Map::new()))
                    .as_object_mut()
                    .expect("projection contexts are objects")
            };
            values.insert(value.attribute_code, native_value_json(value.native)?);
        }
        let mut preview = Map::new();
        preview.insert("default".to_owned(), Value::Object(default));
        preview.extend(contexts);
        Ok(Value::Object(preview))
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
            r#"SELECT relationship_target_entity_id
               FROM attribute_values
               WHERE entity_id = $1
                 AND attribute_id = $2
                 AND context_id IS NOT DISTINCT FROM $3
                 AND relationship_target_entity_id IS NOT NULL
                 AND latest
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
        self.supersede_latest_value(
            transaction,
            entity_id,
            attribute_id,
            context_id,
            Some(target_entity_id),
        )
        .await?;

        Ok(sqlx::query_as::<_, AttributeValue>(
            r#"INSERT INTO attribute_values (id, entity_id, attribute_id, context_id, relationship_target_entity_id, active)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, entity_id, attribute_id, 'null'::jsonb AS value, relationship_target_entity_id, context_id, active, created_at"#,
        )
        .bind(Uuid::new_v4()).bind(entity_id).bind(attribute_id).bind(context_id)
        .bind(target_entity_id).bind(active).fetch_one(&mut **transaction).await?)
    }

    async fn supersede_latest_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Option<Uuid>,
        relationship_target_entity_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        // Preserve the event for history and move only the current-state marker.
        // The caller holds the entity lock before this transition.
        sqlx::query(
            r#"UPDATE attribute_values
               SET latest = false
               WHERE entity_id = $1
                 AND attribute_id = $2
                 AND context_id IS NOT DISTINCT FROM $3
                 AND relationship_target_entity_id IS NOT DISTINCT FROM $4
                 AND latest"#,
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .bind(relationship_target_entity_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
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

pub(crate) fn decode_search_cursor(cursor: &str) -> Option<(DateTime<Utc>, Uuid)> {
    let decoded = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let value = String::from_utf8(decoded).ok()?;
    let (created_at, id) = value.rsplit_once('\0')?;
    Some((created_at.parse().ok()?, id.parse().ok()?))
}

fn encode_search_cursor(created_at: DateTime<Utc>, id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}\0{}", created_at.to_rfc3339(), id))
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

fn entity_preview(row: EntityPreviewRow) -> EntityPreview {
    EntityPreview {
        id: row.id,
        blueprint_version: row.blueprint_version,
        schema_outdated: false,
        created_at: row.created_at,
        display: display_labels(
            &row.preview,
            &row.blueprint_display,
            &row.blueprint_context_fallback,
        ),
        preview: row.preview,
    }
}

fn display_labels(preview: &Value, display: &Value, context_fallback: &Value) -> Value {
    let contexts = preview.as_object().cloned().unwrap_or_default();
    Value::Object(
        contexts
            .keys()
            .map(|context| {
                (
                    context.clone(),
                    display_label(preview, display, context_fallback, context),
                )
            })
            .collect(),
    )
}

fn display_label(
    preview: &Value,
    display: &Value,
    context_fallback: &Value,
    context_code: &str,
) -> Value {
    let Some(definition) = display.get("dropdown_option") else {
        return Value::String(String::new());
    };
    let Some(fields) = definition.get("fields").and_then(Value::as_array) else {
        return Value::String(String::new());
    };
    let separator = definition
        .get("separator")
        .and_then(Value::as_str)
        .unwrap_or(" · ");
    let current = preview.get(context_code).and_then(Value::as_object);
    let default = preview.get("default").and_then(Value::as_object);
    Value::String(
        fields
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|field| {
                current.and_then(|values| values.get(field)).or_else(|| {
                    (context_code == "default"
                        || context_fallback.get(field).and_then(Value::as_str) != Some("none"))
                    .then(|| default.and_then(|values| values.get(field)))
                    .flatten()
                })
            })
            .filter(|value| !value.is_null())
            .map(display_value)
            .collect::<Vec<_>>()
            .join(separator),
    )
}

fn validate_code(value: &str) -> Result<(), RepositoryError> {
    if is_valid_code(value) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCode)
    }
}

fn display_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
