//! Typed, repository-backed catalogue tools for agent runs.
//!
//! Tools never call the Catalog HTTP API: this keeps authorization and audit
//! context in the repository boundary and avoids granting the model a network
//! capability.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    agents::MAX_TOOL_RESULT_BYTES,
    catalog_read_service::CatalogReadService,
    catalog_service::CatalogMutationService,
    constants::{
        DEFAULT_ENTITY_PAGE_SIZE, DEFAULT_PAGE_SIZE, DEFAULT_STALE_AFTER_DAYS, MAX_STALE_AFTER_DAYS,
    },
    file_access::{AllowFileAccess, FileAccessOperation, authorize_file_read},
    repository::{CatalogRepository, EntitySearchSort, RepositoryError},
    search_filters::{intersect_ids, resolve_agent_filter, resolve_agent_relationship_filter},
};
const BLUEPRINT_AUTHORING_GUIDE: &str = include_str!("../../../docs/blueprints.md");
const VIEW_CONFIGURATION_GUIDE: &str = include_str!("../../../docs/views.md");
const JSON_SCHEMA_GUIDE: &str = include_str!("../../../docs/json-schema-validation.md");
const MAX_SEARCH_FILTERS: usize = 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSearchRelationshipFacet {
    field: String,
    #[serde(rename = "selectedIds")]
    selected_ids: Vec<Uuid>,
}

fn valid_saved_filter(filter: &crate::model::SearchFilter) -> bool {
    !filter.field.is_empty()
        && matches!(
            filter.operator.as_str(),
            "eq" | "contains" | "starts_with" | "gt" | "gte" | "lt" | "lte"
        )
        && (filter.value.is_string() || filter.value.is_number() || filter.value.is_boolean())
}

fn attribute_filter_parameters() -> Value {
    json!({"type":"array","maxItems":20,"items":{"type":"object","required":["field","operator","value"],"properties":{"field":{"type":"string"},"operator":{"type":"string","enum":["eq","contains","starts_with","gt","gte","lt","lte"]},"value":{"type":["string","number","boolean"]}},"additionalProperties":false}})
}

fn relationship_filter_parameters() -> Value {
    json!({"type":"array","maxItems":20,"items":{"type":"object","required":["field","selected_target_ids"],"properties":{"field":{"type":"string"},"selected_target_ids":{"type":"array","minItems":1,"maxItems":100,"items":{"type":"string","format":"uuid"}}},"additionalProperties":false}})
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: ToolFunction,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolFunction {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Read,
    Mutation,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unknown tool '{0}'")]
    UnknownTool(String),
    #[error("tool arguments are invalid: {0}")]
    InvalidArguments(String),
    #[error("tool results exceed the configured bound")]
    ResultTooLarge,
    #[error("the initiating user is not authorized to read this catalog data")]
    Forbidden,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// The provider-facing payload of a failed tool call. Declarative check
/// failures add the API error code as `error_code` and the API's
/// `error.details` as `details`, so the agent can name the failed checks and
/// the attributes or linked records to fix.
pub fn tool_error_payload(error: &ToolError) -> Value {
    let mut payload = json!({"code":"tool_error","message":error.to_string()});
    let ToolError::Repository(error) = error else {
        return payload;
    };
    let violations =
        |violations: &[crate::repository::CheckViolation]| json!({ "violations": violations });
    let (code, details) = match error {
        RepositoryError::EntityCheckFailed(items) => {
            ("entity_check_failed", Some(violations(items)))
        }
        RepositoryError::TransitionConditionsUnmet(items) => {
            ("transition_conditions_unmet", Some(violations(items)))
        }
        RepositoryError::RuleViolation(items) => ("rule_violation", Some(violations(items))),
        RepositoryError::PublicationChecksFailed {
            context,
            violations: items,
        } => (
            "publication_checks_failed",
            Some(json!({ "violations": items, "context": context })),
        ),
        RepositoryError::RuleDryRunRequired => ("rule_dry_run_required", None),
        RepositoryError::RuleHasExistingViolations(count) => (
            "rule_has_existing_violations",
            Some(json!({ "existing_violations": count })),
        ),
        _ => return payload,
    };
    payload["error_code"] = json!(code);
    if let Some(mut details) = details {
        // Evidence can list many related entity IDs; keep the payload within
        // the tool result bound by dropping it before anything else.
        if details.to_string().len() > MAX_TOOL_RESULT_BYTES / 2
            && let Some(items) = details["violations"].as_array_mut()
        {
            for item in items {
                if let Some(item) = item.as_object_mut() {
                    item.remove("evidence");
                }
            }
        }
        payload["details"] = details;
    }
    payload
}

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        definition(
            "blueprint_authoring_guide",
            "Get the complete TOML blueprint, view, file-attribute, and JSON Schema authoring syntax. Call this before drafting a blueprint.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "list_blueprints",
            "List the catalogue's current blueprints, including their persisted TOML definitions and compiled attributes.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "get_blueprint_revision",
            "Get one exact blueprint revision by ID and version, including its TOML definition and attributes. Use this instead of listing every blueprint when inspecting a known revision.",
            json!({"type":"object","required":["blueprint_id","version"],"properties":{"blueprint_id":{"type":"string","format":"uuid"},"version":{"type":"integer","minimum":1}},"additionalProperties":false}),
        ),
        definition(
            "list_contexts",
            "List attribute contexts.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "get_context",
            "Read one attribute context by ID, including its parent and data; use before changing a context.",
            json!({"type":"object","required":["context_id"],"properties":{"context_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity",
            "Get one entity by UUID, including its current scalar values, relationship targets, and file metadata in `values`.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_context_preview",
            "Get the resolved, inherited preview values for one entity in a selected attribute context. Use this to answer questions about what the entity preview displays.",
            json!({"type":"object","required":["entity_id","context_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"context_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_changes",
            "Read a bounded page of an entity's audited changes, newest first. Use next_offset to continue.",
            history_page_parameters(),
        ),
        definition(
            "get_value_history",
            "Read a bounded page of an entity's retained prior attribute values. Inspect current values before restoring a history ID; use next_offset to continue.",
            history_page_parameters(),
        ),
        definition(
            "data_health_summary",
            "Read workspace-level entity, blueprint, freshness and relationship health counts. Requires data_health.read.",
            json!({"type":"object","properties":{"stale_after_days":{"type":"integer","minimum":1,"maximum":MAX_STALE_AFTER_DAYS}},"additionalProperties":false}),
        ),
        definition(
            "list_rule_findings",
            "Read a bounded page of rule findings without raw evidence; optionally filter by entity ID. Requires rules.read.",
            diagnostic_page_parameters(Some("entity_id")),
        ),
        definition(
            "get_rule_definition",
            "Inspect a rule's latest or exact revision and its authored definition; requires rules.read.",
            json!({"type":"object","required":["rule_id"],"properties":{"rule_id":{"type":"string","format":"uuid"},"version":{"type":"integer","minimum":1}},"additionalProperties":false}),
        ),
        definition(
            "get_workflow_definition",
            "Inspect a workflow's latest or exact revision and its authored definition; requires workflows.read.",
            json!({"type":"object","required":["workflow_id"],"properties":{"workflow_id":{"type":"string","format":"uuid"},"version":{"type":"integer","minimum":1}},"additionalProperties":false}),
        ),
        definition(
            "list_rule_runs",
            "Read bounded status and finding counts for recent rule runs, optionally filtering by rule ID. No error bodies or internal cursors; requires rules.read.",
            diagnostic_page_parameters(Some("rule_id")),
        ),
        definition(
            "get_workflow_run",
            "Inspect one workflow run's safe status and attempt summary by ID, without its trigger payload or error body; requires workflows.read.",
            json!({"type":"object","required":["run_id"],"properties":{"run_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "list_workflow_runs",
            "Read a bounded page of workflow run status and attempt counts without event payloads or error bodies. Requires workflows.read.",
            diagnostic_page_parameters(None),
        ),
        definition(
            "list_extension_operation_runs",
            "Read bounded extension and connector-operation status. Filter by extension ID or connector job ID; no progress, checkpoint, or input payloads. Requires extensions.manage.",
            json!({"type":"object","properties":{"extension_id":{"type":"string"},"connector_job_id":{"type":"string","format":"uuid"},"limit":{"type":"integer","minimum":1,"maximum":25},"offset":{"type":"integer","minimum":0,"maximum":10000}},"additionalProperties":false}),
        ),
        definition(
            "get_extension_operation_run",
            "Inspect one extension-operation run's safe status by run ID, excluding inputs and checkpoints. Requires extensions.manage.",
            json!({"type":"object","required":["run_id"],"properties":{"run_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "list_blueprint_connector_jobs",
            "Read bounded connector-job declarations for one blueprint without stored inputs or file references. Requires extensions.manage.",
            json!({"type":"object","required":["blueprint_id"],"properties":{"blueprint_id":{"type":"string","format":"uuid"},"limit":{"type":"integer","minimum":1,"maximum":25},"offset":{"type":"integer","minimum":0,"maximum":10000}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_preview_link",
            "Get a navigable link to an existing entity's preview page. Use this for each entity you cite; return the link in your answer. The link is relative to the Attricat web app.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "view_image",
            "View an image file linked to an entity. Use get_entity first to find its file ID. The image is supplied to the model as a bounded display image.",
            json!({"type":"object","required":["file_id"],"properties":{"file_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "read_file",
            "Read a UTF-8 text file linked to an entity. Use get_entity first to find its file ID. The file contents are supplied to the model, up to a fixed safe limit.",
            json!({"type":"object","required":["file_id"],"properties":{"file_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "search_entities",
            "Search entities of a blueprint by scalar values, attribute filters, relationship filters, and system tags. filters use {field, operator, value} with a scalar leaf; relationship_filters use {field, selected_target_ids} with a relationship path and target entity UUIDs. All filters combine with AND. Query terms are whitespace-separated AND terms. Bare free text searches scalar values on the selected blueprint only. Use *:value for global relationship-aware discovery through up to three incoming edges. Use attribute:value for a selected-blueprint attribute; use relationship:value or an explicit path with up to three relationship hops and a scalar leaf; and use blueprint:value or blueprint.attribute:value for the selected blueprint (code or name). Use @id:uuid1,uuid2 for selected-blueprint entity IDs or relationship.@id:uuid1,uuid2 for entities linked to those IDs (at most 100 IDs per term). A trailing * means prefix matching. Results include match_explanations with deterministic match witnesses and relationship paths. Omit blueprint.version to include every published revision; set outdated to true to return only entities that are not on the latest published revision. Use sort with a configured scalar table-column field, blueprint_version, or publication_status and asc or desc direction; ascending blueprint_version puts older schemas first, ascending publication_status puts unpublished entities first and requires sort.context_code for an enabled channel. Relationship table columns use paths of up to three hops. A relationship-path sort without blueprint.version is accepted only when the complete matching result uses one source version. Without sort, results are paginated in ascending creation order.",
            json!({"type":"object","required":["blueprint"],"properties":{"blueprint":{"type":"object","required":["code"],"properties":{"code":{"type":"string"},"version":{"type":"integer","minimum":1}},"additionalProperties":false},"query":{"type":"string"},"filters":attribute_filter_parameters(),"relationship_filters":relationship_filter_parameters(),"system_tags":{"type":"array","items":{"type":"string"}},"outdated":{"type":"boolean"},"sort":{"type":"object","required":["field","direction"],"properties":{"field":{"type":"string"},"direction":{"type":"string","enum":["asc","desc"]},"context_code":{"type":"string"}},"additionalProperties":false},"page":{"type":"object","properties":{"size":{"type":"integer","minimum":1,"maximum":100},"cursor":{"type":"string"}},"additionalProperties":false}},"additionalProperties":false}),
        ),
        definition(
            "create_blueprint",
            "Create a version-1 blueprint from a complete TOML definition. Call blueprint_authoring_guide first. This change requires approval.",
            json!({"type":"object","required":["definition"],"properties":{"definition":{"type":"string","description":"Complete blueprint TOML beginning with format_version, code, name, and kind."}},"additionalProperties":false}),
        ),
        definition(
            "create_blueprint_revision",
            "Create the next draft revision of an existing blueprint from complete revised TOML. Use list_blueprints for the id and blueprint_authoring_guide before drafting. This change requires approval.",
            json!({"type":"object","required":["blueprint_id","definition"],"properties":{"blueprint_id":{"type":"string","format":"uuid"},"definition":{"type":"string","description":"Complete revised TOML; retain the existing blueprint code."}},"additionalProperties":false}),
        ),
        definition(
            "publish_blueprint",
            "Publish an existing draft blueprint revision. Use the id and version returned by create_blueprint, create_blueprint_revision, or list_blueprints. This change requires approval.",
            json!({"type":"object","required":["blueprint_id","version"],"properties":{"blueprint_id":{"type":"string","format":"uuid"},"version":{"type":"integer","minimum":1}},"additionalProperties":false}),
        ),
        definition(
            "create_entity",
            "Create an entity from an existing blueprint. This change requires approval. blueprint must contain the existing blueprint code and optional version; never embed a blueprint definition here. Scalar values use {kind:'scalar', attribute_code:'...', context_id:null, value:<typed JSON value>}; relationships use {kind:'relationship', attribute_code:'...', context_id:null, target_entity_id:'UUID'}.",
            json!({"type":"object","required":["blueprint"],"properties":{"blueprint":{"type":"object","required":["code"],"properties":{"code":{"type":"string"},"version":{"type":"integer"}},"additionalProperties":false},"values":{"type":"array"},"system_tags":{"type":"array","items":{"type":"string"}},"system_metadata":{"type":"object"}},"additionalProperties":false}),
        ),
        definition(
            "delete_entity",
            "Delete an entity. This change requires approval.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "set_entity_values",
            "Set scalar attribute values on an existing entity, optionally in a named attribute context. Each value replaces the current value for its attribute and context. Call get_entity and list_contexts first when the entity's current values or context IDs are unknown. This change requires approval.",
            json!({"type":"object","required":["entity_id","values"],"properties":{"entity_id":{"type":"string","format":"uuid"},"values":{"type":"array","minItems":1,"items":{"type":"object","required":["kind","attribute_code","context_id","value"],"properties":{"kind":{"const":"scalar"},"attribute_code":{"type":"string"},"context_id":{"type":["string","null"],"format":"uuid"},"value":{}},"additionalProperties":false}}},"additionalProperties":false}),
        ),
        definition(
            "remove_entity_values",
            "Remove current scalar overrides for the specified attribute codes and contexts. Inspect current values first; this change requires approval.",
            json!({"type":"object","required":["entity_id","remove_values"],"properties":{"entity_id":{"type":"string","format":"uuid"},"remove_values":{"type":"array","minItems":1,"maxItems":20,"items":{"type":"object","required":["attribute_code"],"properties":{"attribute_code":{"type":"string"},"context_id":{"type":["string","null"],"format":"uuid"}},"additionalProperties":false}}},"additionalProperties":false}),
        ),
        definition(
            "restore_entity_value",
            "Restore one retained value-history entry by ID to its entity. Inspect get_value_history and get_entity first. This change requires approval.",
            json!({"type":"object","required":["entity_id","history_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"history_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "replace_entity_relationships",
            "Replace the complete target set for each specified relationship attribute and context on an entity. An empty target_entity_ids array clears that set. Inspect the entity first and review every target ID; this change requires approval.",
            relationship_mutation_parameters(),
        ),
        definition(
            "remove_entity_relationships",
            "Remove only the specified existing relationship targets on an entity, preserving other targets. Inspect the entity first; this change requires approval.",
            relationship_mutation_parameters(),
        ),
        definition(
            "migrate_entity",
            "Upgrade an entity to the latest published revision of its blueprint. Call preview_entity_migration first to assess compatibility without a write. Supply replacement scalar values, relationship target sets, or discarded attribute codes if needed. This change requires approval; with no remediation input, a ready entity is migrated immediately after approval.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"values":{"type":"array"},"relationships":{"type":"array"},"discard_attributes":{"type":"array","items":{"type":"string"}}},"additionalProperties":false}),
        ),
        definition(
            "preview_entity_migration",
            "Assess an entity's migration to the latest published blueprint without changing it. Returns compatibility status, target version, and issues; inspect before proposing an upgrade.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_publications",
            "List this entity's enabled channel publication status. Use this before proposing channel publication.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_publication_readiness",
            "Check whether an entity passes each enabled channel's required rules and entity checks, without publishing. Returns context_id, context_code, ready, and violations (source, code, message, contexts, attributes, evidence). Use before publishing or to explain publication_checks_failed.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "get_entity_status_transitions",
            "List each status attribute's destinations from the entity's saved state in one context (default context when context_id is omitted). Each destination has allowed, an optional reason (transition_not_allowed or conditions_unmet), and unmet violations from transition conditions or enforcing rules. Use before proposing a status change or to explain transition_conditions_unmet.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"context_id":{"type":["string","null"],"format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "publish_entity",
            "Approve an entity for one enabled channel context. Later entity edits withdraw this approval. This change requires approval.",
            json!({"type":"object","required":["entity_id","context_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"context_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "unpublish_entity",
            "Unpublish an entity from one channel context. This change requires approval.",
            json!({"type":"object","required":["entity_id","context_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"context_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "publish_entity_to_all_channels",
            "Publish or republish an entity in every enabled channel. This change requires approval.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "link_file",
            "Attach an existing workspace file to an entity file attribute. Conversation attachments include their file IDs. This change requires approval.",
            json!({"type":"object","required":["entity_id","attribute_code","file_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"attribute_code":{"type":"string"},"file_id":{"type":"string","format":"uuid"},"context_id":{"type":["string","null"],"format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "list_saved_searches",
            "List saved Explorer searches owned by the initiating user, newest first. Use their IDs with get_saved_search and update_saved_search rather than creating duplicates.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "get_saved_search",
            "Get an owned saved Explorer search by ID including its current state, before updating it.",
            json!({"type":"object","required":["saved_view_id"],"properties":{"saved_view_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "create_saved_search",
            "Save a named Explorer search for the initiating user. Find the blueprint code with list_blueprints first. attributeFilters use {field, operator, value}; relationshipFacets use {field, selectedIds} of target entity UUIDs. The search is private unless visibility is workspace. This change requires approval. Return the saved search link to the user.",
            json!({"type":"object","required":["name","blueprint"],"properties":{"name":{"type":"string"},"description":{"type":"string"},"visibility":{"type":"string","enum":["private","workspace"]},"blueprint":{"type":"string"},"version":{"type":"integer","minimum":1},"all_versions":{"type":"boolean"},"query":{"type":"string"},"attributeFilters":attribute_filter_parameters(),"relationshipFacets":{"type":"array","maxItems":20,"items":{"type":"object","required":["field","selectedIds"],"properties":{"field":{"type":"string"},"selectedIds":{"type":"array","minItems":1,"maxItems":100,"items":{"type":"string","format":"uuid"}}},"additionalProperties":false}}},"additionalProperties":false}),
        ),
        definition(
            "update_saved_search",
            "Update an existing saved Explorer search owned by the initiating user, preserving every omitted field. Get its ID with list_saved_searches and inspect it with get_saved_search first. Provide only changed fields; empty attributeFilters or relationshipFacets arrays clear those filters. This change requires approval. Return the existing saved search link.",
            json!({"type":"object","required":["saved_view_id"],"properties":{"saved_view_id":{"type":"string","format":"uuid"},"name":{"type":"string"},"description":{"type":"string"},"visibility":{"type":"string","enum":["private","workspace"]},"blueprint":{"type":"string"},"version":{"type":"integer","minimum":1},"all_versions":{"type":"boolean"},"query":{"type":"string"},"attributeFilters":attribute_filter_parameters(),"relationshipFacets":{"type":"array","maxItems":20,"items":{"type":"object","required":["field","selectedIds"],"properties":{"field":{"type":"string"},"selectedIds":{"type":"array","minItems":1,"maxItems":100,"items":{"type":"string","format":"uuid"}}},"additionalProperties":false}}},"additionalProperties":false}),
        ),
        definition(
            "update_entity_annotations",
            "Replace specified system_tags and/or system_metadata on an entity without changing values or relationships. Omitted fields remain unchanged; [] or {} clears a field. Inspect get_entity first. Requires approval.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"system_tags":{"type":"array","maxItems":100,"items":{"type":"string"}},"system_metadata":{"type":"object"}},"additionalProperties":false}),
        ),
        definition(
            "update_context",
            "Replace an existing context's parent and data together. Provide the complete data object and an existing parent_id; inspect get_context first. Requires approval.",
            json!({"type":"object","required":["context_id","parent_id","data"],"properties":{"context_id":{"type":"string","format":"uuid"},"parent_id":{"type":"string","format":"uuid"},"data":{"type":"object"}},"additionalProperties":false}),
        ),
        definition(
            "delete_context",
            "Delete an unused non-root attribute context. Inspect get_context first; in-use contexts cannot be deleted. Requires approval.",
            json!({"type":"object","required":["context_id"],"properties":{"context_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "create_context",
            "Create an attribute context. This change requires approval.",
            json!({"type":"object","required":["code","data"],"properties":{"code":{"type":"string"},"data":{"type":"object"},"parent_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
    ]
}

fn safe_extension_operation_run(run: crate::repository::ExtensionOperationRun) -> Value {
    json!({"id":run.id,"extension_id":run.extension_id,"operation_id":run.operation_id,
        "installed_release_id":run.installed_release_id,"status":run.status,
        "schedule_id":run.schedule_id,"connector_job_id":run.connector_job_id,
        "connector_channel_id":run.connector_channel_id,"attempts":run.attempts,
        "last_error_code":run.last_error_code,"outputs_expired":run.outputs_expired,
        "created_at":run.created_at,"completed_at":run.completed_at})
}

fn safe_workflow_run(run: crate::repository::WorkflowRun) -> Value {
    json!({"id":run.id,"workflow_id":run.workflow_id,"workflow_version":run.workflow_version,
        "source":run.source,"status":run.status,"attempts":run.attempts,
        "created_at":run.created_at,"completed_at":run.completed_at,
        "failed_at":run.failed_at,"has_error":run.last_error.is_some()})
}

fn diagnostic_page_parameters(filter: Option<&str>) -> Value {
    let mut properties = json!({
        "limit":{"type":"integer","minimum":1,"maximum":25},
        "offset":{"type":"integer","minimum":0,"maximum":10000}
    });
    if let Some(filter) = filter {
        properties[filter] = json!({"type":"string","format":"uuid"});
    }
    json!({"type":"object","properties":properties,"additionalProperties":false})
}

fn diagnostic_page_arguments(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<(i64, i64), ToolError> {
    let limit = limit.unwrap_or(10);
    let offset = offset.unwrap_or(0);
    if !(1..=25).contains(&limit) || !(0..=10000).contains(&offset) {
        return Err(ToolError::InvalidArguments(
            "limit must be 1-25 and offset must be 0-10000".into(),
        ));
    }
    Ok((limit, offset))
}

fn history_page_parameters() -> Value {
    json!({"type":"object","required":["entity_id"],"properties":{
        "entity_id":{"type":"string","format":"uuid"},
        "limit":{"type":"integer","minimum":1,"maximum":50},
        "offset":{"type":"integer","minimum":0,"maximum":10000}
    },"additionalProperties":false})
}

fn history_page_arguments(arguments: Value) -> Result<(Uuid, i64, i64), ToolError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        entity_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    }
    let input: Input = decode(arguments)?;
    let limit = input.limit.unwrap_or(20);
    let offset = input.offset.unwrap_or(0);
    if !(1..=50).contains(&limit) || !(0..=10000).contains(&offset) {
        return Err(ToolError::InvalidArguments(
            "limit must be 1-50 and offset must be 0-10000".into(),
        ));
    }
    Ok((input.entity_id, limit, offset))
}

fn next_history_offset(has_more: bool, offset: i64, limit: i64) -> Option<i64> {
    has_more
        .then_some(offset + limit)
        .filter(|next| *next <= 10000)
}

fn relationship_mutation_parameters() -> Value {
    json!({"type":"object","required":["entity_id","relationships"],"properties":{
        "entity_id":{"type":"string","format":"uuid"},
        "relationships":{"type":"array","minItems":1,"maxItems":20,"items":{
            "type":"object","required":["attribute_code","target_entity_ids"],"properties":{
                "attribute_code":{"type":"string"},"context_id":{"type":["string","null"],"format":"uuid"},
                "target_entity_ids":{"type":"array","maxItems":100,"items":{"type":"string","format":"uuid"}}
            },"additionalProperties":false
        }}},"additionalProperties":false})
}

fn definition(name: &'static str, description: &'static str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: ToolFunction {
            name,
            description,
            parameters,
        },
    }
}

pub fn kind(name: &str) -> Result<ToolKind, ToolError> {
    match name {
        "blueprint_authoring_guide"
        | "list_blueprints"
        | "get_blueprint_revision"
        | "list_contexts"
        | "get_context"
        | "get_entity"
        | "get_entity_context_preview"
        | "get_entity_changes"
        | "get_value_history"
        | "data_health_summary"
        | "list_rule_findings"
        | "get_rule_definition"
        | "get_workflow_definition"
        | "list_rule_runs"
        | "get_workflow_run"
        | "list_workflow_runs"
        | "list_extension_operation_runs"
        | "get_extension_operation_run"
        | "list_blueprint_connector_jobs"
        | "get_entity_preview_link"
        | "view_image"
        | "read_file"
        | "search_entities"
        | "list_saved_searches"
        | "get_saved_search"
        | "get_entity_publications"
        | "get_entity_publication_readiness"
        | "get_entity_status_transitions"
        | "preview_entity_migration" => Ok(ToolKind::Read),
        "create_blueprint"
        | "create_blueprint_revision"
        | "publish_blueprint"
        | "create_entity"
        | "delete_entity"
        | "set_entity_values"
        | "remove_entity_values"
        | "restore_entity_value"
        | "replace_entity_relationships"
        | "remove_entity_relationships"
        | "migrate_entity"
        | "link_file"
        | "update_entity_annotations"
        | "update_context"
        | "delete_context"
        | "create_context"
        | "create_saved_search"
        | "update_saved_search"
        | "publish_entity"
        | "unpublish_entity"
        | "publish_entity_to_all_channels" => Ok(ToolKind::Mutation),
        _ => Err(ToolError::UnknownTool(name.to_owned())),
    }
}

/// A concise, durable explanation shown to the approver before any mutation
/// reaches a repository method.
pub fn change_summary(name: &str, arguments: &Value) -> Result<String, ToolError> {
    match name {
        "create_blueprint" => {
            Ok("Create a blueprint from the supplied TOML definition.".to_owned())
        }
        "create_blueprint_revision" => Ok(format!(
            "Create a new draft revision for blueprint {}.",
            required_string(arguments, "blueprint_id")?
        )),
        "publish_blueprint" => Ok(format!(
            "Publish blueprint {} version {}.",
            required_string(arguments, "blueprint_id")?,
            arguments
                .get("version")
                .and_then(Value::as_i64)
                .ok_or_else(|| ToolError::InvalidArguments(
                    "version is required and must be an integer".to_owned()
                ))?
        )),
        "create_entity" => Ok(format!(
            "Create an entity of blueprint {}.",
            required_string(arguments, "blueprint.code")?,
        )),
        "delete_entity" => Ok(format!(
            "Delete entity {}.",
            required_string(arguments, "entity_id")?
        )),
        "set_entity_values" => Ok(format!(
            "Set attribute values on entity {}.",
            required_string(arguments, "entity_id")?
        )),
        "update_entity_annotations" => Ok(format!(
            "Update {} on entity {}.",
            match (
                arguments.get("system_tags"),
                arguments.get("system_metadata")
            ) {
                (Some(_), Some(_)) => "system tags and metadata",
                (Some(_), None) => "system tags",
                (None, Some(_)) => "system metadata",
                (None, None) =>
                    return Err(ToolError::InvalidArguments(
                        "provide tags and/or metadata".into()
                    )),
            },
            required_string(arguments, "entity_id")?
        )),
        "update_context" => Ok(format!(
            "Replace parent and data on context {}.",
            required_string(arguments, "context_id")?
        )),
        "delete_context" => Ok(format!(
            "Delete context {}.",
            required_string(arguments, "context_id")?
        )),
        "remove_entity_values" => Ok(format!(
            "Remove {} scalar overrides on entity {}.",
            arguments
                .get("remove_values")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidArguments(
                    "remove_values must be an array".into()
                ))?
                .len(),
            required_string(arguments, "entity_id")?
        )),
        "restore_entity_value" => Ok(format!(
            "Restore history entry {} on entity {}.",
            required_string(arguments, "history_id")?,
            required_string(arguments, "entity_id")?
        )),
        "replace_entity_relationships" | "remove_entity_relationships" => {
            let action = if name == "replace_entity_relationships" {
                "Replace"
            } else {
                "Remove"
            };
            let relationships = arguments
                .get("relationships")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ToolError::InvalidArguments("relationships must be an array".into())
                })?;
            let details = relationships
                .iter()
                .map(|relationship| {
                    let code = relationship
                        .get("attribute_code")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ToolError::InvalidArguments("attribute_code is required".into())
                        })?;
                    let count = relationship
                        .get("target_entity_ids")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            ToolError::InvalidArguments("target_entity_ids must be an array".into())
                        })?
                        .len();
                    Ok(format!("{code} ({count} targets)"))
                })
                .collect::<Result<Vec<_>, ToolError>>()?;
            Ok(format!(
                "{action} relationship targets on entity {}: {}.",
                required_string(arguments, "entity_id")?,
                details.join(", ")
            ))
        }
        "migrate_entity" => Ok(format!(
            "Upgrade entity {} to its latest published blueprint revision.",
            required_string(arguments, "entity_id")?
        )),
        "publish_entity" => Ok(format!(
            "Publish entity {} to channel {}.",
            required_string(arguments, "entity_id")?,
            required_string(arguments, "context_id")?,
        )),
        "unpublish_entity" => Ok(format!(
            "Unpublish entity {} from channel {}.",
            required_string(arguments, "entity_id")?,
            required_string(arguments, "context_id")?,
        )),
        "publish_entity_to_all_channels" => Ok(format!(
            "Publish entity {} to all enabled channels.",
            required_string(arguments, "entity_id")?,
        )),
        "link_file" => Ok(format!(
            "Attach file {} to attribute '{}' on entity {}.",
            required_string(arguments, "file_id")?,
            required_string(arguments, "attribute_code")?,
            required_string(arguments, "entity_id")?,
        )),
        "create_context" => Ok(format!(
            "Create attribute context '{}'.",
            required_string(arguments, "code")?
        )),
        "update_saved_search" => Ok(format!(
            "Update saved Explorer search {} ({} specified fields; {} attribute-filter and {} relationship-facet entries supplied).",
            required_string(arguments, "saved_view_id")?,
            arguments
                .as_object()
                .map_or(0, |fields| fields.len().saturating_sub(1)),
            arguments
                .get("attributeFilters")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            arguments
                .get("relationshipFacets")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        )),
        "create_saved_search" => Ok(format!(
            "Save {} Explorer search '{}' for blueprint '{}' with {} attribute filters and {} relationship facets.",
            arguments
                .get("visibility")
                .and_then(Value::as_str)
                .unwrap_or("private"),
            required_string(arguments, "name")?,
            required_string(arguments, "blueprint")?,
            arguments
                .get("attributeFilters")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            arguments
                .get("relationshipFacets")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        )),
        _ => Err(ToolError::UnknownTool(name.to_owned())),
    }
}

pub async fn execute_read(
    repository: &CatalogRepository,
    actor: Uuid,
    workspace: Uuid,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    if !read_authorized(repository, actor, workspace, name, &arguments).await? {
        return Err(ToolError::Forbidden);
    }
    let result = match name {
        "blueprint_authoring_guide" => json!({
            "blueprints_markdown": BLUEPRINT_AUTHORING_GUIDE,
            "views_markdown": VIEW_CONFIGURATION_GUIDE,
            "json_schema_markdown": JSON_SCHEMA_GUIDE,
        }),
        "list_blueprints" => {
            serde_json::to_value(repository.list_blueprints().await?).expect("models serialize")
        }
        "get_blueprint_revision" => {
            let blueprint_id = parse_uuid(&arguments, "blueprint_id")?;
            let version = arguments.get("version").and_then(Value::as_i64)
                .filter(|version| *version > 0)
                .ok_or_else(|| ToolError::InvalidArguments("version must be positive".into()))?;
            serde_json::to_value(repository.get_blueprint_revision(blueprint_id, version).await?
                .ok_or(RepositoryError::NotFound("blueprint revision"))?)
                .expect("blueprint serializes")
        }
        "list_contexts" => serde_json::to_value(
            repository
                .list_authorized_contexts(actor, workspace)
                .await?,
        )
        .expect("models serialize"),
        "get_context" => serde_json::to_value(repository.get_context_by_id(parse_uuid(&arguments, "context_id")?).await?
            .ok_or(RepositoryError::NotFound("context"))?).expect("context serializes"),
        "list_saved_searches" => json!(repository.list_saved_views(actor, "").await?
            .into_iter().filter(|view| view.owner_user_id == actor && view.kind == "explorer_search")
            .map(|view| json!({"id":view.id,"name":view.name,"description":view.description,"visibility":view.visibility,"blueprint":view.state.get("blueprint"),"updated_at":view.updated_at}))
            .collect::<Vec<_>>()),
        "get_saved_search" => {
            let id = parse_uuid(&arguments, "saved_view_id")?;
            let view = repository.get_saved_view(actor, id, false).await?
                .filter(|view| view.owner_user_id == actor && view.kind == "explorer_search")
                .ok_or(RepositoryError::NotFound("saved search"))?;
            serde_json::to_value(view).expect("saved view serializes")
        }
        "preview_entity_migration" => {
            let preview = repository.preview_entity_migration(parse_uuid(&arguments, "entity_id")?).await?;
            json!({"migration_id":preview.migration_id,"source_version":preview.source_version,
                "target_version":preview.target.blueprint.version,"status":preview.status,"issues":preview.issues})
        }
        "get_entity_publications" => serde_json::to_value(
            repository
                .publication_statuses(parse_uuid(&arguments, "entity_id")?)
                .await?,
        )
        .expect("publication status serializes"),
        "get_entity_publication_readiness" => serde_json::to_value(
            repository
                .publication_readiness(parse_uuid(&arguments, "entity_id")?)
                .await?,
        )
        .expect("publication readiness serializes"),
        "get_entity_status_transitions" => {
            let context_id = match arguments.get("context_id") {
                None | Some(Value::Null) => None,
                Some(_) => Some(parse_uuid(&arguments, "context_id")?),
            };
            serde_json::to_value(
                repository
                    .status_transition_options(parse_uuid(&arguments, "entity_id")?, context_id)
                    .await?,
            )
            .expect("status transitions serialize")
        }
        "get_entity_changes" | "get_value_history" => {
            let (entity_id, limit, offset) = history_page_arguments(arguments)?;
            // Keep the same deleted/not-found behavior as the HTTP endpoints.
            repository.get_entity(entity_id).await?.ok_or(RepositoryError::NotFound("entity"))?;
            if name == "get_entity_changes" {
                let (items, has_more) = repository.entity_audit_changes_page(entity_id, limit, offset).await?;
                json!({"items":items,"next_offset":next_history_offset(has_more, offset, limit)})
            } else {
                let (items, has_more) = repository.value_history_page(entity_id, limit, offset).await?;
                json!({"items":items,"next_offset":next_history_offset(has_more, offset, limit)})
            }
        }
        "data_health_summary" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { stale_after_days: Option<u16> }
            let input: Input = decode(arguments)?;
            let days = input.stale_after_days.unwrap_or(DEFAULT_STALE_AFTER_DAYS);
            if !(1..=MAX_STALE_AFTER_DAYS).contains(&days) {
                return Err(ToolError::InvalidArguments("stale_after_days must be 1-3650".into()));
            }
            serde_json::to_value(repository.data_health_summary(i64::from(days)).await?)
                .expect("health summary serializes")
        }
        "list_rule_findings" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { entity_id: Option<Uuid>, limit: Option<i64>, offset: Option<i64> }
            let input: Input = decode(arguments)?;
            let (limit, offset) = diagnostic_page_arguments(input.limit, input.offset)?;
            let (items, has_more) = repository.rule_findings_page(input.entity_id, limit, offset).await?;
            json!({"items":items.into_iter().map(|finding| json!({
                "id":finding.id,"rule_id":finding.rule_id,"entity_id":finding.entity_id,
                "context_id":finding.context_id,"severity":finding.severity,
                "state":finding.state,"message":finding.message.chars().take(512).collect::<String>(),
                "updated_at":finding.updated_at
            })).collect::<Vec<_>>(),
                "next_offset":next_history_offset(has_more, offset, limit)})
        }
        "get_rule_definition" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { rule_id: Uuid, version: Option<i64> }
            let input: Input = decode(arguments)?;
            let rule = match input.version {
                Some(version) if version > 0 => repository.get_rule_revision(input.rule_id, version).await?,
                None => repository.get_rule(input.rule_id).await?,
                _ => return Err(ToolError::InvalidArguments("version must be positive".into())),
            }.ok_or(RepositoryError::NotFound("rule revision"))?;
            json!({"id":rule.id,"code":rule.code,"name":rule.name,"version":rule.version,
                "status":rule.status,"enabled_version":rule.enabled_version,
                "blueprint_id":rule.blueprint_id,"blueprint_version":rule.blueprint_version,
                "definition":rule.definition})
        }
        "get_workflow_definition" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { workflow_id: Uuid, version: Option<i64> }
            let input: Input = decode(arguments)?;
            let workflow = match input.version {
                Some(version) if version > 0 => repository.get_workflow_revision(input.workflow_id, version).await?,
                None => repository.get_workflow(input.workflow_id).await?,
                _ => return Err(ToolError::InvalidArguments("version must be positive".into())),
            }.ok_or(RepositoryError::NotFound("workflow revision"))?;
            json!({"id":workflow.id,"code":workflow.code,"name":workflow.name,
                "version":workflow.version,"status":workflow.status,
                "enabled_version":workflow.enabled_version,"definition":workflow.definition})
        }
        "list_rule_runs" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { rule_id: Option<Uuid>, limit: Option<i64>, offset: Option<i64> }
            let input: Input = decode(arguments)?;
            let (limit, offset) = diagnostic_page_arguments(input.limit, input.offset)?;
            let (items, has_more) = repository.rule_runs_page(input.rule_id, limit, offset).await?;
            json!({"items":items.into_iter().map(|run| json!({
                "id":run.id,"rule_id":run.rule_id,"rule_version":run.rule_version,
                "scope_entity_id":run.scope_entity_id,"source":run.source,
                "status":run.status,"dry_run":run.dry_run,"attempts":run.attempts,
                "candidates_evaluated":run.candidates_evaluated,"findings_created":run.findings_created,
                "findings_resolved":run.findings_resolved,"has_error":run.last_error.is_some(),
                "created_at":run.created_at,"completed_at":run.completed_at
            })).collect::<Vec<_>>(),
                "next_offset":next_history_offset(has_more, offset, limit)})
        }
        "get_workflow_run" => {
            let run_id = parse_uuid(&arguments, "run_id")?;
            safe_workflow_run(repository.get_workflow_run(run_id).await?
                .ok_or(RepositoryError::NotFound("workflow run"))?)
        }
        "list_workflow_runs" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { limit: Option<i64>, offset: Option<i64> }
            let input: Input = decode(arguments)?;
            let (limit, offset) = diagnostic_page_arguments(input.limit, input.offset)?;
            let (items, has_more) = repository.workflow_runs_page(limit, offset).await?;
            json!({"items":items.into_iter().map(safe_workflow_run).collect::<Vec<_>>(),
                "next_offset":next_history_offset(has_more, offset, limit)})
        }
        "list_extension_operation_runs" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { extension_id: Option<String>, connector_job_id: Option<Uuid>, limit: Option<i64>, offset: Option<i64> }
            let input: Input = decode(arguments)?;
            let (limit, offset) = diagnostic_page_arguments(input.limit, input.offset)?;
            if input.extension_id.as_ref().is_some_and(|id| id.is_empty() || id.len() > 128) {
                return Err(ToolError::InvalidArguments("extension_id must be 1-128 characters".into()));
            }
            let (items, has_more) = repository.extension_operation_runs_page(input.extension_id.as_deref(), input.connector_job_id, limit, offset).await?;
            json!({"items":items.into_iter().map(safe_extension_operation_run).collect::<Vec<_>>(),
                "next_offset":next_history_offset(has_more, offset, limit)})
        }
        "get_extension_operation_run" => {
            safe_extension_operation_run(repository.extension_operation_run(parse_uuid(&arguments, "run_id")?).await?
                .ok_or(RepositoryError::NotFound("extension operation run"))?)
        }
        "list_blueprint_connector_jobs" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input { blueprint_id: Uuid, limit: Option<i64>, offset: Option<i64> }
            let input: Input = decode(arguments)?;
            let (limit, offset) = diagnostic_page_arguments(input.limit, input.offset)?;
            let (items, has_more) = repository.blueprint_connector_jobs_page(input.blueprint_id, limit, offset).await?;
            json!({"items":items.into_iter().map(|job| json!({
                "id":job.id,"blueprint_id":job.blueprint_id,"blueprint_version":job.blueprint_version,
                "code":job.code,"direction":job.direction,"extension_id":job.extension_id,
                "operation_id":job.operation_id,"context_id":job.context_id,
                "interval_seconds":job.interval_seconds,"next_at":job.next_at,"enabled":job.enabled
            })).collect::<Vec<_>>(),
                "next_offset":next_history_offset(has_more, offset, limit)})
        }
        "get_entity_preview_link" => {
            let id = parse_uuid(&arguments, "entity_id")?;
            repository
                .get_entity(id)
                .await?
                .ok_or(RepositoryError::NotFound("entity"))?;
            json!({"entity_id": id, "url": format!("/entities/{id}"), "label": "Entity preview"})
        }
        "get_entity_context_preview" => {
            let entity_id = parse_uuid(&arguments, "entity_id")?;
            let context_id = parse_uuid(&arguments, "context_id")?;
            let preview = repository
                .resolved_preview(entity_id, context_id, 1)
                .await?
                .ok_or(RepositoryError::NotFound("entity"))?;
            serde_json::to_value(preview).expect("preview serializes")
        }
        "get_entity" => {
            let id = parse_uuid(&arguments, "entity_id")?;
            let (entity, values) = CatalogReadService::new(repository)
                .entity_with_values(id)
                .await?;
            let mut output = serde_json::to_value(entity).expect("models serialize");
            output
                .as_object_mut()
                .expect("entity serializes as an object")
                .insert(
                    "values".to_owned(),
                    serde_json::to_value(values).expect("models serialize"),
                );
            output
        }
        "view_image" => {
            let file_id = parse_uuid(&arguments, "file_id")?;
            let metadata = repository.file_metadata(file_id).await?;
            if !metadata.mime_type.starts_with("image/") {
                return Err(ToolError::InvalidArguments(
                    "file_id must identify an image".to_owned(),
                ));
            }
            json!({
                "file_id": file_id,
                "filename": metadata.filename,
                "mime_type": metadata.mime_type,
                "message": "The image is attached to this tool result for visual inspection.",
            })
        }
        "read_file" => {
            let file_id = parse_uuid(&arguments, "file_id")?;
            let metadata = repository.file_metadata(file_id).await?;
            if !is_readable_text_mime(&metadata.mime_type) {
                return Err(ToolError::InvalidArguments(
                    "file_id must identify a UTF-8 text file".to_owned(),
                ));
            }
            json!({
                "file_id": file_id,
                "filename": metadata.filename,
                "mime_type": metadata.mime_type,
                "message": "The text file is attached to this tool result for reading.",
            })
        }
        "search_entities" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                blueprint: crate::model::SearchBlueprint,
                #[serde(default)]
                query: Option<String>,
                #[serde(default)]
                filters: Vec<crate::model::SearchFilter>,
                #[serde(default)]
                relationship_filters: Vec<crate::model::RelationshipFilter>,
                #[serde(default)]
                system_tags: Vec<String>,
                #[serde(default)]
                outdated: bool,
                #[serde(default)]
                sort: Option<crate::model::SearchSort>,
                #[serde(default)]
                page: crate::model::SearchPage,
            }

            let input: Input = decode(arguments)?;
            if input.blueprint.code.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "blueprint.code must not be empty".to_owned(),
                ));
            }
            if input.filters.len() > MAX_SEARCH_FILTERS
                || input.relationship_filters.len() > MAX_SEARCH_FILTERS
            {
                return Err(ToolError::InvalidArguments(format!(
                    "filters and relationship_filters must each contain at most {MAX_SEARCH_FILTERS} items"
                )));
            }
            let limit = input.page.size.unwrap_or(DEFAULT_PAGE_SIZE);
            if limit == 0 || limit > DEFAULT_ENTITY_PAGE_SIZE {
                return Err(ToolError::InvalidArguments(format!(
                    "page.size must be between 1 and {DEFAULT_ENTITY_PAGE_SIZE}"
                )));
            }
            let current = repository
                .get_blueprint_by_code(&input.blueprint.code)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint"))?;
            let selected = match input.blueprint.version {
                Some(version) => Some(
                    repository
                        .get_published_blueprint_by_code_and_version(&input.blueprint.code, version)
                        .await?
                        .map(|blueprint| blueprint.blueprint.version)
                        .ok_or(RepositoryError::NotFound("blueprint"))?,
                ),
                None => None,
            };
            let search_blueprint = match selected {
                Some(version) => repository
                    .get_published_blueprint_by_code_and_version(&input.blueprint.code, version)
                    .await?
                    .expect("selected version was checked above"),
                None => current.clone(),
            };
            let query = input
                .query
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let resolved = if query.is_some() {
                Some(
                    repository
                        .resolve_search(&search_blueprint, selected, query)
                        .await
                        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?,
                )
            } else {
                None
            };
            let mut matching = resolved
                .as_ref()
                .map(|resolved| resolved.ids.iter().copied().collect::<Vec<_>>());
            let mut filters = Vec::with_capacity(input.filters.len());
            for filter in &input.filters {
                filters.push(resolve_agent_filter(repository, &search_blueprint, filter).await?);
            }
            if !filters.is_empty() {
                let ids = repository
                    .filter_entity_ids(current.blueprint.id, selected, &filters)
                    .await?;
                matching = Some(intersect_ids(matching, ids));
            }
            let mut relationship_filters = Vec::with_capacity(input.relationship_filters.len());
            for filter in &input.relationship_filters {
                relationship_filters.push(
                    resolve_agent_relationship_filter(repository, &search_blueprint, filter)
                        .await?,
                );
            }
            if !relationship_filters.is_empty() {
                let ids = repository
                    .filter_relationship_entity_ids(
                        current.blueprint.id,
                        selected,
                        &relationship_filters,
                    )
                    .await?;
                matching = Some(intersect_ids(matching, ids));
            }
            let versions = repository
                .search_result_versions(
                    current.blueprint.id,
                    selected,
                    matching.as_deref(),
                    &input.system_tags,
                    input.outdated,
                    current.blueprint.version,
                )
                .await?;
            let relationship_sort = input
                .sort
                .as_ref()
                .is_some_and(|sort| sort.field.contains('.'));
            if selected.is_none() && relationship_sort && versions.len() != 1 {
                return Err(ToolError::InvalidArguments(
                    "relationship_path_sort_requires_single_result_version".to_owned(),
                ));
            }
            let effective_source_version = match versions.as_slice() {
                [version] => Some(*version),
                _ => selected,
            };
            let sort_blueprint = match effective_source_version {
                Some(version) if version != current.blueprint.version => repository
                    .get_published_blueprint_by_code_and_version(&input.blueprint.code, version)
                    .await?
                    .ok_or(RepositoryError::NotFound("blueprint"))?,
                _ => search_blueprint.clone(),
            };
            let sort = resolve_agent_search_sort(
                repository,
                &sort_blueprint,
                input.sort.as_ref(),
                relationship_sort
                    .then_some(effective_source_version)
                    .flatten(),
            )
            .await?;
            let cursor = match (sort.is_some(), input.page.cursor.as_deref()) {
                (true, _) => None,
                (false, Some(cursor)) => Some(
                    super::repository::decode_search_cursor(cursor).ok_or_else(|| {
                        ToolError::InvalidArguments("page.cursor is invalid".to_owned())
                    })?,
                ),
                (false, None) => None,
            };
            let (mut items, next_cursor) = match sort.as_ref() {
                Some(sort) => {
                    repository
                        .search_entity_previews_sorted(
                            current.blueprint.id,
                            if relationship_sort {
                                effective_source_version
                            } else {
                                selected
                            },
                            limit.into(),
                            input.page.cursor.as_deref(),
                            matching.as_deref(),
                            &input.system_tags,
                            input.outdated,
                            current.blueprint.version,
                            sort,
                        )
                        .await?
                }
                None => {
                    repository
                        .search_entity_previews(
                            current.blueprint.id,
                            selected,
                            None,
                            limit.into(),
                            cursor,
                            matching.as_deref(),
                            &input.system_tags,
                            input.outdated,
                            current.blueprint.version,
                        )
                        .await?
                }
            };
            for item in &mut items {
                item.schema_outdated = item.blueprint_version != current.blueprint.version;
                item.match_explanations = resolved
                    .as_ref()
                    .and_then(|resolved| resolved.explanations.get(&item.id))
                    .cloned()
                    .unwrap_or_default();
            }
            let table_paths = agent_table_paths(&sort_blueprint);
            repository
                .hydrate_table_path_values(&mut items, &table_paths)
                .await?;
            repository
                .hydrate_related_table_previews(
                    &mut items,
                    &agent_table_relationships(&sort_blueprint),
                )
                .await?;
            let result_version_scope = match versions.as_slice() {
                [] => crate::model::SearchResultVersionScope::Empty,
                [version] => crate::model::SearchResultVersionScope::Single { version: *version },
                _ => crate::model::SearchResultVersionScope::Multiple,
            };
            let hidden_outdated_count =
                if selected == Some(current.blueprint.version) && input.page.cursor.is_none() {
                    Some(
                        repository
                            .count_entity_previews(
                                current.blueprint.id,
                                None,
                                None,
                                &[],
                                true,
                                current.blueprint.version,
                                501,
                            )
                            .await?,
                    )
                } else {
                    None
                };
            serde_json::to_value(crate::model::EntitySearchResponse {
                blueprint: sort_blueprint,
                items,
                next_cursor,
                total_count: None,
                total_count_capped: false,
                result_version_scope,
                hidden_outdated_count: hidden_outdated_count.map(|count| count.min(500)),
                hidden_outdated_count_capped: hidden_outdated_count
                    .is_some_and(|count| count > 500),
            })
            .expect("models serialize")
        }
        _ => return Err(ToolError::UnknownTool(name.to_owned())),
    };
    bounded(result)
}

pub async fn execute_mutation(
    repository: &CatalogRepository,
    actor: Uuid,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    use crate::model::{CreateAttributeContext, CreateBlueprint, SearchBlueprint};
    let result = match name {
        "create_blueprint_revision" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                blueprint_id: Uuid,
                definition: String,
            }
            let input: Input = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .create_blueprint_revision(
                        input.blueprint_id,
                        CreateBlueprint {
                            definition: input.definition,
                        },
                    )
                    .await?,
            )
            .expect("models serialize")
        }
        "publish_blueprint" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                blueprint_id: Uuid,
                version: i64,
            }
            let input: Input = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .publish_blueprint_revision(input.blueprint_id, input.version)
                    .await?,
            )
            .expect("models serialize")
        }
        "create_blueprint" => {
            let input: CreateBlueprint = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .create_blueprint(input)
                    .await?,
            )
            .expect("models serialize")
        }
        "create_entity" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                blueprint: SearchBlueprint,
                #[serde(default)]
                values: Vec<crate::model::NewAttributeValue>,
                #[serde(default)]
                system_tags: Vec<String>,
                #[serde(default = "empty_object")]
                system_metadata: Value,
            }
            let input: Input = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .create_entity(crate::model::CreateEntityFormRequest {
                        blueprint: input.blueprint,
                        values: input.values,
                        system_tags: input.system_tags,
                        system_metadata: input.system_metadata,
                    })
                    .await?,
            )
            .expect("models serialize")
        }
        "delete_entity" => {
            CatalogMutationService::new(repository)
                .delete_entity(parse_uuid(&arguments, "entity_id")?)
                .await?;
            json!({"deleted": true})
        }
        "set_entity_values" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                values: Vec<crate::model::NewAttributeValue>,
            }
            let input: Input = decode(arguments)?;
            if input.values.is_empty()
                || input
                    .values
                    .iter()
                    .any(|value| !matches!(value, crate::model::NewAttributeValue::Scalar { .. }))
            {
                return Err(ToolError::InvalidArguments(
                    "values must contain at least one scalar attribute value".to_owned(),
                ));
            }
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .append_values(
                        input.entity_id,
                        crate::model::AppendAttributeValues {
                            expected_updated_at: None,
                            values: input.values,
                        },
                    )
                    .await?,
            )
            .expect("attribute values serialize")
        }
        "remove_entity_values" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                remove_values: Vec<crate::model::AttributeValueSelector>,
            }
            let input: Input = decode(arguments)?;
            if input.remove_values.is_empty()
                || input.remove_values.len() > 20
                || input
                    .remove_values
                    .iter()
                    .any(|value| value.attribute_code.is_empty())
                || input
                    .remove_values
                    .iter()
                    .map(|value| (&value.attribute_code, value.context_id))
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != input.remove_values.len()
            {
                return Err(ToolError::InvalidArguments(
                    "remove_values must contain 1 to 20 distinct attribute/context selectors"
                        .into(),
                ));
            }
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .update_entity(
                        input.entity_id,
                        crate::model::UpdateEntityFormRequest {
                            expected_updated_at: None,
                            values: vec![],
                            relationships: vec![],
                            remove_values: input.remove_values,
                            system_tags: None,
                            system_metadata: None,
                        },
                    )
                    .await?,
            )
            .expect("entity serializes")
        }
        "restore_entity_value" => {
            let entity_id = parse_uuid(&arguments, "entity_id")?;
            let history_id = parse_uuid(&arguments, "history_id")?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .restore_value(entity_id, history_id)
                    .await?,
            )
            .expect("value serializes")
        }
        "replace_entity_relationships" | "remove_entity_relationships" => {
            let (entity_id, relationships) =
                decode_relationship_mutation(arguments, name == "remove_entity_relationships")?;
            let service = CatalogMutationService::new(repository);
            let input = crate::model::RelationshipMutation { relationships };
            let updated = if name == "replace_entity_relationships" {
                service.replace_relationships(entity_id, input).await?
            } else {
                service.remove_relationships(entity_id, input).await?
            };
            serde_json::to_value(updated).expect("relationship values serialize")
        }
        "migrate_entity" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                #[serde(default)]
                values: Vec<crate::model::NewAttributeValue>,
                #[serde(default)]
                relationships: Vec<crate::model::RelationshipTargets>,
                #[serde(default)]
                discard_attributes: Vec<String>,
            }
            let input: Input = decode(arguments)?;
            let preview = repository.preview_entity_migration(input.entity_id).await?;
            if preview.status != "ready"
                && input.values.is_empty()
                && input.relationships.is_empty()
                && input.discard_attributes.is_empty()
            {
                json!({
                    "migrated": false,
                    "status": preview.status,
                    "source_version": preview.source_version,
                    "target_version": preview.target.blueprint.version,
                    "issues": preview.issues,
                })
            } else {
                let entity = CatalogMutationService::new(repository)
                    .migrate_entity(
                        input.entity_id,
                        crate::model::MigrateEntityRequest {
                            migration_id: preview.migration_id,
                            expected_target_version: preview.target.blueprint.version,
                            values: input.values,
                            relationships: input.relationships,
                            discard_attributes: input.discard_attributes,
                            removal_policy: None,
                        },
                    )
                    .await?;
                json!({"migrated": true, "entity": entity})
            }
        }
        "publish_entity" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                context_id: Uuid,
            }
            let input: Input = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .publish_entity(input.entity_id, input.context_id)
                    .await?,
            )
            .expect("publication status serializes")
        }
        "unpublish_entity" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                context_id: Uuid,
            }
            let input: Input = decode(arguments)?;
            CatalogMutationService::new(repository)
                .unpublish_entity(input.entity_id, input.context_id)
                .await?;
            json!({"unpublished": true})
        }
        "publish_entity_to_all_channels" => {
            let entity_id = parse_uuid(&arguments, "entity_id")?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .publish_entity_all_channels(entity_id)
                    .await?,
            )
            .expect("publication statuses serialize")
        }
        "link_file" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                attribute_code: String,
                file_id: Uuid,
                context_id: Option<Uuid>,
            }
            let input: Input = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .link_file(
                        input.entity_id,
                        &input.attribute_code,
                        input.context_id,
                        input.file_id,
                    )
                    .await?,
            )
            .expect("file metadata serializes")
        }
        "create_saved_search" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                name: String,
                blueprint: String,
                description: Option<String>,
                visibility: Option<String>,
                version: Option<i64>,
                #[serde(default)]
                all_versions: bool,
                query: Option<String>,
                #[serde(default, rename = "attributeFilters")]
                attribute_filters: Vec<crate::model::SearchFilter>,
                #[serde(default, rename = "relationshipFacets")]
                relationship_facets: Vec<SavedSearchRelationshipFacet>,
            }
            let input: Input = decode(arguments)?;
            let name = input.name.trim();
            let blueprint = input.blueprint.trim();
            let visibility = input.visibility.as_deref().unwrap_or("private");
            if name.is_empty()
                || name.len() > 120
                || blueprint.is_empty()
                || blueprint.len() > 256
                || !matches!(visibility, "private" | "workspace")
                || input.description.as_ref().is_some_and(|s| s.len() > 500)
                || input.version.is_some_and(|v| v <= 0)
                || (input.version.is_some() && input.all_versions)
                || input.query.as_ref().is_some_and(|q| q.len() > 4096)
                || input.attribute_filters.len() > MAX_SEARCH_FILTERS
                || input.relationship_facets.len() > MAX_SEARCH_FILTERS
                || input
                    .attribute_filters
                    .iter()
                    .any(|filter| !valid_saved_filter(filter))
                || input.relationship_facets.iter().any(|facet| {
                    facet.field.is_empty()
                        || facet.selected_ids.is_empty()
                        || facet.selected_ids.len() > 100
                })
            {
                return Err(ToolError::InvalidArguments(
                    "invalid saved search input".to_owned(),
                ));
            }
            let current = repository
                .get_blueprint_by_code(blueprint)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint"))?;
            let source = if let Some(version) = input.version {
                repository
                    .get_published_blueprint_by_code_and_version(blueprint, version)
                    .await?
                    .ok_or(RepositoryError::NotFound("published blueprint version"))?
            } else {
                current
            };
            for filter in &input.attribute_filters {
                resolve_agent_filter(repository, &source, filter).await?;
            }
            for facet in &input.relationship_facets {
                resolve_agent_relationship_filter(
                    repository,
                    &source,
                    &crate::model::RelationshipFilter {
                        field: facet.field.clone(),
                        selected_target_ids: facet.selected_ids.clone(),
                    },
                )
                .await?;
            }
            let mut state = json!({"blueprint": blueprint});
            if let Some(version) = input.version {
                state["version"] = json!(version);
            }
            if input.all_versions {
                state["allVersions"] = json!(true);
            }
            if let Some(query) = input
                .query
                .as_deref()
                .map(str::trim)
                .filter(|q| !q.is_empty())
            {
                state["query"] = json!(query);
            }
            if !input.attribute_filters.is_empty() {
                state["attributeFilters"] = json!(input.attribute_filters.iter().map(|filter| json!({"field":filter.field,"operator":filter.operator,"value":filter.value})).collect::<Vec<_>>());
            }
            if !input.relationship_facets.is_empty() {
                state["relationshipFacets"] = json!(
                    input
                        .relationship_facets
                        .iter()
                        .map(|facet| json!({"field":facet.field,"selectedIds":facet.selected_ids}))
                        .collect::<Vec<_>>()
                );
            }
            if serde_json::to_vec(&state)
                .expect("search state serializes")
                .len()
                > 32_768
            {
                return Err(ToolError::InvalidArguments(
                    "search state exceeds 32 KiB".to_owned(),
                ));
            }
            let view = repository
                .create_saved_view(
                    actor,
                    Some(name),
                    input.description.as_deref(),
                    visibility,
                    &state,
                )
                .await?;
            json!({"id": view.id, "name": view.name, "url": format!("/?savedView={}", view.id), "visibility": view.visibility})
        }
        "update_saved_search" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                saved_view_id: Uuid,
                name: Option<String>,
                description: Option<String>,
                visibility: Option<String>,
                blueprint: Option<String>,
                version: Option<i64>,
                all_versions: Option<bool>,
                query: Option<String>,
                #[serde(rename = "attributeFilters")]
                attribute_filters: Option<Vec<crate::model::SearchFilter>>,
                #[serde(rename = "relationshipFacets")]
                relationship_facets: Option<Vec<SavedSearchRelationshipFacet>>,
            }
            if arguments.as_object().is_none_or(|fields| fields.len() <= 1) {
                return Err(ToolError::InvalidArguments(
                    "provide at least one saved search change".to_owned(),
                ));
            }
            let input: Input = decode(arguments)?;
            let existing = repository
                .get_saved_view(actor, input.saved_view_id, false)
                .await?
                .filter(|view| view.owner_user_id == actor && view.kind == "explorer_search")
                .ok_or(RepositoryError::NotFound("saved search"))?;
            let name = input
                .name
                .as_deref()
                .unwrap_or(existing.name.as_deref().unwrap_or(""))
                .trim();
            let description = input
                .description
                .as_deref()
                .or(existing.description.as_deref());
            let visibility = input.visibility.as_deref().unwrap_or(&existing.visibility);
            let mut state = existing.state.clone();
            let blueprint = input
                .blueprint
                .as_deref()
                .unwrap_or_else(|| state["blueprint"].as_str().unwrap_or(""))
                .trim()
                .to_owned();
            if name.is_empty()
                || name.len() > 120
                || description.is_some_and(|value| value.len() > 500)
                || !matches!(visibility, "private" | "workspace")
                || blueprint.is_empty()
                || blueprint.len() > 256
                || input.version.is_some_and(|version| version <= 0)
                || input.query.as_ref().is_some_and(|query| query.len() > 4096)
                || input.attribute_filters.as_ref().is_some_and(|filters| {
                    filters.len() > MAX_SEARCH_FILTERS
                        || filters.iter().any(|filter| !valid_saved_filter(filter))
                })
                || input.relationship_facets.as_ref().is_some_and(|facets| {
                    facets.len() > MAX_SEARCH_FILTERS
                        || facets.iter().any(|facet| {
                            facet.field.is_empty()
                                || facet.selected_ids.is_empty()
                                || facet.selected_ids.len() > 100
                        })
                })
            {
                return Err(ToolError::InvalidArguments(
                    "invalid saved search input".to_owned(),
                ));
            }
            if (blueprint != existing.state["blueprint"].as_str().unwrap_or("")
                || input
                    .version
                    .is_some_and(|version| existing.state["version"] != json!(version))
                || input.all_versions == Some(true))
                && ((input.attribute_filters.is_none()
                    && existing.state.get("attributeFilters").is_some())
                    || (input.relationship_facets.is_none()
                        && existing.state.get("relationshipFacets").is_some()))
            {
                return Err(ToolError::InvalidArguments("specify attributeFilters and relationshipFacets when changing blueprint or version to replace or clear old filters".to_owned()));
            }
            state["blueprint"] = json!(blueprint);
            if let Some(version) = input.version {
                state["version"] = json!(version);
                state.as_object_mut().unwrap().remove("allVersions");
            }
            if let Some(all_versions) = input.all_versions {
                if all_versions {
                    state["allVersions"] = json!(true);
                    state.as_object_mut().unwrap().remove("version");
                } else {
                    state.as_object_mut().unwrap().remove("allVersions");
                }
            }
            if let Some(query) = input.query {
                if query.trim().is_empty() {
                    state.as_object_mut().unwrap().remove("query");
                } else {
                    state["query"] = json!(query.trim());
                }
            }
            if let Some(filters) = input.attribute_filters.as_ref() {
                if filters.is_empty() {
                    state.as_object_mut().unwrap().remove("attributeFilters");
                } else {
                    state["attributeFilters"] = json!(filters.iter().map(|filter| json!({"field":filter.field,"operator":filter.operator,"value":filter.value})).collect::<Vec<_>>());
                }
            }
            if let Some(facets) = input.relationship_facets.as_ref() {
                if facets.is_empty() {
                    state.as_object_mut().unwrap().remove("relationshipFacets");
                } else {
                    state["relationshipFacets"] = json!(facets.iter().map(|facet| json!({"field":facet.field,"selectedIds":facet.selected_ids})).collect::<Vec<_>>());
                }
            }
            let current = repository
                .get_blueprint_by_code(&blueprint)
                .await?
                .ok_or(RepositoryError::NotFound("blueprint"))?;
            let source = if let Some(version) = state.get("version").and_then(Value::as_i64) {
                repository
                    .get_published_blueprint_by_code_and_version(&blueprint, version)
                    .await?
                    .ok_or(RepositoryError::NotFound("published blueprint version"))?
            } else {
                current
            };
            if let Some(filters) = input.attribute_filters.as_ref() {
                for filter in filters {
                    resolve_agent_filter(repository, &source, filter).await?;
                }
            }
            if let Some(facets) = input.relationship_facets.as_ref() {
                for facet in facets {
                    resolve_agent_relationship_filter(
                        repository,
                        &source,
                        &crate::model::RelationshipFilter {
                            field: facet.field.clone(),
                            selected_target_ids: facet.selected_ids.clone(),
                        },
                    )
                    .await?;
                }
            }
            if serde_json::to_vec(&state)
                .expect("search state serializes")
                .len()
                > 32_768
            {
                return Err(ToolError::InvalidArguments(
                    "search state exceeds 32 KiB".to_owned(),
                ));
            }
            let view = repository
                .update_saved_view(
                    actor,
                    input.saved_view_id,
                    name,
                    description,
                    visibility,
                    &state,
                )
                .await?
                .ok_or(RepositoryError::NotFound("saved search"))?;
            json!({"id":view.id,"name":view.name,"url":format!("/?savedView={}",view.id),"visibility":view.visibility,"state":view.state})
        }
        "update_entity_annotations" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                entity_id: Uuid,
                system_tags: Option<Vec<String>>,
                system_metadata: Option<Value>,
            }
            let input: Input = decode(arguments)?;
            if input.system_tags.is_none() && input.system_metadata.is_none() {
                return Err(ToolError::InvalidArguments(
                    "provide tags and/or metadata".into(),
                ));
            }
            if input
                .system_tags
                .as_ref()
                .is_some_and(|tags| tags.len() > 100)
                || input
                    .system_metadata
                    .as_ref()
                    .is_some_and(|metadata| !metadata.is_object())
            {
                return Err(ToolError::InvalidArguments(
                    "tags must have at most 100 entries and metadata must be an object".into(),
                ));
            }
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .update_entity(
                        input.entity_id,
                        crate::model::UpdateEntityFormRequest {
                            expected_updated_at: None,
                            values: vec![],
                            relationships: vec![],
                            remove_values: vec![],
                            system_tags: input.system_tags,
                            system_metadata: input.system_metadata,
                        },
                    )
                    .await?,
            )
            .expect("entity serializes")
        }
        "update_context" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                context_id: Uuid,
                parent_id: Uuid,
                data: Value,
            }
            let input: Input = decode(arguments)?;
            if !input.data.is_object() {
                return Err(ToolError::InvalidArguments("data must be an object".into()));
            }
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .update_context(
                        input.context_id,
                        crate::model::UpdateAttributeContext {
                            parent_id: input.parent_id,
                            data: input.data,
                        },
                    )
                    .await?,
            )
            .expect("context serializes")
        }
        "delete_context" => {
            CatalogMutationService::new(repository)
                .delete_context(parse_uuid(&arguments, "context_id")?)
                .await?;
            json!({"deleted":true})
        }
        "create_context" => {
            let input: CreateAttributeContext = decode(arguments)?;
            serde_json::to_value(
                CatalogMutationService::new(repository)
                    .create_context(input)
                    .await?,
            )
            .expect("models serialize")
        }
        _ => return Err(ToolError::UnknownTool(name.to_owned())),
    };
    bounded(result)
}

fn decode_relationship_mutation(
    arguments: Value,
    removing: bool,
) -> Result<(Uuid, Vec<crate::model::RelationshipTargets>), ToolError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        entity_id: Uuid,
        relationships: Vec<crate::model::RelationshipTargets>,
    }
    let input: Input = decode(arguments)?;
    if input.relationships.is_empty() || input.relationships.len() > 20 {
        return Err(ToolError::InvalidArguments(
            "relationships must contain 1 to 20 sets".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for relationship in &input.relationships {
        let Some(code) = relationship
            .attribute_code
            .as_deref()
            .filter(|code| !code.is_empty())
        else {
            return Err(ToolError::InvalidArguments(
                "attribute_code is required".into(),
            ));
        };
        if relationship.attribute_id.is_some()
            || relationship.target_entity_ids.len() > 100
            || (removing && relationship.target_entity_ids.is_empty())
            || !seen.insert((code.to_owned(), relationship.context_id))
            || relationship
                .target_entity_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != relationship.target_entity_ids.len()
        {
            return Err(ToolError::InvalidArguments("relationship sets require unique attribute/context and at most 100 unique targets (nonempty when removing)".into()));
        }
    }
    Ok((input.entity_id, input.relationships))
}

fn agent_table_paths(blueprint: &crate::model::BlueprintWithAttributes) -> HashMap<String, String> {
    let mut paths: HashMap<_, _> = blueprint
        .table_path_attributes
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type.clone()))
        .collect();
    if let Some(fields) = blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("fields"))
        .and_then(Value::as_array)
    {
        for field in fields.iter().filter_map(Value::as_str) {
            if let Some(attribute) = blueprint
                .attributes
                .iter()
                .find(|attribute| attribute.code == field && attribute.value_type != "relationship")
            {
                paths.insert(attribute.code.clone(), attribute.value_type.clone());
            }
        }
    }
    paths
}

fn agent_table_relationships(
    blueprint: &crate::model::BlueprintWithAttributes,
) -> HashMap<String, String> {
    blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("columns"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|column| column.get("field").and_then(Value::as_str))
        .filter_map(|field| field.split_once('.').map(|(relationship, _)| relationship))
        .filter_map(|relationship| {
            blueprint
                .attributes
                .iter()
                .find(|attribute| attribute.code == relationship)
                .filter(|attribute| attribute.value_type == "relationship")
                .and_then(|attribute| {
                    attribute
                        .target_blueprint_code
                        .as_ref()
                        .map(|target| (relationship.to_owned(), target.clone()))
                })
        })
        .collect()
}

async fn resolve_agent_search_sort(
    repository: &CatalogRepository,
    blueprint: &crate::model::BlueprintWithAttributes,
    sort: Option<&crate::model::SearchSort>,
    effective_source_version: Option<i64>,
) -> Result<Option<EntitySearchSort>, ToolError> {
    let Some(sort) = sort else { return Ok(None) };
    let descending = match sort.direction.as_str() {
        "asc" => false,
        "desc" => true,
        _ => {
            return Err(ToolError::InvalidArguments(
                "sort.direction must be asc or desc".to_owned(),
            ));
        }
    };
    if sort.field == "publication_status" {
        let code = sort.context_code.as_deref().ok_or_else(|| {
            ToolError::InvalidArguments(
                "sort.context_code is required for publication_status".to_owned(),
            )
        })?;
        let channel = repository
            .list_publication_channels()
            .await?
            .into_iter()
            .find(|channel| channel.context_code == code && channel.enabled)
            .ok_or_else(|| {
                ToolError::InvalidArguments(
                    "sort.context_code must be an enabled publication channel".to_owned(),
                )
            })?;
        return Ok(Some(EntitySearchSort {
            field: sort.field.clone(),
            relationship_path: Vec::new(),
            leaf_field: sort.field.clone(),
            leaf_blueprint_id: blueprint.blueprint.id,
            value_type: "integer".to_owned(),
            descending,
            effective_source_version,
            publication_context_id: Some(channel.context_id),
        }));
    }
    if sort.context_code.is_some() {
        return Err(ToolError::InvalidArguments(
            "sort.context_code is only valid for publication_status".to_owned(),
        ));
    }
    if sort.field == "blueprint_version" {
        return Ok(Some(EntitySearchSort {
            field: sort.field.clone(),
            relationship_path: Vec::new(),
            leaf_field: sort.field.clone(),
            leaf_blueprint_id: blueprint.blueprint.id,
            value_type: "integer".to_owned(),
            descending,
            effective_source_version,
            publication_context_id: None,
        }));
    }
    let configured = blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("columns"))
        .and_then(Value::as_array)
        .is_some_and(|columns| {
            columns
                .iter()
                .any(|column| column.get("field").and_then(Value::as_str) == Some(&sort.field))
        });
    if !configured {
        return Err(ToolError::InvalidArguments(
            "sort.field must be a configured table column".to_owned(),
        ));
    }
    let parts: Vec<_> = sort.field.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return Err(ToolError::InvalidArguments(
            "sort.field may contain at most three relationship hops and a scalar leaf".to_owned(),
        ));
    }
    let mut current = blueprint.clone();
    let mut relationship_path = Vec::new();
    for relationship_name in &parts[..parts.len() - 1] {
        let relationship = current
            .attributes
            .iter()
            .find(|attribute| {
                attribute.code == *relationship_name && attribute.value_type == "relationship"
            })
            .ok_or_else(|| {
                ToolError::InvalidArguments("sort.field relationship is invalid".to_owned())
            })?;
        if relationship.cardinality.as_deref() != Some("one") {
            return Err(ToolError::InvalidArguments(
                "sort.field relationship path must be single-valued".to_owned(),
            ));
        }
        relationship_path.push(relationship.code.clone());
        current = repository
            .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                || {
                    ToolError::InvalidArguments(
                        "sort.field relationship has no target blueprint".to_owned(),
                    )
                },
            )?)
            .await?
            .ok_or(RepositoryError::NotFound("target blueprint"))?;
    }
    let leaf_field = parts[parts.len() - 1];
    let value_type = current
        .attributes
        .iter()
        .find(|attribute| attribute.code == leaf_field)
        .map(|attribute| attribute.value_type.clone())
        .ok_or_else(|| {
            ToolError::InvalidArguments(
                "sort.field must resolve to a scalar table column".to_owned(),
            )
        })?;
    if !matches!(
        value_type.as_str(),
        "string" | "number" | "integer" | "boolean" | "date" | "datetime" | "time"
    ) {
        return Err(ToolError::InvalidArguments(
            "sort.field must resolve to a scalar table column".to_owned(),
        ));
    }
    Ok(Some(EntitySearchSort {
        field: sort.field.clone(),
        relationship_path,
        leaf_field: leaf_field.to_owned(),
        leaf_blueprint_id: current.blueprint.id,
        value_type,
        descending,
        effective_source_version,
        publication_context_id: None,
    }))
}

async fn read_authorized(
    repository: &CatalogRepository,
    actor: Uuid,
    workspace: Uuid,
    name: &str,
    arguments: &Value,
) -> Result<bool, ToolError> {
    let (permission, target_id, target_code) = match name {
        // This is static product documentation, not workspace catalog data.
        "blueprint_authoring_guide" => return Ok(true),
        "list_blueprints" | "get_blueprint_revision" => ("blueprints.read", None, None),
        "data_health_summary" => ("data_health.read", None, None),
        "list_rule_findings" | "get_rule_definition" | "list_rule_runs" => {
            ("rules.read", None, None)
        }
        "list_workflow_runs" | "get_workflow_definition" | "get_workflow_run" => {
            ("workflows.read", None, None)
        }
        "list_extension_operation_runs"
        | "get_extension_operation_run"
        | "list_blueprint_connector_jobs" => ("extensions.manage", None, None),
        "list_contexts" => ("contexts.read", None, Some("__context_list__")),
        "get_context" => (
            "contexts.read",
            Some(parse_uuid(arguments, "context_id")?),
            None,
        ),
        "get_entity"
        | "get_entity_context_preview"
        | "get_entity_changes"
        | "get_value_history"
        | "get_entity_preview_link"
        | "get_entity_publications"
        | "get_entity_publication_readiness"
        | "get_entity_status_transitions" => (
            "entities.read",
            Some(parse_uuid(arguments, "entity_id")?),
            None,
        ),
        // HTTP migration previews require entity write authority even though
        // the preview itself is non-mutating.
        "preview_entity_migration" => (
            "entities.write",
            Some(parse_uuid(arguments, "entity_id")?),
            None,
        ),
        "view_image" | "read_file" => {
            let file_id = parse_uuid(arguments, "file_id")?;
            return Ok(authorize_file_read(
                repository,
                &AllowFileAccess,
                actor,
                workspace,
                file_id,
                |file_id, entity_id, blueprint_id| FileAccessOperation::AgentRead {
                    file_id,
                    entity_id,
                    blueprint_id,
                },
            )
            .await?);
        }
        // Match the HTTP search endpoint: collection searches require a
        // workspace-wide entities.read grant, rather than exposing partial
        // results for a scoped grant.
        "search_entities" | "list_saved_searches" | "get_saved_search" => {
            ("entities.read", None, None)
        }
        _ => return Err(ToolError::UnknownTool(name.to_owned())),
    };
    Ok(repository
        .is_authorized(actor, workspace, permission, target_id, target_code)
        .await?)
}

fn is_readable_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/ld+json"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/javascript"
        )
}

fn empty_object() -> Value {
    json!({})
}
fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ToolError> {
    serde_json::from_value(value).map_err(|e| ToolError::InvalidArguments(e.to_string()))
}
fn parse_uuid(value: &Value, key: &str) -> Result<Uuid, ToolError> {
    required_string(value, key)?
        .parse()
        .map_err(|_| ToolError::InvalidArguments(format!("{key} must be a UUID")))
}
fn required_string<'a>(value: &'a Value, path: &str) -> Result<&'a str, ToolError> {
    let value = path
        .split('.')
        .try_fold(value, |current, key| current.get(key))
        .and_then(Value::as_str);
    value.ok_or_else(|| {
        ToolError::InvalidArguments(format!("{path} is required and must be a string"))
    })
}
fn bounded(value: Value) -> Result<Value, ToolError> {
    if serde_json::to_vec(&value)
        .map_err(|e| ToolError::InvalidArguments(e.to_string()))?
        .len()
        > MAX_TOOL_RESULT_BYTES
    {
        Err(ToolError::ResultTooLarge)
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ToolError, ToolKind, bounded, change_summary, definitions, kind};
    use crate::agents::MAX_TOOL_RESULT_BYTES;

    #[test]
    fn classifies_every_write_as_an_approval_required_mutation() {
        assert_eq!(kind("list_blueprints").unwrap(), ToolKind::Read);
        assert_eq!(kind("search_entities").unwrap(), ToolKind::Read);
        assert_eq!(kind("list_saved_searches").unwrap(), ToolKind::Read);
        assert_eq!(kind("get_saved_search").unwrap(), ToolKind::Read);
        assert_eq!(kind("get_entity_preview_link").unwrap(), ToolKind::Read);
        for name in [
            "create_saved_search",
            "update_saved_search",
            "create_entity",
            "delete_entity",
            "set_entity_values",
            "migrate_entity",
            "create_context",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Mutation);
        }
        assert!(matches!(kind("fetch_url"), Err(ToolError::UnknownTool(_))));
        assert_eq!(
            change_summary(
                "create_saved_search",
                &json!({"name":"Spring","blueprint":"product"})
            )
            .unwrap(),
            "Save private Explorer search 'Spring' for blueprint 'product' with 0 attribute filters and 0 relationship facets."
        );
    }

    #[test]
    fn targeted_reads_and_relationship_mutations_have_distinct_safety_contracts() {
        let definitions = definitions();
        for name in ["get_blueprint_revision", "preview_entity_migration"] {
            assert_eq!(kind(name).unwrap(), ToolKind::Read);
            assert!(
                definitions
                    .iter()
                    .any(|definition| definition.function.name == name)
            );
        }
        for name in [
            "replace_entity_relationships",
            "remove_entity_relationships",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Mutation);
            let definition = definitions
                .iter()
                .find(|definition| definition.function.name == name)
                .unwrap();
            assert_eq!(
                definition.function.parameters["properties"]["relationships"]["maxItems"],
                20
            );
        }
        let id = uuid::Uuid::new_v4();
        let payload = json!({"entity_id":id,"relationships":[{"attribute_code":"categories","target_entity_ids":[id]}]});
        assert_eq!(
            super::decode_relationship_mutation(payload.clone(), false)
                .unwrap()
                .1
                .len(),
            1
        );
        assert!(super::decode_relationship_mutation(json!({"entity_id":id,"relationships":[{"attribute_code":"categories","target_entity_ids":[]}]}), false).is_ok());
        assert!(super::decode_relationship_mutation(json!({"entity_id":id,"relationships":[{"attribute_code":"categories","target_entity_ids":[]}]}), true).is_err());
        assert!(super::decode_relationship_mutation(json!({"entity_id":id,"relationships":[{"attribute_code":"categories","target_entity_ids":[id,id]}]}), false).is_err());
        assert!(super::decode_relationship_mutation(json!({"entity_id":id,"relationships":[{"attribute_code":"categories","target_entity_ids":[id]},{"attribute_code":"categories","target_entity_ids":[]}]}), false).is_err());
        assert!(super::decode_relationship_mutation(json!({"entity_id":id,"relationships":[{"attribute_code":"categories","attribute_id":id,"target_entity_ids":[id]}]}), false).is_err());
        assert_eq!(
            change_summary("replace_entity_relationships", &payload).unwrap(),
            format!("Replace relationship targets on entity {id}: categories (1 targets).")
        );
    }

    #[test]
    fn extension_diagnostic_tools_are_read_only_and_omit_private_state() {
        for name in [
            "list_extension_operation_runs",
            "get_extension_operation_run",
            "list_blueprint_connector_jobs",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Read);
            assert!(definitions().iter().any(|tool| tool.function.name == name));
        }
        let run = crate::repository::ExtensionOperationRun {
            id: uuid::Uuid::new_v4(),
            schedule_id: None,
            connector_job_id: None,
            connector_channel_id: None,
            extension_id: "example".into(),
            installed_release_id: uuid::Uuid::new_v4(),
            abi_version: "1.4".into(),
            operation_id: "import".into(),
            status: "pending".into(),
            outputs_expired: false,
            progress: json!({"secret":"hidden"}),
            checkpoint: json!({"token":"hidden"}),
            attempts: 0,
            last_error_code: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        let safe = super::safe_extension_operation_run(run);
        assert!(!safe.to_string().contains("hidden"));
        assert!(safe.get("progress").is_none());
        assert!(safe.get("checkpoint").is_none());
    }

    #[test]
    fn annotation_and_context_edits_require_approval() {
        for name in [
            "update_entity_annotations",
            "update_context",
            "delete_context",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Mutation);
            assert!(definitions().iter().any(|tool| tool.function.name == name));
        }
        assert_eq!(kind("get_context").unwrap(), ToolKind::Read);
        assert!(change_summary("update_entity_annotations", &json!({"entity_id":"id"})).is_err());
        assert_eq!(
            change_summary("update_context", &json!({"context_id":"id"})).unwrap(),
            "Replace parent and data on context id."
        );
    }

    #[test]
    fn diagnostic_tools_are_read_only_and_bounded() {
        let definitions = definitions();
        for name in [
            "data_health_summary",
            "list_rule_findings",
            "list_workflow_runs",
            "get_rule_definition",
            "get_workflow_definition",
            "list_rule_runs",
            "get_workflow_run",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Read);
            assert!(definitions.iter().any(|tool| tool.function.name == name));
        }
        assert_eq!(
            super::diagnostic_page_arguments(None, None).unwrap(),
            (10, 0)
        );
        assert!(super::diagnostic_page_arguments(Some(26), None).is_err());
        assert!(super::diagnostic_page_arguments(None, Some(-1)).is_err());
        let schema = &definitions
            .iter()
            .find(|tool| tool.function.name == "list_rule_findings")
            .unwrap()
            .function
            .parameters;
        assert_eq!(schema["properties"]["limit"]["maximum"], 25);
        let run_schema = &definitions
            .iter()
            .find(|tool| tool.function.name == "list_rule_runs")
            .unwrap()
            .function
            .parameters;
        assert!(run_schema["properties"].get("rule_id").is_some());
        assert!(run_schema["properties"].get("entity_id").is_none());
    }

    #[test]
    fn entity_history_and_restore_tools_have_bounded_approval_contracts() {
        let definitions = definitions();
        for name in ["get_entity_changes", "get_value_history"] {
            assert_eq!(kind(name).unwrap(), ToolKind::Read);
            let schema = &definitions
                .iter()
                .find(|tool| tool.function.name == name)
                .unwrap()
                .function
                .parameters;
            assert_eq!(schema["properties"]["limit"]["maximum"], 50);
        }
        for name in ["remove_entity_values", "restore_entity_value"] {
            assert_eq!(kind(name).unwrap(), ToolKind::Mutation);
        }
        let id = uuid::Uuid::new_v4();
        assert_eq!(
            super::history_page_arguments(json!({"entity_id":id})).unwrap(),
            (id, 20, 0)
        );
        assert!(super::history_page_arguments(json!({"entity_id":id,"limit":51})).is_err());
        assert!(super::history_page_arguments(json!({"entity_id":id,"offset":10001})).is_err());
        assert_eq!(super::next_history_offset(true, 10000, 20), None);
        assert_eq!(
            change_summary(
                "restore_entity_value",
                &json!({"entity_id":id,"history_id":id})
            )
            .unwrap(),
            format!("Restore history entry {id} on entity {id}.")
        );
    }

    #[test]
    fn search_entities_definition_supports_outdated_filter() {
        let search = definitions()
            .into_iter()
            .find(|definition| definition.function.name == "search_entities")
            .expect("search_entities definition");
        assert_eq!(
            search.function.parameters["properties"]["filters"],
            super::attribute_filter_parameters()
        );
        assert_eq!(
            search.function.parameters["properties"]["relationship_filters"],
            super::relationship_filter_parameters()
        );
        let update = definitions()
            .into_iter()
            .find(|definition| definition.function.name == "update_saved_search")
            .unwrap();
        assert_eq!(
            update.function.parameters["properties"]["attributeFilters"],
            super::attribute_filter_parameters()
        );
        let saved = definitions()
            .into_iter()
            .find(|definition| definition.function.name == "create_saved_search")
            .unwrap();
        assert_eq!(
            saved.function.parameters["properties"]["attributeFilters"],
            super::attribute_filter_parameters()
        );
    }

    #[test]
    fn filter_inputs_reject_invalid_saved_values_and_intersect_results() {
        let valid: crate::model::SearchFilter = serde_json::from_value(json!({
            "field":"price", "operator":"gte", "value":100
        }))
        .unwrap();
        assert!(super::valid_saved_filter(&valid));
        let invalid: crate::model::SearchFilter = serde_json::from_value(json!({
            "field":"price", "operator":"unknown", "value":100
        }))
        .unwrap();
        assert!(!super::valid_saved_filter(&invalid));
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        assert_eq!(
            crate::search_filters::intersect_ids(Some(vec![first, second]), vec![second]),
            vec![second]
        );
        assert!(crate::search_filters::intersect_ids(Some(vec![first]), vec![second]).is_empty());
    }

    #[test]
    fn check_readiness_tools_are_entity_scoped_reads() {
        let definitions = definitions();
        for name in [
            "get_entity_publication_readiness",
            "get_entity_status_transitions",
        ] {
            assert_eq!(kind(name).unwrap(), ToolKind::Read);
            let definition = definitions
                .iter()
                .find(|tool| tool.function.name == name)
                .unwrap();
            assert_eq!(
                definition.function.parameters["required"],
                json!(["entity_id"])
            );
        }
    }

    #[test]
    fn check_failures_keep_the_api_error_code_and_violations() {
        use crate::repository::{CheckSource, CheckTransition, CheckViolation, RepositoryError};
        let violation = CheckViolation {
            source: CheckSource::TransitionCondition,
            code: "has-root-cause".into(),
            message: "Record the root cause".into(),
            contexts: vec!["default".into()],
            attributes: vec!["root_cause".into()],
            severity: None,
            transition: Some(CheckTransition {
                attribute_code: "status".into(),
                from: Some("open".into()),
                to: Some("closed".into()),
            }),
            evidence: json!({"attribute_code": "root_cause"}),
        };
        let payload = super::tool_error_payload(&ToolError::Repository(
            RepositoryError::TransitionConditionsUnmet(vec![violation.clone()]),
        ));
        assert_eq!(payload["code"], "tool_error");
        assert_eq!(payload["error_code"], "transition_conditions_unmet");
        assert_eq!(
            payload["details"]["violations"][0]["attributes"],
            json!(["root_cause"])
        );
        assert_eq!(
            payload["details"]["violations"][0]["transition"]["to"],
            "closed"
        );

        let payload = super::tool_error_payload(&ToolError::Repository(
            RepositoryError::PublicationChecksFailed {
                context: "web".into(),
                violations: vec![violation.clone()],
            },
        ));
        assert_eq!(payload["error_code"], "publication_checks_failed");
        assert_eq!(payload["details"]["context"], "web");

        let payload = super::tool_error_payload(&ToolError::Repository(
            RepositoryError::RuleHasExistingViolations(3),
        ));
        assert_eq!(payload["details"]["existing_violations"], 3);

        // Large evidence is dropped so the result stays bounded.
        let mut large = violation;
        large.evidence = json!({"ids": "x".repeat(MAX_TOOL_RESULT_BYTES)});
        let payload = super::tool_error_payload(&ToolError::Repository(
            RepositoryError::EntityCheckFailed(vec![large]),
        ));
        assert!(
            payload["details"]["violations"][0]
                .get("evidence")
                .is_none()
        );

        let payload = super::tool_error_payload(&ToolError::Forbidden);
        assert!(payload.get("error_code").is_none());
    }

    #[test]
    fn authoring_guide_fits_the_tool_result_bound() {
        let guide = json!({
            "blueprints_markdown": super::BLUEPRINT_AUTHORING_GUIDE,
            "views_markdown": super::VIEW_CONFIGURATION_GUIDE,
            "json_schema_markdown": super::JSON_SCHEMA_GUIDE,
        });
        assert!(bounded(guide).is_ok());
        assert!(super::JSON_SCHEMA_GUIDE.contains("x-attricat-checks"));
        assert!(super::BLUEPRINT_AUTHORING_GUIDE.contains("rules.enforcement"));
    }

    #[test]
    fn validates_change_summaries_and_bounds_results() {
        assert_eq!(
            change_summary("delete_entity", &json!({"entity_id":"abc"})).unwrap(),
            "Delete entity abc."
        );
        assert_eq!(
            change_summary("set_entity_values", &json!({"entity_id":"abc"})).unwrap(),
            "Set attribute values on entity abc."
        );
        assert_eq!(
            change_summary("migrate_entity", &json!({"entity_id":"abc"})).unwrap(),
            "Upgrade entity abc to its latest published blueprint revision."
        );
        assert!(matches!(
            change_summary("create_context", &json!({})),
            Err(ToolError::InvalidArguments(_))
        ));
        assert!(matches!(
            bounded(json!({"result":"x".repeat(MAX_TOOL_RESULT_BYTES)})),
            Err(ToolError::ResultTooLarge)
        ));
    }
}
