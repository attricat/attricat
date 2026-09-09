use super::*;
use crate::domain_events::{ATTRIBUTE_VALUE_RESTORED_V1, AttributeValueMutationV1};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

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

const FORM_VALUES_SQL: &str = r#"SELECT a.code AS attribute_code, av.context_id, av.relationship_target_entity_id,
                  a.value_type, av.value_text, av.value_number, av.value_integer,
                  av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                  av.value_time_zone
           FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           JOIN entities e ON e.id = av.entity_id
           WHERE av.entity_id = $1
             AND a.blueprint_id = e.blueprint_id
             AND a.blueprint_version = e.blueprint_version
             AND a.value_type <> 'file'
             AND (av.relationship_target_entity_id IS NULL OR av.active)
           ORDER BY a.position, av.relationship_target_entity_id"#;

#[derive(sqlx::FromRow)]
struct FileFormValueRow {
    attribute_code: String,
    context_id: Option<Uuid>,
    file_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct HistoryNativeValueRow {
    id: Uuid,
    entity_id: Uuid,
    attribute_id: Uuid,
    relationship_target_entity_id: Option<Uuid>,
    active: bool,
    context_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    archived_at: DateTime<Utc>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

#[derive(sqlx::FromRow)]
pub(super) struct ProjectionNativeValueRow {
    pub(super) attribute_code: String,
    pub(super) context_code: String,
    #[sqlx(flatten)]
    pub(super) native: NativeValueRow,
}

pub(super) enum NativeValue {
    Text(String),
    Number(Decimal),
    Integer(i64),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
    Time(NaiveTime, String),
}

#[derive(Clone, Copy)]
pub(super) enum ValueType {
    String,
    Number,
    Integer,
    Boolean,
    Date,
    Datetime,
    Time,
    /// File metadata is stored in attribute_file_references, not a native
    /// attribute_values value column.
    File,
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
    pub(super) fn parse(value_type: &str) -> Result<Self, RepositoryError> {
        match value_type {
            "string" => Ok(Self::String),
            "number" => Ok(Self::Number),
            "integer" => Ok(Self::Integer),
            "boolean" => Ok(Self::Boolean),
            "date" => Ok(Self::Date),
            "datetime" => Ok(Self::Datetime),
            "time" => Ok(Self::Time),
            "file" => Ok(Self::File),
            _ => Err(RepositoryError::AttributeValueTypeMismatch),
        }
    }
}

impl NativeValue {
    pub(super) fn parse(value_type: ValueType, value: Value) -> Result<Self, RepositoryError> {
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
            ValueType::File => Err(invalid()),
        }
    }

    pub(super) fn json(&self) -> Value {
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

    pub(super) fn bind<'q, O>(
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
    if matches!(value_type, ValueType::File) {
        return Ok(Value::Null);
    }
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
        ValueType::File => unreachable!("file values return before native decoding"),
    }
    .ok_or(RepositoryError::InvalidStoredAttributeValue)?;
    if populated != 1 {
        return Err(RepositoryError::InvalidStoredAttributeValue);
    }
    Ok(value.json())
}

fn form_attribute_values(
    rows: Vec<FormNativeValueRow>,
) -> Result<Vec<FormAttributeValue>, RepositoryError> {
    rows.into_iter()
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
        .collect()
}

fn history_attribute_value(
    row: HistoryNativeValueRow,
) -> Result<AttributeValueHistory, RepositoryError> {
    Ok(AttributeValueHistory {
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
        archived_at: row.archived_at,
    })
}

impl CatalogRepository {
    pub async fn form_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FormNativeValueRow>(FORM_VALUES_SQL)
            .bind(entity_id)
            .fetch_all(&self.pool)
            .await?;
        let mut values = form_attribute_values(rows)?;
        values.extend(self.file_form_values(entity_id).await?);
        Ok(values)
    }

    pub(crate) async fn file_form_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FileFormValueRow>(
            r#"SELECT a.code AS attribute_code, av.context_id, r.file_id
               FROM attribute_file_references r
               JOIN attribute_values av ON av.id = r.attribute_value_id
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1
                 AND av.workspace_id = $2
                 AND a.value_type = 'file'
               ORDER BY a.position, av.context_id, r.position"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?;
        let mut values: Vec<FormAttributeValue> = Vec::new();
        for row in rows {
            let metadata = self.file_metadata(row.file_id).await?;
            match values.last_mut() {
                Some(FormAttributeValue::File {
                    attribute_code,
                    context_id,
                    files,
                }) if *attribute_code == row.attribute_code && *context_id == row.context_id => {
                    files.push(metadata);
                }
                _ => values.push(FormAttributeValue::File {
                    attribute_code: row.attribute_code,
                    context_id: row.context_id,
                    files: vec![metadata],
                }),
            }
        }
        Ok(values)
    }

    pub(super) async fn form_values_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FormNativeValueRow>(FORM_VALUES_SQL)
            .bind(entity_id)
            .fetch_all(&mut **transaction)
            .await?;
        form_attribute_values(rows)
    }

    pub(super) async fn list_attributes_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Attribute>(
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, value_schema, default_value, file_policy,
                      target_blueprint_code, relationship_cardinality, tags, context_fallback, context_editable, readonly,
                      position, created_at, updated_at, deleted_at
               FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND deleted_at IS NULL
               ORDER BY position"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut **transaction)
        .await?)
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
                  AND av.workspace_id = $2
                  AND (av.relationship_target_entity_id IS NULL OR av.active)"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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

    pub async fn value_history(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<AttributeValueHistory>, RepositoryError> {
        let rows = sqlx::query_as::<_, HistoryNativeValueRow>(
            r#"SELECT h.id, h.entity_id, h.attribute_id, h.relationship_target_entity_id,
                      h.active, h.context_id, h.created_at, h.archived_at, a.value_type,
                      h.value_text, h.value_number, h.value_integer, h.value_boolean,
                      h.value_date, h.value_datetime, h.value_time, h.value_time_zone
               FROM attribute_value_history h
               JOIN attributes a ON a.id = h.attribute_id
               WHERE h.entity_id = $1
               ORDER BY h.archived_at DESC, h.id DESC"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(history_attribute_value).collect()
    }

    pub async fn restore_value(
        &self,
        entity_id: Uuid,
        history_id: Uuid,
    ) -> Result<AttributeValue, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let history = sqlx::query_as::<_, HistoryNativeValueRow>(
            r#"SELECT h.id, h.entity_id, h.attribute_id, h.relationship_target_entity_id,
                      h.active, h.context_id, h.created_at, h.archived_at, a.value_type,
                      h.value_text, h.value_number, h.value_integer, h.value_boolean,
                      h.value_date, h.value_datetime, h.value_time, h.value_time_zone
               FROM attribute_value_history h
               JOIN attributes a ON a.id = h.attribute_id
               WHERE h.id = $1 AND h.entity_id = $2
               FOR UPDATE"#,
        )
        .bind(history_id)
        .bind(entity_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("attribute value history"))?;

        let value = match history.relationship_target_entity_id {
            Some(target_entity_id) if history.active => {
                self.insert_value(
                    &mut transaction,
                    &entity,
                    NewAttributeValue::Relationship {
                        attribute_id: Some(history.attribute_id),
                        attribute_code: None,
                        context_id: history.context_id,
                        target_entity_id,
                    },
                )
                .await?
            }
            Some(target_entity_id) => self
                .archive_current_value(
                    &mut transaction,
                    entity.id,
                    history.attribute_id,
                    history.context_id,
                    Some(target_entity_id),
                )
                .await?
                .ok_or(RepositoryError::NotFound("current relationship value"))?,
            None => {
                self.insert_value(
                    &mut transaction,
                    &entity,
                    NewAttributeValue::Scalar {
                        attribute_id: Some(history.attribute_id),
                        attribute_code: None,
                        context_id: history.context_id,
                        value: native_value_json(history.native)?,
                    },
                )
                .await?
            }
        };
        self.validate_entity_schema(&mut transaction, &entity)
            .await?;
        let preview = Self::build_preview_projection(&mut transaction, entity.id).await?;
        self.store_preview(&mut transaction, entity.id, preview)
            .await?;
        let after = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let changes = Self::audit_changes(entity_id, before, after, true);
        let event = self.core_event(
            ATTRIBUTE_VALUE_RESTORED_V1,
            "entity",
            entity_id,
            serde_json::to_value(AttributeValueMutationV1 {
                entity_id,
                facts: Self::affected_facts(&changes),
            })
            .expect("attribute-value-restored payload is serializable"),
        );
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(value)
    }
}
