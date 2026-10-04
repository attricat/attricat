//! Host side of the shared predicate engine: loads bounded, context-resolved
//! records for [`catalog_validation::predicate`] and enforces entity-schema
//! checks, status transition conditions and enforcing rules on every write
//! of values or system tags.
//!
//! Records come from [`super::record_values`], the loader every other value
//! consumer shares, and resolve with the same context rule.
use super::record_values::{
    ContextTree, RecordState, RecordValues, Selection, load_record, load_records,
};
use super::references::{
    ReferenceQuery, Referrers, referencing_entity_ids, referencing_entity_ids_by_target,
};
use super::status::StatusChange;
use super::structural_constraints::{
    EnforcedUniqueKey, HierarchyField, HierarchyWalk, UniqueKeyScope, enforced_unique_keys,
    key_hash, walk_hierarchy,
};
use super::*;
use catalog_rules::Severity;
use catalog_validation::predicate::{
    self, Check, CycleState, Evaluation, Failure, MAX_CYCLE_VISITS, MAX_LINKED_RECORDS,
    MAX_REFERENCING_RECORDS, Predicate, Record, RecordSet, Related, Requirements,
};
use catalog_validation::unique_key::{UNIQUE_PREDICATE_CASE_SENSITIVE, normalize_key_component};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgConnection;
use std::collections::BTreeMap;

/// At most this many violations are reported in one error response.
pub const MAX_REPORTED_VIOLATIONS: usize = 50;
/// Candidates compared per query while scanning for `unique` duplicates.
/// Scanning continues until every candidate has been compared.
const UNIQUE_CANDIDATE_BATCH: i64 = 500;

/// The declaration that produced a violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSource {
    /// An `x-attricat-checks` entry in the blueprint's entity schema.
    EntityCheck,
    /// A `conditions` entry on a status transition edge.
    TransitionCondition,
    /// An enabled rule, enforcing or required by a publication channel.
    Rule,
    /// The blueprint's JSON entity schema (publication gates only).
    EntitySchema,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CheckTransition {
    pub attribute_code: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// One failed check, condition or rule, reported in `error.details.violations`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckViolation {
    pub source: CheckSource,
    /// Check, condition or rule code.
    pub code: String,
    pub message: String,
    /// Codes of the resolved contexts in which the predicate failed.
    pub contexts: Vec<String>,
    /// Attributes of the entity involved, for highlighting form fields.
    pub attributes: Vec<String>,
    /// The rule's severity; checks and conditions have none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<CheckTransition>,
    pub evidence: Value,
}

/// Whether an entity can be published to an enabled channel right now.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PublicationReadiness {
    pub context_id: Uuid,
    pub context_code: String,
    pub ready: bool,
    /// Failing required checks, shaped like `error.details.violations`.
    pub violations: Vec<CheckViolation>,
}

/// Workspace contexts and the evaluation clock for one check session.
pub(crate) struct CheckScope {
    workspace_id: Uuid,
    pub tree: ContextTree,
    pub now: chrono::DateTime<Utc>,
    /// The first subject family's enforced unique keys, read on first use.
    unique_keys: tokio::sync::OnceCell<(Uuid, Vec<EnforcedUniqueKey>)>,
}

impl CheckScope {
    pub(crate) async fn load(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> Result<Self, RepositoryError> {
        Ok(Self::new(
            workspace_id,
            ContextTree::load(conn, workspace_id).await?,
        ))
    }

    pub(crate) fn new(workspace_id: Uuid, tree: ContextTree) -> Self {
        Self {
            workspace_id,
            tree,
            now: Utc::now(),
            unique_keys: tokio::sync::OnceCell::new(),
        }
    }

    /// The context followed by its ancestors, as values are inherited.
    pub(crate) fn path(&self, context_id: Uuid) -> Result<Vec<Uuid>, RepositoryError> {
        self.tree.path(context_id, true)
    }

    pub(crate) fn code(&self, context_id: Uuid) -> String {
        self.tree.code(context_id)
    }

    pub(crate) fn context_ids(&self) -> Vec<Uuid> {
        self.tree.ids()
    }
}

/// Loads live entities with their current values. Missing or deleted IDs are
/// omitted. Reads see the caller's uncommitted writes.
pub(crate) async fn load_entities(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    ids: &[Uuid],
) -> Result<BTreeMap<Uuid, RecordValues>, RepositoryError> {
    load_records(
        conn,
        workspace_id,
        Selection::Entities(ids),
        None,
        RecordState::After,
    )
    .await
}

fn uuids(value: Option<&Value>) -> Vec<Uuid> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().and_then(|text| text.parse().ok()))
        .collect()
}

/// Records a page of subjects shares, read once for every context: linked
/// and referencing records, which do not depend on the context they are
/// resolved in. Anything not covered is loaded per subject as before.
#[derive(Default)]
pub(crate) struct RelatedPrefetch {
    /// IDs whose live records were requested; missing ones are not live.
    requested: std::collections::HashSet<Uuid>,
    records: BTreeMap<Uuid, RecordValues>,
    /// Referencing entity IDs by subject and `(blueprint, relationship)`.
    referencing: std::collections::HashMap<(Uuid, String, String), Vec<Uuid>>,
}

/// Records loaded at most by one [`prefetch_related`]; beyond this, records
/// are loaded per subject so a page's memory stays bounded.
const PREFETCH_MAX_RECORDS: usize = 5_000;

impl RelatedPrefetch {
    /// The live records of `ids`, or `None` when some were not prefetched.
    fn records(&self, ids: &[Uuid]) -> Option<BTreeMap<Uuid, &RecordValues>> {
        ids.iter().all(|id| self.requested.contains(id)).then(|| {
            ids.iter()
                .filter_map(|id| self.records.get(id).map(|record| (*id, record)))
                .collect()
        })
    }
}

/// Prefetches what [`load_related`] reads for `subjects` in `contexts`.
pub(crate) async fn prefetch_related(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subjects: &BTreeMap<Uuid, RecordValues>,
    contexts: &[Uuid],
    requirements: &Requirements,
) -> Result<RelatedPrefetch, RepositoryError> {
    let mut prefetch = RelatedPrefetch::default();
    let mut ids = std::collections::BTreeSet::new();
    if !requirements.linked.is_empty() {
        for context_id in contexts {
            let path = scope.path(*context_id)?;
            for subject in subjects.values() {
                let record = subject.record(&path);
                for relationship in &requirements.linked {
                    ids.extend(
                        uuids(record.values.get(relationship))
                            .into_iter()
                            .take(MAX_LINKED_RECORDS),
                    );
                }
            }
        }
    }
    let targets: Vec<Uuid> = subjects.keys().copied().collect();
    for (blueprint_code, relationship) in &requirements.referenced_by {
        let by_target = referencing_entity_ids_by_target(
            conn,
            scope.workspace_id,
            &targets,
            relationship,
            blueprint_code,
            MAX_REFERENCING_RECORDS as i64 + 1,
        )
        .await?;
        for target in &targets {
            let referencing = by_target.get(target).cloned().unwrap_or_default();
            ids.extend(referencing.iter().take(MAX_REFERENCING_RECORDS));
            prefetch.referencing.insert(
                (*target, blueprint_code.clone(), relationship.clone()),
                referencing,
            );
        }
    }
    if !ids.is_empty() && ids.len() <= PREFETCH_MAX_RECORDS {
        let ids: Vec<Uuid> = ids.into_iter().collect();
        prefetch.records = load_entities(conn, scope.workspace_id, &ids).await?;
        prefetch.requested = ids.into_iter().collect();
    }
    Ok(prefetch)
}

/// Loads the related data a predicate set needs for one subject and context,
/// reading linked and referencing records from `prefetch` when it has them.
pub(crate) async fn load_related(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    record: &Record,
    context_id: Uuid,
    requirements: &Requirements,
    prefetch: Option<&RelatedPrefetch>,
) -> Result<Related, RepositoryError> {
    let path = scope.path(context_id)?;
    let mut related = Related::default();

    // Collect every linked and referencing ID first so the records load in
    // one query.
    let mut linked = Vec::with_capacity(requirements.linked.len());
    for relationship in &requirements.linked {
        let mut ids = uuids(record.values.get(relationship));
        let truncated = ids.len() > MAX_LINKED_RECORDS;
        ids.truncate(MAX_LINKED_RECORDS);
        linked.push((relationship, ids, truncated));
    }
    let mut referencing = Vec::with_capacity(requirements.referenced_by.len());
    for key @ (blueprint_code, relationship) in &requirements.referenced_by {
        let prefetched = prefetch.and_then(|prefetch| {
            prefetch
                .referencing
                .get(&(subject.id, blueprint_code.clone(), relationship.clone()))
        });
        let mut ids = match prefetched {
            Some(ids) => ids.clone(),
            None => {
                referencing_entity_ids(
                    conn,
                    scope.workspace_id,
                    ReferenceQuery {
                        target: subject.id,
                        attribute_code: relationship,
                        referrers: Referrers::BlueprintCode(blueprint_code),
                        only: None,
                        exclude_target: false,
                        limit: MAX_REFERENCING_RECORDS as i64 + 1,
                    },
                )
                .await?
            }
        };
        let truncated = ids.len() > MAX_REFERENCING_RECORDS;
        ids.truncate(MAX_REFERENCING_RECORDS);
        referencing.push((key, ids, truncated));
    }
    let mut all_ids: Vec<Uuid> = linked
        .iter()
        .flat_map(|(_, ids, _)| ids)
        .chain(referencing.iter().flat_map(|(_, ids, _)| ids))
        .copied()
        .collect();
    all_ids.sort_unstable();
    all_ids.dedup();
    let fetched;
    let loaded = match prefetch.and_then(|prefetch| prefetch.records(&all_ids)) {
        Some(prefetched) => prefetched,
        None => {
            fetched = load_entities(conn, scope.workspace_id, &all_ids).await?;
            fetched.iter().map(|(id, record)| (*id, record)).collect()
        }
    };
    let records_of = |ids: &[Uuid]| -> Vec<Record> {
        ids.iter()
            .filter_map(|id| loaded.get(id))
            .map(|entity| entity.record(&path))
            .collect()
    };

    for (relationship, ids, truncated) in linked {
        related.linked.insert(
            relationship.clone(),
            RecordSet {
                records: records_of(&ids),
                truncated,
            },
        );
    }
    let subject_id = Value::String(subject.id.to_string());
    for (key, ids, truncated) in referencing {
        let mut records = records_of(&ids);
        // The reference must hold in this context, not only somewhere.
        records.retain(|referencing| {
            referencing
                .values
                .get(&key.1)
                .and_then(Value::as_array)
                .is_some_and(|targets| targets.contains(&subject_id))
        });
        related
            .referenced_by
            .insert(key.clone(), RecordSet { records, truncated });
    }
    for key in &requirements.unique {
        let others = duplicates(conn, scope, subject, context_id, &path, key).await?;
        related.duplicates.insert(key.clone(), others);
    }
    for relationship in &requirements.acyclic {
        let state = cycle(conn, scope, subject, record, context_id, relationship).await?;
        related.cycles.insert(relationship.clone(), state);
    }
    Ok(related)
}

/// The normalized key of `record` in the context `path`, or `None` when a
/// component is missing. Only blueprint fields take part, as in unique keys.
fn key_components(
    record: &RecordValues,
    path: &[Uuid],
    key: &[String],
    case_sensitive: bool,
) -> Option<Vec<Value>> {
    key.iter()
        .map(|code| {
            let attribute = record
                .attributes
                .get(code)
                .filter(|attribute| !attribute.entity_scoped)?;
            normalize_key_component(
                &attribute.value_type,
                &attribute.resolve(path)?.key_input(),
                case_sensitive,
            )
        })
        .collect()
}

/// Other live entities of the subject's blueprint family whose `key`
/// resolves, in the same context, to the same normalized values. Values are
/// compared as `[[unique_keys]]` compare them (see
/// [`normalize_key_component`]); strings ignore case. When a declared key of
/// the family covers exactly these attributes, its index answers directly.
/// Otherwise candidates are scanned in batches with no limit.
async fn duplicates(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    context_id: Uuid,
    path: &[Uuid],
    key: &[String],
) -> Result<Vec<String>, RepositoryError> {
    let case_sensitive = UNIQUE_PREDICATE_CASE_SENSITIVE;
    let Some(components) = key_components(subject, path, key, case_sensitive) else {
        return Ok(Vec::new());
    };
    let is_default = scope.tree.default_context()?.id == context_id;
    let memo = scope
        .unique_keys
        .get_or_try_init(|| async {
            enforced_unique_keys(&mut *conn, scope.workspace_id, subject.blueprint_id)
                .await
                .map(|keys| (subject.blueprint_id, keys))
        })
        .await?;
    let other_family;
    let keys = if memo.0 == subject.blueprint_id {
        &memo.1
    } else {
        other_family = enforced_unique_keys(conn, scope.workspace_id, subject.blueprint_id).await?;
        &other_family
    };
    let declared = keys.iter().find(|declared| {
        declared.case_sensitive == case_sensitive
            && (declared.scope == UniqueKeyScope::Context || is_default)
            && declared.attributes.len() == key.len()
            && declared.attributes.iter().all(|code| key.contains(code))
    });
    if let Some(declared) = declared {
        let ordered: Vec<Value> = declared
            .attributes
            .iter()
            .filter_map(|code| key.iter().position(|item| item == code))
            .map(|position| components[position].clone())
            .collect();
        let others: Vec<Uuid> = sqlx::query_scalar(
            "SELECT entity_id FROM entity_unique_key_values WHERE workspace_id = $1 AND blueprint_id = $2 AND key_code = $3 AND context_id = $4 AND key_hash = $5 AND entity_id <> $6 ORDER BY entity_id",
        )
        .bind(scope.workspace_id)
        .bind(subject.blueprint_id)
        .bind(&declared.code)
        .bind(context_id)
        .bind(key_hash(&Value::Array(ordered)))
        .bind(subject.id)
        .fetch_all(&mut *conn)
        .await?;
        return Ok(others.iter().map(Uuid::to_string).collect());
    }

    // Candidates have a row for the first attribute that could normalize to
    // the subject's value in some context; the comparison happens in Rust on
    // the values resolved in this context.
    let first = &key[0];
    let (filter, parameter) = candidate_filter(
        subject.value_type(first).unwrap_or_default(),
        &components[0],
    );
    let query = format!(
        r#"SELECT DISTINCT av.entity_id
           FROM attribute_values av
           JOIN entities e ON e.id = av.entity_id AND e.workspace_id = $1 AND e.deleted_at IS NULL
            AND e.blueprint_id = $2 AND e.id <> $3
           JOIN attributes a ON a.id = av.attribute_id AND a.code = $4 AND a.deleted_at IS NULL
            AND a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version
           WHERE av.workspace_id = $1 AND av.relationship_target_entity_id IS NULL
             AND av.entity_id > $5 AND {filter}
           ORDER BY av.entity_id
           LIMIT $7"#
    );
    let mut others = Vec::new();
    let mut after = Uuid::nil();
    loop {
        let batch: Vec<Uuid> = sqlx::query_scalar(&query)
            .bind(scope.workspace_id)
            .bind(subject.blueprint_id)
            .bind(subject.id)
            .bind(first)
            .bind(after)
            .bind(&parameter)
            .bind(UNIQUE_CANDIDATE_BATCH)
            .fetch_all(&mut *conn)
            .await?;
        let Some(last) = batch.last() else {
            break;
        };
        after = *last;
        let loaded = load_records(
            conn,
            scope.workspace_id,
            Selection::Entities(&batch),
            Some(key),
            RecordState::After,
        )
        .await?;
        others.extend(
            loaded
                .values()
                .filter(|other| {
                    key_components(other, path, key, case_sensitive).as_ref() == Some(&components)
                })
                .map(|other| other.id.to_string()),
        );
        if (batch.len() as i64) < UNIQUE_CANDIDATE_BATCH {
            break;
        }
    }
    Ok(others)
}

/// A SQL condition (on `av`, parameter `$6`) every row whose value
/// normalizes to `component` satisfies.
fn candidate_filter(value_type: &str, component: &Value) -> (&'static str, Option<String>) {
    let text = match component {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    match value_type {
        "string" => {
            // Normalized strings keep each whitespace-free token verbatim
            // (case aside). ASCII case folding is the same in PostgreSQL.
            let token = text
                .split(' ')
                .max_by_key(|token| token.len())
                .unwrap_or_default();
            if token.is_ascii() && !token.is_empty() {
                let escaped = token
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                ("av.value_text ILIKE $6", Some(format!("%{escaped}%")))
            } else {
                ("($6::text IS NULL AND av.value_text IS NOT NULL)", None)
            }
        }
        "number" => ("av.value_number = $6::text::numeric", Some(text)),
        "integer" => ("av.value_integer = $6::text::bigint", Some(text)),
        "boolean" => ("av.value_boolean = $6::text::boolean", Some(text)),
        "date" => ("av.value_date = $6::text::date", Some(text)),
        "datetime" => ("av.value_datetime = $6::text::timestamptz", Some(text)),
        _ => ("$6::text IS NULL", None),
    }
}

/// Whether following `relationship` from the subject, as resolved in the
/// context, returns to it. Shares the hierarchy walk of structural
/// constraints, bounded by [`MAX_CYCLE_VISITS`].
async fn cycle(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    record: &Record,
    context_id: Uuid,
    relationship: &str,
) -> Result<CycleState, RepositoryError> {
    let starts = uuids(record.values.get(relationship));
    let walk = walk_hierarchy(
        conn,
        scope.workspace_id,
        &scope.tree,
        context_id,
        &HierarchyField {
            code: relationship,
            family: None,
        },
        &starts,
        subject.id,
        Some(MAX_CYCLE_VISITS),
    )
    .await?;
    Ok(match walk {
        HierarchyWalk::Clear => CycleState::Clear,
        HierarchyWalk::Cycle(path) => CycleState::Cycle(path.iter().map(Uuid::to_string).collect()),
        HierarchyWalk::LimitReached => CycleState::LimitReached,
    })
}

/// Evaluates predicates for one subject in one context, overriding values
/// of the resolved record first (used to preview a status destination).
pub(crate) async fn evaluate_in_context(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    context_id: Uuid,
    overrides: &[(String, Value)],
    predicates: &[&Predicate],
) -> Result<Vec<Result<(), Failure>>, RepositoryError> {
    evaluate_in_context_with(
        conn, scope, subject, context_id, overrides, predicates, None,
    )
    .await
}

/// [`evaluate_in_context`] reading related records from a page prefetch.
pub(crate) async fn evaluate_in_context_with(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    context_id: Uuid,
    overrides: &[(String, Value)],
    predicates: &[&Predicate],
    prefetch: Option<&RelatedPrefetch>,
) -> Result<Vec<Result<(), Failure>>, RepositoryError> {
    let path = scope.path(context_id)?;
    let mut record = subject.record(&path);
    for (code, value) in overrides {
        if value.is_null() {
            record.values.remove(code);
        } else {
            record.values.insert(code.clone(), value.clone());
        }
    }
    let requirements = Requirements::for_predicates(predicates.iter().copied());
    let related = load_related(
        conn,
        scope,
        subject,
        &record,
        context_id,
        &requirements,
        prefetch,
    )
    .await?;
    let input = Evaluation {
        subject: &record,
        related: &related,
        now: scope.now,
    };
    Ok(predicates
        .iter()
        .map(|predicate| predicate::evaluate(predicate, &input))
        .collect())
}

/// An enabled rule revision with its compiled plan.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnabledRule {
    pub code: String,
    pub context_id: Option<Uuid>,
    pub compiled: catalog_rules::CompiledRule,
}

impl EnabledRule {
    /// Whether the rule rejects every write that leaves the entity violating it.
    fn enforced_on_save(&self) -> bool {
        self.compiled
            .enforcement
            .as_ref()
            .is_some_and(|enforcement| enforcement.on_save)
    }

    /// Whether the rule applies in `context_id`.
    pub(crate) fn applies_in(&self, context_id: Uuid) -> bool {
        self.context_id.is_none_or(|context| context == context_id)
    }
}

/// The enabled rule revisions of one blueprint revision, by code.
pub(crate) async fn enabled_rules(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    blueprint_id: Uuid,
    blueprint_version: i64,
) -> Result<Vec<EnabledRule>, RepositoryError> {
    sqlx::query_as::<_, (String, Option<Uuid>, Value)>(
        "SELECT r.code, r.context_id, r.compiled_plan FROM rules r JOIN rule_lifecycles l ON l.rule_id = r.id AND l.workspace_id = r.workspace_id AND l.enabled_version = r.version WHERE r.workspace_id = $1 AND r.blueprint_id = $2 AND r.blueprint_version = $3 AND r.status = 'published' ORDER BY r.code",
    )
    .bind(workspace_id)
    .bind(blueprint_id)
    .bind(blueprint_version)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(code, context_id, plan)| {
        Ok(EnabledRule {
            code,
            context_id,
            compiled: serde_json::from_value(plan)
                .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?,
        })
    })
    .collect()
}

/// Every enabled rule revision of the workspace, by blueprint revision, in
/// the order [`enabled_rules`] returns them.
pub(crate) async fn enabled_rule_sets(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<super::generations::EnabledRuleSets, RepositoryError> {
    let rows = sqlx::query_as::<_, (Uuid, i64, String, Option<Uuid>, Value)>(
        "SELECT r.blueprint_id, r.blueprint_version, r.code, r.context_id, r.compiled_plan FROM rules r JOIN rule_lifecycles l ON l.rule_id = r.id AND l.workspace_id = r.workspace_id AND l.enabled_version = r.version WHERE r.workspace_id = $1 AND r.status = 'published' ORDER BY r.code",
    )
    .bind(workspace_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut sets = super::generations::EnabledRuleSets::new();
    for (blueprint_id, blueprint_version, code, context_id, plan) in rows {
        sets.entry((blueprint_id, blueprint_version))
            .or_default()
            .push(EnabledRule {
                code,
                context_id,
                compiled: serde_json::from_value(plan)
                    .map_err(|error| RepositoryError::InvalidRuleDefinition(error.to_string()))?,
            });
    }
    Ok(sets)
}

/// The `x-attricat-checks` of an entity schema, failing closed when they are
/// malformed. A blueprint without an entity schema has none.
pub(crate) fn entity_checks(entity_schema: Option<&Value>) -> Result<Vec<Check>, RepositoryError> {
    entity_schema
        .map(predicate::entity_checks)
        .transpose()
        .map_err(RepositoryError::InvalidBlueprintDefinition)
        .map(Option::unwrap_or_default)
}

/// One predicate to evaluate in one context, with its reporting identity.
struct Job<'a> {
    source: CheckSource,
    code: &'a str,
    message: Option<&'a str>,
    predicate: &'a Predicate,
    severity: Option<Severity>,
    transition: Option<CheckTransition>,
}

impl<'a> Job<'a> {
    fn check(check: &'a Check) -> Self {
        Self {
            source: CheckSource::EntityCheck,
            code: &check.code,
            message: check.message.as_deref(),
            predicate: &check.predicate,
            severity: None,
            transition: None,
        }
    }

    fn rule(rule: &'a EnabledRule, transition: Option<CheckTransition>) -> Self {
        Self {
            source: CheckSource::Rule,
            code: &rule.code,
            message: None,
            predicate: &rule.compiled.predicate,
            severity: Some(rule.compiled.severity.clone()),
            transition,
        }
    }
}

/// Jobs grouped by the context they are evaluated in.
type JobsByContext<'a> = BTreeMap<Uuid, Vec<Job<'a>>>;

/// The transition conditions of a status change, failing closed when the
/// stored edge's conditions are malformed.
fn change_conditions(change: &StatusChange) -> Result<Vec<Check>, RepositoryError> {
    catalog_validation::status::transition_conditions(&change.schema, &change.before, &change.after)
        .map_err(RepositoryError::InvalidBlueprintDefinition)
}

fn condition_jobs<'a>(change: &StatusChange, conditions: &'a [Check]) -> Vec<Job<'a>> {
    let transition = change.transition();
    conditions
        .iter()
        .map(|condition| Job {
            source: CheckSource::TransitionCondition,
            code: &condition.code,
            message: condition.message.as_deref(),
            predicate: &condition.predicate,
            severity: None,
            transition: Some(transition.clone()),
        })
        .collect()
}

/// The job of an enforcing rule that guards the change's transition.
fn guard_job<'a>(change: &StatusChange, rule: &'a EnabledRule) -> Option<Job<'a>> {
    let guards = rule
        .compiled
        .enforcement
        .as_ref()
        .is_some_and(|enforcement| {
            enforcement.guards_transition(
                &change.attribute_code,
                change.before.as_str(),
                change.after.as_str(),
            )
        });
    (guards && rule.applies_in(change.context_id))
        .then(|| Job::rule(rule, Some(change.transition())))
}

/// Transition conditions and guarding rules a status change must satisfy.
fn transition_jobs<'a>(
    change: &StatusChange,
    conditions: &'a [Check],
    rules: &'a [EnabledRule],
) -> Vec<Job<'a>> {
    let mut jobs = condition_jobs(change, conditions);
    jobs.extend(rules.iter().filter_map(|rule| guard_job(change, rule)));
    jobs
}

/// Entity checks apply in every context.
fn entity_check_jobs<'a>(contexts: &[Uuid], checks: &'a [Check]) -> JobsByContext<'a> {
    if checks.is_empty() {
        return JobsByContext::new();
    }
    contexts
        .iter()
        .map(|context| (*context, checks.iter().map(Job::check).collect()))
        .collect()
}

/// Each change's transition conditions apply in the change's context.
fn transition_condition_jobs<'a>(
    conditions: &'a [(&StatusChange, Vec<Check>)],
) -> JobsByContext<'a> {
    let mut jobs = JobsByContext::new();
    for (change, conditions) in conditions {
        jobs.entry(change.context_id)
            .or_default()
            .extend(condition_jobs(change, conditions));
    }
    jobs
}

/// On-save rules apply in their context (every context without one); rules
/// that guard a change's transition apply in the change's context.
fn enforcing_rule_jobs<'a>(
    contexts: &[Uuid],
    rules: &'a [EnabledRule],
    changes: &[StatusChange],
) -> JobsByContext<'a> {
    let mut jobs = JobsByContext::new();
    for rule in rules {
        if rule.enforced_on_save() {
            let rule_contexts = rule.context_id.as_slice();
            let rule_contexts = if rule_contexts.is_empty() {
                contexts
            } else {
                rule_contexts
            };
            for context_id in rule_contexts {
                jobs.entry(*context_id)
                    .or_default()
                    .push(Job::rule(rule, None));
            }
        }
        for change in changes {
            if let Some(job) = guard_job(change, rule) {
                jobs.entry(change.context_id).or_default().push(job);
            }
        }
    }
    jobs
}

/// Merges a failure into the report; one violation per declaration lists
/// every context in which it failed.
fn record_violation(
    violations: &mut Vec<CheckViolation>,
    job: &Job<'_>,
    context: String,
    failure: Failure,
) {
    if let Some(existing) = violations.iter_mut().find(|violation| {
        violation.source == job.source
            && violation.code == job.code
            && violation.transition == job.transition
    }) {
        if !existing.contexts.contains(&context) {
            existing.contexts.push(context);
        }
        return;
    }
    if violations.len() >= MAX_REPORTED_VIOLATIONS {
        return;
    }
    violations.push(CheckViolation {
        source: job.source,
        code: job.code.to_owned(),
        message: job.message.map(str::to_owned).unwrap_or(failure.message),
        contexts: vec![context],
        attributes: failure.attributes,
        severity: job.severity.clone(),
        transition: job.transition.clone(),
        evidence: failure.evidence,
    });
}

/// Evaluates several job groups with one evaluation per context and returns
/// each group's violations, recorded as if that group were evaluated alone.
async fn run_job_groups<const N: usize>(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    groups: [&JobsByContext<'_>; N],
) -> Result<[Vec<CheckViolation>; N], RepositoryError> {
    let mut contexts: BTreeMap<Uuid, Vec<(usize, &Job<'_>)>> = BTreeMap::new();
    for (group, jobs) in groups.iter().enumerate() {
        for (context_id, jobs) in jobs.iter() {
            contexts
                .entry(*context_id)
                .or_default()
                .extend(jobs.iter().map(|job| (group, job)));
        }
    }
    let mut violations: [Vec<CheckViolation>; N] = std::array::from_fn(|_| Vec::new());
    for (context_id, jobs) in &contexts {
        let predicates: Vec<&Predicate> = jobs.iter().map(|(_, job)| job.predicate).collect();
        let outcomes =
            evaluate_in_context(conn, scope, subject, *context_id, &[], &predicates).await?;
        for ((group, job), outcome) in jobs.iter().zip(outcomes) {
            if let Err(failure) = outcome {
                record_violation(
                    &mut violations[*group],
                    job,
                    scope.code(*context_id),
                    failure,
                );
            }
        }
    }
    Ok(violations)
}

/// Transition conditions and enforcing `rules` that the subject's saved
/// state plus the change's destination would not satisfy, for the status
/// control. `rules` are the subject's [`enabled_rules`].
pub(super) async fn transition_unmet(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &RecordValues,
    rules: &[EnabledRule],
    change: &StatusChange,
) -> Result<Vec<CheckViolation>, RepositoryError> {
    let conditions = change_conditions(change)?;
    let jobs = transition_jobs(change, &conditions, rules);
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let predicates: Vec<&Predicate> = jobs.iter().map(|job| job.predicate).collect();
    let outcomes = evaluate_in_context(
        conn,
        scope,
        subject,
        change.context_id,
        &[(change.attribute_code.clone(), change.after.clone())],
        &predicates,
    )
    .await?;
    let mut unmet = Vec::new();
    for (job, outcome) in jobs.iter().zip(outcomes) {
        if let Err(failure) = outcome {
            record_violation(&mut unmet, job, change.context_code.clone(), failure);
        }
    }
    Ok(unmet)
}

impl CatalogRepository {
    /// Enforces entity-schema checks, status transition conditions and
    /// enforcing rules on the transaction's final state. Every write that
    /// changes attribute values calls it through `validate_entity_schema`,
    /// with the write's own status `changes`: system transitions (approval
    /// voids) are not guarded. Extension annotation patches, which change
    /// only system tags and metadata, call it through
    /// [`Self::enforce_tag_checks`].
    ///
    /// The three kinds are evaluated in that order and the first kind with a
    /// failure is reported with its own error.
    pub(super) async fn enforce_declarative_checks(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_schema: Option<&Value>,
        tree: &ContextTree,
        subject: &RecordValues,
        changes: &[StatusChange],
    ) -> Result<(), RepositoryError> {
        // A malformed entity check is reported before a malformed rule.
        entity_checks(entity_schema)?;
        let rules = enabled_rules(
            transaction,
            self.workspace_id.0,
            subject.blueprint_id,
            subject.blueprint_version,
        )
        .await?;
        self.enforce_declarative_checks_with(
            transaction,
            entity_schema,
            tree,
            subject,
            changes,
            &rules,
        )
        .await
    }

    /// [`Self::enforce_declarative_checks`] with the revision's enabled rules
    /// when the caller already has them.
    pub(super) async fn enforce_declarative_checks_using(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_schema: Option<&Value>,
        rules: Option<&[EnabledRule]>,
        tree: &ContextTree,
        subject: &RecordValues,
        changes: &[StatusChange],
    ) -> Result<(), RepositoryError> {
        match rules {
            Some(rules) => {
                self.enforce_declarative_checks_with(
                    transaction,
                    entity_schema,
                    tree,
                    subject,
                    changes,
                    rules,
                )
                .await
            }
            None => {
                self.enforce_declarative_checks(transaction, entity_schema, tree, subject, changes)
                    .await
            }
        }
    }

    /// [`Self::enforce_declarative_checks`] with the revision's enabled rules
    /// already loaded.
    async fn enforce_declarative_checks_with(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_schema: Option<&Value>,
        tree: &ContextTree,
        subject: &RecordValues,
        changes: &[StatusChange],
        rules: &[EnabledRule],
    ) -> Result<(), RepositoryError> {
        let checks = entity_checks(entity_schema)?;
        let rules: Vec<EnabledRule> = rules
            .iter()
            .filter(|rule| rule.compiled.enforcement.is_some())
            .cloned()
            .collect();
        let mut conditions = Vec::new();
        for change in changes {
            let declared = change_conditions(change)?;
            if !declared.is_empty() {
                conditions.push((change, declared));
            }
        }
        if checks.is_empty() && conditions.is_empty() && rules.is_empty() {
            return Ok(());
        }
        let scope = CheckScope::new(self.workspace_id.0, tree.clone());
        let contexts = scope.context_ids();
        // One evaluation per context; failures keep their precedence:
        // entity checks, then transition conditions, then rules.
        let [checks, conditions, rules] = run_job_groups(
            transaction,
            &scope,
            subject,
            [
                &entity_check_jobs(&contexts, &checks),
                &transition_condition_jobs(&conditions),
                &enforcing_rule_jobs(&contexts, &rules, changes),
            ],
        )
        .await?;
        if !checks.is_empty() {
            return Err(RepositoryError::EntityCheckFailed(checks));
        }
        if !conditions.is_empty() {
            return Err(RepositoryError::TransitionConditionsUnmet(conditions));
        }
        if !rules.is_empty() {
            return Err(RepositoryError::RuleViolation(rules));
        }
        Ok(())
    }

    /// Enforces entity checks and on-save enforcing rules after a write that
    /// changed only the entity's system tags, when any of them reads the
    /// entity's own tags (`has_tag`, `missing_tag`). Such a write changes no
    /// value, so it has no status transitions and the JSON entity schema
    /// cannot change its verdict.
    pub(super) async fn enforce_tag_checks(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let entity_schema = sqlx::query_scalar::<_, Option<Value>>(
            "SELECT entity_schema FROM blueprints WHERE id = $1 AND version = $2 AND workspace_id = $3",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(self.workspace_id.0)
        .fetch_one(&mut **transaction)
        .await?;
        let checks = entity_checks(entity_schema.as_ref())?;
        let rules = enabled_rules(
            transaction,
            self.workspace_id.0,
            entity.blueprint_id,
            entity.blueprint_version,
        )
        .await?;
        let reads_tags = checks
            .iter()
            .any(|check| check.predicate.reads_subject_tags())
            || rules.iter().any(|rule| {
                rule.enforced_on_save() && rule.compiled.predicate.reads_subject_tags()
            });
        if !reads_tags {
            return Ok(());
        }
        let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
        let record = load_record(
            transaction,
            self.workspace_id.0,
            entity.id,
            RecordState::After,
        )
        .await?
        .ok_or(RepositoryError::NotFound("entity"))?;
        self.enforce_declarative_checks_with(
            transaction,
            entity_schema.as_ref(),
            &tree,
            &record,
            &[],
            &rules,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn change(schema: Value) -> StatusChange {
        StatusChange {
            attribute_code: "status".to_owned(),
            schema,
            context_id: Uuid::nil(),
            context_code: "default".to_owned(),
            before: json!("draft"),
            after: json!("live"),
        }
    }

    #[test]
    fn malformed_transition_conditions_fail_closed() {
        let schema = |conditions: Value| {
            json!({"type": "string", "enum": ["draft", "live"], "x-attricat-status": {
                "version": 1,
                "options": [{"code": "draft", "label": "Draft"}, {"code": "live", "label": "Live"}],
                "transitions": [{"from": "draft", "to": "live", "conditions": conditions}]
            }})
        };
        let valid = change(schema(json!([
            {"code": "owner", "predicate": {"type": "required", "attribute_code": "owner"}}
        ])));
        let conditions = change_conditions(&valid).unwrap();
        let jobs = transition_jobs(&valid, &conditions, &[]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source, CheckSource::TransitionCondition);
        assert_eq!(
            jobs[0].transition.as_ref().unwrap().to.as_deref(),
            Some("live")
        );
        for malformed in [
            json!([{"code": "owner", "predicate": {"type": "unknown"}}]),
            json!([{"code": "owner", "predicate": {"type": "unique", "attribute_codes": ["a"]}}]),
            json!("not a list"),
        ] {
            assert!(
                matches!(
                    change_conditions(&change(schema(malformed.clone()))),
                    Err(RepositoryError::InvalidBlueprintDefinition(_))
                ),
                "{malformed}"
            );
        }
    }

    #[test]
    fn candidate_filters_never_exclude_a_normalized_match() {
        assert_eq!(
            candidate_filter("string", &json!("ab x_%")),
            ("av.value_text ILIKE $6", Some("%x\\_\\%%".to_owned()))
        );
        assert_eq!(
            candidate_filter("string", &json!("żółw")).1,
            None,
            "non-ASCII text is compared without a SQL prefilter"
        );
        assert_eq!(
            candidate_filter("number", &json!("1.5")),
            (
                "av.value_number = $6::text::numeric",
                Some("1.5".to_owned())
            )
        );
        assert_eq!(candidate_filter("time", &json!({"time": "x"})).1, None);
    }
}
