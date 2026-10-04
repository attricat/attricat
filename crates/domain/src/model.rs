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
    pub entity_schema: Option<Value>,
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
    /// Every allowed target blueprint; empty means any entity blueprint.
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
pub struct EntityReusableAttribute {
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
pub struct Entity {
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
    pub entity_id: Uuid,
    pub attribute_id: Uuid,
    pub value: Value,
    pub relationship_target_entity_id: Option<Uuid>,
    pub active: bool,
    pub context_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttributeValueHistory {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub attribute_id: Uuid,
    pub value: Value,
    pub relationship_target_entity_id: Option<Uuid>,
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
    /// Whether the entity schema and blueprint checks must pass in this context.
    pub require_valid_entity: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EntityPublicationStatus {
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
pub struct BlueprintEntityPublicationSummary {
    pub entity_count: i64,
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
    pub require_valid_entity: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EntityAuditChange {
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
        target_entity_id: Uuid,
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
    pub target_entity_ids: Vec<Uuid>,
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
    pub source_entity_id: Uuid,
    pub attribute_code: String,
    pub target_entity_id: Uuid,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchExplanation {
    pub term: String,
    pub matching_entity_id: Uuid,
    pub matching_attribute_code: Option<String>,
    pub traversal_depth: u8,
    pub relationship_path: Vec<MatchPathEdge>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityPreview {
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
    pub related: HashMap<String, Vec<RelatedEntityPreview>>,
    #[serde(default)]
    pub match_explanations: Vec<MatchExplanation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RelatedEntityPreview {
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
pub struct EntityPreviewPage {
    pub items: Vec<EntityPreview>,
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

#[derive(Clone, Debug, Serialize)]
pub struct IncomingRelationshipItem {
    pub id: Uuid,
    pub blueprint_code: String,
    pub blueprint_version: i64,
    pub is_sample: bool,
    pub display: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct IncomingRelationshipsPage {
    pub items: Vec<IncomingRelationshipItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityPreviewResponse {
    pub entity: EntityIdentity,
    pub context: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedEntityPreviewResponse {
    pub entity: EntityIdentity,
    pub requested_context: AttributeContext,
    pub values: Value,
    /// Attached definitions are deliberately a second namespace rather than
    /// fields injected into blueprint-controlled layouts.
    pub reusable_attributes: Vec<EntityReusableAttribute>,
    pub reusable_values: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityHierarchyItem {
    pub id: Uuid,
    pub display: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityHierarchyResponse {
    pub items: Vec<EntityHierarchyItem>,
    pub paths: Vec<Vec<EntityHierarchyItem>>,
    pub truncated: bool,
    pub multiple_parents: bool,
    pub cycle_detected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityIdentity {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub is_sample: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchEntitiesRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub filters: Vec<SearchFilter>,
    /// Filters source entities by targets reached through one to three relationship hops.
    #[serde(default)]
    pub relationship_filters: Vec<RelationshipFilter>,
    /// All requested tags must be present. System tags are outside blueprint data.
    #[serde(default)]
    pub system_tags: Vec<String>,
    /// Restrict results to entities pinned to an older published blueprint revision.
    #[serde(default)]
    pub outdated: bool,
    #[serde(default)]
    pub relationship_tree_facets: Vec<RelationshipTreeFacetRequest>,
    #[serde(default)]
    pub sort: Option<SearchSort>,
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
    pub selected_items: Vec<EntityHierarchyItem>,
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
pub struct EntitySearchResponse {
    pub blueprint: BlueprintWithAttributes,
    pub items: Vec<EntityPreview>,
    pub next_cursor: Option<String>,
    pub total_count: Option<i64>,
    pub total_count_capped: bool,
    pub result_version_scope: SearchResultVersionScope,
    pub hidden_outdated_count: Option<i64>,
    pub hidden_outdated_count_capped: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateEntityFormRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub values: Vec<NewAttributeValue>,
    #[serde(default)]
    pub system_tags: Vec<String>,
    #[serde(default = "empty_json_object")]
    pub system_metadata: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateEntityFormRequest {
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

/// Operations an entity batch may hold.
pub const MAX_ENTITY_BATCH_OPERATIONS: usize = 50;
/// Scalar values, relationship targets and removals across one entity batch.
pub const MAX_ENTITY_BATCH_VALUES: usize = 1000;

/// Writes, status transitions and deletions applied to several entities in
/// one transaction: every operation commits, or none does.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityBatchRequest {
    pub operations: Vec<EntityBatchOperation>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntityBatchOperation {
    /// Creates an entity. A caller-chosen `entity_id` lets later operations
    /// in the same batch link to it.
    Create {
        entity_id: Option<Uuid>,
        blueprint: SearchBlueprint,
        #[serde(default)]
        values: Vec<NewAttributeValue>,
        #[serde(default)]
        system_tags: Vec<String>,
        #[serde(default = "empty_json_object")]
        system_metadata: Value,
    },
    /// Updates an entity like `PUT /v1/entities/{id}`.
    Update {
        entity_id: Uuid,
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
    /// Deletes an entity.
    Delete {
        entity_id: Uuid,
        expected_updated_at: Option<DateTime<Utc>>,
    },
}

impl EntityBatchOperation {
    /// The entity the operation writes, when known before it runs.
    pub fn entity_id(&self) -> Option<Uuid> {
        match self {
            Self::Create { entity_id, .. } => *entity_id,
            Self::Update { entity_id, .. } | Self::Delete { entity_id, .. } => Some(*entity_id),
        }
    }

    /// The permission the operation needs and the entity it is checked on.
    pub fn permission(&self) -> (&'static str, Option<Uuid>) {
        match self {
            Self::Create { .. } => ("entities.write", None),
            Self::Update { entity_id, .. } => ("entities.write", Some(*entity_id)),
            Self::Delete { entity_id, .. } => ("entities.delete", Some(*entity_id)),
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
                        .map(|relationship| relationship.target_entity_ids.len().max(1))
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
pub struct EntityBatchResponse {
    pub operations: Vec<EntityBatchOperationResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EntityBatchOperationResult {
    Create { entity: Entity },
    Update { entity: Entity },
    Delete { entity_id: Uuid },
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
        target_entity_id: Uuid,
    },
    File {
        attribute_code: String,
        context_id: Option<Uuid>,
        files: Vec<FileMetadata>,
    },
}

/// Public metadata for a file attached to an entity attribute.
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
pub struct EntityFormResponse {
    pub can_write: bool,
    pub entity: Entity,
    pub blueprint: BlueprintWithAttributes,
    pub values: Vec<FormAttributeValue>,
    /// Entity-owned, namespace-qualified definitions and their values are kept
    /// separate from blueprint fields so callers cannot accidentally merge the
    /// two namespaces.
    pub reusable_attributes: Vec<EntityReusableAttribute>,
    pub reusable_values: Vec<FormAttributeValue>,
    pub context: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrateEntityRequest {
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
    pub eligible_entities: i64,
    pub removed_attribute_codes: Vec<String>,
    pub entities_with_removed_values: i64,
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
pub struct EntityMigrationPreview {
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
    pub total_entities: i64,
    pub processed_entities: i64,
    pub migrated_entities: i64,
    pub needs_input_entities: i64,
    pub failed_entities: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DataHealthSummary {
    pub active_entities: i64,
    pub entity_blueprints: i64,
    pub contexts: i64,
    pub outdated_entities: i64,
    pub stale_entities: i64,
    pub deleted_relationship_targets: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintHealth {
    pub code: String,
    pub name: String,
    pub current_version: i64,
    pub active_entities: i64,
    pub outdated_entities: i64,
    pub stale_entities: i64,
    pub oldest_updated_at: Option<DateTime<Utc>>,
    pub newest_updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FreshnessBand {
    pub label: String,
    pub entities: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextHealth {
    pub code: String,
    pub direct_entities: i64,
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
    pub active_entities: i64,
    pub outdated_entities: i64,
    pub default_complete_entities: i64,
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

/// Bounded manual workflow input: the target is an existing entity and no caller payload is persisted.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateManualWorkflowRun {
    pub entity_id: Uuid,
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
    pub entity_id: Option<Uuid>,
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
    /// existing violations. Those entities cannot be saved until fixed.
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
    pub scope_entity_id: Option<Uuid>,
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
    pub entity_id: Uuid,
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
