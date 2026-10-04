use super::entity_commands::Revalidation;
use super::write_context::WriteContext;
use super::*;
use crate::domain_events::{ATTRIBUTE_VALUE_RESTORED_V1, AttributeValueMutationV1};
use crate::persistence_rows::{Db, IntoDomain};
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
                  av.value_time_zone, av.value_json
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
    file_id: Option<Uuid>,
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
    Json(Value),
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
    Json,
    /// File metadata is stored in attribute_file_references, not a native
    /// attribute_values value column.
    File,
}

#[derive(Clone, sqlx::FromRow)]
pub struct NativeValueRow {
    pub value_type: String,
    pub value_text: Option<String>,
    pub value_number: Option<Decimal>,
    pub value_integer: Option<i64>,
    pub value_boolean: Option<bool>,
    pub value_date: Option<NaiveDate>,
    pub value_datetime: Option<DateTime<Utc>>,
    pub value_time: Option<NaiveTime>,
    pub value_time_zone: Option<String>,
    pub value_json: Option<Value>,
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
            "json" => Ok(Self::Json),
            "file" => Ok(Self::File),
            _ => Err(RepositoryError::AttributeValueTypeMismatch),
        }
    }
}

/// One native value split into the `attribute_values` value columns.
#[derive(Default)]
pub(super) struct NativeColumns {
    pub text: Option<String>,
    pub number: Option<Decimal>,
    pub integer: Option<i64>,
    pub boolean: Option<bool>,
    pub date: Option<NaiveDate>,
    pub datetime: Option<DateTime<Utc>>,
    pub time: Option<NaiveTime>,
    pub time_zone: Option<String>,
    pub json: Option<Value>,
}

impl NativeValue {
    pub(super) fn into_columns(self) -> NativeColumns {
        let mut columns = NativeColumns::default();
        match self {
            Self::Text(value) => columns.text = Some(value),
            Self::Number(value) => columns.number = Some(value),
            Self::Integer(value) => columns.integer = Some(value),
            Self::Boolean(value) => columns.boolean = Some(value),
            Self::Date(value) => columns.date = Some(value),
            Self::Datetime(value) => columns.datetime = Some(value),
            Self::Time(time, time_zone) => {
                columns.time = Some(time);
                columns.time_zone = Some(time_zone);
            }
            Self::Json(value) => columns.json = Some(value),
        }
        columns
    }

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
            ValueType::Json => Ok(Self::Json(value)),
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
            Self::Json(value) => value.clone(),
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
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Number(value) => query
                .bind(Option::<String>::None)
                .bind(Some(value))
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Integer(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Some(value))
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Boolean(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Some(value))
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Date(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Some(value))
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Datetime(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Some(value))
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Option::<Value>::None),
            Self::Time(time, time_zone) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Some(time))
                .bind(Some(time_zone))
                .bind(Option::<Value>::None),
            Self::Json(value) => query
                .bind(Option::<String>::None)
                .bind(Option::<Decimal>::None)
                .bind(Option::<i64>::None)
                .bind(Option::<bool>::None)
                .bind(Option::<NaiveDate>::None)
                .bind(Option::<DateTime<Utc>>::None)
                .bind(Option::<NaiveTime>::None)
                .bind(Option::<String>::None)
                .bind(Some(value)),
        }
    }
}

pub fn native_value_json(row: NativeValueRow) -> Result<Value, RepositoryError> {
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
        row.value_json.is_some(),
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
        ValueType::Json => row.value_json.map(NativeValue::Json),
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

    pub async fn file_form_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FileFormValueRow>(
            r#"SELECT a.code AS attribute_code, av.context_id, r.file_id
               FROM attribute_values av
               LEFT JOIN attribute_file_references r ON av.id = r.attribute_value_id
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1
                 AND av.workspace_id = $2
                 AND a.value_type = 'file'
                 AND a.blueprint_id = (SELECT blueprint_id FROM entities WHERE id = $1)
                 AND a.blueprint_version = (SELECT blueprint_version FROM entities WHERE id = $1)
               ORDER BY a.position, av.context_id, r.position"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        let mut values: Vec<FormAttributeValue> = Vec::new();
        for row in rows {
            let metadata = match row.file_id {
                Some(id) => vec![self.file_metadata(id).await?],
                None => vec![],
            };
            match values.last_mut() {
                Some(FormAttributeValue::File {
                    attribute_code,
                    context_id,
                    files,
                }) if *attribute_code == row.attribute_code && *context_id == row.context_id => {
                    files.extend(metadata);
                }
                _ => values.push(FormAttributeValue::File {
                    attribute_code: row.attribute_code,
                    context_id: row.context_id,
                    files: metadata,
                }),
            }
        }
        Ok(values)
    }

    pub async fn reusable_form_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FormAttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, FormNativeValueRow>(
            r#"SELECT a.code AS attribute_code, av.context_id, av.relationship_target_entity_id,
                  a.value_type, av.value_text, av.value_number, av.value_integer,
                  av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                  av.value_time_zone, av.value_json
           FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           WHERE av.entity_id = $1 AND a.entity_id = $1 AND a.value_type <> 'file'
             AND (av.relationship_target_entity_id IS NULL OR av.active)
           ORDER BY a.position, av.relationship_target_entity_id"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        let mut values = form_attribute_values(rows)?;
        let file_rows = sqlx::query_as::<_, FileFormValueRow>(
            r#"SELECT a.code AS attribute_code, av.context_id, r.file_id
               FROM attribute_values av
               LEFT JOIN attribute_file_references r ON av.id = r.attribute_value_id
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1 AND a.entity_id = $1 AND a.value_type = 'file'
               ORDER BY a.position, av.context_id, r.position"#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        for row in file_rows {
            let metadata = match row.file_id {
                Some(id) => vec![self.file_metadata(id).await?],
                None => vec![],
            };
            match values.last_mut() {
                Some(FormAttributeValue::File {
                    attribute_code,
                    context_id,
                    files,
                }) if *attribute_code == row.attribute_code && *context_id == row.context_id => {
                    files.extend(metadata)
                }
                _ => values.push(FormAttributeValue::File {
                    attribute_code: row.attribute_code,
                    context_id: row.context_id,
                    files: metadata,
                }),
            }
        }
        Ok(values)
    }

    pub(super) async fn current_blueprint_values_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, CurrentNativeValueRow>(
            r#"SELECT av.id, av.entity_id, av.attribute_id, av.relationship_target_entity_id,
                      av.context_id, av.active, av.created_at, a.value_type, av.value_text,
                      av.value_number, av.value_integer, av.value_boolean, av.value_date,
                      av.value_datetime, av.value_time, av.value_time_zone, av.value_json
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               WHERE av.entity_id = $1 AND av.workspace_id = $2
                 AND a.blueprint_id = $3 AND a.blueprint_version = $4"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut **transaction)
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

    pub(super) async fn list_attributes_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<Attribute>>(
            r#"SELECT id, blueprint_id, blueprint_version, code, name, value_type, value_schema, extension_type, default_value, file_policy,
                      target_blueprint_code, target_blueprint_codes, cardinality, target_cardinality, hierarchy, tags, context_fallback, context_editable, readonly,
                      position, created_at, updated_at, deleted_at
               FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND deleted_at IS NULL
               ORDER BY position"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut **transaction)
        .await?
        .into_domain())
    }

    pub async fn current_values(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        let rows = sqlx::query_as::<_, CurrentNativeValueRow>(
            r#"SELECT av.id, av.entity_id, av.attribute_id, av.relationship_target_entity_id,
                      av.context_id, av.active, av.created_at, a.value_type, av.value_text,
                      av.value_number, av.value_integer, av.value_boolean, av.value_date,
                      av.value_datetime, av.value_time, av.value_time_zone, av.value_json
                FROM attribute_values av
                JOIN attributes a ON a.id = av.attribute_id
                WHERE av.entity_id = $1
                  AND av.workspace_id = $2
                  AND (av.relationship_target_entity_id IS NULL OR av.active)"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
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

    /// Values at the page cursor's database-clock high-water mark. Historical
    /// values survive writes to later pages; cursors expire before the value
    /// history retention window. All branches retain workspace isolation.
    pub async fn extension_catalog_values_at(
        &self,
        entity_id: Uuid,
        snapshot_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AttributeValue>, RepositoryError> {
        if snapshot_at < chrono::Utc::now() - chrono::Duration::days(30) {
            return Err(RepositoryError::InvalidExtension(
                "catalog snapshot has expired".into(),
            ));
        }
        let rows = sqlx::query_as::<_, CurrentNativeValueRow>(
            r#"SELECT v.id,v.entity_id,v.attribute_id,v.relationship_target_entity_id,v.context_id,v.active,v.created_at,a.value_type,
                v.value_text,v.value_number,v.value_integer,v.value_boolean,v.value_date,v.value_datetime,v.value_time,v.value_time_zone,v.value_json
              FROM attribute_values v JOIN attributes a ON a.id=v.attribute_id AND a.workspace_id=v.workspace_id
              WHERE v.entity_id=$1 AND v.workspace_id=$2 AND v.created_at <= $3
                AND (v.relationship_target_entity_id IS NULL OR v.active)
              UNION ALL
              SELECT h.id,h.entity_id,h.attribute_id,h.relationship_target_entity_id,h.context_id,h.active,h.created_at,a.value_type,
                h.value_text,h.value_number,h.value_integer,h.value_boolean,h.value_date,h.value_datetime,h.value_time,h.value_time_zone,h.value_json
              FROM attribute_value_history h JOIN attributes a ON a.id=h.attribute_id AND a.workspace_id=h.workspace_id
              WHERE h.entity_id=$1 AND h.workspace_id=$2 AND h.created_at <= $3 AND h.archived_at > $3
                AND (h.relationship_target_entity_id IS NULL OR h.active)"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(snapshot_at)
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
                      h.value_date, h.value_datetime, h.value_time, h.value_time_zone, h.value_json
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

    pub async fn value_history_page(
        &self,
        entity_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<AttributeValueHistory>, bool), RepositoryError> {
        let rows = sqlx::query_as::<_, HistoryNativeValueRow>(
            r#"SELECT h.id, h.entity_id, h.attribute_id, h.relationship_target_entity_id,
                      h.active, h.context_id, h.created_at, h.archived_at, a.value_type,
                      h.value_text, h.value_number, h.value_integer, h.value_boolean,
                      h.value_date, h.value_datetime, h.value_time, h.value_time_zone, h.value_json
               FROM attribute_value_history h
               JOIN attributes a ON a.id = h.attribute_id
               WHERE h.entity_id = $1
               ORDER BY h.created_at DESC, h.id DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(entity_id)
        .bind(limit + 1)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let has_more = rows.len() as i64 > limit;
        let items = rows
            .into_iter()
            .take(limit as usize)
            .map(history_attribute_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((items, has_more))
    }

    pub async fn restore_value(
        &self,
        entity_id: Uuid,
        history_id: Uuid,
    ) -> Result<AttributeValue, RepositoryError> {
        self.restore_value_checked(entity_id, history_id, None)
            .await
    }

    pub async fn restore_value_checked(
        &self,
        entity_id: Uuid,
        history_id: Uuid,
        expected_updated_at: Option<DateTime<Utc>>,
    ) -> Result<AttributeValue, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.lock_relationship_cardinality_writes(&mut transaction)
            .await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let before = self
            .entity_audit_snapshot(&mut transaction, entity_id)
            .await?;
        let history = sqlx::query_as::<_, HistoryNativeValueRow>(
            r#"SELECT h.id, h.entity_id, h.attribute_id, h.relationship_target_entity_id,
                      h.active, h.context_id, h.created_at, h.archived_at, a.value_type,
                      h.value_text, h.value_number, h.value_integer, h.value_boolean,
                      h.value_date, h.value_datetime, h.value_time, h.value_time_zone, h.value_json
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

        let selector = NewAttributeValue::Scalar {
            attribute_id: Some(history.attribute_id),
            attribute_code: None,
            context_id: history.context_id,
            value: Value::Null,
        };
        let write = WriteContext::load(&mut transaction, self.workspace_id.0, &entity).await?;
        if expected_updated_at.is_some() || Self::has_status_writes(&write, &[selector], &[]) {
            Self::check_status_precondition(&write, &entity, expected_updated_at)?;
        }
        let value = match history.relationship_target_entity_id {
            Some(target_entity_id) if history.active => {
                self.insert_value_in(
                    &mut transaction,
                    &write,
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
                self.insert_value_in(
                    &mut transaction,
                    &write,
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
        self.validate_entity_schema_in(&mut transaction, &write, &entity, Revalidation::Write)
            .await?;
        let preview = write.preview(&mut transaction, entity.id).await?;
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
