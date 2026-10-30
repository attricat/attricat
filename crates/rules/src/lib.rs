//! Strict, inert rule definition parsing. Candidate selection and evaluation are host-owned.
use attricat_domain::{states::RuleInapplicability, value_type::ValueType};
use attricat_validation::{
    is_valid_code,
    predicate::{AttributeTypes, PredicateError, Usage, validate_predicate},
    status::STATUS_KEY,
};
use cron::Schedule;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use thiserror::Error;

pub const MAX_CANDIDATES_PER_RUN: u32 = 10_000;
pub const MAX_PAGE_SIZE: u32 = 500;
const MAX_TRIGGERS: usize = 8;
/// The longest description a rule may carry, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 2_000;
/// Fields of a schedule cron expression, including seconds.
const CRON_FIELDS: usize = 6;
/// Supported definition format versions.
const FORMAT_VERSIONS: [u32; 1] = [1];
/// Matches text with at least one non-whitespace character.
const NON_BLANK_PATTERN: &str = r"\S";
/// Matches [`CRON_FIELDS`] whitespace-separated fields.
const SIX_FIELD_CRON_PATTERN: &str = r"^\s*\S+(?:\s+\S+){5}\s*$";

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("invalid YAML: {0}")]
    Yaml(#[from] attricat_yaml::Error),
    #[error("unsupported format_version {0}")]
    UnsupportedFormatVersion(u32),
    /// The predicate names an attribute the blueprint does not declare.
    #[error("unknown attribute '{0}'")]
    UnknownAttribute(String),
    #[error("{0}")]
    Invalid(String),
}

impl From<PredicateError> for RuleError {
    fn from(error: PredicateError) -> Self {
        match error {
            PredicateError::UnknownAttribute(code) => Self::UnknownAttribute(code),
            PredicateError::Invalid(message) => Self::Invalid(message),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RuleDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub severity: Severity,
    pub triggers: Vec<Trigger>,
    pub predicate: Predicate,
    pub enforcement: Option<Enforcement>,
}

/// How prominently findings from this rule are reported: the same codes
/// that rule findings store.
pub use attricat_domain::states::RuleFindingSeverity as Severity;

/// The JSON Schema of [`Severity`], which `attricat-domain` cannot derive.
struct SeveritySchema;

impl JsonSchema for SeveritySchema {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Severity".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "How prominently findings from this rule are reported.",
            "enum": Severity::codes(),
            "type": "string",
        })
    }
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
pub use attricat_validation::predicate::{CompareOp, Predicate, Quantifier};

/// Synchronous enforcement of an error-severity rule. Enforcing rules reject
/// violating writes with `422 rule_violation` instead of only reporting findings.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Enforcement {
    /// Reject every write that leaves the record violating the rule.
    #[serde(default)]
    pub on_save: bool,
    /// Reject the listed status transitions while the rule is violated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = MAX_ENFORCED_TRANSITIONS))]
    pub transitions: Vec<TransitionSelector>,
}

/// A status change that the rule guards. The predicate is evaluated on the
/// state the transition would produce.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransitionSelector {
    /// Status attribute code.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
    pub attribute_code: String,
    /// Source status code. Omit to guard every change into `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub from: Option<String>,
    /// Destination status code.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub to: String,
}

/// Most status transitions one enforcing rule can guard.
pub(crate) const MAX_ENFORCED_TRANSITIONS: usize = 16;

impl Enforcement {
    /// Enforcing rules run inside write transactions, so their predicates must
    /// be safe for synchronous evaluation; other rules only report findings.
    fn usage(enforcement: Option<&Self>) -> Usage {
        if enforcement.is_some() {
            Usage::Enforced
        } else {
            Usage::Finding
        }
    }

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
    /// What the rule is for, in the words of the people who fix its findings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub severity: Severity,
    pub triggers: Vec<Trigger>,
    pub predicate: Predicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<Enforcement>,
    pub raw_definition_hash: String,
}

/// A data-health rule that reports findings for records matching its predicate.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Attricat rule definition")]
struct Raw {
    /// Definition format version. Only `1` is supported.
    #[schemars(extend("enum" = FORMAT_VERSIONS))]
    format_version: u32,
    /// Stable rule identifier, unique within its owner.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    code: String,
    /// Human-readable rule name.
    #[schemars(regex(pattern = NON_BLANK_PATTERN))]
    name: String,
    /// What the rule checks and why, for the people who fix its findings.
    /// Shown with the rule and its findings; Markdown is not rendered.
    #[schemars(regex(pattern = NON_BLANK_PATTERN), length(max = MAX_DESCRIPTION_CHARS))]
    description: Option<String>,
    #[schemars(with = "SeveritySchema")]
    severity: Severity,
    /// One to eight events that evaluate the rule.
    #[schemars(length(min = 1, max = MAX_TRIGGERS))]
    triggers: Vec<RawTrigger>,
    predicate: Predicate,
    /// Reject violating saves or status transitions. Requires `error` or
    /// `critical` severity and a predicate that is safe for synchronous evaluation.
    enforcement: Option<Enforcement>,
}
/// When the rule is evaluated.
// Variants without fields are empty structs rather than unit variants: serde
// ignores unknown keys next to the tag of a unit variant even with
// `deny_unknown_fields`.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawTrigger {
    /// Evaluated only when run on demand.
    Manual {},
    /// Evaluated on a UTC cron schedule.
    Schedule {
        /// Six-field cron expression, including seconds: `sec min hour day month weekday`.
        #[schemars(regex(pattern = SIX_FIELD_CRON_PATTERN))]
        cron: String,
        /// Schedule time zone. Only `UTC` is supported.
        #[schemars(extend("enum" = ["UTC"]))]
        timezone: String,
    },
    /// Evaluated when a record event occurs.
    Event {
        /// Record event that evaluates the rule.
        #[schemars(extend("enum" = RECORD_EVENTS))]
        event_type: String,
    },
    /// Evaluated after an import finishes.
    PostImport {},
}
pub fn parse(source: &str) -> Result<RuleDefinition, RuleError> {
    let raw: Raw = attricat_yaml::from_str(source)?;
    if !FORMAT_VERSIONS.contains(&raw.format_version) {
        return Err(RuleError::UnsupportedFormatVersion(raw.format_version));
    }
    code(&raw.code, "rule code")?;
    non_empty(&raw.name, "rule name")?;
    if let Some(description) = &raw.description {
        non_empty(description, "rule description")?;
        if description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(RuleError::Invalid(format!(
                "rule description must be at most {MAX_DESCRIPTION_CHARS} characters"
            )));
        }
    }
    if raw.triggers.is_empty() || raw.triggers.len() > MAX_TRIGGERS {
        return Err(RuleError::Invalid(format!(
            "rules require 1-{MAX_TRIGGERS} triggers"
        )));
    }
    let mut schedule_seen = HashSet::new();
    let triggers = raw
        .triggers
        .into_iter()
        .map(|trigger| match trigger {
            RawTrigger::Manual {} => Ok(Trigger::Manual),
            RawTrigger::PostImport {} => Ok(Trigger::PostImport),
            RawTrigger::Event { event_type } => {
                if !RECORD_EVENTS.contains(&event_type.as_str()) {
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
    let usage = Enforcement::usage(raw.enforcement.as_ref());
    validate_predicate(&raw.predicate, None, usage)?;
    if let Some(enforcement) = &raw.enforcement {
        validate_enforcement(enforcement, &raw.severity)?;
    }
    Ok(RuleDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        description: raw.description,
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
    if enforcement.transitions.len() > MAX_ENFORCED_TRANSITIONS {
        return Err(RuleError::Invalid(format!(
            "enforcement allows at most {MAX_ENFORCED_TRANSITIONS} transitions"
        )));
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

/// The attributes of one blueprint revision that a rule is checked against:
/// value types, the option codes of each status attribute, and the values of
/// every attribute with a fixed list of them.
#[derive(Clone, Debug, Default)]
pub struct RevisionAttributes {
    pub types: HashMap<String, ValueType>,
    pub statuses: HashMap<String, Vec<String>>,
    pub options: HashMap<String, Vec<String>>,
}

impl RevisionAttributes {
    /// Adds or replaces one attribute. A status attribute's codes are its
    /// value schema's `enum`.
    pub fn insert(
        &mut self,
        code: &str,
        value_type: ValueType,
        value_schema: Option<&serde_json::Value>,
    ) {
        self.types.insert(code.to_owned(), value_type);
        let values: Option<Vec<String>> = value_schema
            .and_then(|schema| schema.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            });
        match values {
            Some(values) => {
                self.options.insert(code.to_owned(), values);
            }
            None => {
                self.options.remove(code);
            }
        }
        let options = value_schema
            .filter(|schema| schema.get(STATUS_KEY).is_some())
            .and_then(|schema| schema.get("enum"))
            .and_then(serde_json::Value::as_array);
        match options {
            Some(options) => {
                self.statuses.insert(
                    code.to_owned(),
                    options
                        .iter()
                        .filter_map(|option| option.as_str().map(str::to_owned))
                        .collect(),
                );
            }
            None => {
                self.statuses.remove(code);
            }
        }
    }
}

impl AttributeTypes for RevisionAttributes {
    fn attribute_type(&self, code: &str) -> Option<Option<ValueType>> {
        self.types.attribute_type(code)
    }
}

/// Checks that every value a rule compares an attribute with fixed values to,
/// in `one_of` or an `eq`/`ne` comparison, is one of those values, so a
/// misspelt option does not flag every record. Linked records' attributes
/// belong to other blueprints and are not checked here.
pub fn check_option_values(
    rule: &CompiledRule,
    attributes: &RevisionAttributes,
) -> Result<(), String> {
    check_predicate_options(&rule.predicate, attributes)
}

fn check_predicate_options(
    predicate: &Predicate,
    attributes: &RevisionAttributes,
) -> Result<(), String> {
    let check = |attribute_code: &str, value: &serde_json::Value| {
        let (Some(options), Some(value)) = (attributes.options.get(attribute_code), value.as_str())
        else {
            return Ok(());
        };
        if options.iter().any(|option| option == value) {
            Ok(())
        } else {
            Err(format!(
                "'{attribute_code}' has no value \"{value}\"; it allows {}",
                options
                    .iter()
                    .map(|option| format!("\"{option}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    };
    match predicate {
        Predicate::OneOf {
            attribute_code,
            values,
        } => values
            .iter()
            .try_for_each(|value| check(attribute_code, value)),
        Predicate::Compare {
            attribute_code,
            op: CompareOp::Eq | CompareOp::Ne,
            value: Some(value),
            ..
        } => check(attribute_code, value),
        Predicate::AllOf { predicates } | Predicate::AnyOf { predicates } => predicates
            .iter()
            .try_for_each(|predicate| check_predicate_options(predicate, attributes)),
        _ => Ok(()),
    }
}

/// Why a rule does not apply to a blueprint revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NotApplicable {
    pub reason: RuleInapplicability,
    pub message: String,
}

impl std::fmt::Display for NotApplicable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Checks a rule against one revision of its blueprint: the attributes its
/// predicate and guarded transitions name must exist there with types and
/// status codes that suit them. A rule applies to every revision it fits.
pub fn applicability(
    rule: &CompiledRule,
    attributes: &RevisionAttributes,
) -> Result<(), NotApplicable> {
    let usage = Enforcement::usage(rule.enforcement.as_ref());
    validate_predicate(&rule.predicate, Some(attributes), usage).map_err(|error| match error {
        PredicateError::UnknownAttribute(code) => NotApplicable {
            reason: RuleInapplicability::AttributeMissing,
            message: format!("unknown attribute '{code}'"),
        },
        PredicateError::Invalid(message) => NotApplicable {
            reason: RuleInapplicability::AttributeTypeChanged,
            message,
        },
    })?;
    for selector in rule
        .enforcement
        .iter()
        .flat_map(|enforcement| &enforcement.transitions)
    {
        let status_changed = |message: String| NotApplicable {
            reason: RuleInapplicability::StatusChanged,
            message,
        };
        let Some(codes) = attributes.statuses.get(&selector.attribute_code) else {
            return Err(status_changed(format!(
                "enforcement attribute '{}' is not a status attribute",
                selector.attribute_code
            )));
        };
        for code in std::iter::once(&selector.to).chain(&selector.from) {
            if !codes.contains(code) {
                return Err(status_changed(format!(
                    "unknown status '{code}' for '{}'",
                    selector.attribute_code
                )));
            }
        }
    }
    Ok(())
}
pub fn compile(source: &str) -> Result<CompiledRule, RuleError> {
    let rule = parse(source)?;
    Ok(CompiledRule {
        format_version: rule.format_version,
        code: rule.code,
        name: rule.name,
        description: rule.description,
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
    let invalid =
        || RuleError::Invalid("schedule cron must be a valid six-field UTC cron expression".into());
    if value.split_whitespace().count() != CRON_FIELDS {
        return Err(invalid());
    }
    Schedule::from_str(value).map_err(|_| invalid())
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
/// Whether `event_type` changes a record's values or relationships. Every
/// enabled rule re-evaluates the changed record on these events, whatever its
/// triggers, so its findings follow the saved state.
pub fn is_record_change_event(event_type: &str) -> bool {
    RECORD_EVENTS.contains(&event_type)
}

const RECORD_EVENTS: &[&str] = attricat_events::RECORD_TRIGGER_EVENT_TYPES_V1;

#[cfg(test)]
mod tests {
    use super::*;
    const REQUIRED: &str = "format_version: 1\ncode: product-title\nname: Product title required\nseverity: error\ntriggers:\n  - type: manual\n  - type: schedule\n    cron: '0 0 * * * *'\n    timezone: UTC\npredicate:\n  type: required\n  attribute_code: title";
    /// `REQUIRED` with its predicate replaced by a flow mapping.
    fn with_predicate(predicate: &str) -> String {
        REQUIRED.replace(PREDICATE, &format!("predicate: {predicate}"))
    }
    const PREDICATE: &str = "predicate:\n  type: required\n  attribute_code: title";
    #[test]
    fn parses_bounded_host_owned_rule() {
        assert!(parse(REQUIRED).is_ok());
    }
    #[test]
    fn rejects_values_an_attribute_does_not_allow() {
        let mut attributes = RevisionAttributes::default();
        attributes.insert(
            "condition",
            ValueType::String,
            Some(&serde_json::json!({"type": "string", "enum": ["new", "used"]})),
        );
        attributes.insert("title", ValueType::String, None);
        let rule = |predicate: &str| compile(&with_predicate(predicate)).unwrap();
        let allowed = rule("{type: one_of, attribute_code: condition, values: [new]}");
        assert!(check_option_values(&allowed, &attributes).is_ok());
        let misspelt = rule("{type: one_of, attribute_code: condition, values: [brand_new]}");
        assert_eq!(
            check_option_values(&misspelt, &attributes).unwrap_err(),
            "'condition' has no value \"brand_new\"; it allows \"new\", \"used\""
        );
        let nested = rule(
            "{type: any_of, predicates: [{type: compare, attribute_code: condition, op: eq, value: mint}, {type: required, attribute_code: title}]}",
        );
        assert!(check_option_values(&nested, &attributes).is_err());
        // Attributes without fixed values take any value.
        let free = rule("{type: one_of, attribute_code: title, values: [anything]}");
        assert!(check_option_values(&free, &attributes).is_ok());
    }
    #[test]
    fn carries_an_optional_bounded_description() {
        assert_eq!(compile(REQUIRED).unwrap().description, None);
        let described = REQUIRED.replace(
            "severity: error",
            "severity: error\ndescription: Shoppers need a title to find the product.",
        );
        assert_eq!(
            compile(&described).unwrap().description.as_deref(),
            Some("Shoppers need a title to find the product.")
        );
        assert!(
            parse(&REQUIRED.replace("severity: error", "severity: error\ndescription: '  '"))
                .is_err()
        );
        let long = "x".repeat(MAX_DESCRIPTION_CHARS + 1);
        assert!(
            parse(&REQUIRED.replace(
                "severity: error",
                &format!("severity: error\ndescription: '{long}'")
            ))
            .is_err()
        );
    }
    #[test]
    fn definitions_spell_severity_as_findings_store_it() {
        for &severity in Severity::ALL {
            let source = REQUIRED.replace(
                "severity: error",
                &format!("severity: '{}'", severity.as_str()),
            );
            assert_eq!(parse(&source).unwrap().severity, severity);
        }
        assert!(parse(&REQUIRED.replace("severity: error", "severity: fatal")).is_err());
    }
    #[test]
    fn rejects_unbounded_or_invalid_contracts() {
        assert!(parse(&REQUIRED.replace("timezone: UTC", "timezone: Europe/Warsaw")).is_err());
        assert!(
            parse(&REQUIRED.replace("attribute_code: title", "attribute_code: bad code")).is_err()
        );
        assert!(parse(&REQUIRED.replace("type: required", "type: sql")).is_err());
    }
    #[test]
    fn parses_new_predicates_and_enforcement() {
        let range = with_predicate(
            "{type: compare, attribute_code: valid_from, op: lte, other_attribute_code: valid_until}\nenforcement:\n  on_save: true\n  transitions:\n    - {attribute_code: status, to: released}",
        );
        let rule = compile(&range).unwrap();
        let enforcement = rule.enforcement.unwrap();
        assert!(enforcement.on_save);
        assert!(enforcement.guards_transition("status", Some("draft"), Some("released")));
        assert!(!enforcement.guards_transition("status", Some("released"), Some("released")));
        let expiry = with_predicate(
            "{type: relative_date, attribute_code: expires_on, op: gt, offset_days: 30}",
        );
        assert!(parse(&expiry).is_ok());
        let duplicate = with_predicate("{type: unique, attribute_codes: [sku]}");
        assert!(parse(&duplicate).is_ok());
        // Only synchronous-safe predicates can enforce, and only at error severity.
        assert!(parse(&format!("{duplicate}\nenforcement:\n  on_save: true")).is_err());
        assert!(parse(&range.replace("severity: error", "severity: warning")).is_err());
        assert!(parse(&format!("{REQUIRED}\nenforcement: {{}}")).is_err());
    }
    #[test]
    fn applies_only_to_revisions_whose_attributes_fit() {
        let status = serde_json::json!({
            "enum": ["draft", "released"],
            "x-attricat-status": {"version": 1},
        });
        let mut fits = RevisionAttributes::default();
        fits.insert("title", ValueType::String, None);
        fits.insert("status", ValueType::String, Some(&status));
        let guarded = compile(&format!(
            "{REQUIRED}\nenforcement:\n  transitions:\n    - {{attribute_code: status, from: draft, to: released}}"
        ))
        .unwrap();
        assert_eq!(applicability(&guarded, &fits), Ok(()));

        let reason = |attributes: &RevisionAttributes, rule: &CompiledRule| {
            applicability(rule, attributes)
                .expect_err(&rule.code)
                .reason
        };
        let mut missing = RevisionAttributes::default();
        missing.insert("status", ValueType::String, Some(&status));
        assert_eq!(
            reason(&missing, &guarded),
            RuleInapplicability::AttributeMissing
        );
        let ordered = compile(&with_predicate(
            "{type: compare, attribute_code: title, op: gt, value: 3}",
        ))
        .unwrap();
        let mut numeric = fits.clone();
        numeric.insert("title", ValueType::Number, None);
        assert_eq!(applicability(&ordered, &numeric), Ok(()));
        assert_eq!(
            reason(&fits, &ordered),
            RuleInapplicability::AttributeTypeChanged
        );
        let mut plain_status = fits.clone();
        plain_status.insert("status", ValueType::String, None);
        assert_eq!(
            reason(&plain_status, &guarded),
            RuleInapplicability::StatusChanged
        );
        let mut renamed = fits.clone();
        renamed.insert(
            "status",
            ValueType::String,
            Some(&serde_json::json!({"enum": ["new", "released"], "x-attricat-status": {}})),
        );
        assert_eq!(
            reason(&renamed, &guarded),
            RuleInapplicability::StatusChanged
        );
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

    /// The JSON Schema for standalone rule definition YAML, published as
    /// `contracts/rule-definition-v1.schema.json` for the web editor.
    fn definition_json_schema() -> serde_json::Value {
        schemars::generate::SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<Raw>()
            .to_value()
    }

    const CONTRACT: &str = "contracts/rule-definition-v1.schema.json";
    const UPDATE_CONTRACTS: &str = "UPDATE_CONTRACTS";

    #[test]
    fn definition_schema_contract_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join(CONTRACT);
        let mut rendered = serde_json::to_string_pretty(&definition_json_schema()).unwrap();
        rendered.push('\n');
        if std::env::var_os(UPDATE_CONTRACTS).is_some() {
            std::fs::write(&path, rendered).unwrap();
            return;
        }
        assert!(
            std::fs::read_to_string(&path).unwrap_or_default() == rendered,
            "{CONTRACT} is stale; run `just contracts`"
        );
    }

    #[test]
    fn schema_matches_parser_for_representative_definitions() {
        let schema = definition_json_schema();
        for (definition, accepted) in [
            (REQUIRED.to_owned(), true),
            (
                REQUIRED.replace("severity: error", "severity: fatal"),
                false,
            ),
            (REQUIRED.replace("type: required", "type: sql"), false),
            (REQUIRED.replace("format_version: 1\n", ""), false),
            (format!("{REQUIRED}\nlabel: Title"), false),
            (
                REQUIRED.replace("format_version: 1", "format_version: 2"),
                false,
            ),
            (
                REQUIRED.replace("name: Product title required", "name: '  '"),
                false,
            ),
            (
                REQUIRED.replace("cron: '0 0 * * * *'", "cron: '0 * * * *'"),
                false,
            ),
            (
                REQUIRED.replace("type: manual", "type: manual\n    label: x"),
                false,
            ),
            (REQUIRED.replace("type: manual", "type: post_import"), true),
            (
                REQUIRED.replace("type: manual", "type: post_import\n    label: x"),
                false,
            ),
        ] {
            assert_eq!(parse(&definition).is_ok(), accepted, "{definition}");
            let instance = attricat_yaml::from_str::<serde_json::Value>(&definition).unwrap();
            let violations = attricat_validation::validate_json_schema(&schema, &instance).unwrap();
            assert_eq!(
                violations.is_empty(),
                accepted,
                "{definition}: {violations:?}"
            );
        }
    }
}
