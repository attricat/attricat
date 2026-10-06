use super::*;
use super::{entity_projection::display_label, system_annotations::validate_system_tags};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct EntityPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    is_sample: bool,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct SortedEntityPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    is_sample: bool,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
    sort_value: Option<String>,
    sort_is_null: bool,
    sort_target_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct EntitySearchSort {
    pub field: String,
    pub relationship_path: Vec<String>,
    pub leaf_field: String,
    pub leaf_blueprint_id: Uuid,
    pub value_type: String,
    pub descending: bool,
    pub effective_source_version: Option<i64>,
    pub publication_context_id: Option<Uuid>,
    /// Values are resolved for this context path; see [`SearchContext`].
    pub context: SearchContext,
}

/// The context whose values search filters, sorts and table cells read: the
/// requested context first, then its ancestors up to the default context.
/// An attribute resolves to the nearest context on the path with an active
/// value; one with `context_fallback = "none"` only reads the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchContext {
    pub ids: Vec<Uuid>,
    pub codes: Vec<String>,
}

impl SearchContext {
    pub fn requested_id(&self) -> Uuid {
        self.ids[0]
    }

    /// The nearest non-null value of `code` in a preview projection.
    fn resolve_preview<'a>(
        &self,
        preview: &'a Value,
        code: &str,
        inherit: bool,
    ) -> Option<&'a Value> {
        let codes = if inherit {
            &self.codes[..]
        } else {
            &self.codes[..1]
        };
        codes.iter().find_map(|context| {
            preview
                .get(context)
                .and_then(|values| values.get(code))
                .filter(|value| !value.is_null())
        })
    }
}

/// SQL condition selecting the attribute value rows `value` that resolve for
/// the context path bound at `path`: rows of the nearest context on the path
/// with an active value of the same entity attribute. `attribute` supplies
/// `context_fallback`.
fn resolved_context(value: &str, attribute: &str, path: &str) -> String {
    format!(
        "{value}.context_id = ANY({path}::uuid[]) AND ({value}.context_id = ({path}::uuid[])[1] \
         OR ({attribute}.context_fallback <> 'none' AND NOT EXISTS (\
             SELECT 1 FROM attribute_values nearer \
              WHERE nearer.workspace_id = {value}.workspace_id AND nearer.entity_id = {value}.entity_id \
                AND nearer.attribute_id = {value}.attribute_id AND nearer.active \
                AND nearer.context_id = ANY(({path}::uuid[])[1:array_position({path}::uuid[], {value}.context_id) - 1]))))"
    )
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct EntityRelationshipFilter {
    pub field: String,
    pub relationship_path: Vec<String>,
    pub selected_target_ids: Vec<Uuid>,
}

/// Internal string operator: `value` is a JSON array of strings, any of which
/// may match exactly. Resolved from request-level shorthands such as `@me`;
/// clients cannot send it.
pub const SEARCH_FILTER_EQ_ANY: &str = "eq_any";

#[derive(Clone, Debug, serde::Serialize)]
pub struct EntitySearchFilter {
    pub field: String,
    pub reusable: bool,
    pub relationship_path: Vec<String>,
    pub leaf_field: String,
    pub operator: String,
    pub value_type: String,
    pub value: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct SortedSearchCursor {
    version: u8,
    field: String,
    descending: bool,
    effective_source_version: Option<i64>,
    #[serde(default)]
    publication_context_id: Option<Uuid>,
    #[serde(default)]
    context_id: Option<Uuid>,
    is_null: bool,
    value: Option<String>,
    target_id: Uuid,
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
struct TablePathEdgeRow {
    source_id: Uuid,
    target_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct TablePathLeafRow {
    id: Uuid,
    inherit: bool,
    preview: Option<Value>,
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
    is_sample: bool,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct EntityLabelRow {
    id: Uuid,
    blueprint_code: String,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct IncomingRelationshipRow {
    id: Uuid,
    blueprint_code: String,
    blueprint_version: i64,
    is_sample: bool,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

/// Root entities whose active relationships, resolved for the context path
/// `$6`, reach one of `$4` at exactly the path `$3`. `$1` blueprint, `$2`
/// optional version, `$5` workspace. `{edge_context}` is replaced with
/// [`resolved_context`].
const RELATIONSHIP_FILTER_SQL: &str = r#"WITH RECURSIVE reached(root_id, current_id, depth) AS (
                       SELECT e.id, e.id, 0
                         FROM entities e
                        WHERE e.blueprint_id = $1
                          AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                          AND e.workspace_id = $5 AND e.deleted_at IS NULL
                       UNION ALL
                       SELECT reached.root_id, av.relationship_target_entity_id, reached.depth + 1
                         FROM reached
                         JOIN entities source ON source.id = reached.current_id
                          AND source.workspace_id = $5 AND source.deleted_at IS NULL
                         JOIN attributes relationship ON relationship.blueprint_id = source.blueprint_id
                          AND relationship.blueprint_version = source.blueprint_version
                          AND relationship.workspace_id = $5
                          AND relationship.code = $3[reached.depth + 1]
                          AND relationship.value_type = 'relationship'
                         JOIN attribute_values av ON av.entity_id = source.id
                          AND av.workspace_id = $5
                          AND av.attribute_id = relationship.id AND av.active
                          AND {edge_context}
                          AND av.relationship_target_entity_id IS NOT NULL
                         JOIN entities target ON target.id = av.relationship_target_entity_id
                          AND target.workspace_id = $5 AND target.deleted_at IS NULL
                         JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                          AND target_blueprint.version = target.blueprint_version
                          AND target_blueprint.code = relationship.target_blueprint_code
                        WHERE reached.depth < cardinality($3)
                   )
                   SELECT DISTINCT root_id
                     FROM reached
                    WHERE depth = cardinality($3) AND current_id = ANY($4)"#;

/// Root entities whose value of a (possibly dotted) field, resolved for the
/// context path `$9`, satisfies one typed filter. `$1` blueprint, `$2`
/// optional version, `$3` relationship path, `$4` leaf field, `$5` value type,
/// `$6` operator, `$7` value, `$8` workspace. `{edge_context}` and
/// `{value_context}` are replaced with [`resolved_context`].
const SCALAR_FILTER_SQL: &str = r#"WITH RECURSIVE reached(root_id, current_id, depth) AS (
                       SELECT e.id, e.id, 0
                         FROM entities e
                        WHERE e.blueprint_id = $1
                          AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                          AND e.workspace_id = $8 AND e.deleted_at IS NULL
                       UNION ALL
                       SELECT reached.root_id, av.relationship_target_entity_id, reached.depth + 1
                         FROM reached
                         JOIN entities source ON source.id = reached.current_id
                          AND source.workspace_id = $8 AND source.deleted_at IS NULL
                         JOIN attributes relationship ON relationship.blueprint_id = source.blueprint_id
                          AND relationship.blueprint_version = source.blueprint_version
                          AND relationship.workspace_id = $8
                          AND relationship.code = $3[reached.depth + 1]
                          AND relationship.value_type = 'relationship'
                         JOIN attribute_values av ON av.entity_id = source.id
                          AND av.workspace_id = $8
                          AND av.attribute_id = relationship.id AND av.active
                          AND {edge_context}
                          AND av.relationship_target_entity_id IS NOT NULL
                         JOIN entities target ON target.id = av.relationship_target_entity_id
                          AND target.workspace_id = $8 AND target.deleted_at IS NULL
                         JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                          AND target_blueprint.version = target.blueprint_version
                          AND target_blueprint.code = relationship.target_blueprint_code
                        WHERE reached.depth < cardinality($3)
                   )
                   SELECT DISTINCT reached.root_id
                     FROM reached
                     JOIN entities leaf ON leaf.id = reached.current_id
                      AND leaf.workspace_id = $8 AND leaf.deleted_at IS NULL
                     JOIN attributes a ON a.blueprint_id = leaf.blueprint_id
                      AND a.blueprint_version = leaf.blueprint_version
                      AND a.workspace_id = $8
                      AND a.code = $4 AND a.deleted_at IS NULL
                      AND (a.value_type = $5 OR ($6 = 'is_set' AND a.value_type IN
                          ('string','number','integer','boolean','date','datetime','time')))
                     JOIN attribute_values av ON av.entity_id = leaf.id AND av.attribute_id = a.id
                      AND av.workspace_id = $8
                      AND av.active AND av.relationship_target_entity_id IS NULL
                      AND {value_context}
                    WHERE reached.depth = cardinality($3)
                      AND (a.value_type <> 'file' OR EXISTS (
                          SELECT 1 FROM attribute_file_references r
                           WHERE r.attribute_value_id = av.id AND r.workspace_id = $8))
                      AND CASE
                          WHEN $6 = 'is_set' THEN TRUE
                          WHEN $5 = 'string' AND $6 = 'eq' THEN av.value_text = $7
                          WHEN $5 = 'string' AND $6 = 'eq_any' THEN $7::jsonb ? av.value_text
                          WHEN $5 = 'string' AND $6 = 'contains' THEN strpos(lower(av.value_text), lower($7)) > 0
                          WHEN $5 = 'string' AND $6 = 'starts_with' THEN left(lower(av.value_text), char_length($7)) = lower($7)
                          WHEN $5 = 'number' AND $6 = 'eq' THEN av.value_number = $7::numeric
                          WHEN $5 = 'number' AND $6 = 'gt' THEN av.value_number > $7::numeric
                          WHEN $5 = 'number' AND $6 = 'gte' THEN av.value_number >= $7::numeric
                          WHEN $5 = 'number' AND $6 = 'lt' THEN av.value_number < $7::numeric
                          WHEN $5 = 'number' AND $6 = 'lte' THEN av.value_number <= $7::numeric
                          WHEN $5 = 'integer' AND $6 = 'eq' THEN av.value_integer = $7::bigint
                          WHEN $5 = 'integer' AND $6 = 'gt' THEN av.value_integer > $7::bigint
                          WHEN $5 = 'integer' AND $6 = 'gte' THEN av.value_integer >= $7::bigint
                          WHEN $5 = 'integer' AND $6 = 'lt' THEN av.value_integer < $7::bigint
                          WHEN $5 = 'integer' AND $6 = 'lte' THEN av.value_integer <= $7::bigint
                          WHEN $5 = 'boolean' AND $6 = 'eq' THEN av.value_boolean = $7::boolean
                          WHEN $5 = 'date' AND $6 = 'eq' THEN av.value_date = $7::date
                          WHEN $5 = 'date' AND $6 = 'gt' THEN av.value_date > $7::date
                          WHEN $5 = 'date' AND $6 = 'gte' THEN av.value_date >= $7::date
                          WHEN $5 = 'date' AND $6 = 'lt' THEN av.value_date < $7::date
                          WHEN $5 = 'date' AND $6 = 'lte' THEN av.value_date <= $7::date
                          WHEN $5 = 'datetime' AND $6 = 'eq' THEN av.value_datetime = $7::timestamptz
                          WHEN $5 = 'datetime' AND $6 = 'gt' THEN av.value_datetime > $7::timestamptz
                          WHEN $5 = 'datetime' AND $6 = 'gte' THEN av.value_datetime >= $7::timestamptz
                          WHEN $5 = 'datetime' AND $6 = 'lt' THEN av.value_datetime < $7::timestamptz
                          WHEN $5 = 'datetime' AND $6 = 'lte' THEN av.value_datetime <= $7::timestamptz
                          WHEN $5 = 'time' AND $6 = 'eq' THEN av.value_time = $7::time
                          WHEN $5 = 'time' AND $6 = 'gt' THEN av.value_time > $7::time
                          WHEN $5 = 'time' AND $6 = 'gte' THEN av.value_time >= $7::time
                          WHEN $5 = 'time' AND $6 = 'lt' THEN av.value_time < $7::time
                          WHEN $5 = 'time' AND $6 = 'lte' THEN av.value_time <= $7::time
                          ELSE FALSE
                      END"#;

/// Absence is the complement of presence within the same live root family and
/// version scope. This also includes roots with no reached relationship leaf,
/// older revisions without the field, or no attached reusable attribute.
/// An active scalar row counts as present even for empty text, zero or false;
/// a file value is present only while it references at least one file.
fn presence_filter_query(
    sql: &str,
    filter: &EntitySearchFilter,
    workspace_parameter: usize,
) -> String {
    if filter.operator == catalog_validation::saved_search::FILTER_OPERATOR_IS_SET
        && filter.value == "false"
    {
        format!(
            "SELECT e.id FROM entities e WHERE e.blueprint_id = $1
             AND ($2::bigint IS NULL OR e.blueprint_version = $2)
             AND e.workspace_id = ${workspace_parameter} AND e.deleted_at IS NULL
             EXCEPT ({sql})"
        )
    } else {
        sql.to_owned()
    }
}

/// Rewrites each `$n` placeholder of `sql` to `$map(n)`.
fn renumber_parameters(sql: &str, map: impl Fn(usize) -> usize) -> String {
    let mut renumbered = String::with_capacity(sql.len() + 16);
    let mut chars = sql.char_indices().peekable();
    while let Some((_, character)) = chars.next() {
        if character != '$' {
            renumbered.push(character);
            continue;
        }
        let mut digits = String::new();
        while let Some((_, digit)) = chars.peek().filter(|(_, next)| next.is_ascii_digit()) {
            digits.push(*digit);
            chars.next();
        }
        match digits.parse::<usize>() {
            Ok(parameter) => {
                renumbered.push('$');
                renumbered.push_str(&map(parameter).to_string());
            }
            Err(_) => {
                renumbered.push('$');
                renumbered.push_str(&digits);
            }
        }
    }
    renumbered
}

impl CatalogRepository {
    /// The [`SearchContext`] of the context `code`, or `None` when it does not exist.
    pub async fn search_context(
        &self,
        code: &str,
    ) -> Result<Option<SearchContext>, RepositoryError> {
        let Some(requested) = self.get_context_by_code(code).await? else {
            return Ok(None);
        };
        let tree = match self.cached_context_tree().await? {
            Some(tree) => tree,
            None => {
                let mut connection = self.pool.acquire().await?;
                std::sync::Arc::new(
                    super::record_values::ContextTree::load(&mut connection, self.workspace_id.0)
                        .await?,
                )
            }
        };
        let ids = tree.path(requested.id, true)?;
        let codes = ids
            .iter()
            .map(|id| {
                tree.get(*id)
                    .map(|context| context.code.clone())
                    .ok_or(RepositoryError::InvalidContext)
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(SearchContext { ids, codes }))
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
            r#"SELECT target.id, target.blueprint_version, ('attricat.sample'=ANY(target.system_tags)) AS is_sample, target.created_at, target.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.workspace_id = $6
                          AND attribute.blueprint_id = target.blueprint_id
                          AND attribute.blueprint_version = target.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
               FROM attribute_values av
               JOIN entities source ON source.id = av.entity_id
                AND source.workspace_id = $6 AND source.deleted_at IS NULL
               JOIN attributes a ON a.id = av.attribute_id
                AND a.workspace_id = $6 AND a.deleted_at IS NULL
               JOIN entities target ON target.id = av.relationship_target_entity_id
               JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
               WHERE av.entity_id = $1
                 AND av.workspace_id = $6
                 AND a.workspace_id = $6
                 AND a.code = $2
                 AND av.relationship_target_entity_id IS NOT NULL
                  AND av.active
                 AND ($3::uuid IS NULL OR av.relationship_target_entity_id > $3)
                 AND target.workspace_id = $6 AND target.deleted_at IS NULL
                 AND b.workspace_id = $6 AND b.code = $4
               ORDER BY target.id
               LIMIT $5"#,
        )
        .bind(related_from)
        .bind(relationship)
        .bind(cursor)
        .bind(blueprint_code)
        .bind(limit + 1)
        .bind(self.workspace_id.0)
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

    /// Display labels of the live entities among `entity_ids`, in ID order.
    /// Callers authorize the IDs first; unknown and deleted IDs are omitted.
    pub async fn entity_labels(
        &self,
        entity_ids: &[Uuid],
    ) -> Result<Vec<EntityLabel>, RepositoryError> {
        if entity_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, EntityLabelRow>(
            r#"SELECT e.id, b.code AS blueprint_code, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.workspace_id = $2
                          AND attribute.blueprint_id = e.blueprint_id
                          AND attribute.blueprint_version = e.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
               FROM entities e
               JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                AND b.workspace_id = $2
               WHERE e.workspace_id = $2 AND e.deleted_at IS NULL AND e.id = ANY($1)
               ORDER BY e.id"#,
        )
        .bind(entity_ids)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| EntityLabel {
                id: row.id,
                blueprint_code: row.blueprint_code,
                display: super::entity_projection::display_labels(
                    &row.preview,
                    &row.blueprint_views,
                    &row.blueprint_context_fallback,
                ),
            })
            .collect())
    }

    /// The relationship fields whose active values on live entities target
    /// `entity_id`, with the number of distinct source entities per field,
    /// so a caller can find every incoming edge without naming selectors.
    pub async fn incoming_relationship_fields(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<IncomingRelationshipField>, RepositoryError> {
        self.get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        Ok(sqlx::query_as::<_, (String, String, i64)>(
            r#"SELECT b.code, a.code, COUNT(DISTINCT source.id)
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
                AND a.workspace_id = $2 AND a.deleted_at IS NULL
                AND a.value_type = 'relationship'
               JOIN entities source ON source.id = av.entity_id
                AND source.workspace_id = $2 AND source.deleted_at IS NULL
               JOIN blueprints b ON b.id = source.blueprint_id
                AND b.version = source.blueprint_version AND b.workspace_id = $2
               WHERE av.workspace_id = $2 AND av.active
                 AND av.relationship_target_entity_id = $1
               GROUP BY b.code, a.code
               ORDER BY b.code, a.code"#,
        )
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(source_blueprint, field, source_count)| IncomingRelationshipField {
                source_blueprint,
                field,
                source_count,
            },
        )
        .collect())
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
                      ('attricat.sample'=ANY(source.system_tags)) AS is_sample, source.created_at, source.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.workspace_id = $6
                          AND attribute.blueprint_id = source.blueprint_id
                          AND attribute.blueprint_version = source.blueprint_version
                          AND attribute.deleted_at IS NULL) AS blueprint_context_fallback
               FROM entities source
               JOIN blueprints b ON b.id = source.blueprint_id AND b.version = source.blueprint_version
               WHERE source.workspace_id = $6 AND source.deleted_at IS NULL
                 AND b.workspace_id = $6
                 AND EXISTS (
                     SELECT 1
                     FROM attribute_values av
                     JOIN attributes a ON a.id = av.attribute_id
                      AND a.workspace_id = $6 AND a.deleted_at IS NULL
                     JOIN LATERAL jsonb_to_recordset($2::jsonb)
                         AS selector(source_blueprint text, field text)
                         ON selector.source_blueprint = b.code AND selector.field = a.code
                     WHERE av.entity_id = source.id
                       AND av.workspace_id = $6
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
        .bind(self.workspace_id.0)
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
        // Advance the cursor over the fetched page, including hidden rows, so
        // a scoped reader can continue past a page with no readable sources.
        let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
        if let Some(readable) = self.actor_readable_entity_ids(&ids).await? {
            items.retain(|item| readable.contains(&item.id));
        }
        Ok(IncomingRelationshipsPage {
            items: items
                .into_iter()
                .map(|item| IncomingRelationshipItem {
                    id: item.id,
                    blueprint_code: item.blueprint_code,
                    blueprint_version: item.blueprint_version,
                    is_sample: item.is_sample,
                    display: item.display,
                })
                .collect(),
            next_cursor,
        })
    }

    /// Resolves roots whose active relationship values, resolved for `context`, reach a
    /// selected target at the exact requested relationship path depth.
    pub async fn filter_relationship_entity_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        filters: &[EntityRelationshipFilter],
        context: &SearchContext,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let filters: Vec<&EntityRelationshipFilter> = filters
            .iter()
            .filter(|filter| !filter.selected_target_ids.is_empty())
            .collect();
        if filters.is_empty() {
            return Ok(Vec::new());
        }
        // One INTERSECT branch per filter. Shared parameters: $1 blueprint,
        // $2 version, $3 workspace, $4 context path; each filter binds a path
        // and targets.
        let statement = RELATIONSHIP_FILTER_SQL.replace(
            "{edge_context}",
            &resolved_context("av", "relationship", "$6"),
        );
        let sql = filters
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let base = 5 + index * 2;
                format!(
                    "({})",
                    renumber_parameters(&statement, |parameter| match parameter {
                        1 => 1,
                        2 => 2,
                        5 => 3,
                        6 => 4,
                        3 | 4 => base + parameter - 3,
                        _ => unreachable!("the filter statement has six parameters"),
                    })
                )
            })
            .collect::<Vec<_>>()
            .join(" INTERSECT ");
        let mut query = sqlx::query_scalar::<_, Uuid>(&sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(self.workspace_id.0)
            .bind(&context.ids);
        for filter in &filters {
            query = query
                .bind(&filter.relationship_path)
                .bind(&filter.selected_target_ids);
        }
        Ok(query
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect())
    }

    /// Resolves entities which satisfy every typed scalar filter with values resolved for
    /// `context`. Dotted fields traverse at most three relationship hops using each
    /// source entity's pinned blueprint revision.
    pub async fn filter_entity_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        filters: &[EntitySearchFilter],
        context: &SearchContext,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let mut matching: Option<HashSet<Uuid>> = None;
        for filter in filters.iter().filter(|filter| filter.reusable) {
            let ids = self
                .filter_reusable_attribute_ids(blueprint_id, blueprint_version, filter, context)
                .await?;
            let current = match matching {
                Some(current) => current.intersection(&ids).copied().collect(),
                None => ids,
            };
            // Nothing can match every filter once the intersection is empty.
            if current.is_empty() {
                return Ok(Vec::new());
            }
            matching = Some(current);
        }
        let scalar: Vec<&EntitySearchFilter> =
            filters.iter().filter(|filter| !filter.reusable).collect();
        if !scalar.is_empty() {
            // Every scalar filter becomes one INTERSECT branch of a single
            // statement. Shared parameters: $1 blueprint, $2 version,
            // $3 workspace, $4 context path; each filter binds five more.
            let statement = SCALAR_FILTER_SQL
                .replace(
                    "{edge_context}",
                    &resolved_context("av", "relationship", "$9"),
                )
                .replace("{value_context}", &resolved_context("av", "a", "$9"));
            let sql = scalar
                .iter()
                .enumerate()
                .map(|(index, filter)| {
                    let base = 5 + index * 5;
                    format!(
                        "({})",
                        renumber_parameters(
                            &presence_filter_query(&statement, filter, 8),
                            |parameter| match parameter {
                                1 => 1,
                                2 => 2,
                                8 => 3,
                                9 => 4,
                                3..=7 => base + parameter - 3,
                                _ => unreachable!("the filter statement has nine parameters"),
                            }
                        )
                    )
                })
                .collect::<Vec<_>>()
                .join(" INTERSECT ");
            let mut query = sqlx::query_scalar::<_, Uuid>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(self.workspace_id.0)
                .bind(&context.ids);
            for filter in &scalar {
                query = query
                    .bind(&filter.relationship_path)
                    .bind(&filter.leaf_field)
                    .bind(&filter.value_type)
                    .bind(&filter.operator)
                    .bind(&filter.value);
            }
            let ids = query
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .collect::<HashSet<_>>();
            matching = Some(match matching {
                Some(current) => current.intersection(&ids).copied().collect(),
                None => ids,
            });
        }
        Ok(matching.unwrap_or_default().into_iter().collect())
    }

    async fn filter_reusable_attribute_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        filter: &EntitySearchFilter,
        context: &SearchContext,
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        let sql = presence_filter_query(
            &r#"SELECT DISTINCT e.id
                 FROM entities e
                 JOIN attributes a ON a.entity_id = e.id AND a.code = $3
                  AND (a.value_type = $4 OR ($5 = 'is_set' AND a.value_type IN
                      ('string','number','integer','boolean','date','datetime','time')))
                 JOIN reusable_attribute_revisions r ON r.id = a.reusable_attribute_revision_id
                 JOIN attribute_values av ON av.entity_id = e.id AND av.attribute_id = a.id
                WHERE e.blueprint_id = $1
                  AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.workspace_id = $7 AND e.deleted_at IS NULL
                  AND a.workspace_id = $7 AND a.deleted_at IS NULL AND r.searchable
                  AND av.workspace_id = $7 AND av.active AND av.relationship_target_entity_id IS NULL
                  AND {value_context}
                  AND (a.value_type <> 'file' OR EXISTS (
                      SELECT 1 FROM attribute_file_references fr
                       WHERE fr.attribute_value_id = av.id AND fr.workspace_id = $7))
                  AND CASE
                    WHEN $5 = 'is_set' THEN TRUE
                    WHEN $4 = 'string' AND $5 = 'eq' THEN av.value_text = $6
                    WHEN $4 = 'string' AND $5 = 'eq_any' THEN $6::jsonb ? av.value_text
                    WHEN $4 = 'string' AND $5 = 'contains' THEN strpos(lower(av.value_text), lower($6)) > 0
                    WHEN $4 = 'string' AND $5 = 'starts_with' THEN left(lower(av.value_text), char_length($6)) = lower($6)
                    WHEN $4 = 'number' AND $5 = 'eq' THEN av.value_number = $6::numeric
                    WHEN $4 = 'number' AND $5 = 'gt' THEN av.value_number > $6::numeric
                    WHEN $4 = 'number' AND $5 = 'gte' THEN av.value_number >= $6::numeric
                    WHEN $4 = 'number' AND $5 = 'lt' THEN av.value_number < $6::numeric
                    WHEN $4 = 'number' AND $5 = 'lte' THEN av.value_number <= $6::numeric
                    WHEN $4 = 'integer' AND $5 = 'eq' THEN av.value_integer = $6::bigint
                    WHEN $4 = 'integer' AND $5 = 'gt' THEN av.value_integer > $6::bigint
                    WHEN $4 = 'integer' AND $5 = 'gte' THEN av.value_integer >= $6::bigint
                    WHEN $4 = 'integer' AND $5 = 'lt' THEN av.value_integer < $6::bigint
                    WHEN $4 = 'integer' AND $5 = 'lte' THEN av.value_integer <= $6::bigint
                    WHEN $4 = 'boolean' AND $5 = 'eq' THEN av.value_boolean = $6::boolean
                    WHEN $4 = 'date' AND $5 = 'eq' THEN av.value_date = $6::date
                    WHEN $4 = 'date' AND $5 = 'gt' THEN av.value_date > $6::date
                    WHEN $4 = 'date' AND $5 = 'gte' THEN av.value_date >= $6::date
                    WHEN $4 = 'date' AND $5 = 'lt' THEN av.value_date < $6::date
                    WHEN $4 = 'date' AND $5 = 'lte' THEN av.value_date <= $6::date
                    WHEN $4 = 'datetime' AND $5 = 'eq' THEN av.value_datetime = $6::timestamptz
                    WHEN $4 = 'datetime' AND $5 = 'gt' THEN av.value_datetime > $6::timestamptz
                    WHEN $4 = 'datetime' AND $5 = 'gte' THEN av.value_datetime >= $6::timestamptz
                    WHEN $4 = 'datetime' AND $5 = 'lt' THEN av.value_datetime < $6::timestamptz
                    WHEN $4 = 'datetime' AND $5 = 'lte' THEN av.value_datetime <= $6::timestamptz
                    WHEN $4 = 'time' AND $5 = 'eq' THEN av.value_time = $6::time
                    WHEN $4 = 'time' AND $5 = 'gt' THEN av.value_time > $6::time
                    WHEN $4 = 'time' AND $5 = 'gte' THEN av.value_time >= $6::time
                    WHEN $4 = 'time' AND $5 = 'lt' THEN av.value_time < $6::time
                    WHEN $4 = 'time' AND $5 = 'lte' THEN av.value_time <= $6::time
                    ELSE FALSE END"#
                .replace("{value_context}", &resolved_context("av", "a", "$8")),
            filter,
            7,
        );
        Ok(sqlx::query_scalar::<_, Uuid>(&sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(&filter.leaf_field)
            .bind(&filter.value_type)
            .bind(&filter.operator)
            .bind(&filter.value)
            .bind(self.workspace_id.0)
            .bind(&context.ids)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect())
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
        let sql = r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.created_at, e.projections -> 'preview' AS preview,
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
                        JOIN attributes search_attribute ON search_attribute.id = av.attribute_id
                        LEFT JOIN reusable_attribute_revisions reusable_revision ON reusable_revision.id = search_attribute.reusable_attribute_revision_id
                        WHERE av.entity_id = e.id
                          AND av.relationship_target_entity_id IS NULL
                          AND (search_attribute.entity_id IS NULL OR reusable_revision.searchable)
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

    pub async fn search_result_versions(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
    ) -> Result<Vec<i64>, RepositoryError> {
        validate_system_tags(system_tags)?;
        Ok(sqlx::query_scalar::<_, i64>(
            r#"SELECT e.blueprint_version
                 FROM entities e
                WHERE e.blueprint_id = $1
                  AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.workspace_id = $3
                  AND e.deleted_at IS NULL
                  AND ($4::uuid[] IS NULL OR e.id = ANY($4))
                  AND ($5::text[] IS NULL OR e.system_tags @> $5)
                  AND (NOT $6 OR e.blueprint_version <> $7)
                GROUP BY e.blueprint_version
                ORDER BY e.blueprint_version DESC
                LIMIT 2"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(self.workspace_id.0)
        .bind(matching_entity_ids)
        .bind((!system_tags.is_empty()).then_some(system_tags))
        .bind(outdated)
        .bind(current_blueprint_version)
        .fetch_all(&self.pool)
        .await?)
    }

    /// [`Self::search_result_versions`] and, with `count_limit`,
    /// [`Self::count_entity_previews`] over the same matches in one scan.
    #[allow(clippy::too_many_arguments)]
    pub async fn search_result_versions_and_count(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
        count_limit: Option<i64>,
    ) -> Result<(Vec<i64>, Option<i64>), RepositoryError> {
        let Some(count_limit) = count_limit else {
            return Ok((
                self.search_result_versions(
                    blueprint_id,
                    blueprint_version,
                    matching_entity_ids,
                    system_tags,
                    outdated,
                    current_blueprint_version,
                )
                .await?,
                None,
            ));
        };
        validate_system_tags(system_tags)?;
        let (versions, count) = sqlx::query_as::<_, (Vec<i64>, i64)>(
            r#"WITH matches AS MATERIALIZED (
                   SELECT e.blueprint_version
                     FROM entities e
                    WHERE e.blueprint_id = $1
                      AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                      AND e.workspace_id = $3
                      AND e.deleted_at IS NULL
                      AND ($4::uuid[] IS NULL OR e.id = ANY($4))
                      AND ($5::text[] IS NULL OR e.system_tags @> $5)
                      AND (NOT $6 OR e.blueprint_version <> $7)
               )
               SELECT ARRAY(
                          SELECT blueprint_version FROM matches
                           GROUP BY blueprint_version
                           ORDER BY blueprint_version DESC
                           LIMIT 2
                      ),
                      (SELECT COUNT(*) FROM (SELECT 1 FROM matches LIMIT $8) capped)"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(self.workspace_id.0)
        .bind(matching_entity_ids)
        .bind((!system_tags.is_empty()).then_some(system_tags))
        .bind(outdated)
        .bind(current_blueprint_version)
        .bind(count_limit)
        .fetch_one(&self.pool)
        .await?;
        Ok((versions, Some(count)))
    }

    /// Selects a page by a configured scalar table column without hydrating projections for
    /// every candidate. NULL values sort first ascending and last descending; entity IDs make
    /// the ordering stable.
    #[allow(clippy::too_many_arguments)]
    pub async fn search_entity_previews_sorted(
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
                || cursor.effective_source_version != sort.effective_source_version
                || cursor.publication_context_id != sort.publication_context_id
                || cursor.context_id != Some(sort.context.requested_id())
                || cursor.version != 4)
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
        let cursor_target_id = cursor.as_ref().map(|cursor| cursor.target_id);
        if matches!(
            sort.field.as_str(),
            "blueprint_version" | "publication_status"
        ) {
            return self
                .search_entity_previews_sorted_system(
                    blueprint_id,
                    blueprint_version,
                    limit,
                    matching_entity_ids,
                    system_tags,
                    outdated,
                    current_blueprint_version,
                    sort,
                    cursor.as_ref(),
                )
                .await;
        }
        if !sort.relationship_path.is_empty() {
            return self
                .search_entity_previews_sorted_relationship(
                    blueprint_id,
                    blueprint_version,
                    limit,
                    matching_entity_ids,
                    system_tags,
                    outdated,
                    current_blueprint_version,
                    sort,
                    cursor_value,
                    cursor_is_null,
                    cursor_target_id,
                    cursor_id,
                )
                .await;
        }
        let column = native_sort_column(&sort.value_type)?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let null_order = if sort.descending { "LAST" } else { "FIRST" };
        let nulls_first = !sort.descending;
        let joins = if sort.relationship_path.is_empty() {
            "LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = e.blueprint_id
                    AND sort_attribute.blueprint_version = e.blueprint_version
                    AND sort_attribute.workspace_id = $14
                    AND sort_attribute.code = $11 AND sort_attribute.value_type = $12
                 LEFT JOIN attribute_values sort_value ON sort_value.entity_id = e.id
                    AND sort_value.workspace_id = $14
                    AND sort_value.attribute_id = sort_attribute.id
                    AND {value_context}
                    AND sort_value.relationship_target_entity_id IS NULL AND sort_value.active"
                .to_owned()
        } else {
            "LEFT JOIN LATERAL (
                    WITH RECURSIVE reached(current_id, depth) AS (
                        SELECT e.id, 0
                        UNION ALL
                        SELECT edge.relationship_target_entity_id, reached.depth + 1
                        FROM reached
                        JOIN entities path_source ON path_source.id = reached.current_id
                         AND path_source.workspace_id = $14 AND path_source.deleted_at IS NULL
                        JOIN attributes path_attribute ON path_attribute.blueprint_id = path_source.blueprint_id
                         AND path_attribute.blueprint_version = path_source.blueprint_version
                         AND path_attribute.workspace_id = $14
                         AND path_attribute.code = $10[reached.depth + 1]
                         AND path_attribute.value_type = 'relationship'
                         AND path_attribute.cardinality = 'one'
                        JOIN attribute_values edge ON edge.entity_id = path_source.id
                         AND edge.workspace_id = $14
                         AND edge.attribute_id = path_attribute.id AND edge.active
                         AND {edge_context}
                         AND edge.relationship_target_entity_id IS NOT NULL
                        WHERE reached.depth < cardinality($10)
                    )
                    SELECT current_id FROM reached WHERE depth = cardinality($10) LIMIT 1
                 ) resolved_path ON true
                 LEFT JOIN entities target ON target.id = resolved_path.current_id
                    AND target.workspace_id = $14 AND target.deleted_at IS NULL
                 LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = target.blueprint_id
                    AND sort_attribute.blueprint_version = target.blueprint_version
                    AND sort_attribute.workspace_id = $14
                    AND sort_attribute.code = $11 AND sort_attribute.value_type = $12
                 LEFT JOIN attribute_values sort_value ON sort_value.entity_id = target.id
                    AND sort_value.workspace_id = $14
                    AND sort_value.attribute_id = sort_attribute.id
                    AND {value_context}
                    AND sort_value.relationship_target_entity_id IS NULL AND sort_value.active"
                .to_owned()
        }
        .replace(
            "{value_context}",
            &resolved_context("sort_value", "sort_attribute", "$15"),
        )
        .replace("{edge_context}", &resolved_context("edge", "path_attribute", "$15"));
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.created_at, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{{}}'::jsonb)
                         FROM attributes attribute WHERE attribute.blueprint_id = e.blueprint_id
                           AND attribute.blueprint_version = e.blueprint_version AND attribute.deleted_at IS NULL) AS blueprint_context_fallback,
                      sort_value.{column}::text AS sort_value, sort_value.{column} IS NULL AS sort_is_null,
                      e.id AS sort_target_id
                FROM entities e
                JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                {joins}
                WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.workspace_id = $14 AND e.deleted_at IS NULL
                  AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                  AND ($4::text[] IS NULL OR e.system_tags @> $4)
                  AND (NOT $5 OR e.blueprint_version <> $6)
                  AND ($9::uuid IS NULL
                    OR ({nulls_first} AND (
                      (sort_value.{column} IS NULL AND $8 AND e.id > $9)
                      OR (sort_value.{column} IS NOT NULL AND ($8 OR
                        sort_value.{column} {comparison} $7::{column_type}
                        OR (sort_value.{column} = $7::{column_type} AND e.id > $9)))
                    ))
                    OR (NOT {nulls_first} AND (
                      (sort_value.{column} IS NULL AND NOT $8)
                      OR (sort_value.{column} IS NOT NULL AND NOT $8 AND
                        (sort_value.{column} {comparison} $7::{column_type} OR (sort_value.{column} = $7::{column_type} AND e.id > $9)))
                      OR (sort_value.{column} IS NULL AND $8 AND e.id > $9)
                    )))
                ORDER BY sort_value.{column} {direction} NULLS {null_order}, e.id ASC
                LIMIT $13"#,
            column = column,
            column_type = native_sort_cast(&sort.value_type)?,
            nulls_first = nulls_first,
            null_order = null_order,
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
            .bind(&sort.relationship_path)
            .bind(&sort.leaf_field)
            .bind(sort.value_type.as_str())
            .bind(limit + 1)
            .bind(self.workspace_id.0)
            .bind(&sort.context.ids)
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

    /// Sort a built-in entity property independently of blueprint table columns.
    /// Entity ID breaks ties so pagination cannot omit or duplicate rows.
    #[allow(clippy::too_many_arguments)]
    async fn search_entity_previews_sorted_system(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        limit: i64,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
        sort: &EntitySearchSort,
        cursor: Option<&SortedSearchCursor>,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        let cursor_version = cursor
            .map(|cursor| {
                if cursor.is_null {
                    return Err(RepositoryError::InvalidBlueprintDefinition(
                        "page.cursor is invalid".to_owned(),
                    ));
                }
                cursor
                    .value
                    .as_deref()
                    .and_then(|value| value.parse::<i64>().ok())
                    .ok_or_else(|| {
                        RepositoryError::InvalidBlueprintDefinition(
                            "page.cursor is invalid".to_owned(),
                        )
                    })
            })
            .transpose()?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let (expression, join) = if sort.field == "publication_status" {
            (
                "CASE WHEN p.published_at IS NULL THEN 0 ELSE 1 END",
                "LEFT JOIN entity_channel_publications p ON p.workspace_id = e.workspace_id AND p.entity_id = e.id AND p.context_id = $11",
            )
        } else {
            ("e.blueprint_version", "")
        };
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.created_at, e.projections -> 'preview' AS preview,
                         b.views AS blueprint_views,
                         (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{{}}'::jsonb)
                            FROM attributes attribute WHERE attribute.blueprint_id = e.blueprint_id
                              AND attribute.blueprint_version = e.blueprint_version AND attribute.deleted_at IS NULL) AS blueprint_context_fallback,
                         ({expression})::text AS sort_value, false AS sort_is_null, e.id AS sort_target_id
                  FROM entities e
                  JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                  {join}
                  WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                    AND e.workspace_id = $10 AND e.deleted_at IS NULL
                    AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                    AND ($4::text[] IS NULL OR e.system_tags @> $4)
                    AND (NOT $5 OR e.blueprint_version <> $6)
                    AND ($8::uuid IS NULL OR ({expression}) {comparison} $7::bigint
                         OR (({expression}) = $7 AND e.id > $8))
                  ORDER BY {expression} {direction}, e.id ASC
                  LIMIT $9"#
        );
        let query = sqlx::query_as::<_, SortedEntityPreviewRow>(&sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(matching_entity_ids)
            .bind((!system_tags.is_empty()).then_some(system_tags))
            .bind(outdated)
            .bind(current_blueprint_version)
            .bind(cursor_version)
            .bind(cursor.map(|cursor| cursor.id))
            .bind(limit + 1)
            .bind(self.workspace_id.0);
        let mut rows = if sort.field == "publication_status" {
            query
                .bind(sort.publication_context_id)
                .fetch_all(&self.pool)
                .await?
        } else {
            query.fetch_all(&self.pool).await?
        };
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

    /// Sorts a single-valued relationship path from its indexed scalar leaf back to
    /// source entities. Non-null values are paged first; missing/incompatible paths
    /// use a separate source-ID phase so they do not force a full mixed-value sort.
    #[allow(clippy::too_many_arguments)]
    async fn search_entity_previews_sorted_relationship(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        limit: i64,
        matching_entity_ids: Option<&[Uuid]>,
        system_tags: &[String],
        outdated: bool,
        current_blueprint_version: i64,
        sort: &EntitySearchSort,
        cursor_value: Option<String>,
        cursor_is_null: bool,
        cursor_target_id: Option<Uuid>,
        cursor_id: Option<Uuid>,
    ) -> Result<(Vec<EntityPreview>, Option<String>), RepositoryError> {
        let column = native_sort_column(&sort.value_type)?;
        let column_type = native_sort_cast(&sort.value_type)?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let workspace_id = self.workspace_id.0;
        let leaf_attribute_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attributes WHERE workspace_id = $1 AND blueprint_id = $2 AND code = $3 AND value_type = $4 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(sort.leaf_blueprint_id)
        .bind(&sort.leaf_field)
        .bind(&sort.value_type)
        .fetch_all(&self.pool)
        .await?;
        let mut rows = Vec::new();

        if !cursor_is_null {
            let reverse_joins = relationship_sort_reverse_joins(sort.relationship_path.len())?;
            let leaf_context = resolved_context("leaf", "leaf_attribute", "$15");
            let sql = format!(
                r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.created_at, e.projections -> 'preview' AS preview,
                          b.views AS blueprint_views,
                          (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{{}}'::jsonb)
                             FROM attributes attribute WHERE attribute.blueprint_id = e.blueprint_id
                               AND attribute.blueprint_version = e.blueprint_version
                               AND attribute.workspace_id = $14 AND attribute.deleted_at IS NULL) AS blueprint_context_fallback,
                          sort_value.{column}::text AS sort_value, false AS sort_is_null,
                          sort_value.entity_id AS sort_target_id
                     FROM (
                         SELECT leaf.entity_id, leaf.{column}
                           FROM attribute_values leaf
                           JOIN attributes leaf_attribute ON leaf_attribute.id = leaf.attribute_id
                          WHERE leaf.workspace_id = $14 AND {leaf_context}
                            AND leaf.active AND leaf.relationship_target_entity_id IS NULL
                            AND leaf.attribute_id = ANY($17) AND leaf.{column} IS NOT NULL
                            AND ($9::uuid IS NULL OR leaf.{column} {comparison} $7::{column_type}
                                 OR (leaf.{column} = $7::{column_type} AND leaf.entity_id >= $16))
                          ORDER BY leaf.{column} {direction}, leaf.entity_id
                          OFFSET 0
                     ) sort_value
                     {reverse_joins}
                     JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                    WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                      AND e.workspace_id = $14 AND e.deleted_at IS NULL
                      AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                      AND ($4::text[] IS NULL OR e.system_tags @> $4)
                      AND (NOT $5 OR e.blueprint_version <> $6)
                      AND NOT $8
                      AND ($9::uuid IS NULL OR sort_value.{column} {comparison} $7::{column_type}
                           OR (sort_value.{column} = $7::{column_type}
                               AND (sort_value.entity_id > $16
                                    OR (sort_value.entity_id = $16 AND e.id > $9))))
                    ORDER BY sort_value.{column} {direction}, sort_value.entity_id ASC, e.id ASC
                    LIMIT $13"#,
            );
            rows = sqlx::query_as::<_, SortedEntityPreviewRow>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching_entity_ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(cursor_value.as_deref())
                .bind(false)
                .bind(cursor_id)
                .bind(&sort.relationship_path)
                .bind(&sort.leaf_field)
                .bind(sort.value_type.as_str())
                .bind(limit + 1)
                .bind(workspace_id)
                .bind(&sort.context.ids)
                .bind(cursor_target_id)
                .bind(&leaf_attribute_ids)
                .fetch_all(&self.pool)
                .await?;
        }

        if rows.len() <= limit as usize {
            let remaining = limit + 1 - rows.len() as i64;
            let forward_joins = relationship_sort_forward_joins(sort.relationship_path.len())?;
            let null_cursor_id = cursor_is_null.then_some(cursor_id).flatten();
            let sql = format!(
                r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.created_at, e.projections -> 'preview' AS preview,
                          b.views AS blueprint_views,
                          (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{{}}'::jsonb)
                             FROM attributes attribute WHERE attribute.blueprint_id = e.blueprint_id
                               AND attribute.blueprint_version = e.blueprint_version
                               AND attribute.workspace_id = $14 AND attribute.deleted_at IS NULL) AS blueprint_context_fallback,
                          NULL::text AS sort_value, true AS sort_is_null,
                          e.id AS sort_target_id
                     FROM entities e
                     JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                     {forward_joins}
                    WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                      AND e.workspace_id = $14 AND e.deleted_at IS NULL
                      AND sort_value.id IS NULL
                      AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                      AND ($4::text[] IS NULL OR e.system_tags @> $4)
                      AND (NOT $5 OR e.blueprint_version <> $6)
                      AND $7::text IS NULL AND $8
                      AND ($9::uuid IS NULL OR e.id > $9)
                    ORDER BY e.id ASC
                    LIMIT $13"#,
            );
            let mut null_rows = sqlx::query_as::<_, SortedEntityPreviewRow>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching_entity_ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(Option::<String>::None)
                .bind(true)
                .bind(null_cursor_id)
                .bind(&sort.relationship_path)
                .bind(&sort.leaf_field)
                .bind(sort.value_type.as_str())
                .bind(remaining)
                .bind(workspace_id)
                .bind(&sort.context.ids)
                .fetch_all(&self.pool)
                .await?;
            rows.append(&mut null_rows);
        }

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

    /// Resolves configured table paths in bounded, page-level batches. Relationship
    /// metadata is selected from every entity's pinned blueprint revision and scalar
    /// leaves come from preview projections. Every hop and leaf is resolved for `context`.
    pub async fn hydrate_table_path_values(
        &self,
        items: &mut [EntityPreview],
        paths: &HashMap<String, String>,
        context: &SearchContext,
    ) -> Result<(), RepositoryError> {
        if items.is_empty() {
            return Ok(());
        }
        let roots: Vec<_> = items.iter().map(|item| item.id).collect();
        let mut values_by_root: HashMap<Uuid, HashMap<String, Vec<Value>>> = HashMap::new();
        // Direct columns of every type are resolved with one query for file
        // columns and one for scalar columns; dotted paths walk per hop.
        let direct_files: Vec<&str> = paths
            .iter()
            .filter(|(path, leaf_type)| !path.contains('.') && leaf_type.as_str() == "file")
            .map(|(path, _)| path.as_str())
            .collect();
        let (direct_codes, direct_types): (Vec<&str>, Vec<&str>) = paths
            .iter()
            .filter(|(path, leaf_type)| !path.contains('.') && leaf_type.as_str() != "file")
            .map(|(path, leaf_type)| (path.as_str(), leaf_type.as_str()))
            .unzip();
        if !direct_files.is_empty() {
            let files = sqlx::query_as::<_, (Uuid, String, Value)>(
                &r#"SELECT av.entity_id, a.code,
                          jsonb_build_object(
                            'id', f.id,
                            'filename', f.display_filename,
                            'mime_type', f.mime_type,
                            'byte_size', f.byte_size,
                            'sha256', f.sha256,
                            'status', f.status,
                            'variants', COALESCE(
                              jsonb_agg(jsonb_build_object(
                                'kind', fv.kind,
                                'mime_type', fv.mime_type,
                                'width', fv.width,
                                'height', fv.height,
                                'byte_size', fv.byte_size,
                                'sha256', fv.sha256
                              ) ORDER BY fv.kind) FILTER (WHERE fv.id IS NOT NULL),
                              '[]'::jsonb
                            )
                          )
                   FROM attribute_values av
                   JOIN attributes a ON a.id = av.attribute_id
                    AND a.workspace_id = $3 AND a.deleted_at IS NULL
                    AND a.code = ANY($2) AND a.value_type = 'file'
                   JOIN attribute_file_references r ON r.attribute_value_id = av.id
                    AND r.workspace_id = $3
                   JOIN files f ON f.id = r.file_id
                    AND f.workspace_id = $3 AND f.deleted_at IS NULL
                   LEFT JOIN file_variants fv ON fv.file_id = f.id AND fv.workspace_id = $3
                  WHERE av.entity_id = ANY($1) AND av.workspace_id = $3 AND av.active
                    AND {value_context}
                  GROUP BY av.entity_id, a.code, f.id, f.display_filename, f.mime_type, f.byte_size,
                           f.sha256, f.status, r.position
                  ORDER BY av.entity_id, a.code, r.position"#
                    .replace("{value_context}", &resolved_context("av", "a", "$4")),
            )
            .bind(&roots)
            .bind(&direct_files)
            .bind(self.workspace_id.0)
            .bind(&context.ids)
            .fetch_all(&self.pool)
            .await?;
            for (entity_id, code, file) in files {
                values_by_root
                    .entry(entity_id)
                    .or_default()
                    .entry(code)
                    .or_default()
                    .push(file);
            }
        }
        if !direct_codes.is_empty() {
            let compatible: HashMap<(Uuid, String), bool> =
                sqlx::query_as::<_, (Uuid, String, bool)>(
                    r#"SELECT e.id, a.code, a.context_fallback <> 'none'
                     FROM entities e
                     JOIN attributes a ON a.blueprint_id = e.blueprint_id
                      AND a.blueprint_version = e.blueprint_version
                      AND a.workspace_id = $3
                     JOIN UNNEST($2::text[], $4::text[]) AS column_(code, value_type)
                       ON column_.code = a.code AND column_.value_type = a.value_type
                    WHERE e.id = ANY($1) AND e.workspace_id = $3 AND e.deleted_at IS NULL"#,
                )
                .bind(&roots)
                .bind(&direct_codes)
                .bind(self.workspace_id.0)
                .bind(&direct_types)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|(id, code, inherit)| ((id, code), inherit))
                .collect();
            for item in items.iter() {
                for code in &direct_codes {
                    let Some(inherit) = compatible.get(&(item.id, (*code).to_owned())) else {
                        continue;
                    };
                    if let Some(value) = context.resolve_preview(&item.preview, code, *inherit) {
                        values_by_root
                            .entry(item.id)
                            .or_default()
                            .insert((*code).to_owned(), vec![value.clone()]);
                    }
                }
            }
        }
        for (path, expected_leaf_type) in paths {
            let parts: Vec<_> = path.split('.').collect();
            if parts.is_empty() || parts.len() > 4 || parts.len() == 1 {
                continue;
            }
            let mut reached: Vec<(Uuid, Uuid)> = roots.iter().map(|id| (*id, *id)).collect();
            for relationship in &parts[..parts.len() - 1] {
                let current_ids: Vec<_> = reached
                    .iter()
                    .map(|(_, current)| *current)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                if current_ids.is_empty() {
                    break;
                }
                let edges = sqlx::query_as::<_, TablePathEdgeRow>(
                    &r#"SELECT av.entity_id AS source_id, target.id AS target_id
                       FROM attribute_values av
                       JOIN entities source ON source.id = av.entity_id
                        AND source.workspace_id = $3 AND source.deleted_at IS NULL
                       JOIN attributes a ON a.id = av.attribute_id
                        AND a.blueprint_id = source.blueprint_id
                        AND a.blueprint_version = source.blueprint_version
                        AND a.workspace_id = $3
                        AND a.code = $2 AND a.value_type = 'relationship'
                       JOIN entities target ON target.id = av.relationship_target_entity_id
                        AND target.workspace_id = $3 AND target.deleted_at IS NULL
                       JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                        AND target_blueprint.version = target.blueprint_version
                        AND target_blueprint.code = a.target_blueprint_code
                      WHERE av.entity_id = ANY($1) AND av.workspace_id = $3
                        AND {edge_context}
                        AND av.active AND av.relationship_target_entity_id IS NOT NULL"#
                        .replace("{edge_context}", &resolved_context("av", "a", "$4")),
                )
                .bind(current_ids)
                .bind(relationship)
                .bind(self.workspace_id.0)
                .bind(&context.ids)
                .fetch_all(&self.pool)
                .await?;
                let mut targets: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
                for edge in edges {
                    targets
                        .entry(edge.source_id)
                        .or_default()
                        .push(edge.target_id);
                }
                reached = reached
                    .into_iter()
                    .flat_map(|(root, current)| {
                        targets
                            .get(&current)
                            .into_iter()
                            .flatten()
                            .map(move |target| (root, *target))
                    })
                    .collect();
            }
            if reached.is_empty() {
                continue;
            }
            let leaf = parts[parts.len() - 1];
            let leaf_ids: Vec<_> = reached
                .iter()
                .map(|(_, id)| *id)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            // Only the leaf attribute of each context's preview is selected.
            let leaves = sqlx::query_as::<_, TablePathLeafRow>(
                r#"SELECT e.id, a.context_fallback <> 'none' AS inherit,
                          (SELECT jsonb_object_agg(context.key, jsonb_build_object($2, context.value -> $2))
                             FROM jsonb_each(e.projections -> 'preview') context
                            WHERE context.value ? $2) AS preview
                   FROM entities e
                   JOIN attributes a ON a.blueprint_id = e.blueprint_id
                    AND a.blueprint_version = e.blueprint_version
                    AND a.code = $2 AND a.value_type = $3
                    AND a.workspace_id = $4 AND a.deleted_at IS NULL
                  WHERE e.id = ANY($1) AND e.workspace_id = $4 AND e.deleted_at IS NULL"#,
            )
            .bind(leaf_ids)
            .bind(leaf)
            .bind(expected_leaf_type)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?;
            let leaves: HashMap<_, _> = leaves
                .into_iter()
                .filter_map(|row| {
                    let preview = row.preview?;
                    context
                        .resolve_preview(&preview, leaf, row.inherit)
                        .cloned()
                        .map(|value| (row.id, value))
                })
                .collect();
            for (root, id) in reached {
                if let Some(value) = leaves.get(&id) {
                    let values = values_by_root
                        .entry(root)
                        .or_default()
                        .entry(path.clone())
                        .or_default();
                    if !values.contains(value) {
                        values.push(value.clone());
                    }
                }
            }
        }
        for item in items {
            item.table_values = values_by_root.remove(&item.id).unwrap_or_default();
        }
        Ok(())
    }

    /// Adds direct relationship targets used by table columns in one query for the entire page.
    /// The requested target code is matched against the pinned source attribute, so an older
    /// source revision with removed or incompatible relationship metadata gets an empty cell.
    /// Relationship values are resolved for `context`.
    pub async fn hydrate_related_table_previews(
        &self,
        items: &mut [EntityPreview],
        relationships: &HashMap<String, String>,
        context: &SearchContext,
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
            &r#"SELECT source.id AS source_id, a.code AS attribute_code,
                      context.id AS relationship_context_id,
                      context.code AS relationship_context_code,
                      target.id, target.blueprint_id, target.blueprint_version, ('attricat.sample'=ANY(target.system_tags)) AS is_sample,
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
                  AND {edge_context}
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
                ORDER BY source.id, a.code, context.code, av.id"#
                .replace("{edge_context}", &resolved_context("av", "a", "$3")),
        )
        .bind(items.iter().map(|item| item.id).collect::<Vec<_>>())
        .bind(requested)
        .bind(&context.ids)
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
                    is_sample: row.is_sample,
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
            let ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM entities WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND workspace_id = $3 AND deleted_at IS NULL")
                .bind(selected.blueprint.id)
                .bind(selected_version)
                .bind(self.workspace_id.0)
                .fetch_all(&self.pool)
                .await?;
            return Ok(ResolvedSearch {
                ids: ids.into_iter().collect(),
                explanations: HashMap::new(),
            });
        };
        let terms = parse_search_terms(query)?;
        let mut per_term = Vec::with_capacity(terms.len());
        // Global `*:` terms share one request budget. Explicit and bare selectors
        // retain their established, independently-resolved semantics.
        let mut global_budget = GlobalSearchBudget::default();
        for term in terms {
            let plan = self.compile_search_term(selected, &term).await?;
            let global = matches!(plan, TermPlan::Reachable);
            let result = self
                .resolve_search_term(selected, selected_version, term, plan, &mut global_budget)
                .await;
            if global {
                match &result {
                    Ok(_) => metrics::counter!("catalog_global_relationship_search_total", "outcome" => "success").increment(1),
                    Err(RepositoryError::RelationshipSearchBudgetExceeded { dimension }) => {
                        metrics::counter!("catalog_global_relationship_search_total", "outcome" => "budget_exceeded", "dimension" => *dimension).increment(1);
                        tracing::warn!(dimension, "global relationship search budget exceeded");
                    }
                    Err(RepositoryError::RelationshipSearchTimedOut) => {
                        metrics::counter!("catalog_global_relationship_search_total", "outcome" => "timed_out").increment(1);
                        tracing::warn!("global relationship search statement timed out");
                    }
                    Err(_) => metrics::counter!("catalog_global_relationship_search_total", "outcome" => "failed").increment(1),
                }
            }
            per_term.push(result?);
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
        if parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "search selector may contain at most three relationship hops and a scalar leaf"
                    .into(),
            ));
        }
        if let Some((leaf, path)) = parts.split_last()
            && leaf.eq_ignore_ascii_case(ID_SELECTOR)
        {
            let ids = parse_search_ids(term)?;
            let path = match path {
                [name] if alias(name) => {
                    self.ensure_unambiguous_name(name, selected).await?;
                    &[][..]
                }
                path => path,
            };
            let (fields, source_blueprint_ids, target) =
                self.search_relationship_path(selected, path).await?;
            return Ok(TermPlan::Ids {
                fields,
                source_blueprint_ids,
                target_blueprint_id: target.blueprint.id,
                ids,
            });
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
                fields: vec![relationship.code.clone()],
                source_blueprint_ids: vec![selected.blueprint.id],
                target: Box::new(target),
                attribute: None,
            });
        }
        let (fields, source_blueprint_ids, current) = self
            .search_relationship_path(selected, &parts[..parts.len() - 1])
            .await?;
        let leaf = parts[parts.len() - 1];
        let attribute = current
            .attributes
            .iter()
            .find(|a| {
                a.code.eq_ignore_ascii_case(leaf)
                    && !matches!(a.value_type.as_str(), "relationship" | "file" | "json")
            })
            .map(|attribute| attribute.code.clone())
            .ok_or_else(|| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "unknown scalar search-path leaf '{}'",
                    leaf
                ))
            })?;
        Ok(TermPlan::Relationship {
            fields,
            source_blueprint_ids,
            target: Box::new(current),
            attribute: Some(attribute),
        })
    }

    /// Walks named relationships from the selected blueprint, returning the
    /// relationship codes, the blueprint each hop starts from, and the blueprint
    /// the path ends on.
    async fn search_relationship_path(
        &self,
        selected: &BlueprintWithAttributes,
        path: &[&str],
    ) -> Result<(Vec<String>, Vec<Uuid>, BlueprintWithAttributes), RepositoryError> {
        let mut current = selected.clone();
        let mut fields = Vec::new();
        let mut source_blueprint_ids = Vec::new();
        for relationship_name in path {
            let relationship = current
                .attributes
                .iter()
                .find(|a| {
                    a.code.eq_ignore_ascii_case(relationship_name) && a.value_type == "relationship"
                })
                .ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "unknown relationship '{}' in search path",
                        relationship_name
                    ))
                })?;
            fields.push(relationship.code.clone());
            source_blueprint_ids.push(current.blueprint.id);
            current = self
                .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                    || {
                        RepositoryError::InvalidBlueprintDefinition(
                            "relationship has no target blueprint".into(),
                        )
                    },
                )?)
                .await?
                .ok_or(RepositoryError::NotFound("target blueprint"))?;
        }
        Ok((fields, source_blueprint_ids, current))
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
            .bind(name).bind(self.workspace_id.0).fetch_one(&self.pool).await?;
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
        global_budget: &mut GlobalSearchBudget,
    ) -> Result<HashMap<Uuid, MatchExplanation>, RepositoryError> {
        if matches!(plan, TermPlan::Reachable) {
            return self
                .resolve_global_search_term(selected, selected_version, term, global_budget)
                .await;
        }
        let (match_blueprint, attribute, direct_field) = match &plan {
            // Bare terms only search scalar values on the selected blueprint.
            TermPlan::Any => (Some(selected.blueprint.id), None, None),
            TermPlan::Reachable => {
                unreachable!("global terms are resolved in a bounded transaction")
            }
            TermPlan::SelectedAttribute(attribute) => (
                Some(selected.blueprint.id),
                attribute.clone(),
                Some(String::new()),
            ),
            TermPlan::Relationship {
                fields,
                target,
                attribute,
                ..
            } => (
                Some(target.blueprint.id),
                attribute.clone(),
                fields.first().cloned(),
            ),
            TermPlan::Ids {
                target_blueprint_id,
                ..
            } => (Some(*target_blueprint_id), None, None),
        };
        let rows = match &plan {
            TermPlan::Ids { ids, .. } => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM entities WHERE id = ANY($1) AND blueprint_id = $2 AND workspace_id = $3 AND deleted_at IS NULL ORDER BY id",
            )
            .bind(ids)
            .bind(match_blueprint)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|id| (id, None))
            .collect(),
            _ => {
                self.scalar_search_matches(&term, match_blueprint, attribute)
                    .await?
            }
        };
        let mut witnesses: HashMap<Uuid, MatchExplanation> = HashMap::new();
        let mut frontier = VecDeque::new();
        for (id, attribute) in rows {
            if let std::collections::hash_map::Entry::Vacant(e) = witnesses.entry(id) {
                e.insert(MatchExplanation {
                    term: term.original.clone(),
                    matching_entity_id: id,
                    matching_attribute_code: attribute,
                    traversal_depth: 0,
                    relationship_path: vec![],
                });
                frontier.push_back(id);
            }
        }
        let mut visited: HashSet<_> = witnesses.keys().copied().collect();
        let (path_fields, path_source_blueprints) = match &plan {
            TermPlan::Relationship {
                fields,
                source_blueprint_ids,
                ..
            }
            | TermPlan::Ids {
                fields,
                source_blueprint_ids,
                ..
            } => (
                Some(fields.as_slice()),
                Some(source_blueprint_ids.as_slice()),
            ),
            _ => (None, None),
        };
        let max_depth = match path_fields {
            Some(fields) => fields.len(),
            None => 0,
        };
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
                  AND source.workspace_id = $4 AND target.workspace_id = $4
                  AND av.workspace_id = $4 AND a.workspace_id = $4
                  AND av.relationship_target_entity_id = ANY($1) AND a.value_type = 'relationship'
                  AND ($2::uuid IS NULL OR source.blueprint_id = $2)
                  AND ($3::text IS NULL OR a.code = $3)
                ORDER BY source.id, a.code, av.relationship_target_entity_id"#,
            )
            .bind(&level)
            .bind(path_source_blueprints.and_then(|ids| ids.get(ids.len() - depth)))
            .bind(
                path_fields
                    .and_then(|fields| fields.get(fields.len() - depth))
                    .or_else(|| direct_field.as_ref().filter(|v| !v.is_empty())),
            )
            .bind(self.workspace_id.0)
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
                        traversal_depth: depth as u8,
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
        let selected_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM entities WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL AND id = ANY($3) AND workspace_id = $4")
            .bind(selected.blueprint.id)
            .bind(selected_version)
            .bind(ids.keys().copied().collect::<Vec<_>>())
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?;
        Ok(selected_ids
            .into_iter()
            .filter_map(|id| ids.get(&id).cloned().map(|w| (id, w)))
            .collect())
    }

    /// Entities of `match_blueprint` whose scalar values match a term, with the
    /// matching attribute code.
    async fn scalar_search_matches(
        &self,
        term: &SearchTerm,
        match_blueprint: Option<Uuid>,
        attribute: Option<String>,
    ) -> Result<Vec<(Uuid, Option<String>)>, RepositoryError> {
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
                    WHERE workspace_id = $4 AND active
                      AND relationship_target_entity_id IS NULL
                      AND value_text ILIKE $3
                )
                SELECT DISTINCT e.id, a.code
                FROM matching_values av
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN entities e ON e.id = av.entity_id
                WHERE e.deleted_at IS NULL AND e.workspace_id = $4
                  AND a.workspace_id = $4
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)
                ORDER BY e.id, a.code"#
        } else {
            r#"SELECT DISTINCT e.id, a.code
                FROM entities e JOIN attribute_values av ON av.entity_id = e.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                WHERE e.deleted_at IS NULL AND e.workspace_id = $4
                  AND av.workspace_id = $4 AND a.workspace_id = $4
                  AND av.relationship_target_entity_id IS NULL
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)
                  AND COALESCE(av.value_text, av.value_number::text, av.value_integer::text, av.value_boolean::text, av.value_date::text, av.value_datetime::text, av.value_time::text) ILIKE $3
                ORDER BY e.id, a.code"#
        };
        let rows = sqlx::query_as::<_, (Uuid, String)>(sql)
            .bind(match_blueprint)
            .bind(attribute)
            .bind(pattern)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, code)| (id, Some(code)))
            .collect())
    }

    /// Global traversal is deliberately isolated from qualified selector search:
    /// it has a request-shared cardinality budget and runs every read under a
    /// short transaction-local PostgreSQL timeout. A rejected traversal returns
    /// before any result page or facet is computed.
    async fn resolve_global_search_term(
        &self,
        selected: &BlueprintWithAttributes,
        selected_version: Option<i64>,
        term: SearchTerm,
        budget: &mut GlobalSearchBudget,
    ) -> Result<HashMap<Uuid, MatchExplanation>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout = '250ms'")
            .execute(&mut *transaction)
            .await
            .map_err(global_search_error)?;
        let pattern = if term.prefix {
            format!("{}%", term.value)
        } else {
            format!("%{}%", term.value)
        };
        let sql = if text_only_search(&term.value) {
            r#"WITH matching_values AS MATERIALIZED (
                    SELECT entity_id, attribute_id FROM attribute_values
                    WHERE workspace_id = $2 AND active AND relationship_target_entity_id IS NULL
                      AND value_text ILIKE $1
                )
                SELECT DISTINCT e.id, a.code FROM matching_values av
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN entities e ON e.id = av.entity_id
                WHERE e.deleted_at IS NULL AND e.workspace_id = $2 AND a.workspace_id = $2
                ORDER BY e.id, a.code LIMIT $3"#
        } else {
            r#"SELECT DISTINCT e.id, a.code FROM entities e
                JOIN attribute_values av ON av.entity_id = e.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                WHERE e.deleted_at IS NULL AND e.workspace_id = $2 AND av.workspace_id = $2
                  AND a.workspace_id = $2 AND av.active AND av.relationship_target_entity_id IS NULL
                  AND COALESCE(av.value_text, av.value_number::text, av.value_integer::text,
                      av.value_boolean::text, av.value_date::text, av.value_datetime::text,
                      av.value_time::text) ILIKE $1
                ORDER BY e.id, a.code LIMIT $3"#
        };
        let rows = sqlx::query_as::<_, (Uuid, String)>(sql)
            .bind(pattern)
            .bind(self.workspace_id.0)
            .bind(budget.remaining_matches_plus_one()? as i64)
            .fetch_all(&mut *transaction)
            .await
            .map_err(global_search_error)?;
        budget.consume_matches(rows.len())?;

        let mut witnesses = HashMap::new();
        let mut frontier = VecDeque::new();
        for (id, attribute) in rows {
            if let std::collections::hash_map::Entry::Vacant(entry) = witnesses.entry(id) {
                budget.consume_entities(1)?;
                entry.insert(MatchExplanation {
                    term: term.original.clone(),
                    matching_entity_id: id,
                    matching_attribute_code: Some(attribute),
                    traversal_depth: 0,
                    relationship_path: vec![],
                });
                frontier.push_back(id);
            }
        }

        // Each level is read as one ordered batch. Ordering makes the first
        // discovered witness stable even when several incoming paths converge.
        for depth in 1..=3 {
            let level: Vec<_> = frontier.drain(..).collect();
            if level.is_empty() {
                break;
            }
            budget.consume_frontier(level.len())?;
            let edges = sqlx::query_as::<_, (Uuid, String, Uuid)>(
                r#"SELECT DISTINCT source.id, a.code, av.relationship_target_entity_id
                FROM entities source JOIN attribute_values av ON av.entity_id = source.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN entities target ON target.id = av.relationship_target_entity_id
                WHERE source.deleted_at IS NULL AND target.deleted_at IS NULL AND av.active
                  AND source.workspace_id = $2 AND target.workspace_id = $2
                  AND av.workspace_id = $2 AND a.workspace_id = $2
                  AND av.relationship_target_entity_id = ANY($1) AND a.value_type = 'relationship'
                ORDER BY source.id, a.code, av.relationship_target_entity_id LIMIT $3"#,
            )
            .bind(&level)
            .bind(self.workspace_id.0)
            .bind(budget.remaining_edges_plus_one()? as i64)
            .fetch_all(&mut *transaction)
            .await
            .map_err(global_search_error)?;
            budget.consume_edges(edges.len())?;
            // Check the next BFS layer before admitting its nodes to the
            // visited set. Otherwise the equal-sized visited cap can mask an
            // oversized frontier and make that independent guard unreachable.
            let next_frontier: HashSet<_> = edges
                .iter()
                .filter_map(|(source, _, _)| (!witnesses.contains_key(source)).then_some(*source))
                .collect();
            budget.consume_frontier(next_frontier.len())?;
            for (source, field, target) in edges {
                if !witnesses.contains_key(&source) {
                    budget.consume_entities(1)?;
                    let parent = witnesses
                        .get(&target)
                        .expect("edge target belongs to frontier");
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
        }
        let ids: Vec<_> = witnesses.keys().copied().collect();
        let selected_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM entities WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL AND id = ANY($3) AND workspace_id = $4",
        )
        .bind(selected.blueprint.id).bind(selected_version).bind(ids)
        .bind(self.workspace_id.0)
        .fetch_all(&mut *transaction).await.map_err(global_search_error)?;
        transaction.commit().await.map_err(global_search_error)?;
        metrics::histogram!("catalog_global_relationship_search_matches")
            .record(budget.matches as f64);
        metrics::histogram!("catalog_global_relationship_search_entities")
            .record(budget.entities as f64);
        metrics::histogram!("catalog_global_relationship_search_edges").record(budget.edges as f64);
        tracing::info!(
            matches = budget.matches,
            entities = budget.entities,
            edges = budget.edges,
            "global relationship search resolved"
        );
        Ok(selected_ids
            .into_iter()
            .filter_map(|id| witnesses.remove(&id).map(|w| (id, w)))
            .collect())
    }
}

const MAX_GLOBAL_SEARCH_MATCHES: usize = 1_000;
const MAX_GLOBAL_SEARCH_ENTITIES: usize = 5_000;
const MAX_GLOBAL_SEARCH_EDGES: usize = 10_000;
const MAX_GLOBAL_SEARCH_FRONTIER: usize = 5_000;

#[derive(Default)]
struct GlobalSearchBudget {
    matches: usize,
    entities: usize,
    edges: usize,
}

impl GlobalSearchBudget {
    fn remaining_matches_plus_one(&self) -> Result<usize, RepositoryError> {
        Self::remaining(MAX_GLOBAL_SEARCH_MATCHES, self.matches, "matching_values")
    }
    fn remaining_edges_plus_one(&self) -> Result<usize, RepositoryError> {
        Self::remaining(MAX_GLOBAL_SEARCH_EDGES, self.edges, "relationship_edges")
    }
    fn remaining(
        limit: usize,
        used: usize,
        dimension: &'static str,
    ) -> Result<usize, RepositoryError> {
        limit
            .checked_sub(used)
            .and_then(|remaining| remaining.checked_add(1))
            .ok_or(RepositoryError::RelationshipSearchBudgetExceeded { dimension })
    }
    fn consume_matches(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.matches += count;
        self.check(self.matches, MAX_GLOBAL_SEARCH_MATCHES, "matching_values")
    }
    fn consume_frontier(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.check(count, MAX_GLOBAL_SEARCH_FRONTIER, "frontier")
    }
    fn consume_entities(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.entities += count;
        self.check(self.entities, MAX_GLOBAL_SEARCH_ENTITIES, "entities")
    }
    fn consume_edges(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.edges += count;
        self.check(self.edges, MAX_GLOBAL_SEARCH_EDGES, "relationship_edges")
    }
    fn check(
        &self,
        actual: usize,
        limit: usize,
        dimension: &'static str,
    ) -> Result<(), RepositoryError> {
        if actual > limit {
            Err(RepositoryError::RelationshipSearchBudgetExceeded { dimension })
        } else {
            Ok(())
        }
    }
}

fn global_search_error(error: sqlx::Error) -> RepositoryError {
    if error
        .as_database_error()
        .is_some_and(|database| database.code().as_deref() == Some("57014"))
    {
        RepositoryError::RelationshipSearchTimedOut
    } else {
        RepositoryError::Database(error)
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
        fields: Vec<String>,
        source_blueprint_ids: Vec<Uuid>,
        target: Box<BlueprintWithAttributes>,
        attribute: Option<String>,
    },
    /// `@id:a,b` or `relationship.@id:a,b`: entities with one of the listed IDs,
    /// on the selected blueprint or reached through the relationship path.
    Ids {
        fields: Vec<String>,
        source_blueprint_ids: Vec<Uuid>,
        target_blueprint_id: Uuid,
        ids: Vec<Uuid>,
    },
}
/// Path leaf that matches entities by ID instead of by attribute value.
const ID_SELECTOR: &str = "@id";
/// Bounds one `@id` term; saved-view queries are length-limited as well.
const MAX_SEARCH_IDS: usize = 100;
fn parse_search_ids(term: &SearchTerm) -> Result<Vec<Uuid>, RepositoryError> {
    if term.prefix {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "wildcards are not allowed in an ID search term: '{}'",
            term.original
        )));
    }
    let mut ids = term
        .value
        .split(',')
        .map(|id| {
            Uuid::parse_str(id).map_err(|_| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "'{id}' is not an entity ID in search term '{}'",
                    term.original
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable();
    ids.dedup();
    if ids.len() > MAX_SEARCH_IDS {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "an ID search term may list at most {MAX_SEARCH_IDS} entities"
        )));
    }
    Ok(ids)
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod global_search_budget_tests {
    use super::*;

    #[test]
    fn renumbers_placeholders_without_touching_other_dollars() {
        assert_eq!(
            renumber_parameters("a = $1 AND b = $12 AND $3[x] AND '$' AND $", |n| n + 10),
            "a = $11 AND b = $22 AND $13[x] AND '$' AND $"
        );
    }

    #[test]
    fn bounds_each_global_search_dimension_without_partial_results() {
        let mut budget = GlobalSearchBudget::default();
        budget.consume_matches(MAX_GLOBAL_SEARCH_MATCHES).unwrap();
        assert!(matches!(
            budget.consume_matches(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: "matching_values"
            })
        ));

        let mut budget = GlobalSearchBudget::default();
        budget.consume_entities(MAX_GLOBAL_SEARCH_ENTITIES).unwrap();
        assert!(matches!(
            budget.consume_entities(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: "entities"
            })
        ));

        let mut budget = GlobalSearchBudget::default();
        budget.consume_edges(MAX_GLOBAL_SEARCH_EDGES).unwrap();
        assert!(matches!(
            budget.consume_edges(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: "relationship_edges"
            })
        ));
    }
}

fn relationship_sort_reverse_joins(depth: usize) -> Result<String, RepositoryError> {
    if !(1..=3).contains(&depth) {
        return Err(RepositoryError::InvalidBlueprintDefinition(
            "relationship sort path must contain one to three hops".to_owned(),
        ));
    }

    // The correlated lateral boundary keeps the ordered leaf scan outside the
    // reverse traversal. PostgreSQL can then use an incremental sort for source
    // ties instead of sorting the complete source candidate set before LIMIT.
    let mut sql =
        format!("JOIN LATERAL (SELECT path_source.id AS source_id FROM entities target_{depth}");
    for level in (1..=depth).rev() {
        sql.push_str(&format!(
            " JOIN attribute_values edge_{level} ON edge_{level}.relationship_target_entity_id = target_{level}.id \
             AND edge_{level}.workspace_id = $14 AND edge_{level}.active \
             JOIN attributes path_attribute_{level} ON path_attribute_{level}.id = edge_{level}.attribute_id \
             AND path_attribute_{level}.workspace_id = $14 AND path_attribute_{level}.code = $10[{level}] \
             AND path_attribute_{level}.value_type = 'relationship' \
             AND path_attribute_{level}.cardinality = 'one' AND path_attribute_{level}.deleted_at IS NULL \
             AND {context}",
            context = resolved_context(
                &format!("edge_{level}"),
                &format!("path_attribute_{level}"),
                "$15"
            ),
        ));
        if level == 1 {
            sql.push_str(
                " JOIN entities path_source ON path_source.id = edge_1.entity_id \
                 AND path_source.blueprint_id = path_attribute_1.blueprint_id \
                 AND path_source.blueprint_version = path_attribute_1.blueprint_version",
            );
        } else {
            let next = level - 1;
            sql.push_str(&format!(
                " JOIN entities target_{next} ON target_{next}.id = edge_{level}.entity_id \
                 AND target_{next}.workspace_id = $14 AND target_{next}.deleted_at IS NULL \
                 AND target_{next}.blueprint_id = path_attribute_{level}.blueprint_id \
                 AND target_{next}.blueprint_version = path_attribute_{level}.blueprint_version"
            ));
        }
    }
    sql.push_str(&format!(
        " WHERE target_{depth}.id = sort_value.entity_id ORDER BY path_source.id) resolved_source ON true \
         JOIN entities e ON e.id = resolved_source.source_id AND e.workspace_id = $14 AND e.deleted_at IS NULL"
    ));
    Ok(sql)
}

fn relationship_sort_forward_joins(depth: usize) -> Result<String, RepositoryError> {
    if !(1..=3).contains(&depth) {
        return Err(RepositoryError::InvalidBlueprintDefinition(
            "relationship sort path must contain one to three hops".to_owned(),
        ));
    }
    let mut sql = String::new();
    let mut source = String::from("e");
    for level in 1..=depth {
        sql.push_str(&format!(
            " LEFT JOIN attributes path_attribute_{level} ON path_attribute_{level}.blueprint_id = {source}.blueprint_id \
             AND path_attribute_{level}.blueprint_version = {source}.blueprint_version \
             AND path_attribute_{level}.workspace_id = $14 AND path_attribute_{level}.code = $10[{level}] \
             AND path_attribute_{level}.value_type = 'relationship' \
             AND path_attribute_{level}.cardinality = 'one' AND path_attribute_{level}.deleted_at IS NULL \
             LEFT JOIN attribute_values edge_{level} ON edge_{level}.entity_id = {source}.id \
             AND edge_{level}.workspace_id = $14 AND edge_{level}.attribute_id = path_attribute_{level}.id \
             AND edge_{level}.active AND edge_{level}.relationship_target_entity_id IS NOT NULL \
             AND {context} \
             LEFT JOIN entities target_{level} ON target_{level}.id = edge_{level}.relationship_target_entity_id \
             AND target_{level}.workspace_id = $14 AND target_{level}.deleted_at IS NULL",
            context = resolved_context(
                &format!("edge_{level}"),
                &format!("path_attribute_{level}"),
                "$15"
            ),
        ));
        source = format!("target_{level}");
    }
    sql.push_str(&format!(
        " LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = {source}.blueprint_id \
         AND sort_attribute.blueprint_version = {source}.blueprint_version \
         AND sort_attribute.workspace_id = $14 AND sort_attribute.code = $11 \
         AND sort_attribute.value_type = $12 AND sort_attribute.deleted_at IS NULL \
         LEFT JOIN attribute_values sort_value ON sort_value.entity_id = {source}.id \
         AND sort_value.workspace_id = $14 AND sort_value.attribute_id = sort_attribute.id \
         AND sort_value.active AND sort_value.relationship_target_entity_id IS NULL \
         AND {context}",
        context = resolved_context("sort_value", "sort_attribute", "$15"),
    ));
    Ok(sql)
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
        version: 4,
        field: sort.field.clone(),
        descending: sort.descending,
        effective_source_version: sort.effective_source_version,
        publication_context_id: sort.publication_context_id,
        context_id: Some(sort.context.requested_id()),
        is_null: row.sort_is_null,
        value: row.sort_value.clone(),
        target_id: row.sort_target_id,
        id: row.id,
    };
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).expect("cursor serializes"))
}

pub fn decode_search_cursor(cursor: &str) -> Option<(DateTime<Utc>, Uuid)> {
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
        is_sample: row.is_sample,
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
        is_sample: row.is_sample,
        created_at: row.created_at,
        display: super::entity_projection::display_labels(
            &row.preview,
            &row.blueprint_views,
            &row.blueprint_context_fallback,
        ),
        preview: row.preview,
        table_values: HashMap::new(),
        related: HashMap::new(),
        match_explanations: Vec::new(),
    }
}

struct IncomingRelationshipPreview {
    id: Uuid,
    blueprint_code: String,
    blueprint_version: i64,
    is_sample: bool,
    created_at: DateTime<Utc>,
    display: Value,
}

fn incoming_relationship_item(row: IncomingRelationshipRow) -> IncomingRelationshipPreview {
    IncomingRelationshipPreview {
        id: row.id,
        blueprint_code: row.blueprint_code,
        blueprint_version: row.blueprint_version,
        is_sample: row.is_sample,
        created_at: row.created_at,
        display: super::entity_projection::display_labels(
            &row.preview,
            &row.blueprint_views,
            &row.blueprint_context_fallback,
        ),
    }
}
