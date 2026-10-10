use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Workspace {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub bootstrap_owner_email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Blueprint {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub version: i64,
    pub includes: Value,
    pub views: Value,
    pub record_schema: Option<Value>,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub definition: String,
    pub definition_hash: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Attribute {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub code: String,
    /// Human-readable label; clients fall back to the humanized code.
    pub name: Option<String>,
    pub value_type: String,
    pub value_schema: Option<Value>,
    /// Immutable provider/type/release metadata for extension-defined types.
    pub extension_type: Option<Value>,
    pub default_value: Option<Value>,
    pub file_policy: Option<Value>,
    /// The single allowed target blueprint, if exactly one is allowed.
    pub target_blueprint_code: Option<String>,
    /// Every allowed target blueprint; empty means any record blueprint.
    pub target_blueprint_codes: Vec<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    /// `acyclic` or `tree` for self-referencing relationships.
    pub hierarchy: Option<String>,
    pub tags: Value,
    pub context_fallback: String,
    pub context_editable: String,
    pub readonly: bool,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReusableAttribute {
    pub id: Uuid,
    pub definition_id: Uuid,
    pub namespace: String,
    pub code: String,
    pub name: String,
    pub version: i64,
    pub value_type: String,
    pub value_schema: Option<Value>,
    pub default_value: Option<Value>,
    pub file_policy: Option<Value>,
    pub target_blueprint_code: Option<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    pub tags: Value,
    pub context_fallback: String,
    pub context_editable: String,
    pub readonly: bool,
    pub searchable: bool,
    pub facetable: bool,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReusableAttribute {
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReusableAttributeGroup {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub position: i64,
    pub reusable_attribute_revision_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachReusableAttribute {
    pub reusable_attribute_revision_id: Uuid,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReusableAttributeGroup {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub position: i64,
    #[serde(default)]
    pub reusable_attribute_revision_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordReusableAttribute {
    pub attachment_id: Uuid,
    pub attribute_id: Uuid,
    pub definition_id: Uuid,
    pub revision_id: Uuid,
    pub namespace: String,
    pub code: String,
    pub name: String,
    pub version: i64,
    pub value_type: String,
    pub value_schema: Option<Value>,
    pub default_value: Option<Value>,
    pub file_policy: Option<Value>,
    pub target_blueprint_code: Option<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    pub tags: Value,
    pub context_fallback: String,
    pub context_editable: String,
    pub readonly: bool,
    pub searchable: bool,
    pub facetable: bool,
    pub position: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Record {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub projections: Value,
    pub system_tags: Vec<String>,
    pub system_metadata: Value,
    pub is_sample: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AttributeContext {
    pub id: Uuid,
    pub code: String,
    pub data: Value,
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AttributeValue {
    pub id: Uuid,
    pub record_id: Uuid,
    pub attribute_id: Uuid,
    pub value: Value,
    pub relationship_target_record_id: Option<Uuid>,
    pub active: bool,
    pub context_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttributeValueHistory {
    pub id: Uuid,
    pub record_id: Uuid,
    pub attribute_id: Uuid,
    pub value: Value,
    pub relationship_target_record_id: Option<Uuid>,
    pub active: bool,
    pub context_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub archived_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicationChannel {
    pub context_id: Uuid,
    pub context_code: String,
    pub enabled: bool,
    /// Codes of enabled rules that must pass before publication.
    pub required_rule_codes: Vec<String>,
    /// Whether the record schema and blueprint checks must pass in this context.
    pub require_valid_record: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordPublicationStatus {
    pub context_id: Uuid,
    pub context_code: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by_user_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationContextRequest {
    pub context_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlueprintRecordPublicationSummary {
    pub record_count: i64,
    pub channel_count: i64,
    pub publication_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePublicationChannel {
    pub enabled: bool,
    /// Replaces the required rule codes. Omit to keep them.
    #[serde(default)]
    pub required_rule_codes: Option<Vec<String>>,
    /// Omit to keep the current setting.
    #[serde(default)]
    pub require_valid_record: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordAuditChange {
    pub audit_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor_user_id: Option<Uuid>,
    pub actor_display_name: Option<String>,
    pub actor_email: Option<String>,
    /// The actor's ready avatar in this workspace.
    #[serde(default)]
    pub actor_avatar_file_id: Option<Uuid>,
    pub executor_type: String,
    pub agent_run_id: Option<Uuid>,
    pub approval_decision: Option<String>,
    pub approved_by_user_id: Option<Uuid>,
    pub approved_by_display_name: Option<String>,
    #[serde(default)]
    pub approved_by_avatar_file_id: Option<Uuid>,
    pub attribute_id: Uuid,
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub context_code: Option<String>,
    pub change_kind: String,
    pub before_value: Option<Value>,
    pub after_value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBlueprint {
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAttributeContext {
    pub code: String,
    pub data: Value,
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAttributeContext {
    pub parent_id: Uuid,
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NewAttributeValue {
    Scalar {
        attribute_id: Option<Uuid>,
        attribute_code: Option<String>,
        context_id: Option<Uuid>,
        value: Value,
    },
    Relationship {
        attribute_id: Option<Uuid>,
        attribute_code: Option<String>,
        context_id: Option<Uuid>,
        target_record_id: Uuid,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppendAttributeValues {
    pub expected_updated_at: Option<DateTime<Utc>>,
    pub values: Vec<NewAttributeValue>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipMutation {
    pub relationships: Vec<RelationshipTargets>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipTargets {
    pub attribute_id: Option<Uuid>,
    pub attribute_code: Option<String>,
    pub context_id: Option<Uuid>,
    pub target_record_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeValueSelector {
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintWithAttributes {
    pub blueprint: Blueprint,
    pub attributes: Vec<Attribute>,
    #[serde(default)]
    pub table_path_attributes: Vec<TablePathAttribute>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TablePathAttribute {
    pub code: String,
    pub value_type: String,
    pub sortable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchPathEdge {
    pub source_record_id: Uuid,
    pub attribute_code: String,
    pub target_record_id: Uuid,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchExplanation {
    pub term: String,
    pub matching_record_id: Uuid,
    pub matching_attribute_code: Option<String>,
    pub traversal_depth: u8,
    pub relationship_path: Vec<MatchPathEdge>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordPreview {
    pub id: Uuid,
    pub blueprint_version: i64,
    pub schema_outdated: bool,
    pub is_sample: bool,
    #[serde(skip_serializing)]
    pub created_at: DateTime<Utc>,
    pub preview: Value,
    pub display: Value,
    /// Table values keyed by their configured local or relationship path. Direct file
    /// columns contain client-safe file metadata rather than projection values.
    #[serde(default)]
    pub table_values: HashMap<String, Vec<Value>>,
    /// Direct relationship targets required by the current table columns, keyed by relationship
    /// attribute code. Each target is loaded once for the page and carries scalar cache only.
    #[serde(default)]
    pub related: HashMap<String, Vec<RelatedRecordPreview>>,
    #[serde(default)]
    pub match_explanations: Vec<MatchExplanation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelatedRecordPreview {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub is_sample: bool,
    pub relationship_context_id: Uuid,
    pub relationship_context_code: String,
    pub display: Value,
    pub preview: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordPreviewPage {
    pub items: Vec<RecordPreview>,
    pub next_cursor: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingRelationshipSelector {
    pub source_blueprint: String,
    pub field: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingRelationshipsRequest {
    pub relationships: Vec<IncomingRelationshipSelector>,
    pub page: SearchPage,
}

/// A relationship field that currently links some live records to a target,
/// with how many distinct source records it links.
#[derive(Clone, Debug, Serialize)]
pub struct IncomingRelationshipField {
    pub source_blueprint: String,
    pub field: String,
    pub source_count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct IncomingRelationshipItem {
    pub id: Uuid,
    pub blueprint_code: String,
    pub blueprint_version: i64,
    pub is_sample: bool,
    pub display: Value,
}

/// Display labels of one record, per context, for naming it in other views.
#[derive(Clone, Debug, Serialize)]
pub struct RecordLabel {
    pub id: Uuid,
    pub blueprint_code: String,
    pub display: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct IncomingRelationshipsPage {
    pub items: Vec<IncomingRelationshipItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordPreviewResponse {
    pub record: RecordIdentity,
    pub context: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedRecordPreviewResponse {
    pub record: RecordIdentity,
    pub requested_context: AttributeContext,
    pub values: Value,
    /// Attached definitions are deliberately a second namespace rather than
    /// fields injected into blueprint-controlled layouts.
    pub reusable_attributes: Vec<RecordReusableAttribute>,
    pub reusable_values: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordHierarchyItem {
    pub id: Uuid,
    pub display: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordHierarchyResponse {
    pub items: Vec<RecordHierarchyItem>,
    pub paths: Vec<Vec<RecordHierarchyItem>>,
    pub truncated: bool,
    pub multiple_parents: bool,
    pub cycle_detected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordIdentity {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub is_sample: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRecordsRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub filters: Vec<SearchFilter>,
    /// Filters source records by targets reached through one to three relationship hops.
    #[serde(default)]
    pub relationship_filters: Vec<RelationshipFilter>,
    /// All requested tags must be present. System tags are outside blueprint data.
    #[serde(default)]
    pub system_tags: Vec<String>,
    /// Restrict results to records pinned to an older published blueprint revision.
    #[serde(default)]
    pub outdated: bool,
    #[serde(default)]
    pub relationship_tree_facets: Vec<RelationshipTreeFacetRequest>,
    #[serde(default)]
    pub sort: Option<SearchSort>,
    /// Context whose values filters, sorting and table values resolve, with each
    /// attribute's context fallback. Defaults to `default`.
    #[serde(default)]
    pub context_code: Option<String>,
    /// Include a bounded result count. Clients should request this only for the first page.
    #[serde(default)]
    pub include_total: bool,
    #[serde(default)]
    pub page: SearchPage,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipTreeFacetRequest {
    pub source_relationship_field: String,
    #[serde(default)]
    pub hierarchy_field: Option<String>,
    pub context_id: Uuid,
    #[serde(default)]
    pub selected_target_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelationshipTreeFacetItem {
    pub id: Uuid,
    pub parent_ids: Vec<Uuid>,
    pub display: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelationshipTreeFacetResponse {
    pub items: Vec<RelationshipTreeFacetItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipTreeFacetChildrenRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub query: Option<String>,
    pub source_relationship_field: String,
    #[serde(default)]
    pub hierarchy_field: Option<String>,
    pub context_id: Uuid,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub cursor: Option<Uuid>,
    #[serde(default)]
    pub selected_target_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelationshipTreeFacetChildItem {
    pub id: Uuid,
    pub display: String,
    pub count: i64,
    pub has_children: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelationshipTreeFacetChildrenResponse {
    pub items: Vec<RelationshipTreeFacetChildItem>,
    pub selected_items: Vec<RecordHierarchyItem>,
    pub next_cursor: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchBlueprint {
    pub code: String,
    pub version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchFilter {
    pub field: String,
    pub operator: String,
    pub value: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipFilter {
    pub field: String,
    #[serde(default)]
    pub selected_target_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSort {
    pub field: String,
    pub direction: String,
    /// Required when sorting publication status; identifies the selected channel.
    #[serde(default)]
    pub context_code: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchPage {
    pub size: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchResultVersionScope {
    Empty,
    Single { version: i64 },
    Multiple,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordSearchResponse {
    pub blueprint: BlueprintWithAttributes,
    pub items: Vec<RecordPreview>,
    pub next_cursor: Option<String>,
    pub total_count: Option<i64>,
    pub total_count_capped: bool,
    pub result_version_scope: SearchResultVersionScope,
    pub hidden_outdated_count: Option<i64>,
    pub hidden_outdated_count_capped: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRecordFormRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub values: Vec<NewAttributeValue>,
    /// Files staged for the blueprint's file attributes by the caller, which
    /// the create claims and links before validating the new record.
    #[serde(default)]
    pub files: Vec<NewFileAttributeValue>,
    #[serde(default)]
    pub system_tags: Vec<String>,
    #[serde(default = "empty_json_object")]
    pub system_metadata: Value,
}

/// The ordered files of one file attribute in one context (the default
/// context when omitted), written as a single value.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewFileAttributeValue {
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub file_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRecordFormRequest {
    pub expected_updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub values: Vec<NewAttributeValue>,
    #[serde(default)]
    pub relationships: Vec<RelationshipTargets>,
    #[serde(default)]
    pub remove_values: Vec<AttributeValueSelector>,
    /// Omitted fields retain their existing values; use [] or {} to clear.
    pub system_tags: Option<Vec<String>>,
    pub system_metadata: Option<Value>,
}

/// Operations a record batch may hold.
pub const MAX_RECORD_BATCH_OPERATIONS: usize = 50;
/// Scalar values, relationship targets and removals across one record batch.
pub const MAX_RECORD_BATCH_VALUES: usize = 1000;

/// Writes, status transitions and deletions applied to several records in
/// one transaction: every operation commits, or none does.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordBatchRequest {
    pub operations: Vec<RecordBatchOperation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordBatchOperation {
    /// Creates a record. A caller-chosen `record_id` lets later operations
    /// in the same batch link to it.
    Create {
        record_id: Option<Uuid>,
        blueprint: SearchBlueprint,
        #[serde(default)]
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        system_tags: Vec<String>,
        #[serde(default = "empty_json_object")]
        system_metadata: Value,
    },
    /// Updates a record like `PUT /v1/records/{id}`.
    Update {
        record_id: Uuid,
        expected_updated_at: Option<DateTime<Utc>>,
        #[serde(default)]
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        relationships: Vec<RelationshipTargets>,
        #[serde(default)]
        remove_values: Vec<AttributeValueSelector>,
        system_tags: Option<Vec<String>>,
        system_metadata: Option<Value>,
    },
    /// Deletes a record.
    Delete {
        record_id: Uuid,
        expected_updated_at: Option<DateTime<Utc>>,
    },
}

impl RecordBatchOperation {
    /// The record the operation writes, when known before it runs.
    pub fn record_id(&self) -> Option<Uuid> {
        match self {
            Self::Create { record_id, .. } => *record_id,
            Self::Update { record_id, .. } | Self::Delete { record_id, .. } => Some(*record_id),
        }
    }

    /// The permission the operation needs and the record it is checked on.
    pub fn permission(&self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::Create { .. } => ("records.write", None),
            Self::Update { record_id, .. } => ("records.write", Some(*record_id)),
            Self::Delete { record_id, .. } => ("records.delete", Some(*record_id)),
        }
    }

    pub fn value_count(&self) -> usize {
        match self {
            Self::Create { values, .. } => values.len(),
            Self::Update {
                values,
                relationships,
                remove_values,
                ..
            } => {
                values.len()
                    + remove_values.len()
                    + relationships
                        .iter()
                        .map(|relationship| relationship.target_record_ids.len().max(1))
                        .sum::<usize>()
            }
            Self::Delete { .. } => 0,
        }
    }

    pub fn writes_relationships(&self) -> bool {
        match self {
            Self::Create { values, .. } => values
                .iter()
                .any(|value| matches!(value, NewAttributeValue::Relationship { .. })),
            Self::Update {
                values,
                relationships,
                ..
            } => {
                !relationships.is_empty()
                    || values
                        .iter()
                        .any(|value| matches!(value, NewAttributeValue::Relationship { .. }))
            }
            Self::Delete { .. } => false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordBatchResponse {
    pub operations: Vec<RecordBatchOperationResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RecordBatchOperationResult {
    Create { record: Record },
    Update { record: Record },
    Delete { record_id: Uuid },
}

fn empty_json_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormAttributeValue {
    Scalar {
        attribute_code: String,
        context_id: Option<Uuid>,
        value: Value,
    },
    Relationship {
        attribute_code: String,
        context_id: Option<Uuid>,
        target_record_id: Uuid,
    },
    File {
        attribute_code: String,
        context_id: Option<Uuid>,
        files: Vec<FileMetadata>,
    },
}

/// Public metadata for a file attached to a record attribute.
#[derive(Clone, Debug, Serialize)]
pub struct FileMetadata {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub status: String,
    pub variants: Vec<FileVariantMetadata>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileVariantMetadata {
    pub kind: String,
    pub mime_type: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub byte_size: i64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordFormResponse {
    pub can_write: bool,
    pub record: Record,
    pub blueprint: BlueprintWithAttributes,
    pub values: Vec<FormAttributeValue>,
    /// Record-owned, namespace-qualified definitions and their values are kept
    /// separate from blueprint fields so callers cannot accidentally merge the
    /// two namespaces.
    pub reusable_attributes: Vec<RecordReusableAttribute>,
    pub reusable_values: Vec<FormAttributeValue>,
    pub context: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrateRecordRequest {
    pub migration_id: Uuid,
    pub expected_target_version: i64,
    #[serde(default)]
    pub values: Vec<NewAttributeValue>,
    #[serde(default)]
    pub relationships: Vec<RelationshipTargets>,
    #[serde(default)]
    pub discard_attributes: Vec<String>,
    /// Set only by the bulk worker after the batch policy has been approved.
    /// Never accepted from API clients; audit metadata is therefore trusted.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub removal_policy: Option<BlueprintMigrationRemovalPolicy>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintMigrationRemovalPolicy {
    pub disposition: String,
    pub attribute_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartBlueprintMigrationBatchRequest {
    #[serde(default)]
    pub removal_disposition: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintMigrationImpact {
    pub eligible_records: i64,
    pub removed_attribute_codes: Vec<String>,
    pub records_with_removed_values: i64,
    pub removed_values: i64,
    pub requires_removal_disposition: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct MigrationIssue {
    pub attribute_code: Option<String>,
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordMigrationPreview {
    pub source_updated_at: DateTime<Utc>,
    pub migration_id: Uuid,
    pub source_version: i64,
    pub target: BlueprintWithAttributes,
    pub values: Vec<FormAttributeValue>,
    pub status: String,
    pub issues: Vec<MigrationIssue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintMigrationBatch {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub target_version: i64,
    pub status: String,
    pub removal_policy: Value,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintMigrationBatchStatus {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub target_version: i64,
    pub status: String,
    pub removal_policy: Value,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_records: i64,
    pub processed_records: i64,
    pub migrated_records: i64,
    pub needs_input_records: i64,
    pub failed_records: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DataHealthSummary {
    pub active_records: i64,
    pub record_blueprints: i64,
    pub contexts: i64,
    pub outdated_records: i64,
    pub stale_records: i64,
    pub deleted_relationship_targets: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintHealth {
    pub code: String,
    pub name: String,
    pub current_version: i64,
    pub active_records: i64,
    pub outdated_records: i64,
    pub stale_records: i64,
    pub oldest_updated_at: Option<DateTime<Utc>>,
    pub newest_updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FreshnessBand {
    pub label: String,
    pub records: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextHealth {
    pub code: String,
    pub direct_records: i64,
    pub direct_values: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelationshipHealth {
    pub attribute_code: String,
    pub source_blueprint: String,
    pub active_edges: i64,
    pub deleted_targets: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorageHealth {
    pub table: String,
    pub bytes: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompletenessHealth {
    pub code: String,
    pub name: String,
    pub current_version: i64,
    pub active_records: i64,
    pub outdated_records: i64,
    pub default_complete_records: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Workflow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub definition: String,
    pub definition_hash: String,
    pub compiled_plan: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub enabled_version: Option<i64>,
    pub manual_enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkflow {
    pub definition: String,
}

/// Bounded manual workflow input: the target is an existing record and no caller payload is persisted.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateManualWorkflowRun {
    pub record_id: Uuid,
    /// Client-generated opaque key. Reusing it retries the same durable run.
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rule {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub definition: String,
    pub definition_hash: String,
    pub compiled_plan: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub enabled_version: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRule {
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateManualRuleRun {
    pub record_id: Option<Uuid>,
    pub dry_run: bool,
    pub idempotency_key: String,
    /// Published revision to dry-run before enabling it. Defaults to the
    /// enabled revision, or the latest published one for dry runs.
    #[serde(default)]
    pub version: Option<i64>,
}

/// Options for enabling a rule revision.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnableRule {
    /// Enable an enforcing rule although its completed dry run found
    /// existing violations. Those records cannot be saved until fixed.
    #[serde(default)]
    pub accept_existing_violations: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleRun {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub rule_version: i64,
    pub source: String,
    pub dry_run: bool,
    pub scope_record_id: Option<Uuid>,
    pub status: String,
    pub candidate_cursor: Option<Uuid>,
    pub candidates_evaluated: i64,
    pub findings_created: i64,
    pub findings_resolved: i64,
    pub attempts: i32,
    pub last_error: Option<String>,
    /// The run stopped at its candidate cap with candidates left unchecked.
    /// A truncated full dry run does not satisfy the enable gate on its own.
    pub truncated: bool,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuleFinding {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub rule_version: i64,
    pub record_id: Uuid,
    pub context_id: Option<Uuid>,
    pub severity: String,
    pub message: String,
    pub evidence: Value,
    pub state: String,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
