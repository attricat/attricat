use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct BlueprintDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub includes: Vec<IncludeRef>,
    pub attributes: Vec<AttributeDeclaration>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlueprintKind {
    Entity,
    Mixin,
}

impl BlueprintKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Mixin => "mixin",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncludeRef {
    pub alias: String,
    pub code: String,
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttributeDeclaration {
    Local {
        code: String,
        value_type: String,
        target_blueprint: Option<String>,
    },
    Selection {
        code: String,
        include_alias: String,
        attribute_code: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedInclude {
    pub alias: String,
    pub code: String,
    pub version: i64,
    pub attributes: Vec<EffectiveAttribute>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EffectiveAttribute {
    pub code: String,
    pub value_type: String,
    pub target_blueprint: Option<String>,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBlueprint {
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub raw_definition_hash: String,
    pub includes: Vec<IncludeRef>,
    pub attributes: Vec<EffectiveAttribute>,
}

#[derive(Debug, Error)]
pub enum BlueprintError {
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported format version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    #[error("blueprints must define at least one attribute")]
    EmptyAttributes,
    #[error("include alias '{0}' is duplicated")]
    DuplicateIncludeAlias(String),
    #[error("attribute code '{0}' is duplicated")]
    DuplicateAttributeCode(String),
    #[error("attribute '{0}' must define exactly one of value_type or from")]
    InvalidAttributeDeclaration(String),
    #[error("attribute '{code}' selects invalid source '{selection}'")]
    InvalidSelection { code: String, selection: String },
    #[error("attribute '{0}' references an unknown include alias")]
    UnknownIncludeAlias(String),
    #[error("attribute '{attribute}' is not exposed by include '{alias}'")]
    UnknownIncludedAttribute { alias: String, attribute: String },
    #[error("resolved include '{0}' is missing or does not match its declaration")]
    InvalidResolvedInclude(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlueprintDefinition {
    format_version: u32,
    code: String,
    name: String,
    kind: BlueprintKind,
    #[serde(default)]
    includes: Vec<IncludeRef>,
    attributes: Vec<RawAttributeDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttributeDeclaration {
    code: String,
    value_type: Option<String>,
    target_blueprint: Option<String>,
    from: Option<String>,
}

pub fn parse(source: &str) -> Result<BlueprintDefinition, BlueprintError> {
    let raw: RawBlueprintDefinition = toml::from_str(source)?;
    if raw.format_version != 1 {
        return Err(BlueprintError::UnsupportedFormatVersion(raw.format_version));
    }
    validate_non_empty(&raw.code, "blueprint code")?;
    validate_non_empty(&raw.name, "blueprint name")?;
    if raw.attributes.is_empty() {
        return Err(BlueprintError::EmptyAttributes);
    }

    let mut aliases = HashSet::new();
    for include in &raw.includes {
        validate_non_empty(&include.alias, "include alias")?;
        validate_non_empty(&include.code, "include code")?;
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
        validate_non_empty(&attribute.code, "attribute code")?;
        if !codes.insert(attribute.code.clone()) {
            return Err(BlueprintError::DuplicateAttributeCode(attribute.code));
        }

        attributes.push(match (attribute.value_type, attribute.from) {
            (Some(value_type), None) => {
                validate_non_empty(&value_type, "attribute value_type")?;
                if attribute.target_blueprint.is_some() && value_type != "relationship" {
                    return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                }
                if let Some(target_blueprint) = &attribute.target_blueprint {
                    validate_non_empty(target_blueprint, "attribute target_blueprint")?;
                }
                AttributeDeclaration::Local {
                    code: attribute.code,
                    value_type,
                    target_blueprint: attribute.target_blueprint,
                }
            }
            (None, Some(source)) if attribute.target_blueprint.is_none() => {
                let (include_alias, attribute_code) = parse_selection(&attribute.code, &source)?;
                AttributeDeclaration::Selection {
                    code: attribute.code,
                    include_alias,
                    attribute_code,
                }
            }
            _ => return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code)),
        });
    }

    Ok(BlueprintDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        kind: raw.kind,
        includes: raw.includes,
        attributes,
    })
}

pub fn compile(
    definition: BlueprintDefinition,
    resolved_includes: &[ResolvedInclude],
    source: &str,
) -> Result<CompiledBlueprint, BlueprintError> {
    let resolved_by_alias: HashMap<_, _> = resolved_includes
        .iter()
        .map(|include| (include.alias.as_str(), include))
        .collect();

    for include in &definition.includes {
        let resolved = resolved_by_alias
            .get(include.alias.as_str())
            .ok_or_else(|| BlueprintError::InvalidResolvedInclude(include.alias.clone()))?;
        if resolved.code != include.code || resolved.version != include.version {
            return Err(BlueprintError::InvalidResolvedInclude(
                include.alias.clone(),
            ));
        }
    }
    if resolved_by_alias.len() != definition.includes.len() {
        return Err(BlueprintError::InvalidResolvedInclude(
            "unexpected include".to_owned(),
        ));
    }

    let mut attributes = Vec::with_capacity(definition.attributes.len());
    for (position, declaration) in definition.attributes.iter().enumerate() {
        let (code, value_type, target_blueprint) = match declaration {
            AttributeDeclaration::Local {
                code,
                value_type,
                target_blueprint,
            } => (code.clone(), value_type.clone(), target_blueprint.clone()),
            AttributeDeclaration::Selection {
                code,
                include_alias,
                attribute_code,
            } => {
                let include = resolved_by_alias
                    .get(include_alias.as_str())
                    .ok_or_else(|| BlueprintError::UnknownIncludeAlias(include_alias.clone()))?;
                let attribute = include
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == *attribute_code)
                    .ok_or_else(|| BlueprintError::UnknownIncludedAttribute {
                        alias: include_alias.clone(),
                        attribute: attribute_code.clone(),
                    })?;
                (
                    code.clone(),
                    attribute.value_type.clone(),
                    attribute.target_blueprint.clone(),
                )
            }
        };
        attributes.push(EffectiveAttribute {
            code,
            value_type,
            target_blueprint,
            position: position as i64,
        });
    }

    Ok(CompiledBlueprint {
        code: definition.code,
        name: definition.name,
        kind: definition.kind,
        raw_definition_hash: raw_hash(source),
        includes: definition.includes,
        attributes,
    })
}

pub fn raw_hash(source: &str) -> String {
    let hash = Sha256::digest(source.as_bytes());
    format!("{hash:x}")
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
