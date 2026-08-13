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
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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
