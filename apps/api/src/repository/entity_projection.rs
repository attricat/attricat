use super::values::{ProjectionNativeValueRow, native_value_json};
use super::*;
use crate::constants::DEFAULT_PREVIEW_RELATIONSHIP_ITEMS;
use async_recursion::async_recursion;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct PreviewRelationship {
    attribute_code: String,
    context_code: Option<String>,
    target_id: Uuid,
    target_projections: Value,
    target_views: Value,
    target_context_fallback: Value,
    relationship_position: i64,
}

impl CatalogRepository {
    pub async fn resolved_preview(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
        relationship_depth: u8,
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
        let mut values = attributes
            .iter()
            .filter(|attribute| attribute.value_type != "file")
            .filter_map(|attribute| {
                path.iter().enumerate().find_map(|(index, context)| {
                    if index > 0 && attribute.context_fallback == "none" {
                        return None;
                    }
                    preview
                        .get(&context.code)
                        .and_then(Value::as_object)
                        .and_then(|values| values.get(&attribute.code))
                        .map(|value| {
                            (
                                attribute.code.clone(),
                                serde_json::json!({ "value": value, "source_context": { "id": context.id, "code": context.code } }),
                            )
                        })
                })
            })
            .collect::<Map<_, _>>();
        let file_values = self.file_form_values(entity_id).await?;
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.value_type == "file")
        {
            if let Some((files, context)) = path.iter().enumerate().find_map(|(index, context)| {
                if index > 0 && attribute.context_fallback == "none" {
                    return None;
                }
                file_values.iter().find_map(|value| match value {
                    FormAttributeValue::File {
                        attribute_code,
                        context_id,
                        files,
                    } if attribute_code == &attribute.code && *context_id == Some(context.id) => {
                        Some((files, context))
                    }
                    _ => None,
                })
            }) {
                values.insert(
                    attribute.code.clone(),
                    serde_json::json!({
                        "value": files,
                        "source_context": { "id": context.id, "code": context.code },
                    }),
                );
            }
        }
        let enriched_preview = self
            .build_preview(
                entity.id,
                &entity.projections,
                relationship_depth,
                DEFAULT_PREVIEW_RELATIONSHIP_ITEMS.into(),
                &mut HashSet::new(),
            )
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
            entity: EntityIdentity {
                id: entity.id,
                blueprint_id: entity.blueprint_id,
                blueprint_version: entity.blueprint_version,
            },
            requested_context,
            values: Value::Object(values),
        }))
    }

    pub async fn hierarchy(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
        field: &str,
        relationship_depth: u8,
    ) -> Result<Option<EntityHierarchyResponse>, RepositoryError> {
        let Some(entity) = self.get_entity(entity_id).await? else {
            return Ok(None);
        };
        let blueprint = self
            .get_blueprint_revision(entity.blueprint_id, entity.blueprint_version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint version"))?;
        let attribute = blueprint
            .attributes
            .iter()
            .find(|attribute| attribute.code == field)
            .ok_or(RepositoryError::AttributeNotApplicable)?;
        if attribute.value_type != "relationship"
            || attribute.target_blueprint_code.as_deref() != Some(&blueprint.blueprint.code)
        {
            return Err(RepositoryError::InvalidHierarchyRelationship);
        }

        let resolved = self
            .resolved_preview(entity_id, context_id, relationship_depth)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let requested_context = resolved.requested_context.code;
        let current_display = display_label(
            entity.projections.get("preview").unwrap_or(&Value::Null),
            &blueprint.blueprint.views,
            &serde_json::json!({}),
            &requested_context,
        )
        .as_str()
        .unwrap_or_default()
        .to_owned();

        let items = vec![EntityHierarchyItem {
            id: entity.id,
            display: current_display,
        }];
        let relationship = resolved
            .values
            .get(field)
            .and_then(|value| value.get("value"));
        let mut multiple_parents = false;
        let mut truncated = false;
        let mut cycle_detected = false;
        let mut paths = Vec::new();
        collect_hierarchy_paths(
            relationship,
            field,
            items.clone(),
            HashSet::from([entity.id]),
            relationship_depth,
            &mut paths,
            &mut multiple_parents,
            &mut truncated,
            &mut cycle_detected,
        );
        let default_path = paths.first().cloned().unwrap_or(items);
        Ok(Some(EntityHierarchyResponse {
            items: default_path,
            paths,
            truncated,
            multiple_parents,
            cycle_detected,
        }))
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
                            target.projections AS target_projections, b.views AS target_views,
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
                       AND av.active
                )
               SELECT attribute_code, context_code, target_id, target_projections, target_views, target_context_fallback, relationship_position
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
                    &relationship.target_views,
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
    pub(super) async fn store_preview(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        preview: Value,
    ) -> Result<Entity, RepositoryError> {
        Ok(sqlx::query_as::<_, Entity>(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1
                RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, created_at, updated_at, deleted_at"#,
        )
        .bind(entity_id)
        .bind(preview)
        .fetch_one(&mut **transaction)
        .await?)
    }

    pub(super) async fn build_preview_projection(
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
}
const MAX_HIERARCHY_PATHS: usize = 20;

#[allow(clippy::too_many_arguments)]
fn collect_hierarchy_paths(
    relationship: Option<&Value>,
    field: &str,
    path: Vec<EntityHierarchyItem>,
    visited: HashSet<Uuid>,
    remaining_depth: u8,
    paths: &mut Vec<Vec<EntityHierarchyItem>>,
    multiple_parents: &mut bool,
    truncated: &mut bool,
    cycle_detected: &mut bool,
) {
    if paths.len() == MAX_HIERARCHY_PATHS {
        *truncated = true;
        return;
    }
    let Some(relationship) = relationship else {
        let mut path = path;
        path.reverse();
        paths.push(path);
        return;
    };
    if relationship.get("truncated").and_then(Value::as_bool) == Some(true) {
        *truncated = true;
    }
    let Some(targets) = relationship.get("items").and_then(Value::as_array) else {
        let mut path = path;
        path.reverse();
        paths.push(path);
        return;
    };
    if targets.len() > 1 {
        *multiple_parents = true;
    }
    if targets.is_empty() {
        let mut path = path;
        path.reverse();
        paths.push(path);
        return;
    }
    if remaining_depth == 0 {
        *truncated = true;
        let mut path = path;
        path.reverse();
        paths.push(path);
        return;
    }

    for target in targets {
        let Some(id) = target
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            *truncated = true;
            continue;
        };
        if visited.contains(&id) {
            *cycle_detected = true;
            let mut path = path.clone();
            path.reverse();
            paths.push(path);
            continue;
        }
        let mut next_path = path.clone();
        next_path.push(EntityHierarchyItem {
            id,
            display: target
                .get("display")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        });
        let mut next_visited = visited.clone();
        next_visited.insert(id);
        collect_hierarchy_paths(
            target.get(field),
            field,
            next_path,
            next_visited,
            remaining_depth - 1,
            paths,
            multiple_parents,
            truncated,
            cycle_detected,
        );
    }
}

pub(super) fn display_labels(preview: &Value, views: &Value, context_fallback: &Value) -> Value {
    let contexts = preview.as_object().cloned().unwrap_or_default();
    Value::Object(
        contexts
            .keys()
            .map(|context| {
                (
                    context.clone(),
                    display_label(preview, views, context_fallback, context),
                )
            })
            .collect(),
    )
}

pub(super) fn display_label(
    preview: &Value,
    views: &Value,
    context_fallback: &Value,
    context_code: &str,
) -> Value {
    let Some(definition) = views.get("dropdown_option") else {
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

fn display_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
