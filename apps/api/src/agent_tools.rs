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
const BLUEPRINT_AUTHORING_GUIDE: &str = include_str!("../../../docs/blueprints.md");
const VIEW_CONFIGURATION_GUIDE: &str = include_str!("../../../docs/views.md");
const JSON_SCHEMA_GUIDE: &str = include_str!("../../../docs/json-schema-validation.md");

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
            "link_file",
            "Attach an existing workspace file to an entity file attribute. Conversation attachments include their file IDs. This change requires approval.",
            json!({"type":"object","required":["entity_id","attribute_code","file_id"],"properties":{"entity_id":{"type":"string","format":"uuid"},"attribute_code":{"type":"string"},"file_id":{"type":"string","format":"uuid"},"context_id":{"type":["string","null"],"format":"uuid"}},"additionalProperties":false}),
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
        "blueprint_authoring_guide" | "list_blueprints" | "list_contexts" | "get_entity" => {
            Ok(ToolKind::Read)
        }
        "create_blueprint"
        | "create_blueprint_revision"
        | "publish_blueprint"
        | "create_entity"
        | "delete_entity"
        | "link_file"
        | "create_context" => Ok(ToolKind::Mutation),
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
        _ => Err(ToolError::UnknownTool(name.to_owned())),
    }
}

pub async fn execute_read(
    repository: &CatalogRepository,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    let result = match name {
        "blueprint_authoring_guide" => json!({
            "blueprints_markdown": BLUEPRINT_AUTHORING_GUIDE,
            "views_markdown": VIEW_CONFIGURATION_GUIDE,
            "json_schema_markdown": JSON_SCHEMA_GUIDE,
        }),
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
                repository
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
                repository
                    .publish_blueprint_revision(input.blueprint_id, input.version)
                    .await?,
            )
            .expect("models serialize")
        }
        "create_blueprint" => {
            let input: CreateBlueprint = decode(arguments)?;
            serde_json::to_value(repository.create_blueprint(input).await?)
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
                repository
                    .link_file_to_attribute(
                        input.entity_id,
                        &input.attribute_code,
                        input.context_id,
                        input.file_id,
                    )
                    .await?,
            )
            .expect("file metadata serializes")
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ToolError, ToolKind, bounded, change_summary, kind};

    #[test]
    fn classifies_every_write_as_an_approval_required_mutation() {
        assert_eq!(kind("list_blueprints").unwrap(), ToolKind::Read);
        for name in ["create_entity", "delete_entity", "create_context"] {
            assert_eq!(kind(name).unwrap(), ToolKind::Mutation);
        }
        assert!(matches!(kind("fetch_url"), Err(ToolError::UnknownTool(_))));
    }

    #[test]
    fn validates_change_summaries_and_bounds_results() {
        assert_eq!(
            change_summary("delete_entity", &json!({"entity_id":"abc"})).unwrap(),
            "Delete entity abc."
        );
        assert!(matches!(
            change_summary("create_context", &json!({})),
            Err(ToolError::InvalidArguments(_))
        ));
        assert!(matches!(
            bounded(json!({"result":"x".repeat(super::MAX_TOOL_RESULT_BYTES)})),
            Err(ToolError::ResultTooLarge)
        ));
    }
}
