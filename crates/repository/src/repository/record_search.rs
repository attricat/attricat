use super::authorization::{GrantCodes, RECORD_GRANTED_SQL};
use super::*;
use super::{record_projection::display_label, system_annotations::validate_system_tag_filter};
use crate::model::ExportStatusFilter;
use attricat_domain::constants::DEFAULT_CONTEXT_CODE;
use attricat_domain::permissions::Permission;
use attricat_domain::states::{DataQualityFilter, RuleFindingState};
use attricat_validation::saved_search::MAX_RELATIONSHIP_HOPS;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Duration,
};
use tokio::sync::OnceCell;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RecordPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    is_sample: bool,
    system_tags: Vec<String>,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
}

#[derive(sqlx::FromRow)]
struct SortedRecordPreviewRow {
    id: Uuid,
    blueprint_version: i64,
    is_sample: bool,
    system_tags: Vec<String>,
    created_at: DateTime<Utc>,
    preview: Value,
    blueprint_views: Value,
    blueprint_context_fallback: Value,
    sort_value: Option<String>,
    sort_is_null: bool,
    sort_target_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct RecordSearchSort {
    pub field: String,
    pub relationship_path: Vec<String>,
    pub leaf_field: String,
    pub leaf_blueprint_id: Uuid,
    pub value_type: ValueType,
    pub descending: bool,
    pub effective_source_version: Option<i64>,
    pub publication_context_id: Option<Uuid>,
    /// Set for a `system_tag:` sort: records with this tag sort as 1, others
    /// as 0.
    pub system_tag: Option<String>,
    /// Values are resolved for this context path; see [`SearchContext`].
    pub context: SearchContext,
}

/// The context whose values search filters, sorts and table cells read: the
/// requested context first, then its ancestors up to the default context.
/// An attribute resolves to the nearest context on the path with an active
/// value; one with `context_fallback: none` only reads the first.
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
/// with an active value of the same record attribute. `attribute` supplies
/// `context_fallback`.
fn resolved_context(value: &str, attribute: &str, path: &str) -> String {
    format!(
        "{value}.context_id = ANY({path}::uuid[]) AND ({value}.context_id = ({path}::uuid[])[1] \
         OR ({attribute}.context_fallback <> 'none' AND NOT EXISTS (\
             SELECT 1 FROM attribute_values nearer \
              WHERE nearer.workspace_id = {value}.workspace_id AND nearer.record_id = {value}.record_id \
                AND nearer.attribute_id = {value}.attribute_id AND nearer.active \
                AND nearer.context_id = ANY(({path}::uuid[])[1:array_position({path}::uuid[], {value}.context_id) - 1]))))"
    )
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RecordRelationshipFilter {
    pub field: String,
    pub relationship_path: Vec<String>,
    pub selected_target_ids: Vec<Uuid>,
}

/// One hop of a tree facet's source path: a relationship of the previous
/// hop's target blueprint (the source blueprint for the first hop) and the
/// single blueprint it targets. The last hop targets the facet's nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationshipFacetHop {
    pub field: String,
    pub target_blueprint_id: Uuid,
}

/// The `context_path` CTE of the context bound at `$4`, nearest first.
const FACET_CONTEXT_PATH_SQL: &str = r#"context_path AS (
        SELECT id, parent_id, 0 AS depth FROM attribute_contexts WHERE id = $4
        UNION ALL
        SELECT c.id, c.parent_id, path.depth + 1
        FROM attribute_contexts c JOIN context_path path ON c.id = path.parent_id
    )"#;

/// SQL selecting the records of blueprint `blueprint` whose relationship
/// `field`, resolved along `context_path`, targets a record in `allowed`. A
/// value counts only when no nearer context on the path has one for its
/// record, and only in the requested context without fallback. It joins
/// through the target-side relationship indexes, so its cost follows the
/// size of `allowed` rather than the whole relationship.
fn facet_sources_sql(blueprint: &str, field: &str) -> String {
    format!(
        r#"SELECT source.id
           FROM allowed
           JOIN attribute_values av ON av.relationship_target_record_id = allowed.id
            AND av.active AND av.relationship_target_record_id IS NOT NULL
           JOIN attributes a ON a.id = av.attribute_id AND a.blueprint_id = {blueprint} AND a.code = {field}
           JOIN context_path path ON path.id = av.context_id
            AND (path.depth = 0 OR a.context_fallback <> 'none')
           JOIN records source ON source.id = av.record_id AND source.blueprint_id = {blueprint}
            AND source.blueprint_version = a.blueprint_version AND source.deleted_at IS NULL
           WHERE NOT EXISTS (
               SELECT 1 FROM attribute_values nearer
               JOIN context_path nearer_path ON nearer_path.id = nearer.context_id
               WHERE nearer.record_id = av.record_id AND nearer.attribute_id = av.attribute_id
                 AND nearer.active AND nearer.relationship_target_record_id IS NOT NULL
                 AND nearer_path.depth < path.depth
           )"#
    )
}

/// The cache key part naming a facet's source path.
fn facet_path_key(source_path: &[RelationshipFacetHop]) -> String {
    if source_path.is_empty() {
        return "-".to_owned();
    }
    source_path
        .iter()
        .map(|hop| hop.field.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Internal string operator: `value` is a JSON array of strings, any of which
/// may match exactly. Resolved from request-level shorthands such as `@me`;
/// clients cannot send it.
pub const SEARCH_FILTER_EQ_ANY: &str = "eq_any";

/// Applied in a transaction to the counts that run outside a request, on a pool
/// with no statement timeout of its own.
const SCOPE_STATEMENT_TIMEOUT: &str = "SET LOCAL statement_timeout = '30s'";

/// A blueprint's matching records grouped by relationship target; see
/// [`AttricatRepository::relationship_target_groups`].
#[derive(Clone, Debug)]
pub struct RelationshipTargetGroups {
    /// Every matching record.
    pub total: i64,
    /// Matching records that link to any target.
    pub linked: i64,
    pub groups: Vec<(Uuid, i64)>,
    /// More readable targets exist than `groups` holds.
    pub truncated: bool,
}

/// What narrows a search within its blueprint and version. IDs and filters
/// apply together.
#[derive(Clone, Copy)]
pub struct RecordMatching<'a> {
    /// The records a text query or tree facet resolved to, if either applies.
    pub ids: Option<&'a [Uuid]>,
    /// Evaluated inside each statement, so their matches never load into the API.
    pub filters: Option<&'a ResolvedRecordFilters>,
}

impl RecordMatching<'_> {
    /// Nothing narrows the search.
    pub const ALL: Self = Self {
        ids: None,
        filters: None,
    };
}

/// A search's attribute and relationship filters, resolved against its blueprint.
#[derive(Clone, Debug)]
pub struct ResolvedRecordFilters {
    pub context: SearchContext,
    pub filters: Vec<RecordSearchFilter>,
    pub relationship_filters: Vec<RecordRelationshipFilter>,
}

/// Binds the parameters of a [`record_filters_clause`] to any query, after all
/// of the statement's own.
macro_rules! bind_record_filters {
    ($query:expr, $filters:expr, $workspace_id:expr) => {{
        let mut query = $query;
        let filters: Option<&ResolvedRecordFilters> = $filters;
        if let Some(filters) = filters {
            query = query.bind($workspace_id).bind(&filters.context.ids);
            for filter in &filters.filters {
                query = query
                    .bind(&filter.relationship_path)
                    .bind(&filter.leaf_field)
                    .bind(&filter.value);
            }
            for filter in &filters.relationship_filters {
                query = query
                    .bind(&filter.relationship_path)
                    .bind(&filter.selected_target_ids);
            }
        }
        query
    }};
}

/// Which records of a blueprint a search page, count or version scope covers.
#[derive(Clone, Copy)]
pub struct RecordSearchScope<'a> {
    pub blueprint_id: Uuid,
    /// `None` covers every version.
    pub blueprint_version: Option<i64>,
    pub matching: RecordMatching<'a>,
    /// Records must carry every one of these system tags.
    pub system_tags: &'a [String],
    /// Only records on an older version than `current_blueprint_version`.
    pub outdated: bool,
    pub export_status: Option<&'a ExportStatusFilter>,
    pub data_quality: Option<DataQualityFilter>,
    pub current_blueprint_version: i64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RecordSearchFilter {
    pub field: String,
    pub reusable: bool,
    pub relationship_path: Vec<String>,
    pub leaf_field: String,
    pub operator: String,
    pub value_type: ValueType,
    pub value: String,
}

/// Format version of [`SortedSearchCursor`]; cursors of other versions are rejected.
const SORTED_SEARCH_CURSOR_VERSION: u8 = 4;

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
struct TablePathEdgeRow {
    source_id: Uuid,
    code: String,
    target_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct TablePathLeafRow {
    id: Uuid,
    code: String,
    #[sqlx(try_from = "String")]
    value_type: ValueType,
    inherit: bool,
    preview: Option<Value>,
}

/// Table cell values by record and column path.
type TableValues = HashMap<Uuid, HashMap<String, Vec<Value>>>;

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
struct RecordLabelRow {
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

/// The scalar value types `is_set` tests besides a filter's own type.
const PRESENCE_VALUE_TYPES: &str = "'string','number','integer','boolean','date','datetime','time'";

/// How PostgreSQL may run a filter's `EXISTS` subquery.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterPlan {
    /// A semi-join, which can start from the value's index. It's far faster when
    /// few records match, but much slower when a filter has to scan anyway.
    FromValue,
    /// Once per candidate record, which `OFFSET 0` forces. A page stops as soon
    /// as it's full, and candidates a text query already narrowed stay cheap.
    PerRecord,
}

/// An `EXISTS` subquery over tables with their conditions. Every condition sits
/// in `WHERE`, because PostgreSQL only considers a semi-join when outer
/// references appear nowhere else.
fn exists_sql(tables: &[(String, String)], condition: &str, plan: FilterPlan) -> String {
    let from = tables
        .iter()
        .map(|(table, _)| table.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let conditions = tables
        .iter()
        .map(|(_, condition)| condition.as_str())
        .collect::<Vec<_>>()
        .join(" AND ");
    let offset = if plan == FilterPlan::PerRecord {
        "OFFSET 0"
    } else {
        ""
    };
    format!("EXISTS (SELECT 1 FROM {from} WHERE {conditions} AND {condition} {offset})")
}

/// The tables walking `hops` relationships from `e`, with the alias of the
/// record they reach. Codes come from the array parameter `path`. Each hop reads
/// its source record's pinned blueprint revision and resolves its edges for the
/// context path `context`.
fn relationship_hop_tables(
    hops: usize,
    path: usize,
    workspace: usize,
    context: usize,
) -> (Vec<(String, String)>, String) {
    let mut tables = Vec::with_capacity(hops * 4 + 2);
    let mut source = "e".to_owned();
    for hop in 1..=hops {
        let (relationship, edge, target, blueprint) = (
            format!("relationship_{hop}"),
            format!("edge_{hop}"),
            format!("target_{hop}"),
            format!("target_blueprint_{hop}"),
        );
        tables.push((
            format!("attributes {relationship}"),
            format!(
                "{relationship}.blueprint_id = {source}.blueprint_id \
                 AND {relationship}.blueprint_version = {source}.blueprint_version \
                 AND {relationship}.workspace_id = ${workspace} \
                 AND {relationship}.code = ${path}[{hop}] \
                 AND {relationship}.value_type = 'relationship'"
            ),
        ));
        tables.push((
            format!("attribute_values {edge}"),
            format!(
                "{edge}.record_id = {source}.id AND {edge}.workspace_id = ${workspace} \
                 AND {edge}.attribute_id = {relationship}.id AND {edge}.active \
                 AND {edge}.relationship_target_record_id IS NOT NULL AND {}",
                resolved_context(&edge, &relationship, &format!("${context}"))
            ),
        ));
        tables.push((
            format!("records {target}"),
            format!(
                "{target}.id = {edge}.relationship_target_record_id \
                 AND {target}.workspace_id = ${workspace} AND {target}.deleted_at IS NULL"
            ),
        ));
        tables.push((
            format!("blueprints {blueprint}"),
            format!(
                "{blueprint}.id = {target}.blueprint_id \
                 AND {blueprint}.version = {target}.blueprint_version \
                 AND {blueprint}.code = {relationship}.target_blueprint_code"
            ),
        ));
        source = target;
    }
    (tables, source)
}

/// One filter's test of the leaf value `av`, against the text parameter `value`.
/// Columns and operators come from closed lists, never from the request.
fn value_comparison(filter: &RecordSearchFilter, value: usize) -> String {
    const NEVER: &str = "FALSE";
    if filter.operator == attricat_validation::saved_search::FILTER_OPERATOR_IS_SET {
        return format!(
            "(a.value_type <> '{file}' OR EXISTS (SELECT 1 FROM attribute_file_references file_reference \
              WHERE file_reference.attribute_value_id = av.id \
                AND file_reference.workspace_id = av.workspace_id))",
            file = ValueType::File.as_str()
        );
    }
    let Ok((column, column_type)) = native_sort_column(filter.value_type) else {
        return NEVER.to_owned();
    };
    let text = filter.value_type == ValueType::String;
    let ordered = !text && filter.value_type != ValueType::Boolean;
    match filter.operator.as_str() {
        "eq" if text => format!("av.{column} = ${value}"),
        "eq" => format!("av.{column} = ${value}::{column_type}"),
        SEARCH_FILTER_EQ_ANY if text => format!("${value}::jsonb ? av.{column}"),
        "contains" if text => format!("strpos(lower(av.{column}), lower(${value})) > 0"),
        "starts_with" if text => {
            format!("left(lower(av.{column}), char_length(${value})) = lower(${value})")
        }
        "gt" if ordered => format!("av.{column} > ${value}::{column_type}"),
        "gte" if ordered => format!("av.{column} >= ${value}::{column_type}"),
        "lt" if ordered => format!("av.{column} < ${value}::{column_type}"),
        "lte" if ordered => format!("av.{column} <= ${value}::{column_type}"),
        _ => NEVER.to_owned(),
    }
}

/// One attribute filter over `e`. Absence is the complement of presence, so it
/// also holds for records with no reached leaf, older revisions without the
/// field, or no attached reusable attribute. An active scalar row counts as
/// present even for empty text, zero or false; a file value is present only
/// while it references at least one file.
fn attribute_filter_sql(
    filter: &RecordSearchFilter,
    workspace: usize,
    context: usize,
    parameter: usize,
    narrowed: bool,
) -> String {
    let (path, leaf_code, value) = (parameter, parameter + 1, parameter + 2);
    let presence = filter.operator == attricat_validation::saved_search::FILTER_OPERATOR_IS_SET;
    let value_type = filter.value_type.as_str();
    let type_matches = if presence {
        format!("(a.value_type = '{value_type}' OR a.value_type IN ({PRESENCE_VALUE_TYPES}))")
    } else {
        format!("a.value_type = '{value_type}'")
    };
    let current_value = |leaf: &str| {
        (
            "attribute_values av".to_owned(),
            format!(
                "av.record_id = {leaf}.id AND av.attribute_id = a.id \
                 AND av.workspace_id = ${workspace} AND av.active \
                 AND av.relationship_target_record_id IS NULL AND {}",
                resolved_context("av", "a", &format!("${context}"))
            ),
        )
    };
    let tables = if filter.reusable {
        vec![
            (
                "attributes a".to_owned(),
                format!(
                    "a.record_id = e.id AND a.code = ${leaf_code} AND {type_matches} \
                     AND a.workspace_id = ${workspace} AND a.deleted_at IS NULL"
                ),
            ),
            (
                "reusable_attribute_revisions revision".to_owned(),
                "revision.id = a.reusable_attribute_revision_id AND revision.searchable".to_owned(),
            ),
            current_value("e"),
        ]
    } else {
        let (mut tables, leaf) =
            relationship_hop_tables(filter.relationship_path.len(), path, workspace, context);
        tables.push((
            "attributes a".to_owned(),
            format!(
                "a.blueprint_id = {leaf}.blueprint_id \
                 AND a.blueprint_version = {leaf}.blueprint_version \
                 AND a.workspace_id = ${workspace} AND a.code = ${leaf_code} \
                 AND a.deleted_at IS NULL AND {type_matches}"
            ),
        ));
        tables.push(current_value(&leaf));
        tables
    };
    let absence = presence && filter.value == "false";
    let from_value = filter.operator == "eq"
        || filter.operator == SEARCH_FILTER_EQ_ANY
        || (filter.reusable && !absence);
    let plan = if from_value && !narrowed {
        FilterPlan::FromValue
    } else {
        FilterPlan::PerRecord
    };
    let exists = exists_sql(&tables, &value_comparison(filter, value), plan);
    if absence {
        format!("NOT {exists}")
    } else {
        exists
    }
}

/// One `AND` condition per filter restricting `e`, or nothing without filters.
/// Each stays correlated to `e`, so PostgreSQL can start from whichever side is
/// smaller and a page can stop early. Parameters start at `first_parameter`,
/// past the statement's own, and are bound with [`bind_record_filters`]:
/// workspace and context path, then each filter's own in order.
fn record_filters_clause(matching: RecordMatching<'_>, first_parameter: usize) -> String {
    let Some(filters) = matching.filters else {
        return String::new();
    };
    let narrowed = matching.ids.is_some();
    let (workspace, context) = (first_parameter, first_parameter + 1);
    let mut next = first_parameter + 2;
    let mut conditions = Vec::new();
    for filter in &filters.filters {
        conditions.push(attribute_filter_sql(
            filter, workspace, context, next, narrowed,
        ));
        next += 3;
    }
    for filter in &filters.relationship_filters {
        let (tables, _) =
            relationship_hop_tables(filter.relationship_path.len(), next, workspace, context);
        let last_edge = format!("edge_{}", filter.relationship_path.len());
        conditions.push(exists_sql(
            &tables,
            &format!(
                "{last_edge}.relationship_target_record_id = ANY(${}::uuid[])",
                next + 1
            ),
            if narrowed {
                FilterPlan::PerRecord
            } else {
                FilterPlan::FromValue
            },
        ));
        next += 2;
    }
    conditions
        .iter()
        .map(|condition| format!("AND {condition}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `context_path`, `chosen` and `edges(source_id, target_id)` CTEs of one
/// relationship field: `$1` source blueprint, `$2` field, `$4` context, `$5`
/// optional source version. `{sources}` narrows the sources with a join on
/// `av.record_id`, and `{target}` selects targets. Choosing each source's context
/// in one sorted pass, rather than with a per-source LATERAL subquery, keeps this
/// a few index scans and hash joins whose row counts the planner estimates well.
const RESOLVED_EDGES_SQL: &str = r#"context_path AS (
                    SELECT id, parent_id, 0 AS depth FROM attribute_contexts WHERE id = $4
                    UNION ALL
                    SELECT c.id, c.parent_id, path.depth + 1
                    FROM attribute_contexts c JOIN context_path path ON c.id = path.parent_id
                ), chosen AS (
                    SELECT DISTINCT ON (av.record_id) av.record_id, av.attribute_id, av.context_id
                    FROM attributes a
                    JOIN context_path path ON path.depth = 0 OR a.context_fallback <> 'none'
                    JOIN attribute_values av ON av.attribute_id = a.id AND av.context_id = path.id
                     AND av.active AND av.relationship_target_record_id IS NOT NULL
                    {sources}
                    JOIN records source ON source.id = av.record_id AND source.blueprint_id = $1
                     AND source.blueprint_version = a.blueprint_version AND source.deleted_at IS NULL
                    WHERE a.blueprint_id = $1 AND a.code = $2
                      AND ($5::bigint IS NULL OR source.blueprint_version = $5)
                    ORDER BY av.record_id, path.depth
                ), edges(source_id, target_id) AS (
                    SELECT DISTINCT chosen.record_id, av.relationship_target_record_id
                    FROM chosen
                    JOIN attribute_values av ON av.record_id = chosen.record_id
                     AND av.attribute_id = chosen.attribute_id AND av.context_id = chosen.context_id
                     AND av.active AND av.relationship_target_record_id IS NOT NULL
                    JOIN records target ON target.id = av.relationship_target_record_id
                    WHERE {target} AND target.deleted_at IS NULL
                )"#;

/// The IDs from a root down to `node`'s parent, along the shortest way up and
/// the lowest parent IDs among equally short ones. A node that reaches no
/// root, because every way up loops, follows its lowest parents until one
/// repeats.
fn ancestor_path(parents_by_child: &HashMap<Uuid, Vec<Uuid>>, node: Uuid) -> Vec<Uuid> {
    let sorted_parents = |child: &Uuid| {
        let mut parents = parents_by_child.get(child).cloned().unwrap_or_default();
        parents.sort_unstable();
        parents
    };
    let mut child_of: HashMap<Uuid, Uuid> = HashMap::new();
    let mut queue = VecDeque::from([node]);
    let mut seen = HashSet::from([node]);
    let mut root = None;
    while let Some(current) = queue.pop_front() {
        if current != node && !parents_by_child.contains_key(&current) {
            root = Some(current);
            break;
        }
        for parent in sorted_parents(&current) {
            if seen.insert(parent) {
                child_of.insert(parent, current);
                queue.push_back(parent);
            }
        }
    }
    let mut path = Vec::new();
    match root {
        Some(root) => {
            let mut current = root;
            while current != node {
                path.push(current);
                current = child_of[&current];
            }
        }
        None => {
            let mut seen = HashSet::from([node]);
            let mut current = node;
            while let Some(&parent) = sorted_parents(&current).first() {
                if !seen.insert(parent) {
                    break;
                }
                path.push(parent);
                current = parent;
            }
            path.reverse();
        }
    }
    path
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

impl AttricatRepository {
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
    ) -> Result<RecordPreviewPage, RepositoryError> {
        let context_fallback_target = context_fallback_column("target");
        validate_code(blueprint_code)?;
        validate_code(relationship)?;
        let rows = sqlx::query_as::<_, RecordPreviewRow>(
            &format!(r#"SELECT target.id, target.blueprint_version, ('attricat.sample'=ANY(target.system_tags)) AS is_sample, target.system_tags, target.created_at, target.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      {context_fallback_target} AS blueprint_context_fallback
               FROM attribute_values av
               JOIN records source ON source.id = av.record_id
                AND source.workspace_id = $6 AND source.deleted_at IS NULL
               JOIN attributes a ON a.id = av.attribute_id
                AND a.workspace_id = $6 AND a.deleted_at IS NULL
               JOIN records target ON target.id = av.relationship_target_record_id
               JOIN blueprints b ON b.id = target.blueprint_id AND b.version = target.blueprint_version
               WHERE av.record_id = $1
                 AND av.workspace_id = $6
                 AND a.workspace_id = $6
                 AND a.code = $2
                 AND av.relationship_target_record_id IS NOT NULL
                  AND av.active
                 AND ($3::uuid IS NULL OR av.relationship_target_record_id > $3)
                 AND target.workspace_id = $6 AND target.deleted_at IS NULL
                 AND b.workspace_id = $6 AND b.code = $4
               ORDER BY target.id
               LIMIT $5"#),
        )
        .bind(related_from)
        .bind(relationship)
        .bind(cursor)
        .bind(blueprint_code)
        .bind(limit + 1)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<_> = rows.into_iter().map(record_preview).collect();
        let next_cursor = next_page_cursor(&mut items, limit, |item| item.id);
        Ok(RecordPreviewPage { items, next_cursor })
    }

    /// Display labels of the live records among `record_ids`, in ID order.
    /// Callers authorize the IDs first; unknown and deleted IDs are omitted.
    pub async fn record_labels(
        &self,
        record_ids: &[Uuid],
    ) -> Result<Vec<RecordLabel>, RepositoryError> {
        let context_fallback_e = context_fallback_column("e");
        if record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, RecordLabelRow>(&format!(
            r#"SELECT e.id, b.code AS blueprint_code, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      {context_fallback_e} AS blueprint_context_fallback
               FROM records e
               JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                AND b.workspace_id = $2
               WHERE e.workspace_id = $2 AND e.deleted_at IS NULL AND e.id = ANY($1)
               ORDER BY e.id"#
        ))
        .bind(record_ids)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RecordLabel {
                id: row.id,
                blueprint_code: row.blueprint_code,
                display: super::record_projection::display_labels(
                    &row.preview,
                    &row.blueprint_views,
                    &row.blueprint_context_fallback,
                ),
            })
            .collect())
    }

    /// The relationship fields whose active values on live records target
    /// `record_id`, with the number of distinct source records per field,
    /// so a caller can find every incoming edge without naming selectors.
    pub async fn incoming_relationship_fields(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<IncomingRelationshipField>, RepositoryError> {
        self.get_record(record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        Ok(sqlx::query_as::<_, (String, String, i64)>(
            r#"SELECT b.code, a.code, COUNT(DISTINCT source.id)
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
                AND a.workspace_id = $2 AND a.deleted_at IS NULL
                AND a.value_type = 'relationship'
               JOIN records source ON source.id = av.record_id
                AND source.workspace_id = $2 AND source.deleted_at IS NULL
               JOIN blueprints b ON b.id = source.blueprint_id
                AND b.version = source.blueprint_version AND b.workspace_id = $2
               WHERE av.workspace_id = $2 AND av.active
                 AND av.relationship_target_record_id = $1
               GROUP BY b.code, a.code
               ORDER BY b.code, a.code"#,
        )
        .bind(record_id)
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
        record_id: Uuid,
        relationships: Vec<IncomingRelationshipSelector>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<IncomingRelationshipsPage, RepositoryError> {
        let context_fallback_source = context_fallback_column("source");
        self.get_record(record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        for relationship in &relationships {
            validate_code(&relationship.source_blueprint)?;
            validate_code(&relationship.field)?;
        }
        let selectors = serde_json::to_value(relationships)
            .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))?;
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = sqlx::query_as::<_, IncomingRelationshipRow>(
            &format!(r#"SELECT source.id, b.code AS blueprint_code, source.blueprint_version,
                      ('attricat.sample'=ANY(source.system_tags)) AS is_sample, source.created_at, source.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      {context_fallback_source} AS blueprint_context_fallback
               FROM records source
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
                     WHERE av.record_id = source.id
                       AND av.workspace_id = $6
                       AND av.relationship_target_record_id = $1
                       AND av.active
                       AND a.value_type = 'relationship'
                 )
                 AND ($3::timestamptz IS NULL OR (source.created_at, source.id) > ($3, $4))
               ORDER BY source.created_at, source.id
               LIMIT $5"#),
        )
        .bind(record_id)
        .bind(selectors)
        .bind(cursor_created_at)
        .bind(cursor_id)
        .bind(limit + 1)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<_> = rows.into_iter().map(incoming_relationship_item).collect();
        let next_cursor = next_page_cursor(&mut items, limit, |item| {
            encode_search_cursor(item.created_at, item.id)
        });
        // Advance the cursor over the fetched page, including hidden rows, so
        // a scoped reader can continue past a page with no readable sources.
        let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
        if let Some(readable) = self.actor_readable_record_ids(&ids).await? {
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

    pub async fn search_record_previews(
        &self,
        scope: RecordSearchScope<'_>,
        limit: i64,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<(Vec<RecordPreview>, Option<String>), RepositoryError> {
        let context_fallback_e = context_fallback_column("e");
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        validate_system_tag_filter(system_tags)?;
        let export_status_sql = record_filter_clauses(11, export_status, data_quality);
        let filters_sql = record_filters_clause(matching, 12);
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.system_tags, e.created_at, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      {context_fallback_e} AS blueprint_context_fallback
                FROM records e
                JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                WHERE e.workspace_id = $10
                  AND e.blueprint_id = $1
                  AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                 AND e.deleted_at IS NULL
                  AND (
                      $3::timestamptz IS NULL
                      OR (e.created_at, e.id) > ($3, $4)
                   )
                  AND ($5::uuid[] IS NULL OR e.id = ANY($5))
                  {filters_sql}
                  AND ($6::text[] IS NULL OR e.system_tags @> $6)
                  {export_status_sql}
                  AND (NOT $7 OR e.blueprint_version <> $8)
                 ORDER BY e.created_at, e.id
                 LIMIT $9"#
        );
        let (cursor_created_at, cursor_id) = cursor.unzip();
        let rows = bind_record_filters!(
            sqlx::query_as::<_, RecordPreviewRow>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(cursor_created_at)
                .bind(cursor_id)
                .bind(matching.ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(limit + 1)
                .bind(self.workspace_id.0)
                .bind(export_status_param(export_status)),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_all(&self.pool)
        .await?;
        let mut items: Vec<_> = rows.into_iter().map(record_preview).collect();
        let next_cursor = next_page_cursor(&mut items, limit, |item| {
            encode_search_cursor(item.created_at, item.id)
        });
        Ok((items, next_cursor))
    }

    /// Counts matching records up to `limit`, so broad searches never scan the entire catalog.
    pub async fn count_record_previews(
        &self,
        scope: RecordSearchScope<'_>,
        limit: i64,
    ) -> Result<i64, RepositoryError> {
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        validate_system_tag_filter(system_tags)?;
        let (records, matching_sql) = matching_records(matching.ids.is_some(), 3);
        let export_status_sql = record_filter_clauses(9, export_status, data_quality);
        let filters_sql = record_filters_clause(matching, 10);
        let sql = format!(
            r#"SELECT COUNT(*)
                 FROM (
                    SELECT 1
                      FROM {records}
                     WHERE e.workspace_id = $8
                       AND e.blueprint_id = $1
                       AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                       AND e.deleted_at IS NULL
                       {matching_sql}
                       {filters_sql}
                       AND ($4::text[] IS NULL OR e.system_tags @> $4)
                       {export_status_sql}
                       AND (NOT $5 OR e.blueprint_version <> $6)
                     LIMIT $7
                 ) matches"#,
        );
        Ok(bind_record_filters!(
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching.ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(limit)
                .bind(self.workspace_id.0)
                .bind(export_status_param(export_status)),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// Counts up to `limit` of a blueprint's matching records, within
    /// [`SCOPE_STATEMENT_TIMEOUT`] for callers without a request's time limit.
    pub async fn count_matching_records(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching: RecordMatching<'_>,
        limit: i64,
    ) -> Result<i64, RepositoryError> {
        let sql = format!(
            "SELECT COUNT(*) FROM (SELECT 1 {} LIMIT $5) matches",
            matching_records_from(matching)
        );
        let mut transaction = self.pool.begin().await?;
        sqlx::query(SCOPE_STATEMENT_TIMEOUT)
            .execute(&mut *transaction)
            .await?;
        let count = bind_record_filters!(
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching.ids)
                .bind(self.workspace_id.0)
                .bind(limit),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(count)
    }

    /// Up to `limit` of a blueprint's matching records, in no particular order, so
    /// a broad search stops as soon as it has them.
    pub async fn first_matching_record_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching: RecordMatching<'_>,
        limit: i64,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let sql = format!("SELECT e.id {} LIMIT $5", matching_records_from(matching));
        Ok(bind_record_filters!(
            sqlx::query_scalar::<_, Uuid>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching.ids)
                .bind(self.workspace_id.0)
                .bind(limit),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Up to `limit` of a blueprint's matching records, lowest IDs first.
    pub async fn matching_record_ids(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching: RecordMatching<'_>,
        limit: i64,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let sql = format!(
            "SELECT e.id {} ORDER BY e.id LIMIT $5",
            matching_records_from(matching)
        );
        Ok(bind_record_filters!(
            sqlx::query_scalar::<_, Uuid>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching.ids)
                .bind(self.workspace_id.0)
                .bind(limit),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_all(&self.pool)
        .await?)
    }

    async fn search_result_versions(
        &self,
        scope: RecordSearchScope<'_>,
    ) -> Result<Vec<i64>, RepositoryError> {
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        validate_system_tag_filter(system_tags)?;
        let export_status_sql = record_filter_clauses(8, export_status, data_quality);
        let matches = result_version_matches(matching, &export_status_sql, 9);
        let sql = format!(
            r#"SELECT version.version
                 FROM blueprints version
                WHERE version.id = $1
                  AND ($2::bigint IS NULL OR version.version = $2)
                  AND version.workspace_id = $3
                  AND EXISTS (SELECT 1 {matches} AND e.blueprint_version = version.version)
                ORDER BY version.version DESC
                LIMIT {MAX_RESULT_VERSIONS}"#,
        );
        Ok(bind_record_filters!(
            sqlx::query_scalar::<_, i64>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(self.workspace_id.0)
                .bind(matching.ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(export_status_param(export_status)),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// [`Self::search_result_versions`] and, with `count_limit`,
    /// [`Self::count_record_previews`] over the same matches in one statement,
    /// each stopping as soon as it has its answer.
    pub async fn search_result_versions_and_count(
        &self,
        scope: RecordSearchScope<'_>,
        count_limit: Option<i64>,
    ) -> Result<(Vec<i64>, Option<i64>), RepositoryError> {
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        let Some(count_limit) = count_limit else {
            return Ok((
                self.search_result_versions(RecordSearchScope {
                    blueprint_id,
                    blueprint_version,
                    matching,
                    system_tags,
                    outdated,
                    export_status,
                    data_quality,
                    current_blueprint_version,
                })
                .await?,
                None,
            ));
        };
        validate_system_tag_filter(system_tags)?;
        let export_status_sql = record_filter_clauses(9, export_status, data_quality);
        let matches = result_version_matches(matching, &export_status_sql, 10);
        let sql = format!(
            r#"SELECT ARRAY(
                          SELECT version.version
                            FROM blueprints version
                           WHERE version.id = $1
                             AND ($2::bigint IS NULL OR version.version = $2)
                             AND version.workspace_id = $3
                             AND EXISTS (SELECT 1 {matches} AND e.blueprint_version = version.version)
                           ORDER BY version.version DESC
                           LIMIT {MAX_RESULT_VERSIONS}
                      ),
                      (SELECT COUNT(*) FROM (SELECT 1 {matches} LIMIT $8) capped)"#,
        );
        let (versions, count) = bind_record_filters!(
            sqlx::query_as::<_, (Vec<i64>, i64)>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(self.workspace_id.0)
                .bind(matching.ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(count_limit)
                .bind(export_status_param(export_status)),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_one(&self.pool)
        .await?;
        Ok((versions, Some(count)))
    }

    /// Selects a page by a configured scalar table column without hydrating projections for
    /// every candidate. NULL values sort first ascending and last descending; record IDs make
    /// the ordering stable.
    pub async fn search_record_previews_sorted(
        &self,
        scope: RecordSearchScope<'_>,
        limit: i64,
        cursor: Option<&str>,
        sort: &RecordSearchSort,
    ) -> Result<(Vec<RecordPreview>, Option<String>), RepositoryError> {
        let context_fallback_e = context_fallback_column("e");
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        validate_system_tag_filter(system_tags)?;
        let cursor = cursor.map(decode_sorted_search_cursor).transpose()?;
        if let Some(cursor) = &cursor
            && (cursor.field != sort.field
                || cursor.descending != sort.descending
                || cursor.effective_source_version != sort.effective_source_version
                || cursor.publication_context_id != sort.publication_context_id
                || cursor.context_id != Some(sort.context.requested_id())
                || cursor.version != SORTED_SEARCH_CURSOR_VERSION)
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
        if sort.system_tag.is_some()
            || matches!(
                sort.field.as_str(),
                "blueprint_version" | "publication_status"
            )
        {
            return self
                .search_record_previews_sorted_system(
                    RecordSearchScope {
                        blueprint_id,
                        blueprint_version,
                        matching,
                        system_tags,
                        outdated,
                        export_status,
                        data_quality,
                        current_blueprint_version,
                    },
                    limit,
                    sort,
                    cursor.as_ref(),
                )
                .await;
        }
        if !sort.relationship_path.is_empty() {
            return self
                .search_record_previews_sorted_relationship(
                    RecordSearchScope {
                        blueprint_id,
                        blueprint_version,
                        matching,
                        system_tags,
                        outdated,
                        export_status,
                        data_quality,
                        current_blueprint_version,
                    },
                    limit,
                    sort,
                    cursor_value,
                    cursor_is_null,
                    cursor_target_id,
                    cursor_id,
                )
                .await;
        }
        let (column, column_type) = native_sort_column(sort.value_type)?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let null_order = if sort.descending { "LAST" } else { "FIRST" };
        let nulls_first = !sort.descending;
        let joins =
            "LEFT JOIN attributes sort_attribute ON sort_attribute.blueprint_id = e.blueprint_id
                    AND sort_attribute.blueprint_version = e.blueprint_version
                    AND sort_attribute.workspace_id = $13
                    AND sort_attribute.code = $10 AND sort_attribute.value_type = $11
                 LEFT JOIN attribute_values sort_value ON sort_value.record_id = e.id
                    AND sort_value.workspace_id = $13
                    AND sort_value.attribute_id = sort_attribute.id
                    AND {value_context}
                    AND sort_value.relationship_target_record_id IS NULL AND sort_value.active"
                .replace(
                    "{value_context}",
                    &resolved_context("sort_value", "sort_attribute", "$14"),
                );
        let export_status_sql = record_filter_clauses(15, export_status, data_quality);
        let filters_sql = record_filters_clause(matching, 16);
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.system_tags, e.created_at, e.projections -> 'preview' AS preview,
                      b.views AS blueprint_views,
                      {context_fallback_e} AS blueprint_context_fallback,
                      sort_value.{column}::text AS sort_value, sort_value.{column} IS NULL AS sort_is_null,
                      e.id AS sort_target_id
                FROM records e
                JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                {joins}
                WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                  AND e.workspace_id = $13 AND e.deleted_at IS NULL
                  AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                  {filters_sql}
                  AND ($4::text[] IS NULL OR e.system_tags @> $4)
                  {export_status_sql}
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
                LIMIT $12"#,
            column = column,
            column_type = column_type,
            nulls_first = nulls_first,
            null_order = null_order,
        );
        let mut rows = bind_record_filters!(
            sqlx::query_as::<_, SortedRecordPreviewRow>(&sql)
                .bind(blueprint_id)
                .bind(blueprint_version)
                .bind(matching.ids)
                .bind((!system_tags.is_empty()).then_some(system_tags))
                .bind(outdated)
                .bind(current_blueprint_version)
                .bind(cursor_value)
                .bind(cursor_is_null)
                .bind(cursor_id)
                .bind(&sort.leaf_field)
                .bind(sort.value_type.as_str())
                .bind(limit + 1)
                .bind(self.workspace_id.0)
                .bind(&sort.context.ids)
                .bind(export_status_param(export_status)),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_all(&self.pool)
        .await?;
        let next_cursor = next_page_cursor(&mut rows, limit, |row| {
            encode_sorted_search_cursor(sort, row)
        });
        Ok((
            rows.into_iter().map(sorted_record_preview).collect(),
            next_cursor,
        ))
    }

    /// Sort a built-in record property independently of blueprint table columns.
    /// Record ID breaks ties so pagination cannot omit or duplicate rows.
    async fn search_record_previews_sorted_system(
        &self,
        scope: RecordSearchScope<'_>,
        limit: i64,
        sort: &RecordSearchSort,
        cursor: Option<&SortedSearchCursor>,
    ) -> Result<(Vec<RecordPreview>, Option<String>), RepositoryError> {
        let context_fallback_e = context_fallback_column("e");
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
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
        let (expression, join) = if sort.system_tag.is_some() {
            (
                "CASE WHEN $12 = ANY(e.system_tags) THEN 1 ELSE 0 END",
                String::new(),
            )
        } else if sort.field == "publication_status" {
            (
                "CASE WHEN p.published_at IS NULL THEN 0 ELSE 1 END",
                "LEFT JOIN record_channel_publications p ON p.workspace_id = e.workspace_id AND p.record_id = e.id AND p.context_id = $12".to_owned(),
            )
        } else {
            ("e.blueprint_version", String::new())
        };
        let export_status_sql = record_filter_clauses(11, export_status, data_quality);
        let binds_sort_parameter = sort.system_tag.is_some() || sort.field == "publication_status";
        let filters_sql =
            record_filters_clause(matching, if binds_sort_parameter { 13 } else { 12 });
        let sql = format!(
            r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.system_tags, e.created_at, e.projections -> 'preview' AS preview,
                         b.views AS blueprint_views,
                         {context_fallback_e} AS blueprint_context_fallback,
                         ({expression})::text AS sort_value, false AS sort_is_null, e.id AS sort_target_id
                  FROM records e
                  JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                  {join}
                  WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                    AND e.workspace_id = $10 AND e.deleted_at IS NULL
                    AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                    {filters_sql}
                    AND ($4::text[] IS NULL OR e.system_tags @> $4)
                    {export_status_sql}
                    AND (NOT $5 OR e.blueprint_version <> $6)
                    AND ($8::uuid IS NULL OR ({expression}) {comparison} $7::bigint
                         OR (({expression}) = $7 AND e.id > $8))
                  ORDER BY {expression} {direction}, e.id ASC
                  LIMIT $9"#
        );
        let query = sqlx::query_as::<_, SortedRecordPreviewRow>(&sql)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(matching.ids)
            .bind((!system_tags.is_empty()).then_some(system_tags))
            .bind(outdated)
            .bind(current_blueprint_version)
            .bind(cursor_version)
            .bind(cursor.map(|cursor| cursor.id))
            .bind(limit + 1)
            .bind(self.workspace_id.0)
            .bind(export_status_param(export_status));
        let mut rows = if let Some(tag) = &sort.system_tag {
            bind_record_filters!(query.bind(tag), matching.filters, self.workspace_id.0)
                .fetch_all(&self.pool)
                .await?
        } else if sort.field == "publication_status" {
            bind_record_filters!(
                query.bind(sort.publication_context_id),
                matching.filters,
                self.workspace_id.0
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            bind_record_filters!(query, matching.filters, self.workspace_id.0)
                .fetch_all(&self.pool)
                .await?
        };
        let next_cursor = next_page_cursor(&mut rows, limit, |row| {
            encode_sorted_search_cursor(sort, row)
        });
        Ok((
            rows.into_iter().map(sorted_record_preview).collect(),
            next_cursor,
        ))
    }

    /// Sorts a single-valued relationship path from its indexed scalar leaf back to
    /// source records. Non-null values are paged first; missing/incompatible paths
    /// use a separate source-ID phase so they do not force a full mixed-value sort.
    #[allow(clippy::too_many_arguments)]
    async fn search_record_previews_sorted_relationship(
        &self,
        scope: RecordSearchScope<'_>,
        limit: i64,
        sort: &RecordSearchSort,
        cursor_value: Option<String>,
        cursor_is_null: bool,
        cursor_target_id: Option<Uuid>,
        cursor_id: Option<Uuid>,
    ) -> Result<(Vec<RecordPreview>, Option<String>), RepositoryError> {
        let context_fallback_e = context_fallback_column("e");
        let RecordSearchScope {
            blueprint_id,
            blueprint_version,
            matching,
            system_tags,
            outdated,
            export_status,
            data_quality,
            current_blueprint_version,
        } = scope;
        let (column, column_type) = native_sort_column(sort.value_type)?;
        let comparison = if sort.descending { "<" } else { ">" };
        let direction = if sort.descending { "DESC" } else { "ASC" };
        let workspace_id = self.workspace_id.0;
        let leaf_attribute_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attributes WHERE workspace_id = $1 AND blueprint_id = $2 AND code = $3 AND value_type = $4 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(sort.leaf_blueprint_id)
        .bind(&sort.leaf_field)
        .bind(sort.value_type.as_str())
        .fetch_all(&self.pool)
        .await?;
        let mut rows = Vec::new();
        let leaf_export_status = record_filter_clauses(18, export_status, data_quality);
        let null_export_status = record_filter_clauses(16, export_status, data_quality);
        let leaf_filters = record_filters_clause(matching, 19);
        let null_filters = record_filters_clause(matching, 17);

        if !cursor_is_null {
            let reverse_joins = relationship_sort_reverse_joins(sort.relationship_path.len())?;
            let leaf_context = resolved_context("leaf", "leaf_attribute", "$15");
            let sql = format!(
                r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.system_tags, e.created_at, e.projections -> 'preview' AS preview,
                          b.views AS blueprint_views,
                          {context_fallback_e} AS blueprint_context_fallback,
                          sort_value.{column}::text AS sort_value, false AS sort_is_null,
                          sort_value.record_id AS sort_target_id
                     FROM (
                         SELECT leaf.record_id, leaf.{column}
                           FROM attribute_values leaf
                           JOIN attributes leaf_attribute ON leaf_attribute.id = leaf.attribute_id
                          WHERE leaf.workspace_id = $14 AND {leaf_context}
                            AND leaf.active AND leaf.relationship_target_record_id IS NULL
                            AND leaf.attribute_id = ANY($17) AND leaf.{column} IS NOT NULL
                            AND ($9::uuid IS NULL OR leaf.{column} {comparison} $7::{column_type}
                                 OR (leaf.{column} = $7::{column_type} AND leaf.record_id >= $16))
                          ORDER BY leaf.{column} {direction}, leaf.record_id
                          OFFSET 0
                     ) sort_value
                     {reverse_joins}
                     JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                    WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                      AND e.workspace_id = $14 AND e.deleted_at IS NULL
                      AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                      {leaf_filters}
                      AND ($4::text[] IS NULL OR e.system_tags @> $4)
                      {leaf_export_status}
                      AND (NOT $5 OR e.blueprint_version <> $6)
                      AND NOT $8
                      AND ($9::uuid IS NULL OR sort_value.{column} {comparison} $7::{column_type}
                           OR (sort_value.{column} = $7::{column_type}
                               AND (sort_value.record_id > $16
                                    OR (sort_value.record_id = $16 AND e.id > $9))))
                    ORDER BY sort_value.{column} {direction}, sort_value.record_id ASC, e.id ASC
                    LIMIT $13"#,
            );
            rows = bind_record_filters!(
                sqlx::query_as::<_, SortedRecordPreviewRow>(&sql)
                    .bind(blueprint_id)
                    .bind(blueprint_version)
                    .bind(matching.ids)
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
                    .bind(export_status_param(export_status)),
                matching.filters,
                workspace_id
            )
            .fetch_all(&self.pool)
            .await?;
        }

        if rows.len() <= limit as usize {
            let remaining = limit + 1 - rows.len() as i64;
            let forward_joins = relationship_sort_forward_joins(sort.relationship_path.len())?;
            let null_cursor_id = cursor_is_null.then_some(cursor_id).flatten();
            let sql = format!(
                r#"SELECT e.id, e.blueprint_version, ('attricat.sample'=ANY(e.system_tags)) AS is_sample, e.system_tags, e.created_at, e.projections -> 'preview' AS preview,
                          b.views AS blueprint_views,
                          {context_fallback_e} AS blueprint_context_fallback,
                          NULL::text AS sort_value, true AS sort_is_null,
                          e.id AS sort_target_id
                     FROM records e
                     JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                     {forward_joins}
                    WHERE e.blueprint_id = $1 AND ($2::bigint IS NULL OR e.blueprint_version = $2)
                      AND e.workspace_id = $14 AND e.deleted_at IS NULL
                      AND sort_value.id IS NULL
                      AND ($3::uuid[] IS NULL OR e.id = ANY($3))
                      {null_filters}
                      AND ($4::text[] IS NULL OR e.system_tags @> $4)
                      {null_export_status}
                      AND (NOT $5 OR e.blueprint_version <> $6)
                      AND $7::text IS NULL AND $8
                      AND ($9::uuid IS NULL OR e.id > $9)
                    ORDER BY e.id ASC
                    LIMIT $13"#,
            );
            let mut null_rows = bind_record_filters!(
                sqlx::query_as::<_, SortedRecordPreviewRow>(&sql)
                    .bind(blueprint_id)
                    .bind(blueprint_version)
                    .bind(matching.ids)
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
                    .bind(export_status_param(export_status)),
                matching.filters,
                workspace_id
            )
            .fetch_all(&self.pool)
            .await?;
            rows.append(&mut null_rows);
        }

        let next_cursor = next_page_cursor(&mut rows, limit, |row| {
            encode_sorted_search_cursor(sort, row)
        });
        Ok((
            rows.into_iter().map(sorted_record_preview).collect(),
            next_cursor,
        ))
    }

    /// Fills the table cells of a page: configured table paths and direct
    /// relationship targets, resolved concurrently for `context`.
    pub async fn hydrate_table_values(
        &self,
        items: &mut [RecordPreview],
        paths: &HashMap<String, ValueType>,
        relationships: &HashMap<String, String>,
        context: &SearchContext,
    ) -> Result<(), RepositoryError> {
        let (mut values, related) = tokio::try_join!(
            self.table_path_values(items, paths, context),
            self.related_table_previews(items, relationships, context),
        )?;
        for item in items.iter_mut() {
            item.table_values = values.remove(&item.id).unwrap_or_default();
        }
        if let Some(mut related) = related {
            for item in items.iter_mut() {
                item.related = related.remove(&item.id).unwrap_or_default();
            }
        }
        Ok(())
    }

    /// Resolves configured table paths in bounded, page-level batches. Relationship
    /// metadata is selected from every record's pinned blueprint revision and scalar
    /// leaves come from preview projections. Every hop and leaf is resolved for `context`.
    async fn table_path_values(
        &self,
        items: &[RecordPreview],
        paths: &HashMap<String, ValueType>,
        context: &SearchContext,
    ) -> Result<TableValues, RepositoryError> {
        let mut values_by_root = TableValues::new();
        if items.is_empty() {
            return Ok(values_by_root);
        }
        let roots: Vec<_> = items.iter().map(|item| item.id).collect();
        // Direct columns of every type are resolved with one query for file
        // columns and one for scalar columns; dotted paths walk per hop.
        let direct_files: Vec<&str> = paths
            .iter()
            .filter(|(path, leaf_type)| !path.contains('.') && **leaf_type == ValueType::File)
            .map(|(path, _)| path.as_str())
            .collect();
        let (direct_codes, direct_types): (Vec<&str>, Vec<&str>) = paths
            .iter()
            .filter(|(path, leaf_type)| !path.contains('.') && **leaf_type != ValueType::File)
            .map(|(path, leaf_type)| (path.as_str(), leaf_type.as_str()))
            .unzip();
        if !direct_files.is_empty() {
            let files = sqlx::query_as::<_, (Uuid, String, Value)>(
                &r#"SELECT av.record_id, a.code,
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
                  WHERE av.record_id = ANY($1) AND av.workspace_id = $3 AND av.active
                    AND {value_context}
                  GROUP BY av.record_id, a.code, f.id, f.display_filename, f.mime_type, f.byte_size,
                           f.sha256, f.status, r.position
                  ORDER BY av.record_id, a.code, r.position"#
                    .replace("{value_context}", &resolved_context("av", "a", "$4")),
            )
            .bind(&roots)
            .bind(&direct_files)
            .bind(self.workspace_id.0)
            .bind(&context.ids)
            .fetch_all(&self.pool)
            .await?;
            for (record_id, code, file) in files {
                values_by_root
                    .entry(record_id)
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
                     FROM records e
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
        // Relationship paths are walked one depth at a time for every column
        // together, so columns sharing a prefix share its hops and each depth
        // costs one query. `reached` maps a hop prefix to the (root, record)
        // pairs at its end.
        let relationship_paths: Vec<(&str, ValueType, Vec<&str>)> = paths
            .iter()
            .filter_map(|(path, leaf_type)| {
                let parts: Vec<_> = path.split('.').collect();
                (2..=MAX_RELATIONSHIP_HOPS + 1)
                    .contains(&parts.len())
                    .then_some((path.as_str(), *leaf_type, parts))
            })
            .collect();
        let mut reached: HashMap<String, Vec<(Uuid, Uuid)>> =
            HashMap::from([(String::new(), roots.iter().map(|id| (*id, *id)).collect())]);
        let depths = relationship_paths
            .iter()
            .map(|(_, _, parts)| parts.len() - 1)
            .max()
            .unwrap_or(0);
        for depth in 0..depths {
            let hops: HashSet<(String, &str)> = relationship_paths
                .iter()
                .filter(|(_, _, parts)| parts.len() - 1 > depth)
                .map(|(_, _, parts)| (parts[..depth].join("."), parts[depth]))
                .collect();
            let requested: HashSet<(Uuid, &str)> = hops
                .iter()
                .flat_map(|(prefix, relationship)| {
                    reached
                        .get(prefix)
                        .into_iter()
                        .flatten()
                        .map(|(_, current)| (*current, *relationship))
                })
                .collect();
            if requested.is_empty() {
                break;
            }
            let (sources, codes): (Vec<Uuid>, Vec<&str>) = requested.into_iter().unzip();
            let edges = sqlx::query_as::<_, TablePathEdgeRow>(
                &r#"SELECT av.record_id AS source_id, a.code, target.id AS target_id
                   FROM UNNEST($1::uuid[], $2::text[]) AS hop(source_id, code)
                   JOIN attribute_values av ON av.record_id = hop.source_id
                    AND av.workspace_id = $3 AND av.active
                    AND av.relationship_target_record_id IS NOT NULL
                   JOIN records source ON source.id = av.record_id
                    AND source.workspace_id = $3 AND source.deleted_at IS NULL
                   JOIN attributes a ON a.id = av.attribute_id
                    AND a.blueprint_id = source.blueprint_id
                    AND a.blueprint_version = source.blueprint_version
                    AND a.workspace_id = $3
                    AND a.code = hop.code AND a.value_type = 'relationship'
                   JOIN records target ON target.id = av.relationship_target_record_id
                    AND target.workspace_id = $3 AND target.deleted_at IS NULL
                   JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                    AND target_blueprint.version = target.blueprint_version
                    AND target_blueprint.code = a.target_blueprint_code
                  WHERE {edge_context}"#
                    .replace("{edge_context}", &resolved_context("av", "a", "$4")),
            )
            .bind(&sources)
            .bind(&codes)
            .bind(self.workspace_id.0)
            .bind(&context.ids)
            .fetch_all(&self.pool)
            .await?;
            let mut targets: HashMap<(Uuid, String), Vec<Uuid>> = HashMap::new();
            for edge in edges {
                targets
                    .entry((edge.source_id, edge.code))
                    .or_default()
                    .push(edge.target_id);
            }
            for (prefix, relationship) in hops {
                let next: Vec<_> = reached
                    .get(&prefix)
                    .into_iter()
                    .flatten()
                    .flat_map(|(root, current)| {
                        targets
                            .get(&(*current, relationship.to_owned()))
                            .into_iter()
                            .flatten()
                            .map(move |target| (*root, *target))
                    })
                    .collect();
                let path = if prefix.is_empty() {
                    relationship.to_owned()
                } else {
                    format!("{prefix}.{relationship}")
                };
                reached.insert(path, next);
            }
        }
        // Every leaf of every path is read in one query; only the leaf
        // attribute of each context's preview is selected.
        let requested: HashSet<(Uuid, &str, ValueType)> = relationship_paths
            .iter()
            .flat_map(|(_, leaf_type, parts)| {
                let leaf = parts[parts.len() - 1];
                reached
                    .get(&parts[..parts.len() - 1].join("."))
                    .into_iter()
                    .flatten()
                    .map(move |(_, id)| (*id, leaf, *leaf_type))
            })
            .collect();
        if requested.is_empty() {
            return Ok(values_by_root);
        }
        let mut leaf_ids = Vec::with_capacity(requested.len());
        let mut leaf_codes = Vec::with_capacity(requested.len());
        let mut leaf_types = Vec::with_capacity(requested.len());
        for (id, code, value_type) in requested {
            leaf_ids.push(id);
            leaf_codes.push(code);
            leaf_types.push(value_type.as_str());
        }
        let leaves: HashMap<(Uuid, String, ValueType), Value> = sqlx::query_as::<_, TablePathLeafRow>(
            r#"SELECT e.id, leaf.code, leaf.value_type, a.context_fallback <> 'none' AS inherit,
                      (SELECT jsonb_object_agg(context.key, jsonb_build_object(leaf.code, context.value -> leaf.code))
                         FROM jsonb_each(e.projections -> 'preview') context
                        WHERE context.value ? leaf.code) AS preview
               FROM UNNEST($1::uuid[], $2::text[], $3::text[]) AS leaf(id, code, value_type)
               JOIN records e ON e.id = leaf.id
                AND e.workspace_id = $4 AND e.deleted_at IS NULL
               JOIN attributes a ON a.blueprint_id = e.blueprint_id
                AND a.blueprint_version = e.blueprint_version
                AND a.code = leaf.code AND a.value_type = leaf.value_type
                AND a.workspace_id = $4 AND a.deleted_at IS NULL"#,
        )
        .bind(&leaf_ids)
        .bind(&leaf_codes)
        .bind(&leaf_types)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .filter_map(|row| {
            let preview = row.preview?;
            let value = context
                .resolve_preview(&preview, &row.code, row.inherit)
                .cloned()?;
            Some(((row.id, row.code, row.value_type), value))
        })
        .collect();
        for (path, leaf_type, parts) in &relationship_paths {
            let leaf = parts[parts.len() - 1];
            for (root, id) in reached
                .get(&parts[..parts.len() - 1].join("."))
                .into_iter()
                .flatten()
            {
                let key = (*id, leaf.to_owned(), *leaf_type);
                if let Some(value) = leaves.get(&key) {
                    let values = values_by_root
                        .entry(*root)
                        .or_default()
                        .entry((*path).to_owned())
                        .or_default();
                    if !values.contains(value) {
                        values.push(value.clone());
                    }
                }
            }
        }
        Ok(values_by_root)
    }

    /// Adds direct relationship targets used by table columns in one query for the entire page.
    /// The requested target code is matched against the pinned source attribute, so an older
    /// source revision with removed or incompatible relationship metadata gets an empty cell.
    /// Relationship values are resolved for `context`.
    /// `None` leaves the items' `related` maps untouched.
    async fn related_table_previews(
        &self,
        items: &[RecordPreview],
        relationships: &HashMap<String, String>,
        context: &SearchContext,
    ) -> Result<Option<HashMap<Uuid, HashMap<String, Vec<RelatedRecordPreview>>>>, RepositoryError>
    {
        let context_fallback_target = context_fallback_column("target");
        if items.is_empty() || relationships.is_empty() {
            return Ok(None);
        }
        let mut related: HashMap<Uuid, HashMap<String, Vec<RelatedRecordPreview>>> = items
            .iter()
            .map(|item| {
                let targets = relationships
                    .keys()
                    .cloned()
                    .map(|code| (code, Vec::new()))
                    .collect();
                (item.id, targets)
            })
            .collect();
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
            &format!(r#"SELECT source.id AS source_id, a.code AS attribute_code,
                      context.id AS relationship_context_id,
                      context.code AS relationship_context_code,
                      target.id, target.blueprint_id, target.blueprint_version, ('attricat.sample'=ANY(target.system_tags)) AS is_sample,
                      target.projections -> 'preview' AS preview,
                      target_blueprint.views AS blueprint_views,
                      {context_fallback_target} AS blueprint_context_fallback
                 FROM records source
                 JOIN attribute_values av ON av.record_id = source.id
                  AND av.active
                  AND av.relationship_target_record_id IS NOT NULL
                 JOIN attributes a ON a.id = av.attribute_id
                  AND a.blueprint_id = source.blueprint_id
                  AND a.blueprint_version = source.blueprint_version
                  AND a.deleted_at IS NULL
                  AND {{edge_context}}
                 JOIN LATERAL jsonb_to_recordset($2::jsonb)
                    AS requested(attribute_code text, target_blueprint_code text)
                    ON requested.attribute_code = a.code
                   AND requested.target_blueprint_code = a.target_blueprint_code
                 JOIN attribute_contexts context ON context.id = av.context_id
                 JOIN records target ON target.id = av.relationship_target_record_id
                  AND target.deleted_at IS NULL
                 JOIN blueprints target_blueprint ON target_blueprint.id = target.blueprint_id
                  AND target_blueprint.version = target.blueprint_version
                WHERE source.id = ANY($1)
                  AND source.deleted_at IS NULL
                ORDER BY source.id, a.code, context.code, av.id"#)
                .replace("{edge_context}", &resolved_context("av", "a", "$3")),
        )
        .bind(items.iter().map(|item| item.id).collect::<Vec<_>>())
        .bind(requested)
        .bind(&context.ids)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            if let Some(targets) = related
                .get_mut(&row.source_id)
                .and_then(|item| item.get_mut(&row.attribute_code))
            {
                targets.push(RelatedRecordPreview {
                    id: row.id,
                    blueprint_id: row.blueprint_id,
                    blueprint_version: row.blueprint_version,
                    is_sample: row.is_sample,
                    relationship_context_id: row.relationship_context_id,
                    relationship_context_code: row.relationship_context_code,
                    display: super::record_projection::display_labels(
                        &row.preview,
                        &row.blueprint_views,
                        &row.blueprint_context_fallback,
                    ),
                    preview: row.preview,
                });
            }
        }
        Ok(Some(related))
    }

    // Facet traversal inputs are explicit to keep query scope auditable.
    #[allow(clippy::too_many_arguments)]
    /// Source records whose relationship path, resolved in `context_id` hop
    /// by hop, reaches a selected node or one of its descendants, or `None`
    /// without a selection. With an empty `source_path`, the nodes are the
    /// source blueprint's own records and each record is attached to itself,
    /// so a selection matches the selected nodes and their descendants.
    pub async fn relationship_tree_facet_matches(
        &self,
        source_blueprint_id: Uuid,
        source_path: &[RelationshipFacetHop],
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
        selected_target_ids: &[Uuid],
        source_record_ids: Option<&HashSet<Uuid>>,
    ) -> Result<Option<Vec<Uuid>>, RepositoryError> {
        if self.get_context_by_id(context_id).await?.is_none() {
            return Err(RepositoryError::InvalidContext);
        }
        if selected_target_ids.is_empty() {
            return Ok(None);
        }
        let selected: HashSet<_> = selected_target_ids.iter().copied().collect();
        let selected_nodes: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM records WHERE blueprint_id = $1 AND id = ANY($2) AND deleted_at IS NULL",
        )
        .bind(target_blueprint_id)
        .bind(selected.iter().copied().collect::<Vec<_>>())
        .fetch_one(&self.pool)
        .await?;
        if selected_nodes as usize != selected.len() {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
        }
        // The blueprint each hop starts from.
        let hop_sources: Vec<Uuid> = std::iter::once(source_blueprint_id)
            .chain(source_path.iter().map(|hop| hop.target_blueprint_id))
            .collect();
        let last = source_path.len().checked_sub(1);
        // Walks down from the selection and then back over the last hop
        // through the target-side relationship indexes, so the cost follows
        // the size of the selected subtrees rather than the whole relationship.
        let mut matches = sqlx::query_scalar::<_, Uuid>(&format!(
            r#"WITH RECURSIVE {FACET_CONTEXT_PATH_SQL}, allowed AS (
                    SELECT unnest($5::uuid[]) AS id
                    UNION
                    {hierarchy}
                )
                SELECT id FROM allowed WHERE $2::text IS NULL
                UNION
                {sources}"#,
            hierarchy = facet_sources_sql("$3", "$6"),
            sources = facet_sources_sql("$1", "$2"),
        ))
        .bind(last.map_or(source_blueprint_id, |last| hop_sources[last]))
        .bind(last.map(|last| source_path[last].field.as_str()))
        .bind(target_blueprint_id)
        .bind(context_id)
        .bind(selected.into_iter().collect::<Vec<_>>())
        .bind(hierarchy_field)
        .fetch_all(&self.pool)
        .await?;
        // Earlier hops step back the same way from the records reached so far.
        for (index, hop) in source_path.iter().enumerate().rev().skip(1) {
            if matches.is_empty() {
                break;
            }
            matches = sqlx::query_scalar::<_, Uuid>(&format!(
                r#"WITH RECURSIVE {FACET_CONTEXT_PATH_SQL}, allowed AS (
                        SELECT unnest($3::uuid[]) AS id
                    )
                    {sources}"#,
                sources = facet_sources_sql("$1", "$2"),
            ))
            .bind(hop_sources[index])
            .bind(&hop.field)
            .bind(&matches)
            .bind(context_id)
            .fetch_all(&self.pool)
            .await?;
        }
        Ok(Some(
            matches
                .into_iter()
                .collect::<HashSet<_>>()
                .into_iter()
                .filter(|id| source_record_ids.is_none_or(|ids| ids.contains(id)))
                .collect(),
        ))
    }

    // Facet traversal inputs are explicit to keep query scope auditable.
    #[allow(clippy::too_many_arguments)]
    /// One page of a tree facet's roots, or of `parent_id`'s children, with
    /// the distinct sources attached to each node or its descendants through
    /// `source_path`. With an empty path, each source record is its own node;
    /// see [`Self::relationship_tree_facet_matches`].
    pub async fn relationship_tree_facet_children(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        matching_record_ids: Option<&HashSet<Uuid>>,
        source_path: &[RelationshipFacetHop],
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
        parent_id: Option<Uuid>,
        cursor: Option<Uuid>,
        selected_target_ids: &[Uuid],
        include_ids: &[Uuid],
        limit: i64,
    ) -> Result<RelationshipTreeFacetChildrenResponse, RepositoryError> {
        let requested_context = self
            .get_context_by_id(context_id)
            .await?
            .ok_or(RepositoryError::InvalidContext)?;
        // The tree's shape is always read fresh, so added or moved nodes show
        // at once. Without a search term, counts come from a cache shared by
        // the workspace (see [`Self::cached_tree_facet_counts`]); with one,
        // the matching sources are counted for the requested page only.
        // Counting on a cache miss needs the same edges; whichever starts
        // first resolves them for both.
        let hierarchy_edges = OnceCell::new();
        let parents = self.hierarchy_edges(
            &hierarchy_edges,
            target_blueprint_id,
            hierarchy_field,
            context_id,
        );
        let roots = async {
            if parent_id.is_some() {
                return Ok(Vec::new());
            }
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM records WHERE blueprint_id = $1 AND deleted_at IS NULL ORDER BY id",
            )
            .bind(target_blueprint_id)
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::from)
        };
        let cached_counts = async {
            if matching_record_ids.is_some() {
                return Ok(None);
            }
            self.cached_tree_facet_counts(
                source_blueprint_id,
                source_blueprint_version,
                source_path,
                target_blueprint_id,
                hierarchy_field,
                context_id,
                &hierarchy_edges,
            )
            .await
            .map(Some)
        };
        let sources = async {
            let Some(matching) = matching_record_ids else {
                return Ok(Vec::new());
            };
            self.tree_facet_sources(
                source_blueprint_id,
                source_blueprint_version,
                Some(matching),
                source_path,
                context_id,
            )
            .await
        };
        let (parents, nodes, cached_counts, sources) =
            tokio::try_join!(parents, roots, cached_counts, sources)?;

        let mut sources_by_target: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (source, target) in sources {
            sources_by_target.entry(target).or_default().push(source);
        }
        let mut children_by_parent: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        let mut parents_by_child: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for &(child, parent) in parents {
            children_by_parent.entry(parent).or_default().push(child);
            parents_by_child.entry(child).or_default().push(parent);
        }
        let mut selected_ids: Vec<_> = selected_target_ids.to_vec();
        selected_ids.sort_unstable();
        selected_ids.dedup();
        let selected_paths: Vec<Vec<Uuid>> = selected_ids
            .iter()
            .map(|&id| {
                let mut path = ancestor_path(&parents_by_child, id);
                path.push(id);
                path
            })
            .collect();
        let mut page: Vec<Uuid> = match parent_id {
            Some(parent) => {
                let mut children = children_by_parent.get(&parent).cloned().unwrap_or_default();
                children.sort_unstable();
                children.dedup();
                children
            }
            None => nodes
                .into_iter()
                .filter(|id| !parents_by_child.contains_key(id))
                .collect(),
        };
        // Requested nodes of this level outside the page, and those on the way
        // to a selected node, follow it without moving the cursor, so paging
        // on still reaches every node once.
        let level: HashSet<_> = page.iter().copied().collect();
        // UUIDs order bytewise in both PostgreSQL and Rust, so cursors carry over.
        page.retain(|id| cursor.is_none_or(|cursor| *id > cursor));
        page.truncate(limit as usize + 1);
        let next_cursor = next_page_cursor(&mut page, limit, |id| *id);
        let mut included: Vec<_> = include_ids
            .iter()
            .chain(selected_paths.iter().flatten())
            .copied()
            .filter(|id| level.contains(id) && !page.contains(id))
            .collect();
        included.sort_unstable();
        included.dedup();
        page.extend(included);

        let selected: HashSet<_> = selected_target_ids.iter().copied().collect();
        let labeled: HashSet<_> = page.iter().chain(&selected).copied().collect();
        let labels: HashMap<_, _> = sqlx::query_as::<_, RelationshipTreeNodeRow>(
            r#"SELECT record.id, record.projections -> 'preview' AS preview,
                      blueprint.views,
                      (SELECT COALESCE(jsonb_object_agg(attribute.code, attribute.context_fallback), '{}'::jsonb)
                         FROM attributes attribute
                        WHERE attribute.blueprint_id = record.blueprint_id
                          AND attribute.blueprint_version = record.blueprint_version
                          AND attribute.deleted_at IS NULL) AS context_fallback
                 FROM records record
                 JOIN blueprints blueprint ON blueprint.id = record.blueprint_id
                  AND blueprint.version = record.blueprint_version
                WHERE record.blueprint_id = $1
                  AND record.id = ANY($2)
                  AND record.deleted_at IS NULL"#,
        )
        .bind(target_blueprint_id)
        .bind(labeled.into_iter().collect::<Vec<_>>())
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            let display = display_label(
                &row.preview,
                &row.views,
                &row.context_fallback,
                &requested_context.code,
            )
            .as_str()
            .unwrap_or_default()
            .to_owned();
            (row.id, display)
        })
        .collect();
        if selected.iter().any(|id| !labels.contains_key(id)) {
            return Err(RepositoryError::RelationshipTargetTypeMismatch);
        }

        let items = page
            .into_iter()
            .map(|id| {
                if let Some(counts) = &cached_counts {
                    return RelationshipTreeFacetChildItem {
                        id,
                        display: labels.get(&id).cloned().unwrap_or_default(),
                        count: counts.get(&id).copied().unwrap_or_default(),
                        has_children: children_by_parent.contains_key(&id),
                    };
                }
                // A source counts once per node even when it reaches several
                // of the node's descendants or one through several parents.
                let mut visited = HashSet::new();
                let mut counted: HashSet<Uuid> = HashSet::new();
                let mut pending = vec![id];
                while let Some(node) = pending.pop() {
                    if !visited.insert(node) {
                        continue;
                    }
                    counted.extend(sources_by_target.get(&node).into_iter().flatten());
                    pending.extend(children_by_parent.get(&node).into_iter().flatten());
                }
                RelationshipTreeFacetChildItem {
                    id,
                    display: labels.get(&id).cloned().unwrap_or_default(),
                    count: counted.len() as i64,
                    has_children: children_by_parent.contains_key(&id),
                }
            })
            .collect();
        let mut selected_items: Vec<_> = selected
            .into_iter()
            .map(|id| RecordHierarchyItem {
                display: labels[&id].clone(),
                id,
            })
            .collect();
        selected_items.sort_unstable_by_key(|item| item.id);
        Ok(RelationshipTreeFacetChildrenResponse {
            items,
            selected_items,
            selected_paths,
            next_cursor,
        })
    }

    /// The `(child, parent)` edges of a tree facet's hierarchy, resolved once
    /// into `edges` however many callers ask for them.
    async fn hierarchy_edges<'cell>(
        &self,
        edges: &'cell OnceCell<Vec<(Uuid, Uuid)>>,
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
    ) -> Result<&'cell [(Uuid, Uuid)], RepositoryError> {
        edges
            .get_or_try_init(|| async {
                match hierarchy_field {
                    Some(field) => {
                        self.resolved_relationship_edges(
                            target_blueprint_id,
                            None,
                            None,
                            field,
                            target_blueprint_id,
                            context_id,
                        )
                        .await
                    }
                    None => Ok(Vec::new()),
                }
            })
            .await
            .map(Vec::as_slice)
    }

    /// The `(source, node)` pairs a tree facet counts, for `source_ids` only
    /// when given. With an empty `source_path`, each source record is its own
    /// node. Otherwise each hop's edges are resolved in one query and joined
    /// here. Up to [`TREE_FACET_LISTED_SOURCES`] sources are followed one hop
    /// after another, each hop reading only the records the one before
    /// reached, so the cost follows the sources rather than the size of a
    /// large middle hop. Past that, or when every source counts, reading
    /// every hop's whole relationship at once is cheaper.
    async fn tree_facet_sources(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        source_ids: Option<&HashSet<Uuid>>,
        source_path: &[RelationshipFacetHop],
        context_id: Uuid,
    ) -> Result<Vec<(Uuid, Uuid)>, RepositoryError> {
        if source_path.is_empty() {
            return Ok(sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM records WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL",
            )
            .bind(source_blueprint_id)
            .bind(source_blueprint_version)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter(|id| source_ids.is_none_or(|ids| ids.contains(id)))
            .map(|id| (id, id))
            .collect());
        }
        let hop_sources: Vec<_> = std::iter::once(source_blueprint_id)
            .chain(source_path.iter().map(|hop| hop.target_blueprint_id))
            .collect();
        let hop_sources = &hop_sources;
        // Only the searched records are limited to one revision.
        let hop_edges = |index: usize, from: Option<Vec<Uuid>>| async move {
            self.resolved_relationship_edges(
                hop_sources[index],
                source_blueprint_version.filter(|_| index == 0),
                from.as_deref(),
                &source_path[index].field,
                source_path[index].target_blueprint_id,
                context_id,
            )
            .await
        };
        let join = |pairs: Vec<(Uuid, Uuid)>, edges: Vec<(Uuid, Uuid)>| {
            let mut targets: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
            for (source, target) in edges {
                targets.entry(source).or_default().push(target);
            }
            pairs
                .into_iter()
                .flat_map(|(source, reached)| {
                    targets
                        .get(&reached)
                        .into_iter()
                        .flatten()
                        .map(move |target| (source, *target))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let Some(source_ids) = source_ids.filter(|ids| ids.len() <= TREE_FACET_LISTED_SOURCES)
        else {
            let mut hops = futures_util::future::try_join_all(
                (0..source_path.len()).map(|index| hop_edges(index, None)),
            )
            .await?
            .into_iter();
            let first = hops.next().expect("the path has a first hop");
            let mut pairs = hops.fold(first, join);
            if let Some(source_ids) = source_ids {
                pairs.retain(|(source, _)| source_ids.contains(source));
            }
            return Ok(pairs);
        };
        // Each hop holds one value, so it reaches no more records than the
        // sources: the later lists stay within the limit too.
        let mut pairs = hop_edges(0, Some(source_ids.iter().copied().collect())).await?;
        for index in 1..source_path.len() {
            let reached: HashSet<_> = pairs.iter().map(|(_, reached)| *reached).collect();
            if reached.is_empty() {
                break;
            }
            let edges = hop_edges(index, Some(reached.into_iter().collect())).await?;
            pairs = join(pairs, edges);
        }
        Ok(pairs)
    }

    /// Distinct sources per tree node, including the node's descendants, for
    /// every node with any. Resolving every edge of a large relationship takes
    /// a noticeable fraction of a second, and Browse rail pages for one tree
    /// arrive in bursts, so counts are shared by the workspace and may lag
    /// writes by up to [`TREE_FACET_COUNTS_FRESH`]. The next request after
    /// that reloads them; requests arriving during the reload keep the
    /// previous counts.
    #[allow(clippy::too_many_arguments)]
    async fn cached_tree_facet_counts(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        source_path: &[RelationshipFacetHop],
        target_blueprint_id: Uuid,
        hierarchy_field: Option<&str>,
        context_id: Uuid,
        hierarchy_edges: &OnceCell<Vec<(Uuid, Uuid)>>,
    ) -> Result<Arc<HashMap<Uuid, i64>>, RepositoryError> {
        let key = CacheKey::new(
            "tree_facet_counts",
            &[
                &self.workspace_id.0,
                &source_blueprint_id,
                &source_blueprint_version.map_or_else(|| "all".to_owned(), |v| v.to_string()),
                &facet_path_key(source_path),
                &target_blueprint_id,
                &hierarchy_field.unwrap_or("-"),
                &context_id,
            ],
        );
        let policy = Policy::Ttl {
            fresh: TREE_FACET_COUNTS_FRESH,
            stale: TREE_FACET_COUNTS_STALE,
        };
        self.cache
            .fetch(key, &[], policy, || async {
                let parents = self.hierarchy_edges(
                    hierarchy_edges,
                    target_blueprint_id,
                    hierarchy_field,
                    context_id,
                );
                let sources = self.tree_facet_sources(
                    source_blueprint_id,
                    source_blueprint_version,
                    None,
                    source_path,
                    context_id,
                );
                let (parents, sources) = tokio::try_join!(parents, sources)?;
                let mut parents_by_child: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
                for &(child, parent) in parents {
                    parents_by_child.entry(child).or_default().push(parent);
                }
                let mut targets_by_source: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
                for (source, target) in sources {
                    targets_by_source.entry(source).or_default().push(target);
                }
                // Each source adds one to every node it reaches upwards, once
                // even through several targets or parents.
                let mut counts: HashMap<Uuid, i64> = HashMap::new();
                let mut visited = HashSet::new();
                for targets in targets_by_source.into_values() {
                    visited.clear();
                    let mut pending = targets;
                    while let Some(node) = pending.pop() {
                        if visited.insert(node) {
                            *counts.entry(node).or_default() += 1;
                            pending.extend(parents_by_child.get(&node).into_iter().flatten());
                        }
                    }
                }
                Ok::<_, RepositoryError>(counts)
            })
            .await
    }

    /// A blueprint's matching records per target of one relationship field, as
    /// resolved in `context_id`, within [`SCOPE_STATEMENT_TIMEOUT`]. Holds the
    /// first `group_limit` targets `user_id` may read, largest first; `linked`
    /// still counts the records linking only to targets they may not.
    #[allow(clippy::too_many_arguments)]
    pub async fn relationship_target_groups(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        matching: RecordMatching<'_>,
        field: &str,
        target_blueprint_ids: &[Uuid],
        context_id: Uuid,
        user_id: Uuid,
        workspace_id: Uuid,
        group_limit: i64,
    ) -> Result<RelationshipTargetGroups, RepositoryError> {
        let codes = GrantCodes::Permission(Permission::RecordsRead);
        let grants = renumber_parameters(&codes.grant_set_sql(), |parameter| match parameter {
            1 => 8,
            2 => 9,
            3 => 10,
            _ => unreachable!("the grant set has three parameters"),
        });
        let granted = RECORD_GRANTED_SQL.replace("$2", "$9");
        let (records, matching_sql) = matching_records(matching.ids.is_some(), 11);
        let filters_sql = record_filters_clause(matching, 12);
        let edges = RESOLVED_EDGES_SQL
            .replace("{sources}", "JOIN matched ON matched.id = av.record_id")
            .replace("{target}", "target.blueprint_id = ANY($3)");
        let sql = format!(
            r#"WITH RECURSIVE {grants}, matched AS MATERIALIZED (
                    SELECT e.id
                      FROM {records}
                     WHERE e.blueprint_id = $1
                       AND ($5::bigint IS NULL OR e.blueprint_version = $5)
                       AND e.workspace_id = $7 AND e.deleted_at IS NULL
                       {matching_sql}
                       {filters_sql}
                ), {edges}, readable AS (
                    SELECT counted.target_id, counted.sources
                      FROM (SELECT target_id, COUNT(*) AS sources FROM edges GROUP BY target_id) counted
                      JOIN records e ON e.id = counted.target_id AND e.workspace_id = $7
                     WHERE {granted}
                     ORDER BY counted.sources DESC, counted.target_id
                     LIMIT $6::bigint
                )
                SELECT (SELECT COUNT(*) FROM matched),
                       (SELECT COUNT(DISTINCT source_id) FROM edges),
                       ARRAY(SELECT target_id FROM readable ORDER BY sources DESC, target_id),
                       ARRAY(SELECT sources FROM readable ORDER BY sources DESC, target_id)"#
        );
        let mut transaction = self.pool.begin().await?;
        sqlx::query(SCOPE_STATEMENT_TIMEOUT)
            .execute(&mut *transaction)
            .await?;
        let (total, linked, targets, counts) = bind_record_filters!(
            sqlx::query_as::<_, (i64, i64, Vec<Uuid>, Vec<i64>)>(&sql)
                .bind(blueprint_id)
                .bind(field)
                .bind(target_blueprint_ids)
                .bind(context_id)
                .bind(blueprint_version)
                .bind(group_limit + 1)
                .bind(self.workspace_id.0)
                .bind(user_id)
                .bind(workspace_id)
                .bind(codes.codes())
                .bind(matching.ids),
            matching.filters,
            self.workspace_id.0
        )
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let mut groups: Vec<_> = targets.into_iter().zip(counts).collect();
        let truncated = groups.len() as i64 > group_limit;
        groups.truncate(group_limit as usize);
        Ok(RelationshipTargetGroups {
            total,
            linked,
            groups,
            truncated,
        })
    }

    /// `(source, target)` pairs of one relationship field as resolved in
    /// `context_id`: each source reads its values from the nearest context on
    /// the path to the root that has any, unless the field has no fallback.
    /// `source_blueprint_version` limits the sources to one revision, and
    /// `source_ids` to those records, read through the source-side index.
    async fn resolved_relationship_edges(
        &self,
        source_blueprint_id: Uuid,
        source_blueprint_version: Option<i64>,
        source_ids: Option<&[Uuid]>,
        field: &str,
        target_blueprint_id: Uuid,
        context_id: Uuid,
    ) -> Result<Vec<(Uuid, Uuid)>, RepositoryError> {
        let sources = if source_ids.is_some() {
            "JOIN unnest($6::uuid[]) AS listed(id) ON listed.id = av.record_id"
        } else {
            ""
        };
        let query = format!(
            "WITH RECURSIVE {} SELECT source_id, target_id FROM edges",
            RESOLVED_EDGES_SQL
                .replace("{sources}", sources)
                .replace("{target}", "target.blueprint_id = $3")
        );
        let mut query = sqlx::query_as::<_, (Uuid, Uuid)>(&query)
            .bind(source_blueprint_id)
            .bind(field)
            .bind(target_blueprint_id)
            .bind(context_id)
            .bind(source_blueprint_version);
        if let Some(source_ids) = source_ids {
            query = query.bind(source_ids);
        }
        Ok(query.fetch_all(&self.pool).await?)
    }

    /// Resolves the application-owned relationship-aware query plan. SQL only reads
    /// one scalar-match set or one incoming-edge frontier at a time.
    pub async fn resolve_search(
        &self,
        selected: &BlueprintWithAttributes,
        selected_version: Option<i64>,
        query: &str,
    ) -> Result<HashSet<Uuid>, RepositoryError> {
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
                    Ok(_) => metrics::counter!(GLOBAL_SEARCH_TOTAL_METRIC, "outcome" => "success")
                        .increment(1),
                    Err(RepositoryError::RelationshipSearchBudgetExceeded { dimension }) => {
                        metrics::counter!(GLOBAL_SEARCH_TOTAL_METRIC, "outcome" => "budget_exceeded", "dimension" => *dimension).increment(1);
                        tracing::warn!(dimension, "global relationship search budget exceeded");
                    }
                    Err(RepositoryError::RelationshipSearchTimedOut) => {
                        metrics::counter!(GLOBAL_SEARCH_TOTAL_METRIC, "outcome" => "timed_out")
                            .increment(1);
                        tracing::warn!("global relationship search statement timed out");
                    }
                    Err(_) => metrics::counter!(GLOBAL_SEARCH_TOTAL_METRIC, "outcome" => "failed")
                        .increment(1),
                }
            }
            per_term.push(result?);
        }
        Ok(per_term
            .into_iter()
            .reduce(|candidates, ids| candidates.intersection(&ids).copied().collect())
            .unwrap_or_default())
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
        if parts.len() > MAX_RELATIONSHIP_HOPS + 1 || parts.iter().any(|part| part.is_empty()) {
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
                .find(|a| {
                    a.code.eq_ignore_ascii_case(parts[1]) && a.value_type != ValueType::Relationship
                })
                .ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "unknown selected-blueprint attribute '{}'",
                        parts[1]
                    ))
                })?;
            return Ok(TermPlan::SelectedAttribute(Some(attribute.code.clone())));
        }
        if parts.len() == 1 {
            if let Some(attribute) = selected.attributes.iter().find(|a| {
                a.code.eq_ignore_ascii_case(parts[0]) && a.value_type != ValueType::Relationship
            }) {
                return Ok(TermPlan::SelectedAttribute(Some(attribute.code.clone())));
            }
            let relationship = selected
                .attributes
                .iter()
                .find(|a| {
                    a.code.eq_ignore_ascii_case(parts[0]) && a.value_type == ValueType::Relationship
                })
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
                    && !matches!(
                        a.value_type,
                        ValueType::Relationship | ValueType::File | ValueType::Json
                    )
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
                    a.code.eq_ignore_ascii_case(relationship_name)
                        && a.value_type == ValueType::Relationship
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
    ) -> Result<HashSet<Uuid>, RepositoryError> {
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
            TermPlan::Ids { ids, .. } => {
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM records WHERE id = ANY($1) AND blueprint_id = $2 AND workspace_id = $3 AND deleted_at IS NULL",
                )
                .bind(ids)
                .bind(match_blueprint)
                .bind(self.workspace_id.0)
                .fetch_all(&self.pool)
                .await?
            }
            _ => {
                self.scalar_search_matches(&term, match_blueprint, attribute)
                    .await?
            }
        };
        let mut visited: HashSet<Uuid> = rows.into_iter().collect();
        let mut frontier: Vec<Uuid> = visited.iter().copied().collect();
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
            if frontier.is_empty() {
                break;
            }
            let sources = sqlx::query_scalar::<_, Uuid>(
                r#"SELECT DISTINCT source.id
                FROM records source JOIN attribute_values av ON av.record_id = source.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN records target ON target.id = av.relationship_target_record_id
                WHERE source.deleted_at IS NULL AND target.deleted_at IS NULL AND av.active
                  AND source.workspace_id = $4 AND target.workspace_id = $4
                  AND av.workspace_id = $4 AND a.workspace_id = $4
                  AND av.relationship_target_record_id = ANY($1) AND a.value_type = 'relationship'
                  AND ($2::uuid IS NULL OR source.blueprint_id = $2)
                  AND ($3::text IS NULL OR a.code = $3)"#,
            )
            .bind(&frontier)
            .bind(path_source_blueprints.and_then(|ids| ids.get(ids.len() - depth)))
            .bind(
                path_fields
                    .and_then(|fields| fields.get(fields.len() - depth))
                    .or_else(|| direct_field.as_ref().filter(|v| !v.is_empty())),
            )
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?;
            frontier = sources
                .into_iter()
                .filter(|source| visited.insert(*source))
                .collect();
        }
        // Explicit selected-blueprint value queries and all traversals only return selected records.
        // The version restriction is applied here because scalar matches may start on old revisions.
        let selected_ids = sqlx::query_scalar::<_, Uuid>("SELECT id FROM records WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL AND id = ANY($3) AND workspace_id = $4")
            .bind(selected.blueprint.id)
            .bind(selected_version)
            .bind(visited.into_iter().collect::<Vec<_>>())
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?;
        Ok(selected_ids.into_iter().collect())
    }

    /// Records of `match_blueprint` whose scalar values match a term.
    async fn scalar_search_matches(
        &self,
        term: &SearchTerm,
        match_blueprint: Option<Uuid>,
        attribute: Option<String>,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let pattern = search_pattern(term);
        // Alphabetic terms other than booleans cannot match the canonical native
        // renderings of non-text values. Start these searches from the trigram-indexed
        // text values and materialize them so PostgreSQL does not repeat that scan for
        // every attribute.
        let sql = if text_only_search(&term.value) {
            r#"WITH matching_values AS MATERIALIZED (
                    SELECT record_id, attribute_id
                    FROM attribute_values
                    WHERE workspace_id = $4 AND active
                      AND relationship_target_record_id IS NULL
                      AND value_text ILIKE $3
                )
                SELECT DISTINCT e.id
                FROM matching_values av
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN records e ON e.id = av.record_id
                WHERE e.deleted_at IS NULL AND e.workspace_id = $4
                  AND a.workspace_id = $4
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)"#
        } else {
            r#"SELECT DISTINCT e.id
                FROM records e JOIN attribute_values av ON av.record_id = e.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                WHERE e.deleted_at IS NULL AND e.workspace_id = $4
                  AND av.workspace_id = $4 AND a.workspace_id = $4
                  AND av.relationship_target_record_id IS NULL
                  AND ($1::uuid IS NULL OR e.blueprint_id = $1)
                  AND ($2::text IS NULL OR a.code = $2)
                  AND COALESCE(av.value_text, av.value_number::text, av.value_integer::text, av.value_boolean::text, av.value_date::text, av.value_datetime::text, av.value_time::text) ILIKE $3"#
        };
        Ok(sqlx::query_scalar::<_, Uuid>(sql)
            .bind(match_blueprint)
            .bind(attribute)
            .bind(pattern)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?)
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
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout = '250ms'")
            .execute(&mut *transaction)
            .await
            .map_err(global_search_error)?;
        let pattern = search_pattern(&term);
        let sql = if text_only_search(&term.value) {
            r#"WITH matching_values AS MATERIALIZED (
                    SELECT record_id, attribute_id FROM attribute_values
                    WHERE workspace_id = $2 AND active AND relationship_target_record_id IS NULL
                      AND value_text ILIKE $1
                )
                SELECT DISTINCT e.id, a.code FROM matching_values av
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN records e ON e.id = av.record_id
                WHERE e.deleted_at IS NULL AND e.workspace_id = $2 AND a.workspace_id = $2
                ORDER BY e.id, a.code LIMIT $3"#
        } else {
            r#"SELECT DISTINCT e.id, a.code FROM records e
                JOIN attribute_values av ON av.record_id = e.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                WHERE e.deleted_at IS NULL AND e.workspace_id = $2 AND av.workspace_id = $2
                  AND a.workspace_id = $2 AND av.active AND av.relationship_target_record_id IS NULL
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

        let mut visited = HashSet::new();
        let mut frontier = VecDeque::new();
        for (id, _) in rows {
            if visited.insert(id) {
                budget.consume_records(1)?;
                frontier.push_back(id);
            }
        }

        // Each level is read as one ordered batch, so the budgets stop at the
        // same edge every time.
        for _ in 1..=MAX_RELATIONSHIP_HOPS {
            let level: Vec<_> = frontier.drain(..).collect();
            if level.is_empty() {
                break;
            }
            budget.consume_frontier(level.len())?;
            let edges = sqlx::query_as::<_, (Uuid, String, Uuid)>(
                r#"SELECT DISTINCT source.id, a.code, av.relationship_target_record_id
                FROM records source JOIN attribute_values av ON av.record_id = source.id
                JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
                JOIN records target ON target.id = av.relationship_target_record_id
                WHERE source.deleted_at IS NULL AND target.deleted_at IS NULL AND av.active
                  AND source.workspace_id = $2 AND target.workspace_id = $2
                  AND av.workspace_id = $2 AND a.workspace_id = $2
                  AND av.relationship_target_record_id = ANY($1) AND a.value_type = 'relationship'
                ORDER BY source.id, a.code, av.relationship_target_record_id LIMIT $3"#,
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
                .filter_map(|(source, _, _)| (!visited.contains(source)).then_some(*source))
                .collect();
            budget.consume_frontier(next_frontier.len())?;
            for (source, _, _) in edges {
                if visited.insert(source) {
                    budget.consume_records(1)?;
                    frontier.push_back(source);
                }
            }
        }
        let selected_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM records WHERE blueprint_id = $1 AND ($2::bigint IS NULL OR blueprint_version = $2) AND deleted_at IS NULL AND id = ANY($3) AND workspace_id = $4",
        )
        .bind(selected.blueprint.id).bind(selected_version).bind(visited.into_iter().collect::<Vec<_>>())
        .bind(self.workspace_id.0)
        .fetch_all(&mut *transaction).await.map_err(global_search_error)?;
        transaction.commit().await.map_err(global_search_error)?;
        metrics::histogram!("attricat_global_relationship_search_matches")
            .record(budget.matches as f64);
        metrics::histogram!("attricat_global_relationship_search_records")
            .record(budget.records as f64);
        metrics::histogram!("attricat_global_relationship_search_edges")
            .record(budget.edges as f64);
        tracing::info!(
            matches = budget.matches,
            records = budget.records,
            edges = budget.edges,
            "global relationship search resolved"
        );
        Ok(selected_ids.into_iter().collect())
    }
}

const MAX_GLOBAL_SEARCH_MATCHES: usize = 1_000;
const MAX_GLOBAL_SEARCH_RECORDS: usize = 5_000;
const MAX_GLOBAL_SEARCH_EDGES: usize = 10_000;
const MAX_GLOBAL_SEARCH_FRONTIER: usize = 5_000;
const GLOBAL_SEARCH_TOTAL_METRIC: &str = "attricat_global_relationship_search_total";
/// Budget dimensions reported by [`RepositoryError::RelationshipSearchBudgetExceeded`].
const MATCHES_DIMENSION: &str = "matching_values";
const RECORDS_DIMENSION: &str = "records";
const EDGES_DIMENSION: &str = "relationship_edges";
const FRONTIER_DIMENSION: &str = "frontier";

#[derive(Default)]
struct GlobalSearchBudget {
    matches: usize,
    records: usize,
    edges: usize,
}

impl GlobalSearchBudget {
    fn remaining_matches_plus_one(&self) -> Result<usize, RepositoryError> {
        Self::remaining(MAX_GLOBAL_SEARCH_MATCHES, self.matches, MATCHES_DIMENSION)
    }
    fn remaining_edges_plus_one(&self) -> Result<usize, RepositoryError> {
        Self::remaining(MAX_GLOBAL_SEARCH_EDGES, self.edges, EDGES_DIMENSION)
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
        self.check(self.matches, MAX_GLOBAL_SEARCH_MATCHES, MATCHES_DIMENSION)
    }
    fn consume_frontier(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.check(count, MAX_GLOBAL_SEARCH_FRONTIER, FRONTIER_DIMENSION)
    }
    fn consume_records(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.records += count;
        self.check(self.records, MAX_GLOBAL_SEARCH_RECORDS, RECORDS_DIMENSION)
    }
    fn consume_edges(&mut self, count: usize) -> Result<(), RepositoryError> {
        self.edges += count;
        self.check(self.edges, MAX_GLOBAL_SEARCH_EDGES, EDGES_DIMENSION)
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

/// PostgreSQL `query_canceled`, raised when `statement_timeout` expires.
const QUERY_CANCELED_SQLSTATE: &str = "57014";

fn global_search_error(error: sqlx::Error) -> RepositoryError {
    if error
        .as_database_error()
        .is_some_and(|database| database.code().as_deref() == Some(QUERY_CANCELED_SQLSTATE))
    {
        RepositoryError::RelationshipSearchTimedOut
    } else {
        RepositoryError::Database(error)
    }
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
    /// `@id:a,b` or `relationship.@id:a,b`: records with one of the listed IDs,
    /// on the selected blueprint or reached through the relationship path.
    Ids {
        fields: Vec<String>,
        source_blueprint_ids: Vec<Uuid>,
        target_blueprint_id: Uuid,
        ids: Vec<Uuid>,
    },
}
/// Path leaf that matches records by ID instead of by attribute value.
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
                    "'{id}' is not a record ID in search term '{}'",
                    term.original
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable();
    ids.dedup();
    if ids.len() > MAX_SEARCH_IDS {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "an ID search term may list at most {MAX_SEARCH_IDS} records"
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
/// The ILIKE pattern of a term: its text is literal, so `%` and `_` in it are
/// ordinary characters.
fn search_pattern(term: &SearchTerm) -> String {
    let literal = term
        .value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    if term.prefix {
        format!("{literal}%")
    } else {
        format!("%{literal}%")
    }
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
    fn walks_from_a_root_to_a_nodes_parent() {
        let [a, b, c, e] = [1, 2, 3, 5].map(Uuid::from_u128);
        // Equally short ways up take the lowest parent: e → b → a.
        let parents = HashMap::from([(e, vec![c, b]), (c, vec![a]), (b, vec![a])]);
        assert_eq!(ancestor_path(&parents, e), vec![a, b]);
        assert_eq!(ancestor_path(&HashMap::new(), a), Vec::<Uuid>::new());
    }

    #[test]
    fn reaches_a_root_past_a_lowest_parent_that_loops() {
        let [a, b, c, d, root] = [1, 2, 3, 4, 5].map(Uuid::from_u128);
        // d's lowest parent b loops back through a; c leads to the root.
        let parents = HashMap::from([(d, vec![c, b]), (b, vec![a]), (a, vec![d]), (c, vec![root])]);
        assert_eq!(ancestor_path(&parents, d), vec![root, c]);
    }

    #[test]
    fn stops_where_a_path_without_a_root_repeats() {
        let [a, b, d] = [1, 2, 4].map(Uuid::from_u128);
        let parents = HashMap::from([(d, vec![b]), (b, vec![a]), (a, vec![d])]);
        assert_eq!(ancestor_path(&parents, d), vec![a, b]);
    }

    #[test]
    fn bounds_each_global_search_dimension_without_partial_results() {
        let mut budget = GlobalSearchBudget::default();
        budget.consume_matches(MAX_GLOBAL_SEARCH_MATCHES).unwrap();
        assert!(matches!(
            budget.consume_matches(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: MATCHES_DIMENSION
            })
        ));

        let mut budget = GlobalSearchBudget::default();
        budget.consume_records(MAX_GLOBAL_SEARCH_RECORDS).unwrap();
        assert!(matches!(
            budget.consume_records(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: RECORDS_DIMENSION
            })
        ));

        let mut budget = GlobalSearchBudget::default();
        budget.consume_edges(MAX_GLOBAL_SEARCH_EDGES).unwrap();
        assert!(matches!(
            budget.consume_edges(1),
            Err(RepositoryError::RelationshipSearchBudgetExceeded {
                dimension: EDGES_DIMENSION
            })
        ));
    }
}

fn relationship_sort_reverse_joins(depth: usize) -> Result<String, RepositoryError> {
    if !(1..=MAX_RELATIONSHIP_HOPS).contains(&depth) {
        return Err(RepositoryError::InvalidBlueprintDefinition(
            "relationship sort path must contain one to three hops".to_owned(),
        ));
    }

    // The correlated lateral boundary keeps the ordered leaf scan outside the
    // reverse traversal. PostgreSQL can then use an incremental sort for source
    // ties instead of sorting the complete source candidate set before LIMIT.
    let mut sql =
        format!("JOIN LATERAL (SELECT path_source.id AS source_id FROM records target_{depth}");
    for level in (1..=depth).rev() {
        sql.push_str(&format!(
            " JOIN attribute_values edge_{level} ON edge_{level}.relationship_target_record_id = target_{level}.id \
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
                " JOIN records path_source ON path_source.id = edge_1.record_id \
                 AND path_source.blueprint_id = path_attribute_1.blueprint_id \
                 AND path_source.blueprint_version = path_attribute_1.blueprint_version",
            );
        } else {
            let next = level - 1;
            sql.push_str(&format!(
                " JOIN records target_{next} ON target_{next}.id = edge_{level}.record_id \
                 AND target_{next}.workspace_id = $14 AND target_{next}.deleted_at IS NULL \
                 AND target_{next}.blueprint_id = path_attribute_{level}.blueprint_id \
                 AND target_{next}.blueprint_version = path_attribute_{level}.blueprint_version"
            ));
        }
    }
    sql.push_str(&format!(
        " WHERE target_{depth}.id = sort_value.record_id AND target_{depth}.deleted_at IS NULL ORDER BY path_source.id) resolved_source ON true \
         JOIN records e ON e.id = resolved_source.source_id AND e.workspace_id = $14 AND e.deleted_at IS NULL"
    ));
    Ok(sql)
}

fn relationship_sort_forward_joins(depth: usize) -> Result<String, RepositoryError> {
    if !(1..=MAX_RELATIONSHIP_HOPS).contains(&depth) {
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
             LEFT JOIN attribute_values edge_{level} ON edge_{level}.record_id = {source}.id \
             AND edge_{level}.workspace_id = $14 AND edge_{level}.attribute_id = path_attribute_{level}.id \
             AND edge_{level}.active AND edge_{level}.relationship_target_record_id IS NOT NULL \
             AND {context} \
             LEFT JOIN records target_{level} ON target_{level}.id = edge_{level}.relationship_target_record_id \
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
         LEFT JOIN attribute_values sort_value ON sort_value.record_id = {source}.id \
         AND sort_value.workspace_id = $14 AND sort_value.attribute_id = sort_attribute.id \
         AND sort_value.active AND sort_value.relationship_target_record_id IS NULL \
         AND {context}",
        context = resolved_context("sort_value", "sort_attribute", "$15"),
    ));
    Ok(sql)
}

/// The `attribute_values` column a sortable value type is stored in, and the
/// SQL type its cursor value is cast to.
/// Drops the extra row a page query fetched to detect a next page, and
/// returns the cursor of the last row kept when there is one.
fn next_page_cursor<T, C>(
    rows: &mut Vec<T>,
    limit: i64,
    cursor: impl FnOnce(&T) -> C,
) -> Option<C> {
    let limit = limit as usize;
    if rows.len() <= limit {
        return None;
    }
    rows.truncate(limit);
    rows.last().map(cursor)
}

/// The revision's `{code: context_fallback}` map for the record aliased
/// `record`, as a select-list expression.
fn context_fallback_column(record: &str) -> String {
    format!(
        "(SELECT COALESCE(jsonb_object_agg(fallback.code, fallback.context_fallback), '{{}}'::jsonb) \
           FROM attributes fallback \
          WHERE fallback.workspace_id = {record}.workspace_id \
            AND fallback.blueprint_id = {record}.blueprint_id \
            AND fallback.blueprint_version = {record}.blueprint_version \
            AND fallback.deleted_at IS NULL)"
    )
}

fn native_sort_column(
    value_type: ValueType,
) -> Result<(&'static str, &'static str), RepositoryError> {
    match value_type {
        ValueType::String => Ok(("value_text", "text")),
        ValueType::Number => Ok(("value_number", "numeric")),
        ValueType::Integer => Ok(("value_integer", "bigint")),
        ValueType::Boolean => Ok(("value_boolean", "boolean")),
        ValueType::Date => Ok(("value_date", "date")),
        ValueType::Datetime => Ok(("value_datetime", "timestamptz")),
        ValueType::Time => Ok(("value_time", "time")),
        _ => Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "{value_type} is not sortable"
        ))),
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

fn encode_sorted_search_cursor(sort: &RecordSearchSort, row: &SortedRecordPreviewRow) -> String {
    let cursor = SortedSearchCursor {
        version: SORTED_SEARCH_CURSOR_VERSION,
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

/// Most matching sources a tree facet reads by listing them, hop by hop. With
/// 100,000 sources behind a path, listing 11,000 of them reads each hop about
/// four times faster than its whole relationship, while listing all of them
/// is slower than reading everything at once (see `perf/facets.md`).
const TREE_FACET_LISTED_SOURCES: usize = 20_000;

/// How long cached tree facet counts are served before a reload.
const TREE_FACET_COUNTS_FRESH: Duration = Duration::from_secs(30);
/// How long expired counts are kept after that, so requests arriving while
/// one request reloads them get the previous counts instead of waiting.
const TREE_FACET_COUNTS_STALE: Duration = Duration::from_secs(10 * 60);

/// The predicate of an optional [`ExportStatusFilter`] bound as JSON at
/// `parameter`, over the records aliased `e`.
fn export_status_clause(parameter: usize, filter: Option<&ExportStatusFilter>) -> String {
    // Equality or IS NULL, unlike IS NOT DISTINCT FROM, can use the mapping
    // indexes.
    let connection = if filter.is_some_and(|filter| filter.connection.is_some()) {
        format!("export_mapping.connection = ${parameter}->>'connection'")
    } else {
        "export_mapping.connection IS NULL".to_owned()
    };
    format!(
        "AND (${parameter}::jsonb IS NULL OR EXISTS (SELECT 1 FROM extension_export_mappings export_mapping \
         WHERE export_mapping.workspace_id = e.workspace_id AND export_mapping.record_id = e.id \
           AND export_mapping.extension_id = ${parameter}->>'extension_id' \
           AND {connection} AND {current} \
           AND export_mapping.status = ${parameter}->>'status'))",
        current = super::extension_export_state::current_destination("export_mapping")
    )
}

/// The export status and data quality predicates of a search scope.
/// `parameter` is the export status filter's bind position.
fn record_filter_clauses(
    parameter: usize,
    export_status: Option<&ExportStatusFilter>,
    data_quality: Option<DataQualityFilter>,
) -> String {
    format!(
        "{} {}",
        export_status_clause(parameter, export_status),
        data_quality_clause(data_quality)
    )
}

/// Restricts `e` to the records in `filter`. The clause binds nothing: its
/// codes come from closed enums. Open findings are read from the
/// needs-attention tag, which the tags index serves.
fn data_quality_clause(filter: Option<DataQualityFilter>) -> String {
    let findings = |state: &str| {
        format!(
            "EXISTS (SELECT 1 FROM rule_findings data_quality_finding \
              WHERE data_quality_finding.workspace_id = e.workspace_id \
                AND data_quality_finding.record_id = e.id \
                AND data_quality_finding.state {state})"
        )
    };
    let open = format!("e.system_tags @> ARRAY['{NEEDS_ATTENTION_SYSTEM_TAG}']");
    match filter {
        None => String::new(),
        Some(DataQualityFilter::Open) => format!("AND {open}"),
        Some(DataQualityFilter::Acknowledged) => format!(
            "AND {} AND NOT {open}",
            findings(&format!("= '{}'", RuleFindingState::Acknowledged.as_str()))
        ),
        Some(DataQualityFilter::Clean) => format!(
            "AND NOT {}",
            findings(&format!("<> '{}'", RuleFindingState::Resolved.as_str()))
        ),
    }
}

/// The JSON bound for [`export_status_clause`].
fn export_status_param(filter: Option<&ExportStatusFilter>) -> Option<Value> {
    filter.map(|filter| {
        serde_json::json!({
            "extension_id": filter.extension_id,
            "connection": filter.connection,
            "status": filter.status,
        })
    })
}

/// The `records e` source and filter of a query over every search match, for
/// matching IDs bound at `$parameter`. The ID list drives the join: with an
/// `e.id = ANY(...)` filter, a cached generic plan scans every record of the
/// blueprint and compares each one against the whole list.
fn matching_records(matching: bool, parameter: usize) -> (String, String) {
    if matching {
        (
            format!(
                "(SELECT DISTINCT unnest(${parameter}::uuid[]) AS id) matching JOIN records e ON e.id = matching.id"
            ),
            String::new(),
        )
    } else {
        (
            "records e".to_owned(),
            format!("AND ${parameter}::uuid[] IS NULL"),
        )
    }
}

/// Result version scopes tell one version from several, so two is enough.
const MAX_RESULT_VERSIONS: i64 = 2;

/// `FROM ... WHERE` over a page's matches for the version and total statements:
/// `$1` blueprint, `$2` optional version, `$3` workspace, `$4` matching IDs, `$5`
/// system tags, `$6` outdated, `$7` current version.
fn result_version_matches(
    matching: RecordMatching<'_>,
    export_status_sql: &str,
    filters_parameter: usize,
) -> String {
    let (records, matching_sql) = matching_records(matching.ids.is_some(), 4);
    let filters_sql = record_filters_clause(matching, filters_parameter);
    format!(
        "FROM {records}
          WHERE e.blueprint_id = $1
            AND ($2::bigint IS NULL OR e.blueprint_version = $2)
            AND e.workspace_id = $3
            AND e.deleted_at IS NULL
            {matching_sql}
            {filters_sql}
            AND ($5::text[] IS NULL OR e.system_tags @> $5)
            {export_status_sql}
            AND (NOT $6 OR e.blueprint_version <> $7)"
    )
}

/// `FROM ... WHERE` over a blueprint's matching records: `$1` blueprint, `$2`
/// optional version, `$3` matching IDs, `$4` workspace. `$5` is left for the
/// statement's limit, and [`record_filters_clause`] follows it.
fn matching_records_from(matching: RecordMatching<'_>) -> String {
    let (records, matching_sql) = matching_records(matching.ids.is_some(), 3);
    let filters_sql = record_filters_clause(matching, 6);
    format!(
        "FROM {records}
          WHERE e.blueprint_id = $1
            AND ($2::bigint IS NULL OR e.blueprint_version = $2)
            AND e.workspace_id = $4 AND e.deleted_at IS NULL
            {matching_sql}
            {filters_sql}"
    )
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
        String::from(DEFAULT_CONTEXT_CODE),
        Value::Object(Map::new()),
    )]))
}

pub(super) fn empty_projections() -> Value {
    Value::Object(Map::from_iter([(String::from("preview"), empty_preview())]))
}

fn sorted_record_preview(row: SortedRecordPreviewRow) -> RecordPreview {
    record_preview(RecordPreviewRow {
        id: row.id,
        blueprint_version: row.blueprint_version,
        is_sample: row.is_sample,
        system_tags: row.system_tags,
        created_at: row.created_at,
        preview: row.preview,
        blueprint_views: row.blueprint_views,
        blueprint_context_fallback: row.blueprint_context_fallback,
    })
}

fn record_preview(row: RecordPreviewRow) -> RecordPreview {
    RecordPreview {
        id: row.id,
        blueprint_version: row.blueprint_version,
        schema_outdated: false,
        is_sample: row.is_sample,
        system_tags: row.system_tags,
        created_at: row.created_at,
        display: super::record_projection::display_labels(
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
        display: super::record_projection::display_labels(
            &row.preview,
            &row.blueprint_views,
            &row.blueprint_context_fallback,
        ),
    }
}
