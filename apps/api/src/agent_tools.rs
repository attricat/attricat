//! Typed, repository-backed catalogue tools for agent runs.
//!
//! Tools never call the Catalog HTTP API: this keeps authorization and audit
//! context in the repository boundary and avoids granting the model a network
//! capability.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::repository::{CatalogRepository, RepositoryError};

pub const MAX_TOOL_RESULT_BYTES: usize = 64 * 1024;

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
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        definition(
            "list_blueprints",
            "List the catalogue's current blueprints.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "list_contexts",
            "List attribute contexts.",
            json!({"type":"object","additionalProperties":false}),
        ),
        definition(
            "get_entity",
            "Get one entity by UUID.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "create_entity",
            "Create an entity. This change requires approval.",
            json!({"type":"object","required":["blueprint"],"properties":{"blueprint":{"type":"object"},"values":{"type":"array"},"system_tags":{"type":"array","items":{"type":"string"}},"system_metadata":{"type":"object"}},"additionalProperties":false}),
        ),
        definition(
            "delete_entity",
            "Delete an entity. This change requires approval.",
            json!({"type":"object","required":["entity_id"],"properties":{"entity_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
        definition(
            "create_context",
            "Create an attribute context. This change requires approval.",
            json!({"type":"object","required":["code","data"],"properties":{"code":{"type":"string"},"data":{"type":"object"},"parent_id":{"type":"string","format":"uuid"}},"additionalProperties":false}),
        ),
    ]
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
        "list_blueprints" | "list_contexts" | "get_entity" => Ok(ToolKind::Read),
        "create_entity" | "delete_entity" | "create_context" => Ok(ToolKind::Mutation),
        _ => Err(ToolError::UnknownTool(name.to_owned())),
    }
}

/// A concise, durable explanation shown to the approver before any mutation
/// reaches a repository method.
pub fn change_summary(name: &str, arguments: &Value) -> Result<String, ToolError> {
    match name {
        "create_entity" => Ok(format!(
            "Create an entity of blueprint {}.",
            required_string(arguments, "blueprint.code")?
        )),
        "delete_entity" => Ok(format!(
            "Delete entity {}.",
            required_string(arguments, "entity_id")?
        )),
        "create_context" => Ok(format!(
            "Create attribute context '{}'.",
            required_string(arguments, "code")?
        )),
        _ => Err(ToolError::UnknownTool(name.to_owned())),
    }
}

pub async fn execute_read(
    repository: &CatalogRepository,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    let result = match name {
        "list_blueprints" => {
            serde_json::to_value(repository.list_blueprints().await?).expect("models serialize")
        }
        "list_contexts" => {
            serde_json::to_value(repository.list_contexts().await?).expect("models serialize")
        }
        "get_entity" => {
            let id = parse_uuid(&arguments, "entity_id")?;
            serde_json::to_value(
                repository
                    .get_entity(id)
                    .await?
                    .ok_or(RepositoryError::NotFound("entity"))?,
            )
            .expect("models serialize")
        }
        _ => return Err(ToolError::UnknownTool(name.to_owned())),
    };
    bounded(result)
}

pub async fn execute_mutation(
    repository: &CatalogRepository,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    use crate::model::{CreateAttributeContext, SearchBlueprint};
    let result = match name {
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
            let blueprint = match input.blueprint.version {
                Some(version) => {
                    repository
                        .get_blueprint_by_code_and_version(&input.blueprint.code, version)
                        .await?
                }
                None => {
                    repository
                        .get_blueprint_by_code(&input.blueprint.code)
                        .await?
                }
            }
            .ok_or(RepositoryError::NotFound("blueprint"))?;
            serde_json::to_value(
                repository
                    .create_entity_with_values(
                        blueprint.blueprint.id,
                        blueprint.blueprint.version,
                        input.values,
                        input.system_tags,
                        input.system_metadata,
                    )
                    .await?,
            )
            .expect("models serialize")
        }
        "delete_entity" => {
            repository
                .delete_entity(parse_uuid(&arguments, "entity_id")?)
                .await?;
            json!({"deleted": true})
        }
        "create_context" => {
            let input: CreateAttributeContext = decode(arguments)?;
            serde_json::to_value(repository.create_context(input).await?).expect("models serialize")
        }
        _ => return Err(ToolError::UnknownTool(name.to_owned())),
    };
    bounded(result)
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
