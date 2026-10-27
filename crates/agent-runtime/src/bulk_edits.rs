//! `bulk_set_record_values`: the same kind of scalar value edit across many
//! records, approved once.
//!
//! When the agent proposes the tool, [`resolve`] turns its selection (an
//! explicit item list or a search filter) into an explicit list of records,
//! each pinned to its current `updated_at` and carrying its current values.
//! That list is what is stored, summarized and approved. After approval,
//! [`execute`] applies it one record at a time through the same path as
//! `set_record_values`, authorizing and auditing every record, and stops at
//! the first failure, which the workspace's system user notes as a comment on
//! the failed record. Unlike `apply_record_batch`, the edit is not atomic.

use attricat_domain::constants::DEFAULT_CONTEXT_CODE;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    agent_tools::{self, ToolError, agent_search_scope, tool_actor},
    model::{Attribute, RelationshipFilter, SearchBlueprint, SearchFilter},
    repository::{AgentRun, AgentToolCall, AttricatRepository, RepositoryError},
};
use attricat_domain::permissions::Permission;
use attricat_domain::value_type::ValueType;

pub const TOOL: &str = "bulk_set_record_values";

/// Values one bulk edit sets on each record; matches the bound on value
/// removals so a proposal stays reviewable.
pub(crate) const MAX_VALUES_PER_RECORD: usize = agent_tools::MAX_REMOVE_VALUES;
/// Characters of a failure message quoted in the system comment.
const MAX_COMMENT_MESSAGE_CHARS: usize = 2_000;

/// The proposal as the agent supplies it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    #[serde(default)]
    items: Option<Vec<ProposedItem>>,
    #[serde(default)]
    filter: Option<ProposedFilter>,
    #[serde(default)]
    values: Option<Vec<ProposedValue>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedItem {
    record_id: Uuid,
    values: Vec<ProposedValue>,
    /// The context of this item's values that do not name one.
    #[serde(default)]
    context_id: Option<Uuid>,
    #[serde(default)]
    expected_updated_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedFilter {
    blueprint: SearchBlueprint,
    #[serde(default)]
    attribute_filters: Vec<SearchFilter>,
    #[serde(default)]
    relationship_filters: Vec<RelationshipFilter>,
    #[serde(default)]
    context_code: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedValue {
    attribute_code: String,
    #[serde(default)]
    context_id: Option<Uuid>,
    value: Value,
}

/// The resolved proposal: what the approver sees and what is applied.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedBulkEdit {
    /// The agent's original selection: `{"filter": …}` or `{"items": n}`.
    pub selection: Value,
    /// The records' blueprint when they share one.
    pub blueprint: Option<NamedCode>,
    /// The attributes set, in first-use order.
    pub attributes: Vec<NamedCode>,
    pub records: Vec<ResolvedRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedCode {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRecord {
    pub record_id: Uuid,
    pub blueprint_code: String,
    /// The record's default-context display label when proposed.
    pub label: Option<String>,
    pub expected_updated_at: DateTime<Utc>,
    pub values: Vec<ResolvedValue>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedValue {
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub value: Value,
    /// The record's own value when proposed; `null` when it had none.
    pub previous: Value,
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(message.into())
}

/// Resolves a proposed bulk edit into the explicit, version-pinned record
/// list that is stored for approval. Errors go back to the agent as the
/// tool's result, so it can narrow or correct the selection.
pub async fn resolve(
    repository: &AttricatRepository,
    actor: Uuid,
    workspace: Uuid,
    arguments: Value,
    max_records: u32,
) -> Result<Value, ToolError> {
    let proposal: Proposal =
        serde_json::from_value(arguments).map_err(|error| invalid(error.to_string()))?;
    let (selection, targets) = match (proposal.items, proposal.filter) {
        (Some(items), None) => {
            if proposal.values.is_some() {
                return Err(invalid(
                    "with items, give each item its own values; top-level values apply to a filter",
                ));
            }
            (json!({"items": items.len()}), select_items(items)?)
        }
        (None, Some(filter)) => {
            let values = proposal
                .values
                .ok_or_else(|| invalid("a filter needs values to set on every match"))?;
            let selection = json!({"filter": {
                "blueprint": {"code": filter.blueprint.code, "version": filter.blueprint.version},
                "attribute_filters": filter.attribute_filters.iter().map(|filter| json!({"field": filter.field, "operator": filter.operator, "value": filter.value})).collect::<Vec<_>>(),
                "relationship_filters": filter.relationship_filters.iter().map(|filter| json!({"field": filter.field, "selected_target_ids": filter.selected_target_ids})).collect::<Vec<_>>(),
                "context_code": filter.context_code,
            }});
            let ids = select_filter(repository, actor, workspace, &filter).await?;
            if ids.len() > max_records as usize {
                return Err(too_large(ids.len(), max_records));
            }
            let targets = ids
                .into_iter()
                .map(|record_id| Target {
                    record_id,
                    values: values.clone(),
                    expected_updated_at: None,
                })
                .collect();
            (selection, targets)
        }
        _ => return Err(invalid("give either items or a filter, not both")),
    };
    if targets.is_empty() {
        return Err(ToolError::Coded {
            code: "bulk_edit_empty",
            message: "The selection matches no records; nothing would change.".to_owned(),
        });
    }
    if targets.len() > max_records as usize {
        return Err(too_large(targets.len(), max_records));
    }
    let ids: Vec<Uuid> = targets.iter().map(|target| target.record_id).collect();
    let readable = if repository
        .principal_token_permits(
            tool_actor(repository, actor),
            workspace,
            Permission::RecordsRead,
        )
        .await?
    {
        repository
            .authorized_record_ids(actor, workspace, Permission::RecordsRead, &ids)
            .await?
    } else {
        HashSet::new()
    };
    let unreadable: Vec<String> = ids
        .iter()
        .filter(|id| !readable.contains(id))
        .map(Uuid::to_string)
        .collect();
    if !unreadable.is_empty() {
        return Err(invalid(format!(
            "these records do not exist or cannot be read: {}",
            unreadable.join(", ")
        )));
    }
    let labels: HashMap<Uuid, String> = repository
        .record_labels(&ids)
        .await?
        .into_iter()
        .filter_map(|label| {
            let display = label
                .display
                .get(DEFAULT_CONTEXT_CODE)?
                .as_str()?
                .trim()
                .to_owned();
            (!display.is_empty()).then_some((label.id, display))
        })
        .collect();
    let blueprint_names: HashMap<String, String> = repository
        .list_blueprints()
        .await?
        .into_iter()
        .map(|blueprint| (blueprint.code, blueprint.name))
        .collect();
    // Values without a context are in the default context, which current
    // values name by its ID.
    let default_context = repository
        .search_context(DEFAULT_CONTEXT_CODE)
        .await?
        .map(|context| context.requested_id());
    let in_context = |context: Option<Uuid>| context.or(default_context);
    let mut revisions: HashMap<(Uuid, i64), (String, Vec<Attribute>)> = HashMap::new();
    let mut attributes: Vec<NamedCode> = Vec::new();
    let mut records = Vec::with_capacity(targets.len());
    for target in targets {
        let record = repository
            .get_record(target.record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        let key = (record.blueprint_id, record.blueprint_version);
        if let std::collections::hash_map::Entry::Vacant(entry) = revisions.entry(key) {
            let revision = repository
                .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint"))?;
            entry.insert((revision.blueprint.code, revision.attributes));
        }
        let (blueprint_code, blueprint_attributes) = &revisions[&key];
        let reusable = if target
            .values
            .iter()
            .any(|value| value.attribute_code.contains(':'))
        {
            repository.record_reusable_attributes(record.id).await?
        } else {
            Vec::new()
        };
        let current = repository.form_values(record.id).await?;
        let mut values = Vec::with_capacity(target.values.len());
        let mut seen = HashSet::new();
        for value in target.values {
            let (name, value_type, schema) = match blueprint_attributes
                .iter()
                .find(|attribute| attribute.code == value.attribute_code)
            {
                Some(attribute) => (
                    attribute.name.clone(),
                    attribute.value_type,
                    attribute.value_schema.clone(),
                ),
                None => match reusable
                    .iter()
                    .find(|attribute| attribute.code == value.attribute_code)
                {
                    Some(attribute) => (
                        Some(attribute.name.clone()),
                        attribute.value_type,
                        attribute.value_schema.clone(),
                    ),
                    None => {
                        return Err(invalid(format!(
                            "record {} ({blueprint_code}) has no attribute '{}'",
                            record.id, value.attribute_code
                        )));
                    }
                },
            };
            if schema.as_ref().is_some_and(|schema| {
                schema
                    .get(attricat_validation::status::STATUS_KEY)
                    .is_some()
            }) {
                return Err(ToolError::Coded {
                    code: "bulk_edit_status_attribute",
                    message: format!(
                        "'{}' is a status attribute. Bulk edits never change statuses; propose each status change with set_record_values, or apply_record_batch when several must change together.",
                        value.attribute_code
                    ),
                });
            }
            if matches!(value_type, ValueType::Relationship | ValueType::File) {
                return Err(invalid(format!(
                    "'{}' is a {value_type} attribute; bulk_set_record_values sets scalar values only",
                    value.attribute_code
                )));
            }
            if !seen.insert((value.attribute_code.clone(), in_context(value.context_id))) {
                return Err(invalid(format!(
                    "record {} sets '{}' twice in the same context",
                    record.id, value.attribute_code
                )));
            }
            if !attributes
                .iter()
                .any(|attribute| attribute.code == value.attribute_code)
            {
                attributes.push(NamedCode {
                    name: name
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| value.attribute_code.clone()),
                    code: value.attribute_code.clone(),
                });
            }
            let previous = current
                .iter()
                .find_map(|current| match current {
                    crate::model::FormAttributeValue::Scalar {
                        attribute_code,
                        context_id,
                        value: current,
                    } if *attribute_code == value.attribute_code
                        && in_context(*context_id) == in_context(value.context_id) =>
                    {
                        Some(current.clone())
                    }
                    _ => None,
                })
                .unwrap_or(Value::Null);
            values.push(ResolvedValue {
                attribute_code: value.attribute_code,
                context_id: value.context_id,
                value: value.value,
                previous,
            });
        }
        records.push(ResolvedRecord {
            record_id: record.id,
            blueprint_code: blueprint_code.clone(),
            label: labels.get(&record.id).cloned(),
            expected_updated_at: target.expected_updated_at.unwrap_or(record.updated_at),
            values,
        });
    }
    let codes: HashSet<&str> = records
        .iter()
        .map(|record| record.blueprint_code.as_str())
        .collect();
    let blueprint = match codes.into_iter().collect::<Vec<_>>().as_slice() {
        [code] => Some(NamedCode {
            code: (*code).to_owned(),
            name: blueprint_names
                .get(*code)
                .cloned()
                .unwrap_or_else(|| (*code).to_owned()),
        }),
        _ => None,
    };
    Ok(serde_json::to_value(ResolvedBulkEdit {
        selection,
        blueprint,
        attributes,
        records,
    })
    .expect("bulk edits serialize"))
}

struct Target {
    record_id: Uuid,
    values: Vec<ProposedValue>,
    expected_updated_at: Option<DateTime<Utc>>,
}

fn too_large(count: usize, max_records: u32) -> ToolError {
    ToolError::Coded {
        code: "bulk_edit_too_large",
        message: format!(
            "The selection has {count} records, more than this workspace's limit of {max_records} per bulk edit. Narrow the selection with more filters, or split it into smaller edits, and propose again."
        ),
    }
}

fn select_items(items: Vec<ProposedItem>) -> Result<Vec<Target>, ToolError> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .map(|item| {
            if !seen.insert(item.record_id) {
                return Err(invalid(format!(
                    "record {} appears more than once; give each record one item",
                    item.record_id
                )));
            }
            Ok(Target {
                record_id: item.record_id,
                values: checked_values(
                    item.values
                        .into_iter()
                        .map(|value| ProposedValue {
                            context_id: value.context_id.or(item.context_id),
                            ..value
                        })
                        .collect(),
                )?,
                expected_updated_at: item.expected_updated_at,
            })
        })
        .collect()
}

fn checked_values(values: Vec<ProposedValue>) -> Result<Vec<ProposedValue>, ToolError> {
    if values.is_empty() || values.len() > MAX_VALUES_PER_RECORD {
        return Err(invalid(format!(
            "each record needs 1 to {MAX_VALUES_PER_RECORD} values"
        )));
    }
    Ok(values)
}

/// The records a filter matches, like `search_records` with the same
/// filters. Collection searches need workspace-wide read access.
async fn select_filter(
    repository: &AttricatRepository,
    actor: Uuid,
    workspace: Uuid,
    filter: &ProposedFilter,
) -> Result<Vec<Uuid>, ToolError> {
    if !repository
        .principal_may(
            tool_actor(repository, actor),
            workspace,
            Permission::RecordsRead,
            None,
            None,
        )
        .await?
    {
        return Err(ToolError::Forbidden);
    }
    let crate::repository::SearchScope {
        selected,
        search_blueprint,
        matching,
        ..
    } = agent_search_scope(
        repository,
        actor,
        &filter.blueprint,
        None,
        &filter.attribute_filters,
        &filter.relationship_filters,
        filter.context_code.as_deref(),
    )
    .await?;
    let mut ids: Vec<Uuid> = match matching {
        Some(ids) => ids,
        None => repository
            .resolve_search(&search_blueprint, selected, None)
            .await?
            .ids
            .into_iter()
            .collect(),
    };
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// "Set Brand, Season on 412 Product records."
pub fn change_summary(arguments: &Value) -> Result<String, ToolError> {
    let edit: ResolvedBulkEdit = serde::Deserialize::deserialize(arguments)
        .map_err(|error| invalid(format!("unresolved bulk edit: {error}")))?;
    let names = edit
        .attributes
        .iter()
        .map(|attribute| attribute.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let count = edit.records.len();
    let noun = if count == 1 { "record" } else { "records" };
    Ok(match edit.blueprint {
        Some(blueprint) => format!("Set {names} on {count} {} {noun}.", blueprint.name),
        None => format!("Set {names} on {count} {noun}."),
    })
}

/// The outcome of an approved bulk edit, returned to the agent.
#[derive(Debug, Default, Serialize)]
pub struct BulkEditOutcome {
    pub applied: Vec<Uuid>,
    pub failed: Vec<BulkEditFailure>,
    pub not_run: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BulkEditFailure {
    pub record_id: Uuid,
    pub code: String,
    pub message: String,
    /// The error's structured details, which clients translate the code with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl BulkEditFailure {
    /// The failure a tool error payload (`code`, `message`, `details`)
    /// describes for `record_id`.
    fn from_payload(record_id: Uuid, mut payload: Value) -> Self {
        Self {
            record_id,
            code: payload["code"].as_str().unwrap_or("tool_error").to_owned(),
            message: payload["message"].as_str().unwrap_or_default().to_owned(),
            details: payload
                .as_object_mut()
                .and_then(|payload| payload.remove("details")),
        }
    }
}

/// Applies an approved bulk edit record by record and stops at the first
/// failure. `repository` carries the run's authorization actor; `audit`
/// builds each record's audit context.
pub async fn execute(
    repository: &AttricatRepository,
    actor: Uuid,
    workspace: Uuid,
    run: &AgentRun,
    call: &AgentToolCall,
    audit: impl Fn(Uuid) -> crate::repository::AuditContext,
) -> Result<Value, Value> {
    let edit: ResolvedBulkEdit = serde::Deserialize::deserialize(&call.arguments).map_err(
        |_| json!({"code":"tool_error","message":"The approved bulk edit could not be read."}),
    )?;
    let total = edit.records.len();
    let mut outcome = BulkEditOutcome::default();
    let mut records = edit.records.into_iter();
    for record in records.by_ref() {
        let failure = apply_record(repository, actor, workspace, &audit, &record)
            .await
            .err();
        let state = if failure.is_some() {
            "failed"
        } else {
            "applied"
        };
        if failure.is_none() {
            outcome.applied.push(record.record_id);
        }
        // Progress is informational; the tool result is what counts.
        if let Err(error) = repository
            .append_run_event(
                run.id,
                "bulk_progress",
                json!({
                    "tool_call_id": call.id,
                    "record_id": record.record_id,
                    "state": state,
                    "applied": outcome.applied.len(),
                    "total": total,
                }),
            )
            .await
        {
            tracing::warn!(run_id = %run.id, %error, "could not record bulk edit progress");
        }
        if let Some(payload) = failure {
            let failure = BulkEditFailure::from_payload(record.record_id, payload);
            note_failure(
                repository,
                workspace,
                run,
                record.record_id,
                &failure.code,
                &failure.message,
            )
            .await;
            outcome.failed.push(failure);
            break;
        }
    }
    outcome.not_run = records.map(|record| record.record_id).collect();
    Ok(serde_json::to_value(outcome).expect("bulk outcomes serialize"))
}

async fn apply_record(
    repository: &AttricatRepository,
    actor: Uuid,
    workspace: Uuid,
    audit: &impl Fn(Uuid) -> crate::repository::AuditContext,
    record: &ResolvedRecord,
) -> Result<(), Value> {
    let error = |error: ToolError| agent_tools::tool_error_payload(&error);
    if !repository
        .principal_may(
            tool_actor(repository, actor),
            workspace,
            Permission::RecordsWrite,
            Some(record.record_id),
            None,
        )
        .await
        .map_err(|failure| error(failure.into()))?
    {
        return Err(
            json!({"code":"forbidden","message":"The initiating user is not authorized to change this record."}),
        );
    }
    let audited = repository
        .clone()
        .with_audit_context(audit(record.record_id));
    agent_tools::execute_mutation(
        &audited,
        actor,
        "set_record_values",
        json!({
            "record_id": record.record_id,
            "expected_updated_at": record.expected_updated_at,
            "values": record.values.iter().map(|value| json!({
                "kind": "scalar",
                "attribute_code": value.attribute_code,
                "context_id": value.context_id,
                "value": value.value,
            })).collect::<Vec<_>>(),
        }),
    )
    .await
    .map(|_| ())
    .map_err(error)
}

/// The system comment left on the record a bulk edit stopped at.
pub fn failure_comment(conversation_id: Uuid, code: &str, message: &str) -> String {
    let mut message: String = message.chars().take(MAX_COMMENT_MESSAGE_CHARS).collect();
    if message.is_empty() {
        message = "No further detail was reported.".to_owned();
    }
    format!(
        "An agent bulk edit stopped at this record without changing it. Records after it in the edit were not changed either.\n\n**Error:** `{code}`: {message}\n\n[Open the conversation](/agents/{conversation_id})"
    )
}

async fn note_failure(
    repository: &AttricatRepository,
    workspace: Uuid,
    run: &AgentRun,
    record_id: Uuid,
    code: &str,
    message: &str,
) {
    let body = failure_comment(run.conversation_id, code, message);
    let result = async {
        let author = repository.ensure_workspace_system_user(workspace).await?;
        repository
            .create_record_comment(record_id, author, &body)
            .await
    }
    .await;
    if let Err(error) = result {
        // A deleted record cannot take a comment; the tool result still
        // reports the failure.
        tracing::warn!(%record_id, %error, "could not comment on a failed bulk edit record");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(blueprint: Option<&str>, records: usize) -> Value {
        json!({
            "selection": {"items": records},
            "blueprint": blueprint.map(|name| json!({"code": "product", "name": name})),
            "attributes": [{"code": "brand", "name": "Brand"}, {"code": "season", "name": "Season"}],
            "records": (0..records).map(|_| json!({
                "record_id": Uuid::new_v4(),
                "blueprint_code": "product",
                "label": null,
                "expected_updated_at": "2026-10-01T00:00:00Z",
                "values": [],
            })).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn summaries_name_the_attributes_count_and_blueprint() {
        assert_eq!(
            change_summary(&resolved(Some("Product"), 412)).unwrap(),
            "Set Brand, Season on 412 Product records."
        );
        assert_eq!(
            change_summary(&resolved(Some("Product"), 1)).unwrap(),
            "Set Brand, Season on 1 Product record."
        );
        assert_eq!(
            change_summary(&resolved(None, 3)).unwrap(),
            "Set Brand, Season on 3 records."
        );
        // An unresolved proposal never reaches approval.
        assert!(change_summary(&json!({"items": []})).is_err());
    }

    #[test]
    fn a_resolved_list_cannot_be_proposed_directly() {
        assert!(serde_json::from_value::<Proposal>(resolved(Some("Product"), 1)).is_err());
    }

    #[test]
    fn item_contexts_apply_to_values_without_one() {
        let context = Uuid::new_v4();
        let own = Uuid::new_v4();
        let targets = select_items(vec![ProposedItem {
            record_id: Uuid::new_v4(),
            context_id: Some(context),
            expected_updated_at: None,
            values: vec![
                ProposedValue {
                    attribute_code: "brand".into(),
                    context_id: None,
                    value: json!("Acme"),
                },
                ProposedValue {
                    attribute_code: "season".into(),
                    context_id: Some(own),
                    value: json!("Spring"),
                },
            ],
        }])
        .unwrap();
        assert_eq!(targets[0].values[0].context_id, Some(context));
        assert_eq!(targets[0].values[1].context_id, Some(own));
    }

    #[test]
    fn items_name_each_record_once_with_bounded_values() {
        let record = Uuid::new_v4();
        let item = |values: usize| ProposedItem {
            record_id: record,
            context_id: None,
            expected_updated_at: None,
            values: (0..values)
                .map(|index| ProposedValue {
                    attribute_code: format!("a{index}"),
                    context_id: None,
                    value: json!(index),
                })
                .collect(),
        };
        assert!(select_items(vec![item(1), item(1)]).is_err());
        assert!(select_items(vec![item(0)]).is_err());
        assert!(select_items(vec![item(MAX_VALUES_PER_RECORD + 1)]).is_err());
        assert!(select_items(vec![item(MAX_VALUES_PER_RECORD)]).is_ok());
    }

    #[test]
    fn failure_comments_link_the_conversation_and_bound_the_message() {
        let conversation = Uuid::new_v4();
        let body = failure_comment(conversation, "stale_record", &"x".repeat(5_000));
        assert!(body.contains("`stale_record`"));
        assert!(body.contains(&format!("(/agents/{conversation})")));
        assert!(body.chars().count() < 2_500);
    }

    #[test]
    fn failures_keep_the_error_details() {
        let record = Uuid::new_v4();
        let failure = BulkEditFailure::from_payload(
            record,
            json!({"code": "invalid_value", "message": "bad", "details": {"attribute_code": "sku"}}),
        );
        assert_eq!(
            serde_json::to_value(&failure).unwrap(),
            json!({
                "record_id": record,
                "code": "invalid_value",
                "message": "bad",
                "details": {"attribute_code": "sku"},
            })
        );
        let plain =
            BulkEditFailure::from_payload(record, json!({"code": "forbidden", "message": "no"}));
        assert!(
            serde_json::to_value(&plain)
                .unwrap()
                .get("details")
                .is_none()
        );
    }
}
