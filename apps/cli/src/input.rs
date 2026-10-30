//! JSON, YAML and standard-input arguments converted to API bodies.

use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::SourceInput;
use crate::error::CliError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextFile {
    pub(crate) code: String,
    pub(crate) data: Value,
    pub(crate) parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ValueFile {
    pub(crate) values: Vec<ValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ValueInput {
    pub(crate) kind: String,
    pub(crate) attribute_id: Option<Uuid>,
    pub(crate) attribute_code: Option<String>,
    pub(crate) context_id: Option<Uuid>,
    pub(crate) value: Option<Value>,
    pub(crate) target_record_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RelationshipFile {
    pub(crate) relationships: Vec<RelationshipInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveValuesFile {
    pub(crate) remove_values: Vec<RemoveValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveValueInput {
    pub(crate) attribute_code: String,
    pub(crate) context_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RelationshipInput {
    pub(crate) attribute_id: Option<Uuid>,
    pub(crate) attribute_code: Option<String>,
    pub(crate) context_id: Option<Uuid>,
    pub(crate) target_record_ids: Vec<Uuid>,
}

pub(crate) fn json_input(input: &str, label: &str) -> Result<Value, CliError> {
    let source = match fs::read_to_string(input) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => input.to_owned(),
        Err(error) => {
            return Err(CliError::Input(format!(
                "cannot read {label} file: {error}"
            )));
        }
    };
    serde_json::from_str(&source).map_err(|error| {
        CliError::Input(format!(
            "{label} must be JSON or a readable JSON file: {error}"
        ))
    })
}

pub(crate) fn json_array_input(input: &str, label: &str) -> Result<Value, CliError> {
    let value = json_input(input, label)?;
    if value.is_array() {
        Ok(value)
    } else {
        Err(CliError::Input(format!("{label} must be a JSON array")))
    }
}

pub(crate) fn json_array_argument(input: Option<&str>, label: &str) -> Result<Value, CliError> {
    input
        .map(|value| json_array_input(value, label))
        .transpose()
        .map(|value| value.unwrap_or_else(|| json!([])))
}

/// Reads exactly one non-empty standard-input line per label, in order.
pub(crate) fn read_stdin_lines<const N: usize>(labels: [&str; N]) -> Result<[String; N], CliError> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| CliError::Input(error.to_string()))?;
    parse_stdin_lines(&input, labels)
}

pub(crate) fn parse_stdin_lines<const N: usize>(
    input: &str,
    labels: [&str; N],
) -> Result<[String; N], CliError> {
    let mut lines = input.lines();
    let values = labels.map(|label| {
        lines
            .next()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or(label)
    });
    if lines.next().is_some() {
        return Err(CliError::Input(format!(
            "stdin must contain exactly {N} lines"
        )));
    }
    let mut output = Vec::with_capacity(N);
    for (position, value) in values.into_iter().enumerate() {
        output.push(value.map_err(|label| {
            CliError::Input(format!("{label} must be stdin line {}", position + 1))
        })?);
    }
    Ok(output.try_into().expect("one value per label"))
}

pub(crate) fn read_secret_stdin(label: &str) -> Result<String, CliError> {
    let mut secret = String::new();
    io::stdin()
        .read_to_string(&mut secret)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let secret = secret.trim_end_matches(['\r', '\n']);
    if secret.is_empty() {
        return Err(CliError::Input(format!("{label} must not be empty")));
    }
    Ok(secret.to_owned())
}

pub(crate) fn read_source(input: SourceInput) -> Result<String, CliError> {
    match (input.file, input.stdin) {
        (Some(file), false) => {
            fs::read_to_string(file).map_err(|error| CliError::Input(error.to_string()))
        }
        (None, true) => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| CliError::Input(error.to_string()))?;
            Ok(source)
        }
        _ => Err(CliError::Input(
            "provide exactly one of --file or --stdin".to_owned(),
        )),
    }
}

pub(crate) fn context_body_from_file(path: &PathBuf) -> Result<Value, CliError> {
    let input: ContextFile = parse_yaml_file(path)?;
    Ok(json!({
        "code": input.code,
        "data": input.data,
        "parent_id": input.parent_id,
    }))
}

pub(crate) fn values_body_from_file(
    path: &PathBuf,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: ValueFile = parse_yaml_file(path)?;
    values_body(input, context_id)
}

pub(crate) fn values_body(
    input: ValueFile,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let values = input
        .values
        .into_iter()
        .map(|value| {
            let (attribute_key, attribute_value) = match (value.attribute_id, value.attribute_code)
            {
                (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                _ => {
                    return Err(CliError::Input(
                        "provide exactly one of attribute_id or attribute_code".to_owned(),
                    ));
                }
            };

            match value.kind.as_str() {
                "scalar" => {
                    if value.target_record_id.is_some() {
                        return Err(CliError::Input(
                            "scalar values must not include target_record_id".to_owned(),
                        ));
                    }
                    let payload = value
                        .value
                        .ok_or_else(|| CliError::Input("scalar values require value".to_owned()))?;
                    let mut output = json!({
                        "kind": "scalar",
                        "context_id": value.context_id.or(default_context_id),
                        "value": payload,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                "relationship" => {
                    if value.value.is_some() {
                        return Err(CliError::Input(
                            "relationship values must not include value".to_owned(),
                        ));
                    }
                    let target_record_id = value.target_record_id.ok_or_else(|| {
                        CliError::Input("relationship values require target_record_id".to_owned())
                    })?;
                    let mut output = json!({
                        "kind": "relationship",
                        "context_id": value.context_id.or(default_context_id),
                        "target_record_id": target_record_id,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                _ => Err(CliError::Input(
                    "value kind must be scalar or relationship".to_owned(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "values": values }))
}

pub(crate) fn relationships_body_from_file(
    path: &PathBuf,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RelationshipFile = parse_yaml_file(path)?;
    relationships_body(input, context_id)
}

pub(crate) fn relationships_body(
    input: RelationshipFile,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let relationships = input
        .relationships
        .into_iter()
        .map(|relationship| {
            let (attribute_key, attribute_value) =
                match (relationship.attribute_id, relationship.attribute_code) {
                    (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                    (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                    _ => {
                        return Err(CliError::Input(
                            "provide exactly one of attribute_id or attribute_code".to_owned(),
                        ));
                    }
                };
            let mut output = json!({
                "context_id": relationship.context_id.or(default_context_id),
                "target_record_ids": relationship.target_record_ids,
            });
            output[attribute_key] = attribute_value;
            Ok(output)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "relationships": relationships }))
}

pub(crate) fn json_object_input(input: &str, label: &str) -> Result<Value, CliError> {
    let value = json_input(input, label)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(CliError::Input(format!("{label} must be a JSON object")))
    }
}

pub(crate) fn json_string_array_input(input: &str, label: &str) -> Result<Value, CliError> {
    let value = json_input(input, label)?;
    if value
        .as_array()
        .is_some_and(|items| items.iter().all(Value::is_string))
    {
        Ok(value)
    } else {
        Err(CliError::Input(format!(
            "{label} must be a JSON array of strings"
        )))
    }
}

pub(crate) fn form_update_body(
    values_path: Option<&PathBuf>,
    relationships_path: Option<&PathBuf>,
    remove_values_path: Option<&PathBuf>,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let values = match values_path {
        Some(path) => values_body_from_file(path, context_id)?["values"].clone(),
        None => Value::Array(Vec::new()),
    };
    if values
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["kind"] == "relationship"))
    {
        return Err(CliError::Input(
            "--values accepts scalar values only; use --relationships for relationship sets"
                .to_owned(),
        ));
    }
    let relationships = match relationships_path {
        Some(path) => relationships_body_from_file(path, context_id)?["relationships"].clone(),
        None => Value::Array(Vec::new()),
    };
    let remove_values = match remove_values_path {
        Some(path) => remove_values_body_from_file(path, context_id)?,
        None => Value::Array(Vec::new()),
    };
    Ok(json!({ "values": values, "relationships": relationships, "remove_values": remove_values }))
}

pub(crate) fn remove_values_body_from_file(
    path: &PathBuf,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RemoveValuesFile = parse_yaml_file(path)?;
    Ok(Value::Array(
        input
            .remove_values
            .into_iter()
            .map(|value| {
                json!({
                    "attribute_code": value.attribute_code,
                    "context_id": value.context_id.or(default_context_id),
                })
            })
            .collect(),
    ))
}

pub(crate) fn parse_yaml_file<T: DeserializeOwned>(path: &PathBuf) -> Result<T, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    attricat_yaml::from_str(&source)
        .map_err(|error| CliError::Input(format!("invalid YAML: {error}")))
}
