//! Strict, inert rule definition parsing. Candidate selection and evaluation are host-owned.
use catalog_validation::is_valid_code;
use cron::Schedule;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, str::FromStr};
use thiserror::Error;

pub const MAX_CANDIDATES_PER_RUN: u32 = 10_000;
pub const MAX_PAGE_SIZE: u32 = 500;

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported format_version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RuleDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub severity: Severity,
    pub triggers: Vec<Trigger>,
    pub predicate: Predicate,
}

/// How prominently findings from this rule are reported.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Manual,
    Schedule { cron: String, timezone: String },
    Event { event_type: String },
    PostImport,
}

/// A deliberately small predicate set. It cannot express SQL, templates, calls, or selectors.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Predicate {
    Required {
        attribute_code: String,
    },
    Stale {
        attribute_code: String,
        max_age_seconds: u64,
    },
    HasTag {
        tag: String,
    },
    MissingTag {
        tag: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CompiledRule {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub severity: Severity,
    pub triggers: Vec<Trigger>,
    pub predicate: Predicate,
    pub raw_definition_hash: String,
}

/// A data-health rule that reports findings for entities matching its predicate.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Raw {
    /// Definition format version. Only `1` is supported.
    format_version: u32,
    /// Stable rule identifier, unique within its owner.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    code: String,
    /// Human-readable rule name.
    name: String,
    severity: Severity,
    /// One to eight events that evaluate the rule.
    #[schemars(length(min = 1, max = 8))]
    triggers: Vec<RawTrigger>,
    predicate: RawPredicate,
}
/// When the rule is evaluated.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawTrigger {
    /// Evaluated only when run on demand.
    Manual,
    /// Evaluated on a UTC cron schedule.
    Schedule {
        /// Six-field cron expression, including seconds: `sec min hour day month weekday`.
        cron: String,
        /// Schedule time zone. Only `UTC` is supported.
        #[schemars(extend("enum" = ["UTC"]))]
        timezone: String,
    },
    /// Evaluated when an entity event occurs.
    Event {
        /// Entity event that evaluates the rule.
        #[schemars(extend("enum" = ENTITY_EVENTS))]
        event_type: String,
    },
    /// Evaluated after an import finishes.
    PostImport,
}
/// The condition that produces a finding.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawPredicate {
    /// Reports entities without a value for the attribute.
    Required {
        /// Attribute that must have a value.
        #[schemars(regex(pattern = catalog_validation::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
    },
    /// Reports entities whose attribute value has not changed within the age limit.
    Stale {
        /// Attribute whose last change is checked.
        #[schemars(regex(pattern = catalog_validation::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
        /// Maximum value age in seconds, from 1 to 31536000 (one year).
        #[schemars(range(min = 1, max = 31_536_000))]
        max_age_seconds: u64,
    },
    /// Reports entities with the tag.
    HasTag {
        /// Tag of at most 128 bytes.
        #[schemars(length(min = 1, max = 128))]
        tag: String,
    },
    /// Reports entities without the tag.
    MissingTag {
        /// Tag of at most 128 bytes.
        #[schemars(length(min = 1, max = 128))]
        tag: String,
    },
}

pub fn parse(source: &str) -> Result<RuleDefinition, RuleError> {
    let raw: Raw = toml::from_str(source)?;
    if raw.format_version != 1 {
        return Err(RuleError::UnsupportedFormatVersion(raw.format_version));
    }
    code(&raw.code, "rule code")?;
    non_empty(&raw.name, "rule name")?;
    if raw.triggers.is_empty() || raw.triggers.len() > 8 {
        return Err(RuleError::Invalid("rules require 1-8 triggers".into()));
    }
    let mut schedule_seen = HashSet::new();
    let triggers = raw
        .triggers
        .into_iter()
        .map(|trigger| match trigger {
            RawTrigger::Manual => Ok(Trigger::Manual),
            RawTrigger::PostImport => Ok(Trigger::PostImport),
            RawTrigger::Event { event_type } => {
                if !ENTITY_EVENTS.contains(&event_type.as_str()) {
                    return Err(RuleError::Invalid(format!(
                        "unsupported rule event type '{event_type}'"
                    )));
                }
                Ok(Trigger::Event { event_type })
            }
            RawTrigger::Schedule { cron, timezone } => {
                if timezone != "UTC" {
                    return Err(RuleError::Invalid("schedule timezone must be UTC".into()));
                }
                parse_six_field_cron(&cron)?;
                if !schedule_seen.insert(cron.clone()) {
                    return Err(RuleError::Invalid("duplicate schedule trigger".into()));
                }
                Ok(Trigger::Schedule { cron, timezone })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let predicate = match raw.predicate {
        RawPredicate::Required { attribute_code } => {
            code(&attribute_code, "predicate attribute_code")?;
            Predicate::Required { attribute_code }
        }
        RawPredicate::Stale {
            attribute_code,
            max_age_seconds,
        } => {
            code(&attribute_code, "predicate attribute_code")?;
            if !(1..=31_536_000).contains(&max_age_seconds) {
                return Err(RuleError::Invalid(
                    "stale max_age_seconds must be between 1 and 31536000".into(),
                ));
            }
            Predicate::Stale {
                attribute_code,
                max_age_seconds,
            }
        }
        RawPredicate::HasTag { tag } => {
            validate_tag(&tag)?;
            Predicate::HasTag { tag }
        }
        RawPredicate::MissingTag { tag } => {
            validate_tag(&tag)?;
            Predicate::MissingTag { tag }
        }
    };
    Ok(RuleDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        severity: raw.severity,
        triggers,
        predicate,
    })
}
/// Compiles an inline `[[rules]]` blueprint table. Blueprint ownership supplies
/// the format version, so embedding cannot relax the standalone rule contract.
pub fn compile_embedded(value: toml::Value) -> Result<CompiledRule, RuleError> {
    let mut table = value
        .as_table()
        .cloned()
        .ok_or_else(|| RuleError::Invalid("embedded rule must be a TOML table".into()))?;
    if table
        .insert("format_version".into(), toml::Value::Integer(1))
        .is_some()
    {
        return Err(RuleError::Invalid(
            "embedded rule cannot set format_version".into(),
        ));
    }
    compile(
        &toml::to_string(&toml::Value::Table(table))
            .map_err(|error| RuleError::Invalid(error.to_string()))?,
    )
}
/// Schema for a blueprint's `rules` array: standalone rules without
/// `format_version`, which the owning blueprint supplies.
pub fn embedded_rules_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut rule = Raw::json_schema(generator);
    if let Some(properties) = rule
        .get_mut("properties")
        .and_then(|properties| properties.as_object_mut())
    {
        properties.remove("format_version");
    }
    if let Some(required) = rule
        .get_mut("required")
        .and_then(|required| required.as_array_mut())
    {
        required.retain(|field| field != "format_version");
    }
    json_schema!({ "type": "array", "items": rule })
}
pub fn compile(source: &str) -> Result<CompiledRule, RuleError> {
    let rule = parse(source)?;
    Ok(CompiledRule {
        format_version: rule.format_version,
        code: rule.code,
        name: rule.name,
        severity: rule.severity,
        triggers: rule.triggers,
        predicate: rule.predicate,
        raw_definition_hash: raw_hash(source),
    })
}
pub fn raw_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
pub fn parse_six_field_cron(value: &str) -> Result<Schedule, RuleError> {
    if value.split_whitespace().count() != 6 {
        return Err(RuleError::Invalid(
            "schedule cron must be a valid six-field UTC cron expression".into(),
        ));
    }
    Schedule::from_str(value).map_err(|_| {
        RuleError::Invalid("schedule cron must be a valid six-field UTC cron expression".into())
    })
}
fn code(value: &str, name: &str) -> Result<(), RuleError> {
    if is_valid_code(value) {
        Ok(())
    } else {
        Err(RuleError::Invalid(format!("invalid {name}")))
    }
}
fn non_empty(value: &str, name: &str) -> Result<(), RuleError> {
    if value.trim().is_empty() {
        Err(RuleError::Invalid(format!("{name} cannot be empty")))
    } else {
        Ok(())
    }
}
fn validate_tag(value: &str) -> Result<(), RuleError> {
    if value.trim().is_empty() || value.len() > 128 {
        Err(RuleError::Invalid(
            "tag must be a non-empty string no longer than 128 bytes".into(),
        ))
    } else {
        Ok(())
    }
}
const ENTITY_EVENTS: &[&str] = &[
    "entity.created.v1",
    "entity.updated.v1",
    "entity.migrated.v1",
    "attribute_value.changed.v1",
    "attribute_value.restored.v1",
    "relationship.changed.v1",
];

#[cfg(test)]
mod tests {
    use super::*;
    const REQUIRED: &str = "format_version=1\ncode='product-title'\nname='Product title required'\nseverity='error'\n[[triggers]]\ntype='manual'\n[[triggers]]\ntype='schedule'\ncron='0 0 * * * *'\ntimezone='UTC'\n[predicate]\ntype='required'\nattribute_code='title'";
    #[test]
    fn parses_bounded_host_owned_rule() {
        assert!(parse(REQUIRED).is_ok());
    }
    #[test]
    fn rejects_unbounded_or_invalid_contracts() {
        assert!(parse(&REQUIRED.replace("timezone='UTC'", "timezone='Europe/Warsaw'")).is_err());
        assert!(
            parse(&REQUIRED.replace("attribute_code='title'", "attribute_code='bad code'"))
                .is_err()
        );
        assert!(parse(&REQUIRED.replace("type='required'", "type='sql'")).is_err());
    }
}
