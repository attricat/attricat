use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct Workspace {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub bootstrap_owner_email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct Attribute {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub code: String,
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
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct Entity {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub projections: Value,
    pub system_tags: Vec<String>,
    pub system_metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct AttributeContext {
    pub id: Uuid,
    pub code: String,
    pub data: Value,
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
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

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct EntityAuditChange {
    pub audit_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor_user_id: Option<Uuid>,
    pub actor_display_name: Option<String>,
    pub actor_email: Option<String>,
    pub executor_type: String,
    pub agent_run_id: Option<Uuid>,
    pub approval_decision: Option<String>,
    pub approved_by_user_id: Option<Uuid>,
    pub approved_by_display_name: Option<String>,
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

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct EntityPreview {
    pub id: Uuid,
    pub blueprint_version: i64,
    pub schema_outdated: bool,
    #[serde(skip_serializing)]
    pub created_at: DateTime<Utc>,
    pub preview: Value,
    pub display: Value,
    /// Scalar table values keyed by their configured local or relationship path.
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchEntitiesRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub filters: Vec<SearchFilter>,
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
pub struct SearchSort {
    pub field: String,
    pub direction: String,
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
        files: Vec<crate::repository::FileMetadata>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityFormResponse {
    pub entity: Entity,
    pub blueprint: BlueprintWithAttributes,
    pub values: Vec<FormAttributeValue>,
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
}

#[derive(Clone, Debug, Serialize)]
pub struct MigrationIssue {
    pub attribute_code: Option<String>,
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityMigrationPreview {
    pub migration_id: Uuid,
    pub source_version: i64,
    pub target: BlueprintWithAttributes,
    pub values: Vec<FormAttributeValue>,
    pub status: String,
    pub issues: Vec<MigrationIssue>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct BlueprintMigrationBatch {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub target_version: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct DataHealthSummary {
    pub active_entities: i64,
    pub entity_blueprints: i64,
    pub contexts: i64,
    pub outdated_entities: i64,
    pub stale_entities: i64,
    pub deleted_relationship_targets: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
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

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct FreshnessBand {
    pub label: String,
    pub entities: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ContextHealth {
    pub code: String,
    pub direct_entities: i64,
    pub direct_values: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct RelationshipHealth {
    pub attribute_code: String,
    pub source_blueprint: String,
    pub active_edges: i64,
    pub deleted_targets: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct StorageHealth {
    pub table: String,
    pub bytes: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct CompletenessHealth {
    pub code: String,
    pub name: String,
    pub current_version: i64,
    pub active_entities: i64,
    pub outdated_entities: i64,
    pub default_complete_entities: i64,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkflow {
    pub definition: String,
}
