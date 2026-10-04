//! Host side of the shared predicate engine: loads bounded, context-resolved
//! records for [`catalog_validation::predicate`] and enforces entity-schema
//! checks, status transition conditions and enforcing rules on every write.
use super::status::StatusChange;
use super::values::{NativeValueRow, native_value_json};
use super::*;
use catalog_validation::predicate::{
    self, Check, CycleState, Evaluation, Failure, MAX_CYCLE_VISITS, MAX_LINKED_RECORDS,
    MAX_REFERENCING_RECORDS, Predicate, Record, RecordSet, Related, Requirements,
};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgConnection;
use std::collections::{BTreeMap, HashMap, VecDeque};

/// At most this many violations are reported in one error response.
pub const MAX_REPORTED_VIOLATIONS: usize = 50;
/// Candidates fetched for one `unique` key before values are compared.
const MAX_UNIQUE_CANDIDATES: i64 = 200;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
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

#[derive(Clone, Debug)]
pub(crate) struct ContextNode {
    pub id: Uuid,
    pub code: String,
    pub parent_id: Option<Uuid>,
}

/// Workspace contexts and the evaluation clock for one check session.
pub(crate) struct CheckScope {
    workspace_id: Uuid,
    pub contexts: Vec<ContextNode>,
    pub now: chrono::DateTime<Utc>,
}

impl CheckScope {
    pub(crate) async fn load(
        conn: &mut PgConnection,
        workspace_id: Uuid,
    ) -> Result<Self, RepositoryError> {
        let contexts = sqlx::query_as::<_, (Uuid, String, Option<Uuid>)>(
            "SELECT id, code, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(workspace_id)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(id, code, parent_id)| ContextNode {
            id,
            code,
            parent_id,
        })
        .collect();
        Ok(Self {
            workspace_id,
            contexts,
            now: Utc::now(),
        })
    }

    /// The context followed by its ancestors, as values are inherited.
    pub(crate) fn path(&self, context_id: Uuid) -> Result<Vec<Uuid>, RepositoryError> {
        let mut path = Vec::new();
        let mut current = Some(context_id);
        while let Some(id) = current {
            if path.contains(&id) {
                return Err(RepositoryError::InvalidContext);
            }
            path.push(id);
            current = self
                .contexts
                .iter()
                .find(|context| context.id == id)
                .and_then(|context| context.parent_id);
        }
        Ok(path)
    }

    pub(crate) fn code(&self, context_id: Uuid) -> String {
        self.contexts
            .iter()
            .find(|context| context.id == context_id)
            .map(|context| context.code.clone())
            .unwrap_or_default()
    }

    pub(crate) fn context_ids(&self) -> Vec<Uuid> {
        self.contexts.iter().map(|context| context.id).collect()
    }
}

#[derive(Debug, Default)]
struct StoredAttribute {
    value_type: String,
    fallback_none: bool,
    by_context: HashMap<Uuid, (Value, chrono::DateTime<Utc>)>,
}

/// One entity's current direct values in every context.
#[derive(Debug, Default)]
pub(crate) struct StoredEntity {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    tags: Vec<String>,
    attributes: HashMap<String, StoredAttribute>,
}

impl StoredEntity {
    /// Resolves values for a context path, honouring `context_fallback = "none"`.
    pub(crate) fn resolve(&self, path: &[Uuid]) -> Record {
        let mut record = Record {
            id: self.id.to_string(),
            tags: self.tags.clone(),
            ..Record::default()
        };
        for (code, attribute) in &self.attributes {
            let candidates = if attribute.fallback_none {
                &path[..path.len().min(1)]
            } else {
                path
            };
            if let Some((value, changed_at)) = candidates
                .iter()
                .find_map(|context| attribute.by_context.get(context))
            {
                record.values.insert(code.clone(), value.clone());
                record.changed_at.insert(code.clone(), *changed_at);
            }
        }
        record
    }

    fn value_type(&self, code: &str) -> Option<&str> {
        self.attributes
            .get(code)
            .map(|attribute| attribute.value_type.as_str())
    }
}

#[derive(sqlx::FromRow)]
struct CheckValueRow {
    entity_id: Uuid,
    attribute_code: String,
    context_fallback: String,
    context_id: Uuid,
    relationship_target_entity_id: Option<Uuid>,
    created_at: chrono::DateTime<Utc>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

/// Loads live entities with their current values. Missing or deleted IDs are
/// omitted. Reads see the caller's uncommitted writes.
pub(crate) async fn load_entities(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, StoredEntity>, RepositoryError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut entities: HashMap<Uuid, StoredEntity> = sqlx::query_as::<_, (Uuid, Uuid, Vec<String>)>(
        "SELECT id, blueprint_id, system_tags FROM entities WHERE workspace_id = $1 AND id = ANY($2) AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(ids)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(id, blueprint_id, tags)| {
        (
            id,
            StoredEntity {
                id,
                blueprint_id,
                tags,
                attributes: HashMap::new(),
            },
        )
    })
    .collect();
    let rows = sqlx::query_as::<_, CheckValueRow>(
        r#"SELECT av.entity_id, a.code AS attribute_code, a.context_fallback, av.context_id,
                  av.relationship_target_entity_id, av.created_at, a.value_type,
                  av.value_text, av.value_number, av.value_integer, av.value_boolean,
                  av.value_date, av.value_datetime, av.value_time, av.value_time_zone, av.value_json
           FROM attribute_values av
           JOIN entities e ON e.id = av.entity_id AND e.workspace_id = $1 AND e.deleted_at IS NULL
           JOIN attributes a ON a.id = av.attribute_id AND a.deleted_at IS NULL
            AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version)
                 OR a.entity_id = e.id)
           WHERE av.entity_id = ANY($2)
             AND (av.relationship_target_entity_id IS NULL OR av.active)"#,
    )
    .bind(workspace_id)
    .bind(ids)
    .fetch_all(&mut *conn)
    .await?;
    for row in rows {
        let Some(entity) = entities.get_mut(&row.entity_id) else {
            continue;
        };
        let attribute = entity
            .attributes
            .entry(row.attribute_code)
            .or_insert_with(|| StoredAttribute {
                value_type: row.native.value_type.clone(),
                fallback_none: row.context_fallback == "none",
                ..StoredAttribute::default()
            });
        if let Some(target) = row.relationship_target_entity_id {
            let entry = attribute
                .by_context
                .entry(row.context_id)
                .or_insert_with(|| (Value::Array(Vec::new()), row.created_at));
            if let Value::Array(targets) = &mut entry.0 {
                targets.push(Value::String(target.to_string()));
            }
            entry.1 = entry.1.max(row.created_at);
            continue;
        }
        let value = native_value_json(row.native)?;
        if !value.is_null() {
            attribute
                .by_context
                .insert(row.context_id, (value, row.created_at));
        }
    }
    Ok(entities)
}

fn uuids(value: Option<&Value>) -> Vec<Uuid> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().and_then(|text| text.parse().ok()))
        .collect()
}

/// Loads the related data a predicate set needs for one subject and context.
pub(crate) async fn load_related(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &StoredEntity,
    record: &Record,
    path: &[Uuid],
    requirements: &Requirements,
) -> Result<Related, RepositoryError> {
    let mut related = Related::default();
    for relationship in &requirements.linked {
        let mut ids = uuids(record.values.get(relationship));
        let truncated = ids.len() > MAX_LINKED_RECORDS;
        ids.truncate(MAX_LINKED_RECORDS);
        let loaded = load_entities(conn, scope.workspace_id, &ids).await?;
        let records = ids
            .iter()
            .filter_map(|id| loaded.get(id))
            .map(|entity| entity.resolve(path))
            .collect();
        related
            .linked
            .insert(relationship.clone(), RecordSet { records, truncated });
    }
    for (blueprint_code, relationship) in &requirements.referenced_by {
        let mut ids: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT DISTINCT av.entity_id
               FROM attribute_values av
               JOIN entities e ON e.id = av.entity_id AND e.workspace_id = $1 AND e.deleted_at IS NULL
               JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version
                AND b.workspace_id = $1 AND b.code = $3
               JOIN attributes a ON a.id = av.attribute_id AND a.code = $4 AND a.deleted_at IS NULL
                AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version)
                     OR a.entity_id = e.id)
               WHERE av.relationship_target_entity_id = $2 AND av.active
               LIMIT $5"#,
        )
        .bind(scope.workspace_id)
        .bind(subject.id)
        .bind(blueprint_code)
        .bind(relationship)
        .bind(MAX_REFERENCING_RECORDS as i64 + 1)
        .fetch_all(&mut *conn)
        .await?;
        let truncated = ids.len() > MAX_REFERENCING_RECORDS;
        ids.truncate(MAX_REFERENCING_RECORDS);
        ids.sort();
        let loaded = load_entities(conn, scope.workspace_id, &ids).await?;
        let subject_id = Value::String(subject.id.to_string());
        let records = ids
            .iter()
            .filter_map(|id| loaded.get(id))
            .map(|entity| entity.resolve(path))
            // The reference must hold in this context, not only somewhere.
            .filter(|referencing| {
                referencing
                    .values
                    .get(relationship)
                    .and_then(Value::as_array)
                    .is_some_and(|targets| targets.contains(&subject_id))
            })
            .collect();
        related.referenced_by.insert(
            (blueprint_code.clone(), relationship.clone()),
            RecordSet { records, truncated },
        );
    }
    for key in &requirements.unique {
        let others = duplicates(conn, scope, subject, record, path, key).await?;
        related.duplicates.insert(key.clone(), others);
    }
    for relationship in &requirements.acyclic {
        let state = cycle(conn, scope, subject, record, path, relationship).await?;
        related.cycles.insert(relationship.clone(), state);
    }
    Ok(related)
}

async fn duplicates(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &StoredEntity,
    record: &Record,
    path: &[Uuid],
    key: &[String],
) -> Result<Vec<String>, RepositoryError> {
    let Some(first) = key.first() else {
        return Ok(Vec::new());
    };
    let Some(value) = record.values.get(first) else {
        return Ok(Vec::new());
    };
    // Typed equality on the first key attribute keeps the candidate scan
    // indexed and bounded; the remaining attributes are compared in Rust.
    let (column, cast) = match subject.value_type(first) {
        Some("string") => ("value_text", "text"),
        Some("integer") => ("value_integer", "bigint"),
        Some("number") => ("value_number", "numeric"),
        Some("boolean") => ("value_boolean", "boolean"),
        Some("date") => ("value_date", "date"),
        Some("datetime") => ("value_datetime", "timestamptz"),
        _ => return Ok(Vec::new()),
    };
    let literal = match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let query = format!(
        r#"SELECT DISTINCT av.entity_id
           FROM attribute_values av
           JOIN entities e ON e.id = av.entity_id AND e.workspace_id = $1 AND e.deleted_at IS NULL
            AND e.blueprint_id = $2 AND e.id <> $3
           JOIN attributes a ON a.id = av.attribute_id AND a.code = $4 AND a.deleted_at IS NULL
            AND a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version
           WHERE av.relationship_target_entity_id IS NULL AND av.{column} = $5::text::{cast}
           LIMIT $6"#
    );
    let mut ids: Vec<Uuid> = sqlx::query_scalar(&query)
        .bind(scope.workspace_id)
        .bind(subject.blueprint_id)
        .bind(subject.id)
        .bind(first)
        .bind(literal)
        .bind(MAX_UNIQUE_CANDIDATES)
        .fetch_all(&mut *conn)
        .await?;
    ids.sort();
    let loaded = load_entities(conn, scope.workspace_id, &ids).await?;
    Ok(ids
        .iter()
        .filter_map(|id| loaded.get(id))
        .filter(|other| {
            let other = other.resolve(path);
            key.iter()
                .all(|code| other.values.get(code) == record.values.get(code))
        })
        .map(|other| other.id.to_string())
        .collect())
}

async fn cycle(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &StoredEntity,
    record: &Record,
    path: &[Uuid],
    relationship: &str,
) -> Result<CycleState, RepositoryError> {
    let mut parents: HashMap<Uuid, Uuid> = HashMap::new();
    let mut frontier: VecDeque<Uuid> = VecDeque::new();
    for target in uuids(record.values.get(relationship)) {
        if target == subject.id {
            return Ok(CycleState::Cycle(vec![
                subject.id.to_string(),
                subject.id.to_string(),
            ]));
        }
        if parents.insert(target, subject.id).is_none() {
            frontier.push_back(target);
        }
    }
    while !frontier.is_empty() {
        if parents.len() > MAX_CYCLE_VISITS {
            return Ok(CycleState::LimitReached);
        }
        let batch: Vec<Uuid> = frontier.drain(..).collect();
        let loaded = load_entities(conn, scope.workspace_id, &batch).await?;
        for id in batch {
            let Some(entity) = loaded.get(&id) else {
                continue;
            };
            for target in uuids(entity.resolve(path).values.get(relationship)) {
                if target == subject.id {
                    let mut chain = vec![subject.id.to_string(), id.to_string()];
                    let mut current = id;
                    while let Some(parent) = parents.get(&current).copied() {
                        chain.push(parent.to_string());
                        if parent == subject.id {
                            break;
                        }
                        current = parent;
                    }
                    chain.reverse();
                    return Ok(CycleState::Cycle(chain));
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = parents.entry(target) {
                    entry.insert(id);
                    frontier.push_back(target);
                }
            }
        }
    }
    Ok(CycleState::Clear)
}

/// Evaluates predicates for one subject in one context, overriding values
/// of the resolved record first (used to preview a status destination).
pub(crate) async fn evaluate_in_context(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &StoredEntity,
    context_id: Uuid,
    overrides: &[(String, Value)],
    predicates: &[&Predicate],
) -> Result<Vec<Result<(), Failure>>, RepositoryError> {
    let path = scope.path(context_id)?;
    let mut record = subject.resolve(&path);
    for (code, value) in overrides {
        if value.is_null() {
            record.values.remove(code);
        } else {
            record.values.insert(code.clone(), value.clone());
        }
    }
    let requirements = Requirements::for_predicates(predicates.iter().copied());
    let related = load_related(conn, scope, subject, &record, &path, &requirements).await?;
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
pub(crate) struct EnabledRule {
    pub code: String,
    pub context_id: Option<Uuid>,
    pub compiled: catalog_rules::CompiledRule,
}

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

/// One predicate to evaluate in one context, with its reporting identity.
struct Job<'a> {
    source: CheckSource,
    code: &'a str,
    message: Option<&'a str>,
    predicate: &'a Predicate,
    severity: Option<String>,
    transition: Option<CheckTransition>,
}

fn severity(rule: &catalog_rules::CompiledRule) -> String {
    serde_json::to_value(&rule.severity)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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

async fn run_jobs(
    conn: &mut PgConnection,
    scope: &CheckScope,
    subject: &StoredEntity,
    jobs: &BTreeMap<Uuid, Vec<Job<'_>>>,
) -> Result<Vec<CheckViolation>, RepositoryError> {
    let mut violations = Vec::new();
    for (context_id, jobs) in jobs {
        let predicates: Vec<&Predicate> = jobs.iter().map(|job| job.predicate).collect();
        let outcomes =
            evaluate_in_context(conn, scope, subject, *context_id, &[], &predicates).await?;
        for (job, outcome) in jobs.iter().zip(outcomes) {
            if let Err(failure) = outcome {
                record_violation(&mut violations, job, scope.code(*context_id), failure);
            }
        }
    }
    Ok(violations)
}

impl CatalogRepository {
    /// Enforces entity-schema checks, status transition conditions and
    /// enforcing rules on the transaction's final state. Called by every
    /// write path through `validate_entity_schema`.
    pub(super) async fn enforce_declarative_checks(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        entity_schema: Option<&Value>,
    ) -> Result<(), RepositoryError> {
        let checks = entity_schema
            .map(predicate::entity_checks)
            .transpose()
            .map_err(RepositoryError::InvalidBlueprintDefinition)?
            .unwrap_or_default();
        let changes = self.effective_status_changes(transaction, entity).await?;
        let rules = enabled_rules(
            transaction,
            self.workspace_id.0,
            entity.blueprint_id,
            entity.blueprint_version,
        )
        .await?;
        let enforcing: Vec<_> = rules
            .iter()
            .filter(|rule| rule.compiled.enforcement.is_some())
            .collect();
        let conditions: Vec<(&StatusChange, Vec<Check>)> = changes
            .iter()
            .map(|change| {
                (
                    change,
                    catalog_validation::status::transition_conditions(
                        &change.schema,
                        &change.before,
                        &change.after,
                    ),
                )
            })
            .filter(|(_, conditions)| !conditions.is_empty())
            .collect();
        if checks.is_empty() && conditions.is_empty() && enforcing.is_empty() {
            return Ok(());
        }
        let scope = CheckScope::load(transaction, self.workspace_id.0).await?;
        let all_contexts = scope.context_ids();

        let mut check_jobs: BTreeMap<Uuid, Vec<Job>> = BTreeMap::new();
        for context_id in &all_contexts {
            for check in &checks {
                check_jobs.entry(*context_id).or_default().push(Job {
                    source: CheckSource::EntityCheck,
                    code: &check.code,
                    message: check.message.as_deref(),
                    predicate: &check.predicate,
                    severity: None,
                    transition: None,
                });
            }
        }
        let mut condition_jobs: BTreeMap<Uuid, Vec<Job>> = BTreeMap::new();
        for (change, conditions) in &conditions {
            for condition in conditions {
                condition_jobs
                    .entry(change.context_id)
                    .or_default()
                    .push(Job {
                        source: CheckSource::TransitionCondition,
                        code: &condition.code,
                        message: condition.message.as_deref(),
                        predicate: &condition.predicate,
                        severity: None,
                        transition: Some(change.transition()),
                    });
            }
        }
        let mut rule_jobs: BTreeMap<Uuid, Vec<Job>> = BTreeMap::new();
        for rule in &enforcing {
            let Some(enforcement) = &rule.compiled.enforcement else {
                continue;
            };
            let contexts = rule
                .context_id
                .map(|context| vec![context])
                .unwrap_or_else(|| all_contexts.clone());
            if enforcement.on_save {
                for context_id in &contexts {
                    rule_jobs.entry(*context_id).or_default().push(Job {
                        source: CheckSource::Rule,
                        code: &rule.code,
                        message: None,
                        predicate: &rule.compiled.predicate,
                        severity: Some(severity(&rule.compiled)),
                        transition: None,
                    });
                }
            }
            for change in &changes {
                if contexts.contains(&change.context_id)
                    && enforcement.guards_transition(
                        &change.attribute_code,
                        change.before.as_str(),
                        change.after.as_str(),
                    )
                {
                    rule_jobs.entry(change.context_id).or_default().push(Job {
                        source: CheckSource::Rule,
                        code: &rule.code,
                        message: None,
                        predicate: &rule.compiled.predicate,
                        severity: Some(severity(&rule.compiled)),
                        transition: Some(change.transition()),
                    });
                }
            }
        }

        let subject = load_entities(transaction, self.workspace_id.0, &[entity.id])
            .await?
            .remove(&entity.id)
            .unwrap_or_else(|| StoredEntity {
                id: entity.id,
                blueprint_id: entity.blueprint_id,
                ..StoredEntity::default()
            });
        let violations = run_jobs(transaction, &scope, &subject, &check_jobs).await?;
        if !violations.is_empty() {
            return Err(RepositoryError::EntityCheckFailed(violations));
        }
        let violations = run_jobs(transaction, &scope, &subject, &condition_jobs).await?;
        if !violations.is_empty() {
            return Err(RepositoryError::TransitionConditionsUnmet(violations));
        }
        let violations = run_jobs(transaction, &scope, &subject, &rule_jobs).await?;
        if !violations.is_empty() {
            return Err(RepositoryError::RuleViolation(violations));
        }
        Ok(())
    }

    /// Transition conditions and enforcing rules that the saved state plus the
    /// change's destination would not satisfy, for the status control.
    pub(super) async fn transition_unmet(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
    ) -> Result<Vec<CheckViolation>, RepositoryError> {
        let conditions = catalog_validation::status::transition_conditions(
            &change.schema,
            &change.before,
            &change.after,
        );
        let rules = enabled_rules(
            transaction,
            self.workspace_id.0,
            entity.blueprint_id,
            entity.blueprint_version,
        )
        .await?;
        let transition = change.transition();
        let mut jobs: Vec<Job> = conditions
            .iter()
            .map(|condition| Job {
                source: CheckSource::TransitionCondition,
                code: &condition.code,
                message: condition.message.as_deref(),
                predicate: &condition.predicate,
                severity: None,
                transition: Some(transition.clone()),
            })
            .collect();
        for rule in &rules {
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
            if guards
                && rule
                    .context_id
                    .is_none_or(|context| context == change.context_id)
            {
                jobs.push(Job {
                    source: CheckSource::Rule,
                    code: &rule.code,
                    message: None,
                    predicate: &rule.compiled.predicate,
                    severity: Some(severity(&rule.compiled)),
                    transition: Some(transition.clone()),
                });
            }
        }
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let scope = CheckScope::load(transaction, self.workspace_id.0).await?;
        let subject = load_entities(transaction, self.workspace_id.0, &[entity.id])
            .await?
            .remove(&entity.id)
            .ok_or(RepositoryError::NotFound("entity"))?;
        let predicates: Vec<&Predicate> = jobs.iter().map(|job| job.predicate).collect();
        let outcomes = evaluate_in_context(
            transaction,
            &scope,
            &subject,
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
}
