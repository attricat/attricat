use super::entity_projection::display_label;
use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct EntityPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct RelationshipTreeNodeRow {
    id: Uuid,
    preview: Value,
    views: Value,
    context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct RelationshipTreeChildRow {
    id: Uuid,
    preview: Value,
    display: Value,
    context_fallback: Value,
    count: i64,
    has_children: bool,
}

#[derive(sqlx::FromRow)]
struct IncomingRelationshipRow {
    id: Uuid,
    blueprint_code: String,
    blueprint_version: i64,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

impl CatalogRepository {
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
                      b.views AS blueprint_views,
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

    pub async fn incoming_relationships(
        &self,
        entity_id: Uuid,
        relationships: Vec<IncomingRelationshipSelector>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<IncomingRelationshipsPage, RepositoryError> {
        self.get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        for relationship in &relationships {
            validate_code(&relationship.source_blueprint)?;
            validate_code(&relationship.field)?;
        }
        let selectors = serde_json::to_value(relationships)
            .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))?;
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = sqlx::query_as::<_, IncomingRelationshipRow>(
            r#"SELECT source.id, b.code AS blueprint_code, source.blueprint_version,
                      source.created_at, source.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.blueprint_id = source.blueprint_id
                          AND attribute.blueprint_version = source.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
               FROM entities source
               JOIN blueprints b ON b.id = source.blueprint_id AND b.version = source.blueprint_version
               WHERE source.deleted_at IS NULL
                 AND EXISTS (
                     SELECT 1
                     FROM attribute_values av
                     JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                     JOIN LATERAL jsonb_to_recordset($2::jsonb)
                         AS selector(source_blueprint text, field text)
                         ON selector.source_blueprint = b.code AND selector.field = a.code
                     WHERE av.entity_id = source.id
                       AND av.relationship_target_entity_id = $1
                       AND av.active
                       AND a.value_type = 'relationship'
                 )
                 AND ($3::timestamptz IS NULL OR (source.created_at, source.id) > ($3, $4))
               ORDER BY source.created_at, source.id
               LIMIT $5"#,
        )
        .bind(entity_id)
        .bind(selectors)
        .bind(cursor_created_at)
        .bind(cursor_id)
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<_> = rows.into_iter().map(incoming_relationship_item).collect();
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items
                .last()
                .map(|item| encode_search_cursor(item.created_at, item.id))
        } else {
            None
        };
        Ok(IncomingRelationshipsPage {
            items: items
                .into_iter()
                .map(|item| IncomingRelationshipItem {
                    id: item.id,
                    blueprint_code: item.blueprint_code,
                    blueprint_version: item.blueprint_version,
                    display: item.display,
                })
                .collect(),
            next_cursor,
        })
    }

    pub async fn search_entity_previews(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        query: Option<&str>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
        matching_entity_ids: Option<&[Uuid]>,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        let sql = r#"SELECT e.id, e.blueprint_version, e.created_at, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
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
                  AND ($6::uuid[] IS NULL OR e.id = ANY($6))
                 ORDER BY e.created_at, e.id
                 LIMIT $7"#;
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = sqlx::query_as::<_, EntityPreviewRow>(sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(query)
            .bind(cursor_created_at)
            .bind(cursor_id)
            .bind(matching_entity_ids)
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

    pub async fn relationship_tree_facet(
        &self,
        source_blueprint_id: Uuid,
        source_field: &str,
        target_blueprint_id: Uuid,
        hierarchy_field: &str,
        context_id: Uuid,
        selected_target_ids: &[Uuid],
        source_entity_ids: &HashSet<Uuid>,
    ) -> Result<(RelationshipTreeFacetResponse, Option<Vec<Uuid>>), RepositoryError> {
        let context_exists = self.get_context_by_id(context_id).await?.is_some();
        if !context_exists {
            return Err(RepositoryError::InvalidContext);
        }
        let nodes = sqlx::query_as::<_, RelationshipTreeNodeRow>(
            r#"SELECT e.id, e.projections -> 'preview' AS preview, b.views,
                      (SELECT COALESCE(jsonb_object_agg(a.code, a.context_fallback), '{}'::jsonb)
                         FROM attributes a
                        WHERE a.blueprint_id = e.blueprint_id
                          AND a.blueprint_version = e.blueprint_version AND a.deleted_at IS NULL) AS context_fallback
                 FROM entities e JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                WHERE e.blueprint_id = $1 AND e.deleted_at IS NULL
                ORDER BY e.id"#,
        )
        .bind(target_blueprint_id)
        .fetch_all(&self.pool)
        .await?;
        let node_ids: HashSet<_> = nodes.iter().map(|node| node.id).collect();
        if selected_target_ids.iter().any(|id| !node_ids.contains(id)) {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
        }

        let edges = self
            .resolved_relationship_edges(
                source_blueprint_id,
                source_field,
                target_blueprint_id,
                context_id,
            )
            .await?;
        let parents = self
            .resolved_relationship_edges(
                target_blueprint_id,
                hierarchy_field,
                target_blueprint_id,
                context_id,
            )
            .await?;
        let mut parents_by_child: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (child, parent) in parents {
            if node_ids.contains(&child) && node_ids.contains(&parent) {
                parents_by_child.entry(child).or_default().push(parent);
            }
        }
        let mut children_by_parent: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (child, parents) in &parents_by_child {
            for parent in parents {
                children_by_parent.entry(*parent).or_default().push(*child);
            }
        }
        let selected: HashSet<_> = selected_target_ids.iter().copied().collect();
        let mut allowed = selected.clone();
        let mut pending: Vec<_> = selected.into_iter().collect();
        while let Some(parent) = pending.pop() {
            for child in children_by_parent.get(&parent).into_iter().flatten() {
                if allowed.insert(*child) {
                    pending.push(*child);
                }
            }
        }
        let matching_entity_ids = (!selected_target_ids.is_empty()).then(|| {
            edges
                .iter()
                .filter(|(source, target)| {
                    source_entity_ids.contains(source) && allowed.contains(target)
                })
                .map(|(source, _)| *source)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect()
        });
        let mut counts: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
        for (source, target) in edges {
            if !source_entity_ids.contains(&source) {
                continue;
            }
            let mut pending = vec![target];
            let mut visited = HashSet::new();
            while let Some(node) = pending.pop() {
                if !visited.insert(node) {
                    continue;
                }
                counts.entry(node).or_default().insert(source);
                pending.extend(parents_by_child.get(&node).into_iter().flatten().copied());
            }
        }
        let requested_context = self
            .get_context_by_id(context_id)
            .await?
            .expect("context was checked above");
        Ok((
            RelationshipTreeFacetResponse {
                items: nodes
                    .into_iter()
                    .map(|node| RelationshipTreeFacetItem {
                        id: node.id,
                        parent_ids: parents_by_child.remove(&node.id).unwrap_or_default(),
                        display: display_label(
                            &node.preview,
                            &node.views,
                            &node.context_fallback,
                            &requested_context.code,
                        )
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                        count: counts
                            .get(&node.id)
                            .map_or(0, |sources| sources.len() as i64),
                    })
                    .collect(),
            },
            matching_entity_ids,
        ))
    }

    pub async fn relationship_tree_facet_children(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        query: Option<&str>,
        source_field: &str,
        target_blueprint_id: Uuid,
        hierarchy_field: &str,
        context_id: Uuid,
        parent_id: Option<Uuid>,
        cursor: Option<Uuid>,
        limit: i64,
    ) -> Result<RelationshipTreeFacetChildrenResponse, RepositoryError> {
        let requested_context = self
            .get_context_by_id(context_id)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        let rows = sqlx::query_as::<_, RelationshipTreeChildRow>(
            r#"WITH RECURSIVE context_path AS (
                    SELECT id, parent_id, 0 AS depth FROM attribute_contexts WHERE id = $7
                    UNION ALL
                    SELECT c.id, c.parent_id, path.depth + 1
                    FROM attribute_contexts c JOIN context_path path ON c.id = path.parent_id
                ), source_edges AS (
                    SELECT DISTINCT source.id AS source_id, av.relationship_target_entity_id AS target_id
                    FROM entities source
                    JOIN LATERAL (
                        SELECT av.context_id
                        FROM attribute_values av
                        JOIN attributes a ON a.id = av.attribute_id
                         AND a.blueprint_id = source.blueprint_id AND a.blueprint_version = source.blueprint_version
                        JOIN context_path path ON path.id = av.context_id
                        WHERE av.entity_id = source.id AND av.active
                          AND av.relationship_target_entity_id IS NOT NULL
                          AND a.code = $4 AND (path.depth = 0 OR a.context_fallback <> 'none')
                        ORDER BY path.depth LIMIT 1
                    ) chosen ON true
                    JOIN attribute_values av ON av.entity_id = source.id AND av.context_id = chosen.context_id
                    JOIN attributes a ON a.id = av.attribute_id
                     AND a.blueprint_id = source.blueprint_id AND a.blueprint_version = source.blueprint_version
                    JOIN entities target ON target.id = av.relationship_target_entity_id
                    WHERE source.blueprint_id = $1
                      AND ($2::bigint IS NULL OR source.blueprint_version = $2)
                      AND source.deleted_at IS NULL AND target.deleted_at IS NULL
                      AND target.blueprint_id = $5 AND a.code = $4 AND av.active
                      AND av.relationship_target_entity_id IS NOT NULL
                      AND ($3::text IS NULL OR EXISTS (
                          SELECT 1 FROM attribute_values value
                          WHERE value.entity_id = source.id
                            AND value.relationship_target_entity_id IS NULL
                            AND COALESCE(value.value_text, value.value_number::text,
                                value.value_integer::text, value.value_boolean::text,
                                value.value_date::text, value.value_datetime::text,
                                value.value_time::text) ILIKE '%' || $3 || '%'
                      ))
                ), parent_edges AS (
                    SELECT DISTINCT child.id AS child_id, av.relationship_target_entity_id AS parent_id
                    FROM entities child
                    JOIN LATERAL (
                        SELECT av.context_id
                        FROM attribute_values av
                        JOIN attributes a ON a.id = av.attribute_id
                         AND a.blueprint_id = child.blueprint_id AND a.blueprint_version = child.blueprint_version
                        JOIN context_path path ON path.id = av.context_id
                        WHERE av.entity_id = child.id AND av.active
                          AND av.relationship_target_entity_id IS NOT NULL
                          AND a.code = $6 AND (path.depth = 0 OR a.context_fallback <> 'none')
                        ORDER BY path.depth LIMIT 1
                    ) chosen ON true
                    JOIN attribute_values av ON av.entity_id = child.id AND av.context_id = chosen.context_id
                    JOIN attributes a ON a.id = av.attribute_id
                     AND a.blueprint_id = child.blueprint_id AND a.blueprint_version = child.blueprint_version
                    JOIN entities parent ON parent.id = av.relationship_target_entity_id
                    WHERE child.blueprint_id = $5 AND child.deleted_at IS NULL
                      AND parent.blueprint_id = $5 AND parent.deleted_at IS NULL
                      AND a.code = $6 AND av.active AND av.relationship_target_entity_id IS NOT NULL
                ), ancestors AS (
                    SELECT source_id, target_id AS node_id, ARRAY[target_id] AS path FROM source_edges
                    UNION ALL
                    SELECT ancestors.source_id, edges.parent_id, ancestors.path || edges.parent_id
                    FROM ancestors JOIN parent_edges edges ON edges.child_id = ancestors.node_id
                    WHERE NOT edges.parent_id = ANY(ancestors.path)
                ), counts AS (
                    SELECT node_id, COUNT(DISTINCT source_id)::bigint AS count
                    FROM ancestors GROUP BY node_id
                )
                SELECT entity.id, entity.projections -> 'preview' AS preview, blueprint.views AS display,
                       (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                          FROM attributes attribute
                         WHERE attribute.blueprint_id = entity.blueprint_id
                           AND attribute.blueprint_version = entity.blueprint_version
                           AND attribute.deleted_at IS NULL) AS context_fallback,
                       COALESCE(counts.count, 0) AS count,
                       EXISTS (SELECT 1 FROM parent_edges edge WHERE edge.parent_id = entity.id) AS has_children
                FROM entities entity
                JOIN blueprints blueprint ON blueprint.id = entity.blueprint_id AND blueprint.version = entity.blueprint_version
                LEFT JOIN counts ON counts.node_id = entity.id
                WHERE entity.blueprint_id = $5 AND entity.deleted_at IS NULL
                  AND ($8::uuid IS NULL
                       AND NOT EXISTS (SELECT 1 FROM parent_edges edge WHERE edge.child_id = entity.id)
                       OR $8::uuid IS NOT NULL AND EXISTS (
                           SELECT 1 FROM parent_edges edge
                           WHERE edge.child_id = entity.id AND edge.parent_id = $8
                       ))
                  AND ($9::uuid IS NULL OR entity.id > $9)
                ORDER BY entity.id LIMIT $10"#,
        )
        .bind(source_blueprint_id)
        .bind(source_blueprint_version)
        .bind(query)
        .bind(source_field)
        .bind(target_blueprint_id)
        .bind(hierarchy_field)
        .bind(context_id)
        .bind(parent_id)
        .bind(cursor)
        .bind(limit + 1)
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<_> = rows
            .into_iter()
            .map(|row| RelationshipTreeFacetChildItem {
                id: row.id,
                display: display_label(
                    &row.preview,
                    &row.display,
                    &row.context_fallback,
                    &requested_context.code,
                )
                .as_str()
                .unwrap_or_default()
                .to_owned(),
                count: row.count,
                has_children: row.has_children,
            })
            .collect();
        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|item| item.id)
        } else {
            None
        };
        Ok(RelationshipTreeFacetChildrenResponse { items, next_cursor })
    }

    async fn resolved_relationship_edges(
        &self,
        source_blueprint_id: Uuid,
        field: &str,
        target_blueprint_id: Uuid,
        context_id: Uuid,
    ) -> Result<Vec<(Uuid, Uuid)>, RepositoryError> {
        Ok(sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"WITH RECURSIVE context_path AS (
                    SELECT id, parent_id, 0 AS depth FROM attribute_contexts WHERE id = $4
                    UNION ALL
                    SELECT c.id, c.parent_id, path.depth + 1
                    FROM attribute_contexts c JOIN context_path path ON c.id = path.parent_id
                )
                SELECT DISTINCT source.id, av.relationship_target_entity_id
                FROM entities source
                JOIN LATERAL (
                    SELECT av.context_id
                    FROM attribute_values av
                    JOIN attributes a ON a.id = av.attribute_id
                     AND a.blueprint_id = source.blueprint_id AND a.blueprint_version = source.blueprint_version
                    JOIN context_path path ON path.id = av.context_id
                    WHERE av.entity_id = source.id AND av.active
                      AND av.relationship_target_entity_id IS NOT NULL
                      AND a.code = $2 AND (path.depth = 0 OR a.context_fallback <> 'none')
                    ORDER BY path.depth LIMIT 1
                ) chosen ON true
                JOIN attribute_values av ON av.entity_id = source.id AND av.context_id = chosen.context_id
                JOIN attributes a ON a.id = av.attribute_id
                 AND a.blueprint_id = source.blueprint_id AND a.blueprint_version = source.blueprint_version
                JOIN entities target ON target.id = av.relationship_target_entity_id
                WHERE source.blueprint_id = $1 AND source.deleted_at IS NULL
                  AND a.code = $2 AND av.active AND av.relationship_target_entity_id IS NOT NULL
                  AND target.blueprint_id = $3 AND target.deleted_at IS NULL"#,
        )
        .bind(source_blueprint_id)
        .bind(field)
        .bind(target_blueprint_id)
        .bind(context_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn search_matching_entity_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        query: Option<&str>,
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        let ids = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT e.id
                 FROM entities e
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
                  )"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(query)
        .fetch_all(&self.pool)
        .await?;
        Ok(ids.into_iter().collect())
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

pub(super) fn empty_preview() -> Value {
    Value::Object(Map::from_iter([(
        String::from("default"),
        Value::Object(Map::new()),
    )]))
}

pub(super) fn empty_projections() -> Value {
    Value::Object(Map::from_iter([(String::from("preview"), empty_preview())]))
}

fn entity_preview(row: EntityPreviewRow) -> EntityPreview {
    EntityPreview {
        id: row.id,
        blueprint_version: row.blueprint_version,
        schema_outdated: false,
        created_at: row.created_at,
        display: super::entity_projection::display_labels(
            &row.preview,
            &row.blueprint_views,
            &row.blueprint_context_fallback,
        ),
        preview: row.preview,
    }
}

struct IncomingRelationshipPreview {
    id: Uuid,
    blueprint_code: String,
    blueprint_version: i64,
    created_at: DateTime<Utc>,
    display: Value,
}

fn incoming_relationship_item(row: IncomingRelationshipRow) -> IncomingRelationshipPreview {
    IncomingRelationshipPreview {
        id: row.id,
        blueprint_code: row.blueprint_code,
        blueprint_version: row.blueprint_version,
        created_at: row.created_at,
        display: super::entity_projection::display_labels(
            &row.preview,
            &row.blueprint_views,
            &row.blueprint_context_fallback,
        ),
    }
}
