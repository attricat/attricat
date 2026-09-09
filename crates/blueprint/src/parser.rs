use std::collections::{HashMap, HashSet};

use catalog_validation::{is_valid_code, validate_json_schema_definition};
use serde::Deserialize;

use crate::{
    AttributeDeclaration, BlueprintDefinition, BlueprintError, BlueprintKind, FilePolicy,
    IncludeRef, LocalAttributeDeclaration, ViewDefinition,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlueprintDefinition {
    format_version: u32,
    code: String,
    name: String,
    kind: BlueprintKind,
    #[serde(default)]
    includes: Vec<IncludeRef>,
    #[serde(default)]
    views: HashMap<String, ViewDefinition>,
    entity_schema: Option<String>,
    /// Namespaced extension metadata is preserved in the immutable source
    /// definition and intentionally ignored by the core blueprint compiler.
    #[serde(default)]
    extensions: HashMap<String, toml::Value>,
    attributes: Vec<RawAttributeDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttributeDeclaration {
    code: String,
    value_type: Option<String>,
    value_schema: Option<String>,
    default_value: Option<serde_json::Value>,
    cardinality: Option<String>,
    ordered: Option<bool>,
    #[serde(default)]
    allowed_mime_groups: Vec<String>,
    #[serde(default)]
    allowed_extensions: Vec<String>,
    max_bytes: Option<u64>,
    #[serde(default)]
    purposes: Vec<String>,
    image_only: Option<bool>,
    target_blueprint: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_context_fallback")]
    context_fallback: String,
    #[serde(default = "default_context_editable")]
    context_editable: String,
    #[serde(default)]
    readonly: bool,
    from: Option<String>,
}

fn default_context_fallback() -> String {
    "default".to_owned()
}

fn default_context_editable() -> String {
    "all".to_owned()
}

pub fn parse(source: &str) -> Result<BlueprintDefinition, BlueprintError> {
    let raw: RawBlueprintDefinition = toml::from_str(source)?;
    // Parsing this field here makes `[extensions.<extension-id>]` a supported,
    // namespaced escape hatch while retaining strict validation for every core
    // blueprint field. Extension runtimes read their own data from the saved
    // immutable `definition` text.
    let _extension_metadata = &raw.extensions;
    if raw.format_version != 1 {
        return Err(BlueprintError::UnsupportedFormatVersion(raw.format_version));
    }
    validate_code(&raw.code, "blueprint code")?;
    validate_non_empty(&raw.name, "blueprint name")?;
    if raw.attributes.is_empty() {
        return Err(BlueprintError::EmptyAttributes);
    }
    let entity_schema = parse_json_schema(raw.entity_schema, "entity_schema")?;
    if entity_schema.is_some() && raw.kind != BlueprintKind::Entity {
        return Err(BlueprintError::EntitySchemaOnMixin);
    }

    let mut aliases = HashSet::new();
    for include in &raw.includes {
        validate_code(&include.alias, "include alias")?;
        validate_code(&include.code, "include code")?;
        if include.version <= 0 {
            return Err(BlueprintError::EmptyField("include version"));
        }
        if !aliases.insert(include.alias.clone()) {
            return Err(BlueprintError::DuplicateIncludeAlias(include.alias.clone()));
        }
    }

    let mut codes = HashSet::new();
    let mut attributes = Vec::with_capacity(raw.attributes.len());
    for attribute in raw.attributes {
        validate_code(&attribute.code, "attribute code")?;
        if !codes.insert(attribute.code.clone()) {
            return Err(BlueprintError::DuplicateAttributeCode(attribute.code));
        }

        attributes.push(
            match (attribute.value_type.clone(), attribute.from.clone()) {
                (Some(value_type), None) => {
                    validate_non_empty(&value_type, "attribute value_type")?;
                    if !matches!(
                        value_type.as_str(),
                        "string"
                            | "number"
                            | "integer"
                            | "boolean"
                            | "date"
                            | "datetime"
                            | "time"
                            | "relationship"
                            | "file"
                    ) {
                        return Err(BlueprintError::UnsupportedValueType {
                            code: attribute.code,
                            value_type,
                        });
                    }
                    if attribute.target_blueprint.is_some() && value_type != "relationship" {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    if value_type == "relationship" && attribute.value_schema.is_some() {
                        return Err(BlueprintError::RelationshipValueSchema {
                            code: attribute.code,
                        });
                    }
                    if attribute.default_value.is_some()
                        && matches!(value_type.as_str(), "relationship" | "file")
                    {
                        return Err(BlueprintError::DefaultValueOnNonScalarAttribute {
                            code: attribute.code,
                        });
                    }
                    let has_file_policy = (attribute.cardinality.is_some()
                        && value_type != "relationship")
                        || attribute.ordered.is_some()
                        || !attribute.allowed_mime_groups.is_empty()
                        || !attribute.allowed_extensions.is_empty()
                        || attribute.max_bytes.is_some()
                        || !attribute.purposes.is_empty()
                        || attribute.image_only.is_some();
                    if value_type != "file" && has_file_policy {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    let relationship_cardinality = if value_type == "relationship" {
                        match attribute.cardinality.as_deref() {
                            None => None,
                            Some("one_to_one") => Some("one_to_one".to_owned()),
                            Some(_) => {
                                return Err(BlueprintError::InvalidRelationshipCardinality(
                                    attribute.code,
                                ));
                            }
                        }
                    } else {
                        None
                    };
                    if value_type == "file"
                        && (attribute.value_schema.is_some()
                            || attribute.target_blueprint.is_some())
                    {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    let file_policy = if value_type == "file" {
                        Some(parse_file_policy(&attribute, &attribute.code)?)
                    } else {
                        None
                    };
                    let value_schema = parse_json_schema(
                        attribute.value_schema,
                        &format!("attribute '{}'.value_schema", attribute.code),
                    )?;
                    if let Some(target_blueprint) = &attribute.target_blueprint {
                        validate_code(target_blueprint, "attribute target_blueprint")?;
                    }
                    if !matches!(attribute.context_fallback.as_str(), "default" | "none") {
                        return Err(BlueprintError::InvalidContextFallback {
                            code: attribute.code,
                            context_fallback: attribute.context_fallback,
                        });
                    }
                    if !matches!(attribute.context_editable.as_str(), "all" | "default") {
                        return Err(BlueprintError::InvalidContextEditable {
                            code: attribute.code,
                            context_editable: attribute.context_editable,
                        });
                    }
                    let mut tags = HashSet::new();
                    for tag in &attribute.tags {
                        if tag.trim().is_empty() || !tags.insert(tag.as_str()) {
                            return Err(BlueprintError::InvalidAttributeTag(attribute.code));
                        }
                    }
                    AttributeDeclaration::Local(Box::new(LocalAttributeDeclaration {
                        code: attribute.code,
                        value_type,
                        value_schema,
                        default_value: attribute.default_value,
                        file_policy,
                        target_blueprint: attribute.target_blueprint,
                        relationship_cardinality,
                        tags: attribute.tags,
                        context_fallback: attribute.context_fallback,
                        context_editable: attribute.context_editable,
                        readonly: attribute.readonly,
                    }))
                }
                (None, Some(source)) if attribute.target_blueprint.is_none() => {
                    let (include_alias, attribute_code) =
                        parse_selection(&attribute.code, &source)?;
                    AttributeDeclaration::Selection {
                        code: attribute.code,
                        include_alias,
                        attribute_code,
                    }
                }
                _ => return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code)),
            },
        );
    }

    Ok(BlueprintDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        kind: raw.kind,
        includes: raw.includes,
        views: raw.views,
        entity_schema,
        attributes,
    })
}

fn parse_file_policy(
    attribute: &RawAttributeDeclaration,
    code: &str,
) -> Result<FilePolicy, BlueprintError> {
    let cardinality = attribute
        .cardinality
        .clone()
        .unwrap_or_else(|| "one".to_owned());
    if !matches!(cardinality.as_str(), "one" | "many") {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    let ordered = attribute.ordered.unwrap_or(cardinality == "many");
    if cardinality == "one" && ordered {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    let validate_unique_non_empty = |values: &[String]| {
        let mut seen = HashSet::new();
        values
            .iter()
            .all(|value| !value.trim().is_empty() && seen.insert(value))
    };
    if !validate_unique_non_empty(&attribute.allowed_mime_groups)
        || !validate_unique_non_empty(&attribute.allowed_extensions)
        || !validate_unique_non_empty(&attribute.purposes)
        || attribute.max_bytes == Some(0)
        || attribute.allowed_extensions.iter().any(|extension| {
            !extension
                .trim_start_matches('.')
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        })
        || attribute
            .purposes
            .iter()
            .any(|purpose| !is_valid_code(purpose))
    {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    Ok(FilePolicy {
        cardinality,
        ordered,
        allowed_mime_groups: attribute.allowed_mime_groups.clone(),
        allowed_extensions: attribute.allowed_extensions.clone(),
        max_bytes: attribute.max_bytes,
        purposes: attribute.purposes.clone(),
        image_only: attribute.image_only.unwrap_or(false),
    })
}

fn parse_json_schema(
    source: Option<String>,
    field: &str,
) -> Result<Option<serde_json::Value>, BlueprintError> {
    let Some(source) = source else {
        return Ok(None);
    };
    let schema =
        serde_json::from_str(&source).map_err(|error| BlueprintError::InvalidJsonSchema {
            field: field.to_owned(),
            message: error.to_string(),
        })?;
    validate_json_schema_definition(&schema).map_err(|message| {
        BlueprintError::InvalidJsonSchema {
            field: field.to_owned(),
            message,
        }
    })?;
    Ok(Some(schema))
}

pub(crate) fn validate_code(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if !is_valid_code(value) {
        return Err(BlueprintError::InvalidCode { field });
    }
    Ok(())
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if value.trim().is_empty() {
        return Err(BlueprintError::EmptyField(field));
    }
    Ok(())
}

fn parse_selection(code: &str, source: &str) -> Result<(String, String), BlueprintError> {
    let Some((include_alias, attribute_code)) = source.split_once('.') else {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    };
    if include_alias.is_empty() || attribute_code.is_empty() || attribute_code.contains('.') {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    }
    if code != attribute_code {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    }

    Ok((include_alias.to_owned(), attribute_code.to_owned()))
}
