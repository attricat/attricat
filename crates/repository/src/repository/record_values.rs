//! The one loader and resolution rule for entity values per context.
//!
//! Every consumer that needs "the value of attribute X in context C" (entity
//! checks, transition conditions and rules, unique keys, the JSON entity
//! schema, publication gates, record locks and approval digests) loads
//! [`RecordValues`] and resolves them through [`ContextTree::path`] with
//! [`resolve_on_path`], so they cannot disagree.
//!
//! A context holds a *direct value* for an attribute when it has a row for it:
//!
//! - scalars: the native JSON value (stored rows are never null);
//! - files: the ordered `[{id, sha256}]` references; an explicit empty file
//!   value (`[]`) is a direct value and blocks inheritance;
//! - relationships: the sorted active target IDs, present only when at least
//!   one target is active (inactive legacy rows are not values), matching the
//!   preview projection that reads use.
//!
//! The nearest context on the path with a direct value wins; an attribute with
//! `context_fallback = "none"` only looks at the requested context.
use super::RepositoryError;
use super::values::{NativeValueRow, native_value_json};
use catalog_validation::predicate::Record;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use sqlx::PgConnection;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// One attribute context; the root has no parent.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct ContextNode {
    pub id: Uuid,
    pub code: String,
    pub parent_id: Option<Uuid>,
}

/// The workspace's context hierarchy, loaded once per operation.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContextTree {
    nodes: Vec<ContextNode>,
}

impl ContextTree {
    pub(crate) async fn load(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> Result<Self, RepositoryError> {
        Ok(Self::new(
            sqlx::query_as::<_, ContextNode>(
                "SELECT id, code, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
            )
            .bind(workspace_id)
            .fetch_all(conn)
            .await?,
        ))
    }

    pub(crate) fn new(nodes: Vec<ContextNode>) -> Self {
        Self { nodes }
    }

    /// Contexts ordered by code.
    pub(crate) fn nodes(&self) -> &[ContextNode] {
        &self.nodes
    }

    pub(crate) fn ids(&self) -> Vec<Uuid> {
        self.nodes.iter().map(|node| node.id).collect()
    }

    pub(crate) fn get(&self, id: Uuid) -> Option<&ContextNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The context's code, or its ID for an unknown context.
    pub(crate) fn code(&self, id: Uuid) -> String {
        self.get(id)
            .map(|node| node.code.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// The root context (`default`).
    pub(crate) fn default_context(&self) -> Result<&ContextNode, RepositoryError> {
        self.nodes
            .iter()
            .find(|node| node.parent_id.is_none() && node.code == "default")
            .or_else(|| self.nodes.iter().find(|node| node.parent_id.is_none()))
            .ok_or(RepositoryError::InvalidContext)
    }

    /// The context followed by its ancestors, nearest first, as values are
    /// inherited. Without `inherit` only the context itself. Unknown contexts
    /// and parent cycles are rejected.
    pub(crate) fn path(&self, id: Uuid, inherit: bool) -> Result<Vec<Uuid>, RepositoryError> {
        let mut node = self.get(id).ok_or(RepositoryError::InvalidContext)?;
        let mut path = vec![node.id];
        if !inherit {
            return Ok(path);
        }
        while let Some(parent) = node.parent_id {
            node = self.get(parent).ok_or(RepositoryError::InvalidContext)?;
            if path.contains(&node.id) {
                return Err(RepositoryError::InvalidContext);
            }
            path.push(node.id);
        }
        Ok(path)
    }

    /// Number of ancestors; the default context has depth zero.
    pub(crate) fn depth(&self, id: Uuid) -> Result<usize, RepositoryError> {
        Ok(self.path(id, true)?.len() - 1)
    }
}

/// The resolution rule: the nearest context on `path` with a direct value
/// wins. An attribute that does not inherit (`context_fallback = "none"`)
/// only looks at the first context of the path.
pub(crate) fn resolve_on_path<T>(
    path: &[Uuid],
    inherit: bool,
    mut direct: impl FnMut(Uuid) -> Option<T>,
) -> Option<(Uuid, T)> {
    let candidates = if inherit {
        path
    } else {
        &path[..path.len().min(1)]
    };
    candidates
        .iter()
        .find_map(|context| direct(*context).map(|value| (*context, value)))
}

/// Which snapshot of an entity's values to load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordState {
    /// The state at the start of the current transaction.
    Before,
    /// The state including this transaction's writes.
    After,
}

/// One attribute's direct value in one context.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DirectValue {
    pub value: Value,
    pub changed_at: DateTime<Utc>,
    /// The exact decimal text of a `number`; `value` is a JSON number.
    pub exact: Option<String>,
}

impl DirectValue {
    /// The value to normalize as a unique-key component: numbers as their
    /// exact decimal text, everything else as stored.
    pub(crate) fn key_input(&self) -> Value {
        self.exact
            .clone()
            .map(Value::String)
            .unwrap_or_else(|| self.value.clone())
    }
}

/// One attribute's direct values of an entity, by context.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AttributeValues {
    pub value_type: String,
    /// `context_fallback` is not `none`.
    pub inherit: bool,
    /// One of the entity's own additional (reusable) attributes rather than a
    /// field of its blueprint revision.
    pub entity_scoped: bool,
    pub by_context: HashMap<Uuid, DirectValue>,
}

impl AttributeValues {
    pub(crate) fn resolve(&self, path: &[Uuid]) -> Option<&DirectValue> {
        resolve_on_path(path, self.inherit, |context| self.by_context.get(&context))
            .map(|(_, value)| value)
    }
}

/// One live entity's direct values in every context.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RecordValues {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub tags: Vec<String>,
    pub attributes: BTreeMap<String, AttributeValues>,
}

impl RecordValues {
    pub(crate) fn empty(id: Uuid, blueprint_id: Uuid, blueprint_version: i64) -> Self {
        Self {
            id,
            blueprint_id,
            blueprint_version,
            ..Self::default()
        }
    }

    /// The resolved value of one attribute for the context `path`.
    pub(crate) fn resolve(&self, code: &str, path: &[Uuid]) -> Option<&DirectValue> {
        self.attributes.get(code)?.resolve(path)
    }

    /// Every resolved attribute, as the predicate engine reads a record.
    pub(crate) fn record(&self, path: &[Uuid]) -> Record {
        let mut record = Record {
            id: self.id.to_string(),
            tags: self.tags.clone(),
            ..Record::default()
        };
        for (code, attribute) in &self.attributes {
            if let Some(direct) = attribute.resolve(path) {
                record.values.insert(code.clone(), direct.value.clone());
                record.changed_at.insert(code.clone(), direct.changed_at);
            }
        }
        record
    }

    /// The document validated by the blueprint's JSON entity schema: resolved
    /// values of the blueprint revision's fields (not the entity's own
    /// additional attributes). Missing values are omitted.
    pub(crate) fn schema_document(&self, path: &[Uuid]) -> Map<String, Value> {
        self.attributes
            .iter()
            .filter(|(_, attribute)| !attribute.entity_scoped)
            .filter_map(|(code, attribute)| {
                attribute
                    .resolve(path)
                    .map(|direct| (code.clone(), direct.value.clone()))
            })
            .collect()
    }

    pub(crate) fn value_type(&self, code: &str) -> Option<&str> {
        self.attributes
            .get(code)
            .map(|attribute| attribute.value_type.as_str())
    }
}

/// Which entities to load.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Selection<'a> {
    /// These entities, of any blueprint family.
    Entities(&'a [Uuid]),
    /// Every live entity of a blueprint family.
    Family(Uuid),
}

#[derive(sqlx::FromRow)]
struct ValueRow {
    entity_id: Uuid,
    attribute_code: String,
    context_fallback: String,
    entity_scoped: bool,
    context_id: Uuid,
    relationship_target_entity_id: Option<Uuid>,
    active: bool,
    created_at: DateTime<Utc>,
    files: Option<Value>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

const VALUE_COLUMNS: &str = "v.entity_id, a.code AS attribute_code, a.value_type, a.context_fallback, (a.entity_id IS NOT NULL) AS entity_scoped, v.context_id, v.relationship_target_entity_id, v.active, v.created_at, v.value_text, v.value_number, v.value_integer, v.value_boolean, v.value_date, v.value_datetime, v.value_time, v.value_time_zone, v.value_json";
const CURRENT_FILES: &str = "(SELECT COALESCE(jsonb_agg(jsonb_build_object('id', f.id, 'sha256', f.sha256) ORDER BY r.position), '[]'::jsonb) FROM attribute_file_references r JOIN files f ON f.id = r.file_id AND f.workspace_id = r.workspace_id WHERE r.attribute_value_id = v.id AND r.workspace_id = v.workspace_id)";
const ARCHIVED_FILES: &str = "(SELECT COALESCE(jsonb_agg(jsonb_build_object('id', f.id, 'sha256', f.sha256) ORDER BY r.position), '[]'::jsonb) FROM attribute_file_reference_history r JOIN files f ON f.id = r.file_id AND f.workspace_id = r.workspace_id WHERE r.attribute_value_history_id = v.id AND r.attribute_value_history_archived_at = v.archived_at)";
/// `$1` workspace, `$2` entity IDs, `$3` optional attribute codes.
const VALUE_FILTER: &str = "JOIN attributes a ON a.id = v.attribute_id AND a.deleted_at IS NULL WHERE v.workspace_id = $1 AND v.entity_id = ANY($2) AND ($3::text[] IS NULL OR a.code = ANY($3))";

fn value_query(state: RecordState) -> String {
    let select = |table: &str, files: &str| {
        format!(
            "SELECT {VALUE_COLUMNS}, CASE WHEN a.value_type = 'file' THEN {files} END AS files FROM {table} v {VALUE_FILTER}"
        )
    };
    match state {
        RecordState::After => select("attribute_values", CURRENT_FILES),
        // `now()` is the transaction start: rows created earlier are the
        // original state, and rows archived by this transaction restore what
        // it replaced. Intermediate rows of this transaction are excluded.
        RecordState::Before => format!(
            "{} AND v.created_at < now() UNION ALL {} AND v.archived_at = now() AND v.created_at < now()",
            select("attribute_values", CURRENT_FILES),
            select("attribute_value_history", ARCHIVED_FILES),
        ),
    }
}

/// Loads live entities with their direct values. Missing or deleted IDs are
/// omitted. `codes` restricts the attributes loaded. Reads see the caller's
/// uncommitted writes.
pub(crate) async fn load_records(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    selection: Selection<'_>,
    codes: Option<&[String]>,
    state: RecordState,
) -> Result<BTreeMap<Uuid, RecordValues>, RepositoryError> {
    let (ids, family) = match selection {
        Selection::Entities([]) => return Ok(BTreeMap::new()),
        Selection::Entities(ids) => (Some(ids), None),
        Selection::Family(blueprint_id) => (None, Some(blueprint_id)),
    };
    let mut records: BTreeMap<Uuid, RecordValues> =
        sqlx::query_as::<_, (Uuid, Uuid, i64, Vec<String>)>(
            "SELECT id, blueprint_id, blueprint_version, system_tags FROM entities WHERE workspace_id = $1 AND deleted_at IS NULL AND ($2::uuid[] IS NULL OR id = ANY($2)) AND ($3::uuid IS NULL OR blueprint_id = $3)",
        )
        .bind(workspace_id)
        .bind(ids)
        .bind(family)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(id, blueprint_id, blueprint_version, tags)| {
            (
                id,
                RecordValues {
                    id,
                    blueprint_id,
                    blueprint_version,
                    tags,
                    attributes: BTreeMap::new(),
                },
            )
        })
        .collect();
    if records.is_empty() {
        return Ok(records);
    }
    let live: Vec<Uuid> = records.keys().copied().collect();
    let rows = sqlx::query_as::<_, ValueRow>(&value_query(state))
        .bind(workspace_id)
        .bind(&live)
        .bind(codes)
        .fetch_all(&mut *conn)
        .await?;
    for row in rows {
        if let Some(record) = records.get_mut(&row.entity_id) {
            add_row(record, row)?;
        }
    }
    Ok(records)
}

/// Loads one live entity, or `None` when it is missing or deleted.
pub(crate) async fn load_record(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    entity_id: Uuid,
    state: RecordState,
) -> Result<Option<RecordValues>, RepositoryError> {
    Ok(load_records(
        conn,
        workspace_id,
        Selection::Entities(&[entity_id]),
        None,
        state,
    )
    .await?
    .remove(&entity_id))
}

fn add_row(record: &mut RecordValues, row: ValueRow) -> Result<(), RepositoryError> {
    let attribute = record
        .attributes
        .entry(row.attribute_code)
        .or_insert_with(|| AttributeValues {
            value_type: row.native.value_type.clone(),
            inherit: row.context_fallback != "none",
            entity_scoped: row.entity_scoped,
            by_context: HashMap::new(),
        });
    if row.native.value_type == "relationship" {
        let (Some(target), true) = (row.relationship_target_entity_id, row.active) else {
            return Ok(());
        };
        let direct = attribute
            .by_context
            .entry(row.context_id)
            .or_insert_with(|| DirectValue {
                value: Value::Array(Vec::new()),
                changed_at: row.created_at,
                exact: None,
            });
        direct.changed_at = direct.changed_at.max(row.created_at);
        if let Value::Array(targets) = &mut direct.value {
            let target = Value::String(target.to_string());
            let position = targets
                .binary_search_by(|item| item.as_str().cmp(&target.as_str()))
                .unwrap_or_else(|position| position);
            if targets.get(position) != Some(&target) {
                targets.insert(position, target);
            }
        }
        return Ok(());
    }
    let exact = row.native.value_number.map(|number| number.to_string());
    let value = if row.native.value_type == "file" {
        row.files.unwrap_or_else(|| Value::Array(Vec::new()))
    } else {
        native_value_json(row.native)?
    };
    attribute.by_context.insert(
        row.context_id,
        DirectValue {
            value,
            changed_at: row.created_at,
            exact,
        },
    );
    Ok(())
}

/// Codes of every attribute any of `records` holds, sorted and deduplicated.
pub(crate) fn attribute_codes<'a>(records: &[&'a RecordValues]) -> Vec<&'a str> {
    let mut seen = HashSet::new();
    let mut codes: Vec<&str> = records
        .iter()
        .flat_map(|record| record.attributes.keys().map(String::as_str))
        .filter(|code| seen.insert(*code))
        .collect();
    codes.sort_unstable();
    codes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tree() -> ContextTree {
        ContextTree::new(vec![
            ContextNode {
                id: Uuid::from_u128(1),
                code: "default".into(),
                parent_id: None,
            },
            ContextNode {
                id: Uuid::from_u128(2),
                code: "fr".into(),
                parent_id: Some(Uuid::from_u128(1)),
            },
            ContextNode {
                id: Uuid::from_u128(3),
                code: "fr-ca".into(),
                parent_id: Some(Uuid::from_u128(2)),
            },
        ])
    }

    fn direct(value: Value) -> DirectValue {
        DirectValue {
            value,
            changed_at: DateTime::<Utc>::MIN_UTC,
            exact: None,
        }
    }

    fn attribute(value_type: &str, inherit: bool, values: &[(u128, Value)]) -> AttributeValues {
        AttributeValues {
            value_type: value_type.into(),
            inherit,
            entity_scoped: false,
            by_context: values
                .iter()
                .map(|(context, value)| (Uuid::from_u128(*context), direct(value.clone())))
                .collect(),
        }
    }

    #[test]
    fn context_paths_follow_parents_and_reject_cycles() {
        let tree = tree();
        let [default, fr, fr_ca] = [1u128, 2, 3].map(Uuid::from_u128);
        assert_eq!(tree.path(fr_ca, true).unwrap(), [fr_ca, fr, default]);
        assert_eq!(tree.path(fr_ca, false).unwrap(), [fr_ca]);
        assert_eq!(tree.depth(fr_ca).unwrap(), 2);
        assert_eq!(tree.default_context().unwrap().id, default);
        assert!(tree.path(Uuid::from_u128(9), true).is_err());
        let cyclic = ContextTree::new(vec![
            ContextNode {
                id: fr,
                code: "fr".into(),
                parent_id: Some(fr_ca),
            },
            ContextNode {
                id: fr_ca,
                code: "fr-ca".into(),
                parent_id: Some(fr),
            },
        ]);
        assert!(cyclic.path(fr, true).is_err());
    }

    #[test]
    fn nearest_direct_value_wins_and_fallback_none_stops_inheritance() {
        let tree = tree();
        let path = tree.path(Uuid::from_u128(3), true).unwrap();
        let mut record = RecordValues::empty(Uuid::from_u128(10), Uuid::nil(), 1);
        record.attributes.insert(
            "title".into(),
            attribute(
                "string",
                true,
                &[(1, json!("Shirt")), (2, json!("Chemise"))],
            ),
        );
        record.attributes.insert(
            "slug".into(),
            attribute("string", false, &[(1, json!("shirt"))]),
        );
        // An explicit empty file value is a direct value: it does not reveal
        // the inherited photo.
        record.attributes.insert(
            "photo".into(),
            attribute(
                "file",
                true,
                &[(1, json!([{"id": "f", "sha256": "x"}])), (2, json!([]))],
            ),
        );
        let resolved = record.record(&path);
        assert_eq!(resolved.values["title"], json!("Chemise"));
        assert!(!resolved.values.contains_key("slug"));
        assert_eq!(resolved.values["photo"], json!([]));
        let default = record.record(&tree.path(Uuid::from_u128(1), true).unwrap());
        assert_eq!(default.values["photo"], json!([{"id": "f", "sha256": "x"}]));
        assert_eq!(default.values["slug"], json!("shirt"));
    }

    #[test]
    fn schema_documents_exclude_entity_scoped_attributes() {
        let mut record = RecordValues::empty(Uuid::from_u128(10), Uuid::nil(), 1);
        record.attributes.insert(
            "title".into(),
            attribute("string", true, &[(1, json!("A"))]),
        );
        let mut own = attribute("string", true, &[(1, json!("B"))]);
        own.entity_scoped = true;
        record.attributes.insert("acme:note".into(), own);
        let path = [Uuid::from_u128(1)];
        assert_eq!(
            Value::Object(record.schema_document(&path)),
            json!({"title": "A"})
        );
        assert_eq!(record.record(&path).values.len(), 2);
    }

    #[test]
    fn relationship_rows_keep_sorted_active_targets_only() {
        let mut record = RecordValues::empty(Uuid::from_u128(10), Uuid::nil(), 1);
        let row = |target: u128, context: u128, active: bool| ValueRow {
            entity_id: Uuid::from_u128(10),
            attribute_code: "parts".into(),
            context_fallback: "inherit".into(),
            entity_scoped: false,
            context_id: Uuid::from_u128(context),
            relationship_target_entity_id: Some(Uuid::from_u128(target)),
            active,
            created_at: DateTime::<Utc>::MIN_UTC,
            files: None,
            native: NativeValueRow {
                value_type: "relationship".into(),
                value_text: None,
                value_number: None,
                value_integer: None,
                value_boolean: None,
                value_date: None,
                value_datetime: None,
                value_time: None,
                value_time_zone: None,
                value_json: None,
            },
        };
        for (target, context, active) in [(3, 1, true), (2, 1, true), (4, 2, false)] {
            add_row(&mut record, row(target, context, active)).unwrap();
        }
        let fr = [Uuid::from_u128(2), Uuid::from_u128(1)];
        assert_eq!(
            record.resolve("parts", &fr).unwrap().value,
            json!([
                Uuid::from_u128(2).to_string(),
                Uuid::from_u128(3).to_string()
            ]),
            "an inactive row is not a value, so fr inherits"
        );
    }
}
