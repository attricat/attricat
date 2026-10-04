//! Strict, inert rule definition parsing. Candidate selection and evaluation are host-owned.
use catalog_validation::{
    is_valid_code,
    predicate::{AttributeTypes, Usage, validate_predicate},
};
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
    pub enforcement: Option<Enforcement>,
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

/// Rules use the shared declarative predicate engine. Predicates cannot
/// express SQL, templates, calls or selectors.
pub use catalog_validation::predicate::{CompareOp, Predicate, Quantifier};

/// Synchronous enforcement of an error-severity rule. Enforcing rules reject
/// violating writes with `422 rule_violation` instead of only reporting findings.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Enforcement {
    /// Reject every write that leaves the entity violating the rule.
    #[serde(default)]
    pub on_save: bool,
    /// Reject the listed status transitions while the rule is violated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 16))]
    pub transitions: Vec<TransitionSelector>,
}

/// A status change that the rule guards. The predicate is evaluated on the
/// state the transition would produce.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionSelector {
    /// Status attribute code.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
    pub attribute_code: String,
    /// Source status code. Omit to guard every change into `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    pub from: Option<String>,
    /// Destination status code.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    pub to: String,
}

impl Enforcement {
    /// Whether this rule guards the change of `attribute` from `before` to `after`.
    pub fn guards_transition(
        &self,
        attribute: &str,
        before: Option<&str>,
        after: Option<&str>,
    ) -> bool {
        before != after
            && self.transitions.iter().any(|selector| {
                selector.attribute_code == attribute
                    && Some(selector.to.as_str()) == after
                    && selector
                        .from
                        .as_deref()
                        .is_none_or(|from| Some(from) == before)
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CompiledRule {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub severity: Severity,
    pub triggers: Vec<Trigger>,
    pub predicate: Predicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<Enforcement>,
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
    predicate: Predicate,
    /// Reject violating saves or status transitions. Requires `error` or
    /// `critical` severity and a predicate that is safe for synchronous evaluation.
    enforcement: Option<Enforcement>,
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
    let usage = if raw.enforcement.is_some() {
        Usage::Enforced
    } else {
        Usage::Finding
    };
    validate_predicate(&raw.predicate, None, usage).map_err(RuleError::Invalid)?;
    if let Some(enforcement) = &raw.enforcement {
        validate_enforcement(enforcement, &raw.severity)?;
    }
    Ok(RuleDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        severity: raw.severity,
        triggers,
        predicate: raw.predicate,
        enforcement: raw.enforcement,
    })
}

fn validate_enforcement(enforcement: &Enforcement, severity: &Severity) -> Result<(), RuleError> {
    if !matches!(severity, Severity::Error | Severity::Critical) {
        return Err(RuleError::Invalid(
            "enforcing rules require error or critical severity".into(),
        ));
    }
    if !enforcement.on_save && enforcement.transitions.is_empty() {
        return Err(RuleError::Invalid(
            "enforcement needs on_save or at least one transition".into(),
        ));
    }
    if enforcement.transitions.len() > 16 {
        return Err(RuleError::Invalid(
            "enforcement allows at most 16 transitions".into(),
        ));
    }
    for selector in &enforcement.transitions {
        code(&selector.attribute_code, "enforcement attribute_code")?;
        code(&selector.to, "enforcement transition to")?;
        if let Some(from) = &selector.from {
            code(from, "enforcement transition from")?;
        }
    }
    Ok(())
}

/// Type-checks a rule against the attribute types of its blueprint revision.
pub fn validate_against_attributes(
    rule: &CompiledRule,
    attributes: &dyn AttributeTypes,
) -> Result<(), RuleError> {
    let usage = if rule.enforcement.is_some() {
        Usage::Enforced
    } else {
        Usage::Finding
    };
    validate_predicate(&rule.predicate, Some(attributes), usage).map_err(RuleError::Invalid)?;
    for selector in rule
        .enforcement
        .iter()
        .flat_map(|enforcement| &enforcement.transitions)
    {
        if attributes
            .attribute_type(&selector.attribute_code)
            .is_none()
        {
            return Err(RuleError::Invalid(format!(
                "unknown enforcement attribute '{}'",
                selector.attribute_code
            )));
        }
    }
    Ok(())
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
        enforcement: rule.enforcement,
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
    #[test]
    fn parses_new_predicates_and_enforcement() {
        let range = REQUIRED.replace(
            "type='required'\nattribute_code='title'",
            "type='compare'\nattribute_code='valid_from'\nop='lte'\nother_attribute_code='valid_until'\n[enforcement]\non_save=true\n[[enforcement.transitions]]\nattribute_code='status'\nto='released'",
        );
        let rule = compile(&range).unwrap();
        let enforcement = rule.enforcement.unwrap();
        assert!(enforcement.on_save);
        assert!(enforcement.guards_transition("status", Some("draft"), Some("released")));
        assert!(!enforcement.guards_transition("status", Some("released"), Some("released")));
        let expiry = REQUIRED.replace(
            "type='required'\nattribute_code='title'",
            "type='relative_date'\nattribute_code='expires_on'\nop='gt'\noffset_days=30",
        );
        assert!(parse(&expiry).is_ok());
        let duplicate = REQUIRED.replace(
            "type='required'\nattribute_code='title'",
            "type='unique'\nattribute_codes=['sku']",
        );
        assert!(parse(&duplicate).is_ok());
        // Only synchronous-safe predicates can enforce, and only at error severity.
        assert!(parse(&format!("{duplicate}\n[enforcement]\non_save=true")).is_err());
        assert!(parse(&range.replace("severity='error'", "severity='warning'")).is_err());
        assert!(parse(&format!("{REQUIRED}\n[enforcement]")).is_err());
    }
    #[test]
    fn stored_plans_without_enforcement_still_load() {
        let plan = serde_json::json!({
            "format_version": 1, "code": "x", "name": "x", "severity": "error",
            "triggers": [{"type": "manual"}],
            "predicate": {"type": "required", "attribute_code": "title"},
            "raw_definition_hash": "h"
        });
        let rule: CompiledRule = serde_json::from_value(plan).unwrap();
        assert!(rule.enforcement.is_none());
    }
}
