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
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
pub struct Entity {
    pub id: Uuid,
    pub code: String,
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
pub struct CreateEntity {
    pub code: String,
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

#[derive(Clone, Debug, Serialize)]
pub struct BlueprintWithAttributes {
    pub blueprint: Blueprint,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct EntityPreview {
    pub id: Uuid,
    pub code: String,
    pub preview: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityPreviewPage {
    pub items: Vec<EntityPreview>,
    pub next_cursor: Option<Uuid>,
}
