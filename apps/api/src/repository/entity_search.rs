use super::*;
use super::{entity_commands::validate_system_tags, entity_projection::display_label};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet, VecDeque};
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
struct SortedEntityPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
    sort_value: Option<String>,
    sort_is_null: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EntitySearchSort {
    pub field: String,
    pub relationship: Option<String>,
    pub value_type: String,
    pub descending: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct EntitySearchFilter {
    pub field: String,
    pub operator: String,
    pub value_type: String,
    pub value: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct SortedSearchCursor {
    version: u8,
    field: String,
    descending: bool,
    is_null: bool,
    value: Option<String>,
    id: Uuid,
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
struct RelatedTablePreviewRow {
    source_id: Uuid,
    attribute_code: String,
    relationship_context_id: Uuid,
    relationship_context_code: String,
    id: Uuid,
    blueprint_id: Uuid,
    blueprint_version: i64,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
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

    /// Resolves entities which satisfy every typed scalar filter in the default context.
    pub(crate) async fn filter_entity_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        filters: &[EntitySearchFilter],
    ) -> Result<Vec<Uuid>, RepositoryError> {
        if filters.is_empty() {
            return Ok(Vec::new());
        }
        let filters = serde_json::to_value(filters)
            .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))?;
        Ok(sqlx::query_scalar::<_, Uuid>(
            r#"SELECT e.id
                 FROM entities e
                WHERE e.blueprint_id = $1
                  AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.deleted_at IS NULL
                  AND NOT EXISTS (
                      SELECT 1
                        FROM jsonb_to_recordset($3::jsonb)
                          AS criterion(field text, operator text, value_type text, value text)
                       WHERE NOT EXISTS (
                           SELECT 1
                             FROM attribute_values av
                             JOIN attributes a ON a.id = av.attribute_id
                              AND a.blueprint_id = e.blueprint_id
                              AND a.blueprint_version = e.blueprint_version
                              AND a.deleted_at IS NULL
                            WHERE av.entity_id = e.id
                              AND av.active
                              AND av.relationship_target_entity_id IS NULL
                              AND av.context_id = (SELECT id FROM attribute_contexts WHERE code = 'default')
                              AND a.code = criterion.field
                              AND a.value_type = criterion.value_type
                              AND CASE
                                  WHEN criterion.value_type = 'string' AND criterion.operator = 'eq'
                                      THEN av.value_text = criterion.value
                                  WHEN criterion.value_type = 'string' AND criterion.operator = 'contains'
                                      THEN strpos(lower(av.value_text), lower(criterion.value)) > 0
                                  WHEN criterion.value_type = 'string' AND criterion.operator = 'starts_with'
                                      THEN left(lower(av.value_text), char_length(criterion.value)) = lower(criterion.value)
                                  WHEN criterion.value_type = 'number' AND criterion.operator = 'eq'
                                      THEN av.value_number = criterion.value::numeric
                                  WHEN criterion.value_type = 'number' AND criterion.operator = 'gt'
                                      THEN av.value_number > criterion.value::numeric
                                  WHEN criterion.value_type = 'number' AND criterion.operator = 'gte'
                                      THEN av.value_number >= criterion.value::numeric
                                  WHEN criterion.value_type = 'number' AND criterion.operator = 'lt'
                                      THEN av.value_number < criterion.value::numeric
                                  WHEN criterion.value_type = 'number' AND criterion.operator = 'lte'
                                      THEN av.value_number <= criterion.value::numeric
                                  WHEN criterion.value_type = 'integer' AND criterion.operator = 'eq'
                                      THEN av.value_integer = criterion.value::bigint
                                  WHEN criterion.value_type = 'integer' AND criterion.operator = 'gt'
                                      THEN av.value_integer > criterion.value::bigint
                                  WHEN criterion.value_type = 'integer' AND criterion.operator = 'gte'
                                      THEN av.value_integer >= criterion.value::bigint
                                  WHEN criterion.value_type = 'integer' AND criterion.operator = 'lt'
                                      THEN av.value_integer < criterion.value::bigint
                                  WHEN criterion.value_type = 'integer' AND criterion.operator = 'lte'
                                      THEN av.value_integer <= criterion.value::bigint
                                  WHEN criterion.value_type = 'boolean' AND criterion.operator = 'eq'
                                      THEN av.value_boolean = criterion.value::boolean
                                  WHEN criterion.value_type = 'date' AND criterion.operator = 'eq'
                                      THEN av.value_date = criterion.value::date
                                  WHEN criterion.value_type = 'date' AND criterion.operator = 'gt'
                                      THEN av.value_date > criterion.value::date
                                  WHEN criterion.value_type = 'date' AND criterion.operator = 'gte'
                                      THEN av.value_date >= criterion.value::date
                                  WHEN criterion.value_type = 'date' AND criterion.operator = 'lt'
                                      THEN av.value_date < criterion.value::date
                                  WHEN criterion.value_type = 'date' AND criterion.operator = 'lte'
                                      THEN av.value_date <= criterion.value::date
                                  WHEN criterion.value_type = 'datetime' AND criterion.operator = 'eq'
                                      THEN av.value_datetime = criterion.value::timestamptz
                                  WHEN criterion.value_type = 'datetime' AND criterion.operator = 'gt'
                                      THEN av.value_datetime > criterion.value::timestamptz
                                  WHEN criterion.value_type = 'datetime' AND criterion.operator = 'gte'
                                      THEN av.value_datetime >= criterion.value::timestamptz
                                  WHEN criterion.value_type = 'datetime' AND criterion.operator = 'lt'
                                      THEN av.value_datetime < criterion.value::timestamptz
                                  WHEN criterion.value_type = 'datetime' AND criterion.operator = 'lte'
                                      THEN av.value_datetime <= criterion.value::timestamptz
                                  WHEN criterion.value_type = 'time' AND criterion.operator = 'eq'
                                      THEN av.value_time = criterion.value::time
                                  WHEN criterion.value_type = 'time' AND criterion.operator = 'gt'
                                      THEN av.value_time > criterion.value::time
                                  WHEN criterion.value_type = 'time' AND criterion.operator = 'gte'
                                      THEN av.value_time >= criterion.value::time
                                  WHEN criterion.value_type = 'time' AND criterion.operator = 'lt'
                                      THEN av.value_time < criterion.value::time
                                  WHEN criterion.value_type = 'time' AND criterion.operator = 'lte'
                                      THEN av.value_time <= criterion.value::time
                                  ELSE FALSE
                              END
                       )
                  )"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(filters)
        .fetch_all(&self.pool)
        .await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn search_entity_previews(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        query: Option<&str>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        validate_system_tags(system_tags)?;
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
                  AND ($7::text[] IS NULL OR e.system_tags @> $7)
                  AND (NOT $8 OR e.blueprint_version <> $9)
                 ORDER BY e.created_at, e.id
                 LIMIT $10"#;
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = sqlx::query_as::<_, EntityPreviewRow>(sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(query)
            .bind(cursor_created_at)
            .bind(cursor_id)
            .bind(matching_entity_ids)
            .bind((!system_tags.is_empty()).then_some(system_tags))
            .bind(outdated)
            .bind(current_blueprint_version)
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

    /// Counts matching entities up to `limit`, so broad searches never scan the entire catalog.
    #[allow(clippy::too_many_arguments)]
    pub async fn count_entity_previews(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
        limit: i64,
    ) -> Result<i64, RepositoryError> {
        validate_system_tags(system_tags)?;
        Ok(sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)
                 FROM (
                    SELECT 1
                      FROM entities e
                     WHERE e.blueprint_id = $1
                       AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                       AND e.deleted_at IS NULL
                       AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                       AND ($4::text[] IS NULL OR e.system_tags @> $4)
                       AND (NOT $5 OR e.blueprint_version <> $6)
                     LIMIT $7
                 ) matches"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(matching_entity_ids)
        .bind((!system_tags.is_empty()).then_some(system_tags))
        .bind(outdated)
        .bind(current_blueprint_version)
        .bind(limit)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Selects a page by a configured scalar table column without hydrating projections for
    /// every candidate. NULL values are always last and entity IDs make the ordering stable.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn search_entity_previews_sorted(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        limit: i64,
        cursor: Option<&str>,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
        sort: &EntitySearchSort,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        validate_system_tags(system_tags)?;
        let cursor = cursor.map(decode_sorted_search_cursor).transpose()?;
        if let Some(cursor) = &cursor
            && (cursor.field != sort.field
                || cursor.descending != sort.descending
                || cursor.version != 1)
        {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "page.cursor does not match sort".to_owned(),
            ));
        }
        let cursor_value = cursor.as_ref().and_then(|cursor| cursor.value.clone());
        let cursor_is_null = cursor
            .as_ref()
            .map(|cursor| cursor.is_null)
            .unwrap_or(false);
        let cursor_id = cursor.as_ref().map(|cursor| cursor.id);
        let column = native_sort_column(&sort.value_type)?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let joins = match &sort.relationship {
            None => "LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = e.blueprint_id
                    AND sort_attribute.blueprint_version = e.blueprint_version
                    AND sort_attribute.code = $10 AND sort_attribute.value_type = $11
                 LEFT JOIN attribute_values sort_value ON sort_value.entity_id = e.id
                    AND sort_value.attribute_id = sort_attribute.id
                    AND sort_value.context_id = (SELECT id FROM attribute_contexts WHERE code = 'default')
                    AND sort_value.relationship_target_entity_id IS NULL AND sort_value.active"
                .to_owned(),
            Some(_) => "LEFT JOIN attributes relationship_attribute ON relationship_attribute.blueprint_id = e.blueprint_id
                    AND relationship_attribute.blueprint_version = e.blueprint_version
                    AND relationship_attribute.code = $10 AND relationship_attribute.value_type = 'relationship'
                 LEFT JOIN attribute_values edge ON edge.entity_id = e.id
                    AND edge.attribute_id = relationship_attribute.id AND edge.active
                    AND edge.relationship_target_entity_id IS NOT NULL
                 LEFT JOIN entities target ON target.id = edge.relationship_target_entity_id AND target.deleted_at IS NULL
                 LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = target.blueprint_id
                    AND sort_attribute.blueprint_version = target.blueprint_version
                    AND sort_attribute.code = $11 AND sort_attribute.value_type = $12
                 LEFT JOIN attribute_values sort_value ON sort_value.entity_id = target.id
                    AND sort_value.attribute_id = sort_attribute.id
                    AND sort_value.context_id = (SELECT id FROM attribute_contexts WHERE code = 'default')
                    AND sort_value.relationship_target_entity_id IS NULL AND sort_value.active"
                .to_owned(),
        };
        let (field_bind, type_bind) = match &sort.relationship {
            Some(relationship) => (
                relationship.as_str(),
                sort.field.split_once('.').map_or("", |(_, field)| field),
            ),
            None => (sort.field.as_str(), sort.value_type.as_str()),
        };
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, e.created_at, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{{}}'::jsonb)
                         FROM attributes attribute WHERE attribute.blueprint_id = e.blueprint_id
                           AND attribute.blueprint_version = e.blueprint_version AND attribute.deleted_at IS NULL) AS blueprint_context_fallback,
                      sort_value.{column}::text AS sort_value, sort_value.{column} IS NULL AS sort_is_null
                FROM entities e
                JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                {joins}
                WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.deleted_at IS NULL
                  AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                  AND ($4::text[] IS NULL OR e.system_tags @> $4)
                  AND (NOT $5 OR e.blueprint_version <> $6)
                  AND ($9::uuid IS NULL
                    OR (sort_value.{column} IS NULL AND NOT $8)
                    OR (sort_value.{column} IS NOT NULL AND NOT $8 AND
                        (sort_value.{column} {comparison} $7::{column_type} OR (sort_value.{column} = $7::{column_type} AND e.id > $9)))
                    OR (sort_value.{column} IS NULL AND $8 AND e.id > $9))
                ORDER BY sort_value.{column} {direction} NULLS LAST, e.id ASC
                LIMIT $13"#,
            column = column,
            column_type = native_sort_cast(&sort.value_type)?,
        );
        let rows = sqlx::query_as::<_, SortedEntityPreviewRow>(&sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(matching_entity_ids)
            .bind((!system_tags.is_empty()).then_some(system_tags))
            .bind(outdated)
            .bind(current_blueprint_version)
            .bind(cursor_value)
            .bind(cursor_is_null)
            .bind(cursor_id)
            .bind(field_bind)
            .bind(type_bind)
            .bind(sort.value_type.as_str())
            .bind(limit + 1)
            .fetch_all(&self.pool)
            .await?;
        let mut rows = rows;
        let next_cursor = if rows.len() > limit as usize {
            rows.pop();
            rows.last()
                .map(|row| encode_sorted_search_cursor(sort, row))
        } else {
            None
        };
        Ok((
            rows.into_iter().map(sorted_entity_preview).collect(),
            next_cursor,
        ))
    }

    /// Adds direct relationship targets used by table columns in one query for the entire page.
    /// The requested target code is matched against the pinned source attribute, so an older
    /// source revision with removed or incompatible relationship metadata gets an empty cell.
    pub async fn hydrate_related_table_previews(
        &self,
        items: &mut [EntityPreview],
        relationships: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        if items.is_empty() || relationships.is_empty() {
            return Ok(());
        }
        for item in items.iter_mut() {
            item.related = relationships
                .keys()
                .cloned()
                .map(|code| (code, Vec::new()))
                .collect();
        }
        let requested = serde_json::to_value(
            relationships
                .iter()
                .map(|(attribute_code, target_blueprint_code)| {
                    serde_json::json!({
                        "attribute_code": attribute_code,
                        "target_blueprint_code": target_blueprint_code,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))?;
        let rows = sqlx::query_as::<_, RelatedTablePreviewRow>(
            r#"SELECT source.id AS source_id, a.code AS attribute_code,
                      context.id AS relationship_context_id,
                      context.code AS relationship_context_code,
                      target.id, target.blueprint_id, target.blueprint_version,
                      target.projections -> 'preview' AS preview,
                      target_blueprint.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(target_attribute.code, target_attribute.context_fallback), '{}'::jsonb)
                         FROM attributes target_attribute
                        WHERE target_attribute.blueprint_id = target.blueprint_id
                          AND target_attribute.blueprint_version = target.blueprint_version
                          AND target_attribute.deleted_at IS NULL) AS blueprint_context_fallback
                 FROM entities source
                 JOIN attribute_values av ON av.entity_id = source.id
                  AND av.active
                  AND av.relationship_target_entity_id IS NOT NULL
                 JOIN attributes a ON a.id = av.attribute_id
                  AND a.blueprint_id = source.blueprint_id
                  AND a.blueprint_version = source.blueprint_version
                  AND a.deleted_at IS NULL
                 JOIN LATERAL jsonb_to_recordset($2::jsonb)
                    AS requested(attribute_code text, target_blueprint_code text)
                    ON requested.attribute_code = a.code
                   AND requested.target_blueprint_code = a.target_blueprint_code
                 JOIN attribute_contexts context ON context.id = av.context_id
                 JOIN entities target ON target.id = av.relationship_target_entity_id
                  AND target.deleted_at IS NULL
                 JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                  AND target_blueprint.version = target.blueprint_version
                WHERE source.id = ANY($1)
                  AND source.deleted_at IS NULL
                ORDER BY source.id, a.code, context.code, av.id"#,
        )
        .bind(items.iter().map(|item| item.id).collect::<Vec<_>>())
        .bind(requested)
        .fetch_all(&self.pool)
        .await?;
        let mut by_id: HashMap<_, _> = items.iter_mut().map(|item| (item.id, item)).collect();
        for row in rows {
            if let Some(item) = by_id.get_mut(&row.source_id)
                && let Some(targets) = item.related.get_mut(&row.attribute_code)
            {
                targets.push(RelatedEntityPreview {
                    id: row.id,
                    blueprint_id: row.blueprint_id,
                    blueprint_version: row.blueprint_version,
                    relationship_context_id: row.relationship_context_id,
                    relationship_context_code: row.relationship_context_code,
                    display: super::entity_projection::display_labels(
                        &row.preview,
                        &row.blueprint_views,
                        &row.blueprint_context_fallback,
                    ),
                    preview: row.preview,
                });
            }
        }
        Ok(())
    }

    // Facet traversal inputs are explicit to keep query scope auditable.
    #[allow(clippy::too_many_arguments)]
    pub async fn relationship_tree_facet(
        &self,
        source_blueprint_id: Uuid,
        source_field: &str,
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
        selected_target_ids: &[Uuid],
        source_entity_ids: Option<&HashSet<Uuid>>,
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
        let parents = match hierarchy_field {
            Some(field) => {
                self.resolved_relationship_edges(
                    target_blueprint_id,
                    field,
                    target_blueprint_id,
                    context_id,
                )
                .await?
            }
            None => Vec::new(),
        };
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
                    source_entity_ids.is_none_or(|ids| ids.contains(source))
                        && allowed.contains(target)
                })
                .map(|(source, _)| *source)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect()
        });
        let mut counts: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
        for (source, target) in edges {
            if source_entity_ids.is_some_and(|ids| !ids.contains(&source)) {
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

    // Facet traversal inputs are explicit to keep query scope auditable.
    #[allow(clippy::too_many_arguments)]
    pub async fn relationship_tree_facet_children(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        matching_entity_ids: Option<&HashSet<Uuid>>,
        source_field: &str,
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
        parent_id: Option<Uuid>,
        cursor: Option<Uuid>,
        selected_target_ids: &[Uuid],
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
                      AND ($3::uuid[] IS NULL OR source.id = ANY($3))
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
        .bind(matching_entity_ids.map(|ids| ids.iter().copied().collect::<Vec<_>>()))
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
        let selected_rows = if selected_target_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, RelationshipTreeNodeRow>(
                r#"SELECT entity.id, entity.projections -> 'preview' AS preview,
                          blueprint.views,
                          (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                             FROM attributes attribute
                            WHERE attribute.blueprint_id = entity.blueprint_id
                              AND attribute.blueprint_version = entity.blueprint_version
                              AND attribute.deleted_at IS NULL) AS context_fallback
                     FROM entities entity
                     JOIN blueprints blueprint ON blueprint.id = entity.blueprint_id
                      AND blueprint.version = entity.blueprint_version
                    WHERE entity.blueprint_id = $1
                      AND entity.id = ANY($2)
                      AND entity.deleted_at IS NULL
                    ORDER BY entity.id"#,
            )
            .bind(target_blueprint_id)
            .bind(selected_target_ids)
            .fetch_all(&self.pool)
            .await?
        };
        if selected_rows.len() != selected_target_ids.iter().collect::<HashSet<_>>().len() {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
        }
        let selected_items = selected_rows
            .into_iter()
            .map(|row| EntityHierarchyItem {
                id: row.id,
                display: display_label(
                    &row.preview,
                    &row.views,
                    &row.context_fallback,
                    &requested_context.code,
                )
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            })
            .collect();
        Ok(RelationshipTreeFacetChildrenResponse {
            items,
            selected_items,
            next_cursor,
        })
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

    /// Resolves the application-owned relationship-aware query plan. SQL only reads
    /// one scalar-match set or one incoming-edge frontier at a time.
    pub async fn resolve_search(
        &self,
        selected: &BlueprintWithAttributes,
        selected_version: Option<i64>,
        query: Option<&str>,
    ) -> Result<ResolvedSearch, RepositoryError> {
        let Some(query) = query else {
            let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM entities WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL")
                .bind(selected.blueprint.id).bind(selected_version).fetch_all(&self.pool).await?;
            return Ok(ResolvedSearch {
                ids: ids.into_iter().collect(),
                explanations: HashMap::new(),
            });
        };
        let terms = parse_search_terms(query)?;
        let mut per_term = Vec::with_capacity(terms.len());
        for term in terms {
            let plan = self.compile_search_term(selected, &term).await?;
            per_term.push(
                self.resolve_search_term(selected, selected_version, term, plan)
                    .await?,
            );
        }
        let mut candidates: Option<HashSet<Uuid>> = None;
        let mut explanations: HashMap<Uuid, Vec<MatchExplanation>> = HashMap::new();
        for matches in per_term {
            let ids: HashSet<_> = matches.keys().copied().collect();
            candidates = Some(match candidates {
                Some(existing) => existing.intersection(&ids).copied().collect(),
                None => ids,
            });
            for (id, explanation) in matches {
                explanations.entry(id).or_default().push(explanation);
            }
        }
        let ids = candidates.unwrap_or_default();
        explanations.retain(|id, _| ids.contains(id));
        Ok(ResolvedSearch { ids, explanations })
    }

    async fn compile_search_term(
        &self,
        selected: &BlueprintWithAttributes,
        term: &SearchTerm,
    ) -> Result<TermPlan, RepositoryError> {
        let alias = |name: &str| {
            name.eq_ignore_ascii_case(&selected.blueprint.code)
                || name.eq_ignore_ascii_case(&selected.blueprint.name)
        };
        let Some(selector) = &term.selector else {
            return Ok(TermPlan::Any);
        };
        if selector == "*" {
            return Ok(TermPlan::Reachable);
        }
        let parts: Vec<_> = selector.split('.').collect();
        if parts.len() > 2 || parts.iter().any(|part| part.is_empty()) {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "search selector must have one or two parts".into(),
            ));
        }
        if parts.len() == 1 && alias(parts[0]) {
            self.ensure_unambiguous_name(parts[0], selected).await?;
            return Ok(TermPlan::SelectedAttribute(None));
        }
        if parts.len() == 2 && alias(parts[0]) {
            self.ensure_unambiguous_name(parts[0], selected).await?;
            let attribute = selected
                .attributes
                .iter()
                .find(|a| a.code.eq_ignore_ascii_case(parts[1]) && a.value_type != "relationship")
                .ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "unknown selected-blueprint attribute '{}'",
                        parts[1]
                    ))
                })?;
            return Ok(TermPlan::SelectedAttribute(Some(attribute.code.clone())));
        }
        if parts.len() == 1 {
            if let Some(attribute) = selected
                .attributes
                .iter()
                .find(|a| a.code.eq_ignore_ascii_case(parts[0]) && a.value_type != "relationship")
            {
                return Ok(TermPlan::SelectedAttribute(Some(attribute.code.clone())));
            }
            let relationship = selected
                .attributes
                .iter()
                .find(|a| a.code.eq_ignore_ascii_case(parts[0]) && a.value_type == "relationship")
                .ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "unknown search selector '{}'",
                        parts[0]
                    ))
                })?;
            let target = self
                .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                    || {
                        RepositoryError::InvalidBlueprintDefinition(
                            "relationship has no target blueprint".into(),
                        )
                    },
                )?)
                .await?
                .ok_or(RepositoryError::NotFound("target blueprint"))?;
            return Ok(TermPlan::Relationship {
                field: relationship.code.clone(),
                target: Box::new(target),
                attribute: None,
            });
        }
        let relationship = selected
            .attributes
            .iter()
            .find(|a| a.code.eq_ignore_ascii_case(parts[0]) && a.value_type == "relationship")
            .ok_or_else(|| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "unknown relationship '{}'",
                    parts[0]
                ))
            })?;
        let target = self
            .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                || {
                    RepositoryError::InvalidBlueprintDefinition(
                        "relationship has no target blueprint".into(),
                    )
                },
            )?)
            .await?
            .ok_or(RepositoryError::NotFound("target blueprint"))?;
        let attribute = target
            .attributes
            .iter()
            .find(|a| a.code.eq_ignore_ascii_case(parts[1]) && a.value_type != "relationship")
            .map(|attribute| attribute.code.clone())
            .ok_or_else(|| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "unknown relationship attribute '{}'",
                    parts[1]
                ))
            })?;
        Ok(TermPlan::Relationship {
            field: relationship.code.clone(),
            target: Box::new(target),
            attribute: Some(attribute),
        })
    }

    async fn ensure_unambiguous_name(
        &self,
        name: &str,
        selected: &BlueprintWithAttributes,
    ) -> Result<(), RepositoryError> {
        if !name.eq_ignore_ascii_case(&selected.blueprint.name) {
            return Ok(());
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT code) FROM blueprints WHERE lower(name) = lower($1) AND status = 'published' AND workspace_id = $2 AND deleted_at IS NULL")
            .bind(name).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_one(&self.pool).await?;
        if count > 1 {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "blueprint name '{}' is ambiguous; use its code",
                name
            )));
        }
        Ok(())
    }

    async fn resolve_search_term(
        &self,
        selected: &BlueprintWithAttributes,
        selected_version: Option<i64>,
        term: SearchTerm,
        plan: TermPlan,
    ) -> Result<HashMap<Uuid, MatchExplanation>, RepositoryError> {
        let (match_blueprint, attribute, direct_field) = match &plan {
            // Bare terms only search scalar values on the selected blueprint.
            TermPlan::Any => (Some(selected.blueprint.id), None, None),
            // `*:` explicitly opts into global relationship-aware discovery.
            TermPlan::Reachable => (None, None, None),
            TermPlan::SelectedAttribute(attribute) => (
                Some(selected.blueprint.id),
                attribute.clone(),
                Some(String::new()),
            ),
            TermPlan::Relationship {
                field,
                target,
                attribute,
            } => (
                Some(target.blueprint.id),
                attribute.clone(),
                Some(field.clone()),
            ),
        };
        let pattern = if term.prefix {
            format!("{}%", term.value)
        } else {
            format!("%{}%", term.value)
        };
        // Alphabetic terms other than booleans cannot match the canonical native
        // renderings of non-text values. Start these searches from the trigram-indexed
        // text values and materialize them so PostgreSQL does not repeat that scan for
        // every attribute.
        let sql = if text_only_search(&term.value) {
            r#"WITH matching_values AS MATERIALIZED (
                    SELECT entity_id, attribute_id
                    FROM attribute_values
                    WHERE active AND relationship_target_entity_id IS NULL
                      AND value_text ILIKE $3
                )
                SELECT DISTINCT e.id, a.code
                FROM matching_values av
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN entities e ON e.id = av.entity_id
                WHERE e.deleted_at IS NULL
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)
                ORDER BY e.id, a.code"#
        } else {
            r#"SELECT DISTINCT e.id, a.code
                FROM entities e JOIN attribute_values av ON av.entity_id = e.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                WHERE e.deleted_at IS NULL AND av.relationship_target_entity_id IS NULL
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)
                  AND COALESCE(av.value_text, av.value_number::text, av.value_integer::text, av.value_boolean::text, av.value_date::text, av.value_datetime::text, av.value_time::text) ILIKE $3
                ORDER BY e.id, a.code"#
        };
        let rows = sqlx::query_as::<_, (Uuid, String)>(sql)
            .bind(match_blueprint)
            .bind(attribute)
            .bind(pattern)
            .fetch_all(&self.pool)
            .await?;
        let mut witnesses: HashMap<Uuid, MatchExplanation> = HashMap::new();
        let mut frontier = VecDeque::new();
        for (id, attribute) in rows {
            if let std::collections::hash_map::Entry::Vacant(e) = witnesses.entry(id) {
                e.insert(MatchExplanation {
                    term: term.original.clone(),
                    matching_entity_id: id,
                    matching_attribute_code: Some(attribute),
                    traversal_depth: 0,
                    relationship_path: vec![],
                });
                frontier.push_back(id);
            }
        }
        let mut visited: HashSet<_> = witnesses.keys().copied().collect();
        let max_depth = match &plan {
            TermPlan::Relationship { .. } => 1,
            TermPlan::SelectedAttribute(_) | TermPlan::Any => 0,
            TermPlan::Reachable => 3,
        };
        let restrict_source = matches!(&plan, TermPlan::Relationship { .. });
        for depth in 1..=max_depth {
            let level: Vec<_> = frontier.drain(..).collect();
            if level.is_empty() {
                break;
            }
            let edges = sqlx::query_as::<_, (Uuid, String, Uuid)>(
                r#"SELECT DISTINCT source.id, a.code, av.relationship_target_entity_id
                FROM entities source JOIN attribute_values av ON av.entity_id = source.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN entities target ON target.id = av.relationship_target_entity_id
                WHERE source.deleted_at IS NULL AND target.deleted_at IS NULL AND av.active
                  AND av.relationship_target_entity_id = ANY($1) AND a.value_type = 'relationship'
                  AND ($2::uuid IS NULL OR source.blueprint_id = $2)
                  AND ($3::text IS NULL OR a.code = $3)
                ORDER BY source.id, a.code, av.relationship_target_entity_id"#,
            )
            .bind(&level)
            .bind(restrict_source.then_some(selected.blueprint.id))
            .bind(direct_field.as_ref().filter(|v| !v.is_empty()))
            .fetch_all(&self.pool)
            .await?;
            for (source, field, target) in edges {
                if !visited.insert(source) {
                    continue;
                }
                let parent = witnesses.get(&target).expect("frontier has a witness");
                let mut path = Vec::with_capacity(parent.relationship_path.len() + 1);
                path.push(MatchPathEdge {
                    source_entity_id: source,
                    attribute_code: field,
                    target_entity_id: target,
                });
                path.extend(parent.relationship_path.clone());
                witnesses.insert(
                    source,
                    MatchExplanation {
                        term: term.original.clone(),
                        matching_entity_id: parent.matching_entity_id,
                        matching_attribute_code: parent.matching_attribute_code.clone(),
                        traversal_depth: depth,
                        relationship_path: path,
                    },
                );
                frontier.push_back(source);
            }
        }
        let ids = witnesses
            .into_iter()
            .filter(|(id, _)| {
                // Explicit selected-blueprint value queries and all traversals only return selected entities.
                // The version restriction is applied here because scalar matches may start on old revisions.
                *id != Uuid::nil()
            })
            .collect::<HashMap<_, _>>();
        let selected_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM entities WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL AND id = ANY($3)")
            .bind(selected.blueprint.id).bind(selected_version).bind(ids.keys().copied().collect::<Vec<_>>()).fetch_all(&self.pool).await?;
        Ok(selected_ids
            .into_iter()
            .filter_map(|id| ids.get(&id).cloned().map(|w| (id, w)))
            .collect())
    }
}

#[derive(Default)]
pub struct ResolvedSearch {
    pub ids: HashSet<Uuid>,
    pub explanations: HashMap<Uuid, Vec<MatchExplanation>>,
}
#[derive(Clone)]
struct SearchTerm {
    original: String,
    selector: Option<String>,
    value: String,
    prefix: bool,
}
enum TermPlan {
    Any,
    Reachable,
    SelectedAttribute(Option<String>),
    Relationship {
        field: String,
        target: Box<BlueprintWithAttributes>,
        attribute: Option<String>,
    },
}
fn parse_search_terms(query: &str) -> Result<Vec<SearchTerm>, RepositoryError> {
    query
        .split_whitespace()
        .map(|raw| {
            let (selector, value) = match raw.split_once(':') {
                Some((s, v)) if !s.is_empty() && !v.is_empty() && !v.contains(':') => {
                    (Some(s.to_owned()), v)
                }
                Some(_) => {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "malformed search term '{raw}'"
                    )));
                }
                None => (None, raw),
            };
            let prefix = value.ends_with('*');
            let value = value.strip_suffix('*').unwrap_or(value);
            if value.is_empty() || value.contains('*') {
                return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                    "wildcard is only allowed at the end of a search term: '{raw}'"
                )));
            }
            Ok(SearchTerm {
                original: raw.to_owned(),
                selector,
                value: value.to_owned(),
                prefix,
            })
        })
        .collect()
}
fn text_only_search(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized != "true"
        && normalized != "false"
        && value.chars().any(|character| character.is_alphabetic())
}

fn native_sort_column(value_type: &str) -> Result<&'static str, RepositoryError> {
    match value_type {
        "string" => Ok("value_text"),
        "number" => Ok("value_number"),
        "integer" => Ok("value_integer"),
        "boolean" => Ok("value_boolean"),
        "date" => Ok("value_date"),
        "datetime" => Ok("value_datetime"),
        "time" => Ok("value_time"),
        _ => Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "{value_type} is not sortable"
        ))),
    }
}

fn native_sort_cast(value_type: &str) -> Result<&'static str, RepositoryError> {
    match value_type {
        "string" => Ok("text"),
        "number" => Ok("numeric"),
        "integer" => Ok("bigint"),
        "boolean" => Ok("boolean"),
        "date" => Ok("date"),
        "datetime" => Ok("timestamptz"),
        "time" => Ok("time"),
        _ => native_sort_column(value_type).map(|_| unreachable!()),
    }
}

fn decode_sorted_search_cursor(cursor: &str) -> Result<SortedSearchCursor, RepositoryError> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| {
        RepositoryError::InvalidBlueprintDefinition("page.cursor is invalid".to_owned())
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        RepositoryError::InvalidBlueprintDefinition("page.cursor is invalid".to_owned())
    })
}

fn encode_sorted_search_cursor(sort: &EntitySearchSort, row: &SortedEntityPreviewRow) -> String {
    let cursor = SortedSearchCursor {
        version: 1,
        field: sort.field.clone(),
        descending: sort.descending,
        is_null: row.sort_is_null,
        value: row.sort_value.clone(),
        id: row.id,
    };
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).expect("cursor serializes"))
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

fn sorted_entity_preview(row: SortedEntityPreviewRow) -> EntityPreview {
    entity_preview(EntityPreviewRow {
        id: row.id,
        blueprint_version: row.blueprint_version,
        created_at: row.created_at,
        preview: row.preview,
        blueprint_views: row.blueprint_views,
        blueprint_context_fallback: row.blueprint_context_fallback,
    })
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
        related: HashMap::new(),
        match_explanations: Vec::new(),
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
