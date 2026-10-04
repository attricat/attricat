use super::record_values::{ContextNode, ContextTree, resolve_on_path};
use super::values::{ProjectionNativeValueRow, native_value_json};
use super::*;
use crate::constants::DEFAULT_PREVIEW_RELATIONSHIP_ITEMS;
use crate::persistence_rows::{Db, IntoDomain};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// One value source's direct value in a context.
type DirectLookup<'a> = Box<dyn FnMut(&ContextNode) -> Option<Value> + 'a>;

#[derive(sqlx::FromRow)]
struct PreviewRelationship {
    source_id: Uuid,
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
        let attributes = self
            .list_attributes(entity.blueprint_id, entity.blueprint_version)
            .await?;
        self.resolved_preview_for(&entity, &attributes, context_id, relationship_depth)
            .await
            .map(Some)
    }

    /// [`Self::resolved_preview`] for an entity and attributes the caller
    /// already loaded.
    async fn resolved_preview_for(
        &self,
        entity: &Entity,
        attributes: &[Attribute],
        context_id: Uuid,
        relationship_depth: u8,
    ) -> Result<ResolvedEntityPreviewResponse, RepositoryError> {
        let entity_id = entity.id;
        let requested_context = self
            .get_context_by_id(context_id)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        let preview = entity
            .projections
            .get("preview")
            .and_then(Value::as_object)
            .ok_or(RepositoryError::InvalidPreview)?;
        let tree = {
            let mut connection = self.pool.acquire().await?;
            ContextTree::load(&mut connection, self.workspace_id.0).await?
        };
        let path: Vec<ContextNode> = tree
            .path(requested_context.id, true)?
            .into_iter()
            .filter_map(|id| tree.get(id).cloned())
            .collect();
        let ids: Vec<Uuid> = path.iter().map(|context| context.id).collect();
        // The shared resolution rule, applied to this read's value sources.
        let resolve = |inherit: bool, mut direct: DirectLookup<'_>| {
            resolve_on_path(&ids, inherit, |id| {
                path.iter()
                    .find(|context| context.id == id)
                    .and_then(|context| direct(context).map(|value| (context.id, context.code.clone(), value)))
            })
            .map(|(_, (id, code, value))| {
                serde_json::json!({ "value": value, "source_context": { "id": id, "code": code } })
            })
        };
        let mut values = attributes
            .iter()
            .filter(|attribute| attribute.value_type != "file")
            .filter_map(|attribute| {
                resolve(
                    attribute.context_fallback != "none",
                    Box::new(|context| {
                        preview
                            .get(&context.code)
                            .and_then(Value::as_object)
                            .and_then(|values| values.get(&attribute.code))
                            .cloned()
                    }),
                )
                .map(|value| (attribute.code.clone(), value))
            })
            .collect::<Map<_, _>>();
        let file_values = self.file_form_values(entity_id).await?;
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.value_type == "file")
        {
            if let Some(value) = resolve(
                attribute.context_fallback != "none",
                Box::new(|context| {
                    file_values.iter().find_map(|value| match value {
                        FormAttributeValue::File {
                            attribute_code,
                            context_id,
                            files,
                        } if attribute_code == &attribute.code
                            && *context_id == Some(context.id) =>
                        {
                            Some(serde_json::json!(files))
                        }
                        _ => None,
                    })
                }),
            ) {
                values.insert(attribute.code.clone(), value);
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
                resolve(
                    attribute.context_fallback != "none",
                    Box::new(|context| {
                        enriched_preview
                            .get(&context.code)
                            .and_then(Value::as_object)
                            .and_then(|values| values.get(&attribute.code))
                            .filter(|value| value.get("items").is_some())
                            .cloned()
                    }),
                )
                .map(|value| (attribute.code.clone(), value))
            })
            .collect::<Map<_, _>>();
        values.extend(relationships);
        let reusable_attributes = self.entity_reusable_attributes(entity.id).await?;
        let reusable_form_values = self.reusable_form_values(entity.id).await?;
        let reusable_values = reusable_attributes
            .iter()
            .filter_map(|attribute| {
                resolve(
                    attribute.context_fallback != "none",
                    Box::new(|context| {
                        reusable_form_values.iter().find_map(|value| match value {
                            FormAttributeValue::Scalar {
                                attribute_code,
                                context_id,
                                value,
                            } if attribute_code == &attribute.code
                                && *context_id == Some(context.id) =>
                            {
                                Some(value.clone())
                            }
                            FormAttributeValue::Relationship {
                                attribute_code,
                                context_id,
                                target_entity_id,
                            } if attribute_code == &attribute.code
                                && *context_id == Some(context.id) =>
                            {
                                Some(serde_json::json!({ "items": [{ "id": target_entity_id }] }))
                            }
                            FormAttributeValue::File {
                                attribute_code,
                                context_id,
                                files,
                            } if attribute_code == &attribute.code
                                && *context_id == Some(context.id) =>
                            {
                                Some(serde_json::json!(files))
                            }
                            _ => None,
                        })
                    }),
                )
                .map(|value| (attribute.code.clone(), value))
            })
            .collect::<Map<_, _>>();
        Ok(ResolvedEntityPreviewResponse {
            entity: EntityIdentity {
                id: entity.id,
                blueprint_id: entity.blueprint_id,
                blueprint_version: entity.blueprint_version,
                is_sample: entity.is_sample,
            },
            requested_context,
            values: Value::Object(values),
            reusable_attributes,
            reusable_values: Value::Object(reusable_values),
        })
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
            .resolved_preview_for(
                &entity,
                &blueprint.attributes,
                context_id,
                relationship_depth,
            )
            .await?;
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
    /// The entity and its preview with related entities expanded.
    pub async fn preview(
        &self,
        entity_id: Uuid,
        relationship_depth: u8,
        relationship_limit: i64,
    ) -> Result<Option<(Entity, Value)>, RepositoryError> {
        let Some(entity) = self.get_entity(entity_id).await? else {
            return Ok(None);
        };
        let preview = self
            .build_preview(
                entity.id,
                &entity.projections,
                relationship_depth,
                relationship_limit,
                &mut HashSet::new(),
            )
            .await?;
        Ok(Some((entity, preview)))
    }

    /// Expands `projections`' preview with related entities up to
    /// `relationship_depth` hops. Relationship rows are fetched breadth first,
    /// one query per depth for every entity reached at it; the preview is
    /// then assembled in memory. An entity already on the expansion path
    /// (`path`) is not expanded again.
    async fn build_preview(
        &self,
        entity_id: Uuid,
        projections: &Value,
        relationship_depth: u8,
        relationship_limit: i64,
        path: &mut HashSet<Uuid>,
    ) -> Result<Value, RepositoryError> {
        let mut rows: HashMap<(Uuid, u8), Vec<PreviewRelationship>> = HashMap::new();
        let mut frontier = vec![(entity_id, path.iter().copied().collect::<Vec<_>>())];
        let mut depth = relationship_depth;
        while !frontier.is_empty() {
            let expanding: Vec<(Uuid, Vec<Uuid>)> = frontier
                .into_iter()
                .filter(|(id, ancestors)| !ancestors.contains(id))
                .collect();
            let ids: Vec<Uuid> = expanding
                .iter()
                .map(|(id, _)| *id)
                .filter(|id| !rows.contains_key(&(*id, depth)))
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            if !ids.is_empty() {
                // Fetch one extra edge to report truncation without a separate
                // count. At depth zero one edge is still enough to indicate
                // that data exists.
                let fetch_limit = if depth == 0 {
                    1
                } else {
                    relationship_limit + 1
                };
                for id in &ids {
                    rows.entry((*id, depth)).or_default();
                }
                for row in self.preview_relationships(&ids, fetch_limit).await? {
                    rows.get_mut(&(row.source_id, depth))
                        .expect("every fetched source was requested")
                        .push(row);
                }
            }
            if depth == 0 {
                break;
            }
            frontier = expanding
                .iter()
                .flat_map(|(id, ancestors)| {
                    let rows = &rows[&(*id, depth)];
                    rows.iter()
                        .filter(|row| row.relationship_position <= relationship_limit)
                        .map(|row| {
                            let mut ancestors = ancestors.clone();
                            ancestors.push(*id);
                            (row.target_id, ancestors)
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            depth -= 1;
        }
        assemble_preview(
            entity_id,
            projections,
            relationship_depth,
            relationship_limit,
            path,
            &rows,
        )
    }

    /// Relationship edges of every source in `entity_ids`, at most `limit`
    /// per source, attribute and context.
    async fn preview_relationships(
        &self,
        entity_ids: &[Uuid],
        limit: i64,
    ) -> Result<Vec<PreviewRelationship>, RepositoryError> {
        Ok(sqlx::query_as::<_, PreviewRelationship>(
            r#"WITH relationships AS (
                    SELECT av.entity_id AS source_id, a.code AS attribute_code, c.code AS context_code,
                           target.id AS target_id, target.projections AS target_projections,
                           b.views AS target_views, target.blueprint_id AS target_blueprint_id,
                           target.blueprint_version AS target_blueprint_version,
                           ROW_NUMBER() OVER (PARTITION BY av.entity_id, a.id, av.context_id ORDER BY target.id)
                               AS relationship_position
                    FROM attribute_values av
                    JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                    JOIN entities target ON target.id = av.relationship_target_entity_id AND target.deleted_at IS NULL
                    JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
                    LEFT JOIN attribute_contexts c ON c.id = av.context_id
                    WHERE av.entity_id = ANY($1)
                      AND av.relationship_target_entity_id IS NOT NULL
                      AND av.active
                ), selected AS (
                    SELECT * FROM relationships WHERE relationship_position <= $2
                ), fallbacks AS (
                    -- Context fallback per attribute, once per distinct target revision.
                    SELECT revision.blueprint_id, revision.blueprint_version,
                           COALESCE(
                               jsonb_object_agg(attribute.code, attribute.context_fallback)
                                   FILTER (WHERE attribute.code IS NOT NULL),
                               '{}'::jsonb
                           ) AS context_fallback
                    FROM (SELECT DISTINCT target_blueprint_id AS blueprint_id,
                                 target_blueprint_version AS blueprint_version
                          FROM selected) revision
                    LEFT JOIN attributes attribute ON attribute.blueprint_id = revision.blueprint_id
                     AND attribute.blueprint_version = revision.blueprint_version
                     AND attribute.deleted_at IS NULL
                    GROUP BY revision.blueprint_id, revision.blueprint_version
                )
               SELECT selected.source_id, selected.attribute_code, selected.context_code,
                      selected.target_id, selected.target_projections, selected.target_views,
                      fallbacks.context_fallback AS target_context_fallback,
                      selected.relationship_position
               FROM selected
               JOIN fallbacks ON fallbacks.blueprint_id = selected.target_blueprint_id
                AND fallbacks.blueprint_version = selected.target_blueprint_version
               ORDER BY selected.source_id, selected.attribute_code,
                        selected.context_code NULLS FIRST, selected.relationship_position"#,
        )
        .bind(entity_ids)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub(super) async fn store_preview(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        preview: Value,
    ) -> Result<Entity, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<Entity>>(
            r#"UPDATE entities
               SET projections = jsonb_set(projections, '{preview}', $2, true), updated_at = now()
               WHERE id = $1
                RETURNING id, blueprint_id, blueprint_version, projections, system_tags, system_metadata, ('attricat.sample'=ANY(system_tags)) AS is_sample, created_at, updated_at, deleted_at"#,
        )
        .bind(entity_id)
        .bind(preview)
        .fetch_one(&mut **transaction)
        .await?
        .into_domain())
    }

    pub(super) async fn build_preview_projection(
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<Value, RepositoryError> {
        let values = sqlx::query_as::<_, ProjectionNativeValueRow>(
            r#"SELECT a.code AS attribute_code, c.code AS context_code,
                      a.value_type, av.value_text, av.value_number, av.value_integer,
                      av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                      av.value_time_zone, av.value_json
                FROM attribute_values av
                JOIN entities e ON e.id = av.entity_id
                JOIN attributes a ON a.id = av.attribute_id
                 AND ((a.blueprint_id = e.blueprint_id
                 AND a.blueprint_version = e.blueprint_version)
                 OR (a.entity_id = e.id AND a.value_schema ?| ARRAY['x-attricat-status', 'x-attricat-principal']))
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

/// Builds one entity's expanded preview from prefetched relationship rows,
/// following exactly the rules the per-entity recursion applied.
fn assemble_preview(
    entity_id: Uuid,
    projections: &Value,
    relationship_depth: u8,
    relationship_limit: i64,
    path: &mut HashSet<Uuid>,
    rows: &HashMap<(Uuid, u8), Vec<PreviewRelationship>>,
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
    let relationships = rows
        .get(&(entity_id, relationship_depth))
        .map(Vec::as_slice)
        .unwrap_or_default();
    for relationship in relationships {
        let context_code = relationship
            .context_code
            .clone()
            .unwrap_or_else(|| "default".to_owned());
        let context = preview
            .as_object_mut()
            .expect("preview was validated as an object")
            .entry(context_code.clone())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or(RepositoryError::InvalidPreview)?;
        let relationship_preview = context
            .entry(relationship.attribute_code.clone())
            .or_insert_with(|| serde_json::json!({ "items": [], "truncated": false }))
            .as_object_mut()
            .ok_or(RepositoryError::InvalidPreview)?;
        if relationship_depth == 0 || relationship.relationship_position > relationship_limit {
            relationship_preview.insert("truncated".to_owned(), Value::Bool(true));
            continue;
        }

        let target_preview = assemble_preview(
            relationship.target_id,
            &relationship.target_projections,
            relationship_depth - 1,
            relationship_limit,
            path,
            rows,
        )?;
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
