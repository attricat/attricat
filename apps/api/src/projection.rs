use serde_json::{Map, Value};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::repository::RepositoryError;

pub(crate) struct PreviewProjectionBuilder;

#[derive(FromRow)]
struct PreviewValue {
    attribute_code: String,
    context_code: Option<String>,
    value: Value,
}

impl PreviewProjectionBuilder {
    pub(crate) async fn build(
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Value, RepositoryError> {
        let values = sqlx::query_as::<_, PreviewValue>(
            r#"SELECT a.code AS attribute_code,
                    c.code AS context_code,
                    COALESCE(CASE a.value_type
                      WHEN 'string' THEN to_jsonb(av.value_text)
                      WHEN 'number' THEN to_jsonb(av.value_number)
                      WHEN 'integer' THEN to_jsonb(av.value_integer)
                      WHEN 'boolean' THEN to_jsonb(av.value_boolean)
                      WHEN 'date' THEN to_jsonb(av.value_date::text)
                      WHEN 'datetime' THEN to_jsonb(av.value_datetime)
                      WHEN 'time' THEN jsonb_build_object('time', av.value_time::text, 'time_zone', av.value_time_zone)
                    END, 'null'::jsonb) AS value
                FROM attribute_values av
               JOIN entities e ON e.id = av.entity_id
               JOIN attributes a
                 ON a.id = av.attribute_id
                AND a.blueprint_id = e.blueprint_id
                AND a.blueprint_version = e.blueprint_version
               LEFT JOIN attribute_contexts c ON c.id = av.context_id
                WHERE av.entity_id = $1
                  AND av.relationship_target_entity_id IS NULL
                  AND av.latest
                  AND a.deleted_at IS NULL
                ORDER BY a.position, c.code NULLS FIRST"#,
        )
        .bind(entity_id)
        .fetch_all(&mut **transaction)
        .await?;

        // Keep context-free values under an explicit key. This gives consumers
        // a stable fallback without treating a missing context as special.
        let mut default = Map::new();
        let mut contexts = Map::new();

        for value in values {
            match value.context_code {
                Some(context_code) => {
                    // `default` denotes the absence of a context, so allowing
                    // a stored context with that code would make reads ambiguous.
                    if context_code == "default" {
                        return Err(RepositoryError::ReservedContextCode);
                    }

                    let context = contexts
                        .entry(context_code)
                        .or_insert_with(|| Value::Object(Map::new()));
                    context
                        .as_object_mut()
                        .expect("projection contexts are objects")
                        .insert(value.attribute_code, value.value);
                }
                None => {
                    default.insert(value.attribute_code, value.value);
                }
            }
        }

        let mut preview = Map::new();
        preview.insert("default".to_owned(), Value::Object(default));
        preview.extend(contexts);

        Ok(Value::Object(preview))
    }
}
