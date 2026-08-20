use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct Blueprint {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub version: i64,
    pub includes: Value,
    pub display: Value,
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
    pub target_blueprint_code: Option<String>,
    pub tags: Value,
    pub context_fallback: String,
    pub context_editable: String,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBlueprint {
    pub definition: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateEntity {
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    #[serde(default)]
    pub projections: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateAttributeContext {
    pub code: String,
    pub data: Value,
    #[serde(default = "default_context_id")]
    pub parent_id: Uuid,
}

fn default_context_id() -> Uuid {
    Uuid::from_u128(0x00000000000040008000000000000001)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAttributeContext {
    pub parent_id: Uuid,
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
pub struct AppendAttributeValues {
    pub values: Vec<NewAttributeValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelationshipMutation {
    pub relationships: Vec<RelationshipTargets>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationshipTargets {
    pub attribute_id: Option<Uuid>,
    pub attribute_code: Option<String>,
    pub context_id: Option<Uuid>,
    pub target_entity_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttributeValueSelector {
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintWithAttributes {
    pub blueprint: Blueprint,
    pub attributes: Vec<Attribute>,
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
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityPreviewPage {
    pub items: Vec<EntityPreview>,
    pub next_cursor: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityPreviewResponse {
    pub entity: EntityIdentity,
    pub context: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedEntityPreviewResponse {
    pub requested_context: AttributeContext,
    pub values: Value,
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
    #[serde(default)]
    pub page: SearchPage,
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

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchPage {
    pub size: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntitySearchResponse {
    pub blueprint: BlueprintWithAttributes,
    pub items: Vec<EntityPreview>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateEntityFormRequest {
    pub blueprint: SearchBlueprint,
    #[serde(default)]
    pub values: Vec<NewAttributeValue>,
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
