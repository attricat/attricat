//! The one declarative predicate engine shared by rules, entity-schema checks,
//! status transition conditions and publication gates.
//!
//! Predicates are inert data. Validation is strict and type-aware when the
//! attribute types are known. Evaluation is pure: the host loads a bounded
//! [`Related`] snapshot described by [`Requirements`], then [`evaluate`] decides.
use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Entity-schema keyword holding the blueprint's declarative checks.
pub const CHECKS_KEY: &str = "x-attricat-checks";
pub const MAX_ENTITY_CHECKS: usize = 32;
pub const MAX_TRANSITION_CONDITIONS: usize = 16;
pub const MAX_DEPTH: usize = 4;
pub const MAX_NODES: usize = 32;
pub const MAX_BRANCHES: usize = 16;
/// Linked records loaded per relationship for one subject and context.
pub const MAX_LINKED_RECORDS: usize = 200;
/// Referencing records loaded per `referenced_by` predicate.
pub const MAX_REFERENCING_RECORDS: usize = 1_000;
/// Entities visited while following a relationship for `acyclic`.
pub const MAX_CYCLE_VISITS: usize = 1_000;
pub const MAX_ONE_OF_VALUES: usize = 100;
pub const MAX_UNIQUE_ATTRIBUTES: usize = 4;
pub const MAX_OFFSET_DAYS: i64 = 36_500;
pub const MAX_MESSAGE_LENGTH: usize = 500;

/// Comparison operator. `lt`–`gte` order numbers, dates and datetimes;
/// `eq`/`ne` compare any scalar or relationship target set; `disjoint`
/// requires two relationship target sets without a common target.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Disjoint,
}

impl CompareOp {
    fn is_ordering(self) -> bool {
        matches!(self, Self::Lt | Self::Lte | Self::Gt | Self::Gte)
    }
    fn phrase(self) -> &'static str {
        match self {
            Self::Eq => "equal to",
            Self::Ne => "different from",
            Self::Lt => "less than",
            Self::Lte => "at most",
            Self::Gt => "greater than",
            Self::Gte => "at least",
            Self::Disjoint => "without common targets with",
        }
    }
}

/// How many linked records must satisfy a `linked` predicate.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quantifier {
    /// Every linked record satisfies the predicate. Holds with no links.
    #[default]
    All,
    /// At least one linked record satisfies the predicate.
    Any,
    /// No linked record satisfies the predicate.
    None,
}

/// A requirement on one record. A predicate *holds* when the data is
/// acceptable; a rule reports a finding, a check rejects a save and a
/// transition condition blocks a transition when it does not hold.
///
/// Comparisons with a missing operand hold: combine them with `required`
/// when a value must exist.
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    /// The attribute has a value. Relationships need at least one target.
    Required {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
    },
    /// The attribute's current value changed within the age limit. Rules only.
    Stale {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
        /// Maximum value age in seconds, from 1 to 31536000 (one year).
        #[schemars(range(min = 1, max = 31_536_000))]
        max_age_seconds: u64,
    },
    /// The entity has the system tag.
    HasTag {
        #[schemars(length(min = 1, max = 128))]
        tag: String,
    },
    /// The entity does not have the system tag.
    MissingTag {
        #[schemars(length(min = 1, max = 128))]
        tag: String,
    },
    /// Compares an attribute with another attribute or a literal value. Set
    /// exactly one of `other_attribute_code`, `subject_attribute_code`
    /// (inside `linked` or `referenced_by` only) and `value`.
    Compare {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
        op: CompareOp,
        /// Another attribute of the same record.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(regex(pattern = crate::CODE_PATTERN))]
        other_attribute_code: Option<String>,
        /// An attribute of the record being checked, when comparing from a linked record.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(regex(pattern = crate::CODE_PATTERN))]
        subject_attribute_code: Option<String>,
        /// A literal string, number or boolean. Write dates as `"2026-01-31"` strings.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
    },
    /// The attribute's value is one of the listed values, such as status codes.
    OneOf {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
        #[schemars(length(min = 1, max = 100))]
        values: Vec<Value>,
    },
    /// Compares a date or datetime attribute with now plus `offset_days`.
    RelativeDate {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        attribute_code: String,
        /// One of `lt`, `lte`, `gt`, `gte`.
        op: CompareOp,
        /// Days added to the current time; negative values look back.
        #[serde(default)]
        #[schemars(range(min = -36_500, max = 36_500))]
        offset_days: i64,
    },
    /// No other live entity of the blueprint revision has the same values. Rules only.
    Unique {
        #[schemars(length(min = 1, max = 4))]
        attribute_codes: Vec<String>,
    },
    /// Checks the records linked through a relationship attribute (one hop).
    Linked {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        relationship_code: String,
        #[serde(default)]
        quantifier: Quantifier,
        /// Evaluated on each linked record. Cannot follow further links.
        predicate: Box<Predicate>,
    },
    /// Counts records of another blueprint whose relationship targets this
    /// record and that match the optional predicate. Set `min`, `max` or both.
    ReferencedBy {
        #[schemars(regex(pattern = crate::CODE_PATTERN))]
        blueprint_code: String,
        #[schemars(regex(pattern = crate::CODE_PATTERN))]
        relationship_code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        predicate: Option<Box<Predicate>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(max = 1_000))]
        min: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(max = 1_000))]
        max: Option<u32>,
    },
    /// Following the relationship from this record never returns to it. Rules only.
    Acyclic {
        #[schemars(regex(pattern = crate::CODE_PATTERN), extend("x-attricat-reference" = "attribute"))]
        relationship_code: String,
    },
    /// Every nested predicate holds.
    AllOf {
        #[schemars(length(min = 1, max = 16))]
        predicates: Vec<Predicate>,
    },
    /// At least one nested predicate holds.
    AnyOf {
        #[schemars(length(min = 1, max = 16))]
        predicates: Vec<Predicate>,
    },
}

/// A named predicate in an entity schema (`x-attricat-checks`) or on a status
/// transition edge (`conditions`).
#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// Stable identifier reported in errors.
    #[schemars(regex(pattern = crate::CODE_PATTERN))]
    pub code: String,
    /// Message shown when the check fails. Defaults to a generated message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 500))]
    pub message: Option<String>,
    pub predicate: Predicate,
}

/// Where a predicate is used. Enforced predicates run synchronously inside a
/// write transaction, so they must be bounded to the record and one hop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    Finding,
    Enforced,
}

/// Attribute type lookup for static validation.
pub trait AttributeTypes {
    /// `None` when the attribute is not declared; `Some(None)` when it is
    /// declared but its value type is not known yet.
    fn attribute_type(&self, code: &str) -> Option<Option<&str>>;
}

impl AttributeTypes for HashMap<String, Option<String>> {
    fn attribute_type(&self, code: &str) -> Option<Option<&str>> {
        self.get(code).map(Option::as_deref)
    }
}

impl AttributeTypes for HashMap<String, String> {
    fn attribute_type(&self, code: &str) -> Option<Option<&str>> {
        self.get(code).map(|value| Some(value.as_str()))
    }
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    current: Option<&'a dyn AttributeTypes>,
    subject: Option<&'a dyn AttributeTypes>,
    nested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Number,
    Date,
    Datetime,
    Set,
    Text,
    Boolean,
    Other,
    File,
}

fn category(value_type: &str) -> Category {
    match value_type {
        "number" | "integer" => Category::Number,
        "date" => Category::Date,
        "datetime" => Category::Datetime,
        "relationship" => Category::Set,
        "string" => Category::Text,
        "boolean" => Category::Boolean,
        "file" => Category::File,
        _ => Category::Other,
    }
}

/// Strictly validates one predicate. Pass `attributes` when the record's
/// attribute types are known; unknown codes are then rejected.
pub fn validate_predicate(
    predicate: &Predicate,
    attributes: Option<&dyn AttributeTypes>,
    usage: Usage,
) -> Result<(), String> {
    let mut nodes = 0;
    walk(
        predicate,
        Scope {
            current: attributes,
            subject: None,
            nested: false,
        },
        1,
        &mut nodes,
        usage,
    )
}

/// Validates a check list: unique codes, bounded size and valid predicates.
pub fn validate_checks(
    checks: &[Check],
    attributes: Option<&dyn AttributeTypes>,
    usage: Usage,
    max: usize,
) -> Result<(), String> {
    if checks.len() > max {
        return Err(format!("at most {max} checks are allowed"));
    }
    let mut codes = HashSet::new();
    for check in checks {
        if !crate::is_valid_code(&check.code) || check.code.len() > 128 {
            return Err(format!("invalid check code '{}'", check.code));
        }
        if !codes.insert(check.code.as_str()) {
            return Err(format!("duplicate check code '{}'", check.code));
        }
        if let Some(message) = &check.message
            && (message.trim().is_empty() || message.chars().count() > MAX_MESSAGE_LENGTH)
        {
            return Err(format!(
                "check '{}' message must be 1-{MAX_MESSAGE_LENGTH} characters",
                check.code
            ));
        }
        validate_predicate(&check.predicate, attributes, usage)
            .map_err(|message| format!("check '{}': {message}", check.code))?;
    }
    Ok(())
}

/// Reads and syntactically validates an entity schema's `x-attricat-checks`.
pub fn entity_checks(schema: &Value) -> Result<Vec<Check>, String> {
    let Some(value) = schema.get(CHECKS_KEY) else {
        return Ok(Vec::new());
    };
    let checks: Vec<Check> = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid {CHECKS_KEY}: {error}"))?;
    validate_checks(&checks, None, Usage::Enforced, MAX_ENTITY_CHECKS)
        .map_err(|message| format!("invalid {CHECKS_KEY}: {message}"))?;
    Ok(checks)
}

fn walk(
    predicate: &Predicate,
    scope: Scope<'_>,
    depth: usize,
    nodes: &mut usize,
    usage: Usage,
) -> Result<(), String> {
    *nodes += 1;
    if depth > MAX_DEPTH {
        return Err(format!("predicates can be nested at most {MAX_DEPTH} deep"));
    }
    if *nodes > MAX_NODES {
        return Err(format!("a predicate can have at most {MAX_NODES} parts"));
    }
    let rules_only = |name: &str| {
        if usage == Usage::Enforced {
            Err(format!(
                "'{name}' is not safe for synchronous evaluation and is only available in rules that report findings"
            ))
        } else if scope.nested {
            Err(format!("'{name}' cannot be used on linked records"))
        } else {
            Ok(())
        }
    };
    match predicate {
        Predicate::Required { attribute_code } => {
            attribute(scope.current, attribute_code)?;
        }
        Predicate::Stale {
            attribute_code,
            max_age_seconds,
        } => {
            rules_only("stale")?;
            attribute(scope.current, attribute_code)?;
            if !(1..=31_536_000).contains(max_age_seconds) {
                return Err("stale max_age_seconds must be between 1 and 31536000".into());
            }
        }
        Predicate::HasTag { tag } | Predicate::MissingTag { tag } => {
            if tag.trim().is_empty() || tag.len() > 128 {
                return Err("tag must be a non-empty string no longer than 128 bytes".into());
            }
        }
        Predicate::Compare {
            attribute_code,
            op,
            other_attribute_code,
            subject_attribute_code,
            value,
        } => {
            let left = attribute(scope.current, attribute_code)?.map(category);
            let operands = usize::from(other_attribute_code.is_some())
                + usize::from(subject_attribute_code.is_some())
                + usize::from(value.is_some());
            if operands != 1 {
                return Err(
                    "compare needs exactly one of other_attribute_code, subject_attribute_code or value"
                        .into(),
                );
            }
            if subject_attribute_code.is_some() && !scope.nested {
                return Err(
                    "subject_attribute_code is only available inside linked or referenced_by"
                        .into(),
                );
            }
            let right = if let Some(code) = other_attribute_code {
                if code == attribute_code {
                    return Err("compare needs two different attributes".into());
                }
                attribute(scope.current, code)?.map(category)
            } else if let Some(code) = subject_attribute_code {
                attribute(scope.subject, code)?.map(category)
            } else {
                None
            };
            for side in [left, right].into_iter().flatten() {
                if side == Category::File {
                    return Err("file attributes cannot be compared".into());
                }
                if op.is_ordering()
                    && !matches!(side, Category::Number | Category::Date | Category::Datetime)
                {
                    return Err(format!(
                        "'{}' orders only numbers, dates and datetimes",
                        serde_name(op)
                    ));
                }
                if *op == CompareOp::Disjoint && side != Category::Set {
                    return Err("'disjoint' compares relationship attributes only".into());
                }
            }
            if let (Some(left), Some(right)) = (left, right)
                && left != right
            {
                return Err(format!(
                    "'{attribute_code}' and the compared attribute have incompatible types"
                ));
            }
            if let Some(value) = value {
                literal(value, left, *op)?;
            }
        }
        Predicate::OneOf {
            attribute_code,
            values,
        } => {
            let kind = attribute(scope.current, attribute_code)?.map(category);
            if matches!(kind, Some(Category::Set | Category::File)) {
                return Err("one_of does not apply to relationship or file attributes".into());
            }
            if values.is_empty() || values.len() > MAX_ONE_OF_VALUES {
                return Err(format!("one_of needs 1-{MAX_ONE_OF_VALUES} values"));
            }
            for value in values {
                literal(value, kind, CompareOp::Eq)?;
            }
        }
        Predicate::RelativeDate {
            attribute_code,
            op,
            offset_days,
        } => {
            let kind = attribute(scope.current, attribute_code)?.map(category);
            if kind.is_some_and(|kind| !matches!(kind, Category::Date | Category::Datetime)) {
                return Err("relative_date needs a date or datetime attribute".into());
            }
            if !op.is_ordering() {
                return Err("relative_date op must be lt, lte, gt or gte".into());
            }
            if offset_days.abs() > MAX_OFFSET_DAYS {
                return Err(format!(
                    "offset_days must be between -{MAX_OFFSET_DAYS} and {MAX_OFFSET_DAYS}"
                ));
            }
        }
        Predicate::Unique { attribute_codes } => {
            rules_only("unique")?;
            if attribute_codes.is_empty() || attribute_codes.len() > MAX_UNIQUE_ATTRIBUTES {
                return Err(format!(
                    "unique needs 1-{MAX_UNIQUE_ATTRIBUTES} attribute codes"
                ));
            }
            let mut seen = HashSet::new();
            for code in attribute_codes {
                if !seen.insert(code) {
                    return Err(format!("duplicate unique attribute '{code}'"));
                }
                if attribute(scope.current, code)?
                    .map(category)
                    .is_some_and(|kind| {
                        matches!(kind, Category::Set | Category::File | Category::Other)
                    })
                {
                    return Err(
                        "unique keys must be string, number, boolean or date attributes".into(),
                    );
                }
            }
        }
        Predicate::Acyclic { relationship_code } => {
            rules_only("acyclic")?;
            relationship(scope.current, relationship_code)?;
        }
        Predicate::Linked {
            relationship_code,
            predicate,
            ..
        } => {
            if scope.nested {
                return Err("linked predicates follow one relationship hop only".into());
            }
            relationship(scope.current, relationship_code)?;
            walk_nested(predicate, scope, depth, nodes, usage)?;
        }
        Predicate::ReferencedBy {
            blueprint_code,
            relationship_code,
            predicate,
            min,
            max,
        } => {
            if scope.nested {
                return Err("referenced_by follows one relationship hop only".into());
            }
            for code in [blueprint_code, relationship_code] {
                if !crate::is_valid_code(code) {
                    return Err(format!("invalid code '{code}'"));
                }
            }
            match (min, max) {
                (None, None) => return Err("referenced_by needs min, max or both".into()),
                (Some(min), Some(max)) if min > max => {
                    return Err("referenced_by min cannot exceed max".into());
                }
                _ => {}
            }
            if [min, max]
                .into_iter()
                .flatten()
                .any(|bound| *bound as usize > MAX_REFERENCING_RECORDS)
            {
                return Err(format!(
                    "referenced_by bounds cannot exceed {MAX_REFERENCING_RECORDS}"
                ));
            }
            if let Some(predicate) = predicate {
                walk_nested(predicate, scope, depth, nodes, usage)?;
            }
        }
        Predicate::AllOf { predicates } | Predicate::AnyOf { predicates } => {
            if predicates.is_empty() || predicates.len() > MAX_BRANCHES {
                return Err(format!(
                    "all_of and any_of need 1-{MAX_BRANCHES} predicates"
                ));
            }
            for predicate in predicates {
                walk(predicate, scope, depth + 1, nodes, usage)?;
            }
        }
    }
    Ok(())
}

fn walk_nested(
    predicate: &Predicate,
    scope: Scope<'_>,
    depth: usize,
    nodes: &mut usize,
    usage: Usage,
) -> Result<(), String> {
    walk(
        predicate,
        Scope {
            current: None,
            subject: scope.current,
            nested: true,
        },
        depth + 1,
        nodes,
        usage,
    )
}

fn serde_name(op: &CompareOp) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn attribute<'a>(
    types: Option<&'a dyn AttributeTypes>,
    code: &str,
) -> Result<Option<&'a str>, String> {
    if !crate::is_valid_code(code) {
        return Err(format!("invalid attribute code '{code}'"));
    }
    match types {
        None => Ok(None),
        Some(types) => types
            .attribute_type(code)
            .ok_or_else(|| format!("unknown attribute '{code}'")),
    }
}

fn relationship(types: Option<&dyn AttributeTypes>, code: &str) -> Result<(), String> {
    match attribute(types, code)? {
        Some(value_type) if value_type != "relationship" => {
            Err(format!("'{code}' is not a relationship attribute"))
        }
        _ => Ok(()),
    }
}

fn literal(value: &Value, kind: Option<Category>, op: CompareOp) -> Result<(), String> {
    let valid = match (kind, value) {
        (_, Value::Null | Value::Array(_) | Value::Object(_)) => false,
        (Some(Category::Set | Category::File), _) => false,
        (Some(Category::Number), Value::Number(_)) => true,
        (Some(Category::Date), Value::String(text)) => parse_date(text).is_some(),
        (Some(Category::Datetime), Value::String(text)) => parse_datetime(text).is_some(),
        (Some(Category::Text), Value::String(_)) => true,
        (Some(Category::Boolean), Value::Bool(_)) => true,
        (Some(Category::Other), _) => !op.is_ordering(),
        (Some(_), _) => false,
        (None, Value::Number(_)) => true,
        (None, Value::String(text)) => {
            !op.is_ordering() || parse_date(text).is_some() || parse_datetime(text).is_some()
        }
        (None, Value::Bool(_)) => !op.is_ordering(),
    };
    if valid {
        Ok(())
    } else {
        Err(format!("value {value} does not match the attribute type"))
    }
}

impl Predicate {
    /// Whether the predicate is bounded to the record and one relationship hop
    /// and so can run inside a write transaction.
    pub fn is_sync_safe(&self) -> bool {
        match self {
            Self::Stale { .. } | Self::Unique { .. } | Self::Acyclic { .. } => false,
            Self::Linked { predicate, .. } => predicate.is_sync_safe(),
            Self::ReferencedBy { predicate, .. } => predicate
                .as_ref()
                .is_none_or(|predicate| predicate.is_sync_safe()),
            Self::AllOf { predicates } | Self::AnyOf { predicates } => {
                predicates.iter().all(Self::is_sync_safe)
            }
            _ => true,
        }
    }

    /// Records the related data the host must load before evaluation.
    pub fn collect_requirements(&self, requirements: &mut Requirements) {
        match self {
            Self::Stale { attribute_code, .. } => {
                requirements.change_times.insert(attribute_code.clone());
            }
            Self::Unique { attribute_codes } => {
                requirements.unique.insert(attribute_codes.clone());
            }
            Self::Acyclic { relationship_code } => {
                requirements.acyclic.insert(relationship_code.clone());
            }
            Self::Linked {
                relationship_code, ..
            } => {
                requirements.linked.insert(relationship_code.clone());
            }
            Self::ReferencedBy {
                blueprint_code,
                relationship_code,
                ..
            } => {
                requirements
                    .referenced_by
                    .insert((blueprint_code.clone(), relationship_code.clone()));
            }
            Self::AllOf { predicates } | Self::AnyOf { predicates } => {
                for predicate in predicates {
                    predicate.collect_requirements(requirements);
                }
            }
            _ => {}
        }
    }

    /// Attributes of the checked record that the predicate reads, in
    /// declaration order. Forms highlight these fields on a violation.
    pub fn subject_attributes(&self) -> Vec<String> {
        let mut attributes = Vec::new();
        self.collect_subject_attributes(false, &mut attributes);
        attributes
    }

    fn collect_subject_attributes(&self, nested: bool, attributes: &mut Vec<String>) {
        let mut push = |code: &String| {
            if !attributes.contains(code) {
                attributes.push(code.clone());
            }
        };
        match self {
            Self::Required { attribute_code }
            | Self::Stale { attribute_code, .. }
            | Self::OneOf { attribute_code, .. }
            | Self::RelativeDate { attribute_code, .. }
                if !nested =>
            {
                push(attribute_code)
            }
            Self::Compare {
                attribute_code,
                other_attribute_code,
                subject_attribute_code,
                ..
            } => {
                if !nested {
                    push(attribute_code);
                    if let Some(code) = other_attribute_code {
                        push(code);
                    }
                } else if let Some(code) = subject_attribute_code {
                    push(code);
                }
            }
            Self::Unique { attribute_codes } => attribute_codes.iter().for_each(push),
            Self::Acyclic { relationship_code } => push(relationship_code),
            Self::Linked {
                relationship_code,
                predicate,
                ..
            } => {
                push(relationship_code);
                predicate.collect_subject_attributes(true, attributes);
            }
            Self::ReferencedBy {
                predicate: Some(predicate),
                ..
            } => predicate.collect_subject_attributes(true, attributes),
            Self::AllOf { predicates } | Self::AnyOf { predicates } => {
                for predicate in predicates {
                    predicate.collect_subject_attributes(nested, attributes);
                }
            }
            _ => {}
        }
    }
}

/// Related data a predicate needs beyond the record's own resolved values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requirements {
    /// Relationship attributes whose targets must be loaded.
    pub linked: BTreeSet<String>,
    /// `(blueprint_code, relationship_code)` pairs of referencing records.
    pub referenced_by: BTreeSet<(String, String)>,
    pub unique: BTreeSet<Vec<String>>,
    pub acyclic: BTreeSet<String>,
    /// Attributes whose last change time is needed.
    pub change_times: BTreeSet<String>,
}

impl Requirements {
    pub fn for_predicates<'a>(predicates: impl IntoIterator<Item = &'a Predicate>) -> Self {
        let mut requirements = Self::default();
        for predicate in predicates {
            predicate.collect_requirements(&mut requirements);
        }
        requirements
    }
}

/// One record's resolved values in a single context.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub id: String,
    /// Scalars in native JSON form; relationships as arrays of target UUID strings.
    pub values: Map<String, Value>,
    pub tags: Vec<String>,
    pub changed_at: HashMap<String, DateTime<Utc>>,
}

/// A bounded record list. `truncated` means the host stopped at its limit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordSet {
    pub records: Vec<Record>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CycleState {
    #[default]
    Clear,
    /// Entity IDs on the cycle, starting and ending at the record.
    Cycle(Vec<String>),
    LimitReached,
}

/// Host-loaded related data, keyed as in [`Requirements`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Related {
    pub linked: HashMap<String, RecordSet>,
    pub referenced_by: HashMap<(String, String), RecordSet>,
    /// Other entity IDs sharing each unique key.
    pub duplicates: HashMap<Vec<String>, Vec<String>>,
    pub cycles: HashMap<String, CycleState>,
}

pub struct Evaluation<'a> {
    pub subject: &'a Record,
    pub related: &'a Related,
    pub now: DateTime<Utc>,
}

/// Why a predicate does not hold.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Failure {
    pub message: String,
    /// Attributes of the checked record involved in the failure.
    pub attributes: Vec<String>,
    pub evidence: Value,
}

/// Evaluates a check, applying its custom message.
pub fn evaluate_check(check: &Check, input: &Evaluation<'_>) -> Result<(), Failure> {
    evaluate(&check.predicate, input).map_err(|mut failure| {
        if let Some(message) = &check.message {
            failure.message = message.clone();
        }
        failure
    })
}

/// Evaluates a predicate against the subject record.
pub fn evaluate(predicate: &Predicate, input: &Evaluation<'_>) -> Result<(), Failure> {
    evaluate_on(predicate, input.subject, input).map_err(|(message, evidence)| Failure {
        message,
        attributes: predicate.subject_attributes(),
        evidence,
    })
}

type Outcome = Result<(), (String, Value)>;

fn present(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(_) => true,
    }
}

fn evaluate_on(predicate: &Predicate, current: &Record, input: &Evaluation<'_>) -> Outcome {
    match predicate {
        Predicate::Required { attribute_code } => {
            if present(current.values.get(attribute_code)) {
                Ok(())
            } else {
                Err((
                    format!("'{attribute_code}' is required"),
                    json!({ "attribute_code": attribute_code }),
                ))
            }
        }
        Predicate::Stale {
            attribute_code,
            max_age_seconds,
        } => {
            let changed = current.changed_at.get(attribute_code);
            let stale = changed
                .is_none_or(|time| (input.now - *time).num_seconds() > *max_age_seconds as i64);
            if stale {
                Err((
                    format!("Attribute '{attribute_code}' is older than {max_age_seconds} seconds"),
                    json!({"attribute_code": attribute_code, "last_changed_at": changed, "max_age_seconds": max_age_seconds}),
                ))
            } else {
                Ok(())
            }
        }
        Predicate::HasTag { tag } => {
            if current.tags.iter().any(|item| item == tag) {
                Ok(())
            } else {
                Err((
                    format!("Entity is missing required system tag '{tag}'"),
                    json!({ "tag": tag }),
                ))
            }
        }
        Predicate::MissingTag { tag } => {
            if current.tags.iter().any(|item| item == tag) {
                Err((
                    format!("Entity has prohibited system tag '{tag}'"),
                    json!({ "tag": tag }),
                ))
            } else {
                Ok(())
            }
        }
        Predicate::Compare {
            attribute_code,
            op,
            other_attribute_code,
            subject_attribute_code,
            value,
        } => {
            let left = current.values.get(attribute_code);
            let (right, right_label) = if let Some(code) = other_attribute_code {
                (current.values.get(code), format!("'{code}'"))
            } else if let Some(code) = subject_attribute_code {
                (
                    input.subject.values.get(code),
                    format!("'{code}' of this record"),
                )
            } else {
                (
                    value.as_ref(),
                    value.as_ref().map(Value::to_string).unwrap_or_default(),
                )
            };
            let (Some(left), Some(right)) = (
                left.filter(|value| present(Some(value))),
                right.filter(|value| present(Some(value))),
            ) else {
                return Ok(());
            };
            let evidence =
                json!({ "attribute_code": attribute_code, "value": left, "compared_with": right });
            match compare(*op, left, right) {
                Some(true) => Ok(()),
                Some(false) => Err((
                    format!("'{attribute_code}' must be {} {right_label}", op.phrase()),
                    evidence,
                )),
                None => Err((
                    format!("'{attribute_code}' cannot be compared with {right_label}"),
                    evidence,
                )),
            }
        }
        Predicate::OneOf {
            attribute_code,
            values,
        } => match current.values.get(attribute_code) {
            Some(value) if present(Some(value)) && !values.contains(value) => Err((
                format!(
                    "'{attribute_code}' must be one of {}",
                    values
                        .iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                json!({ "attribute_code": attribute_code, "value": value }),
            )),
            _ => Ok(()),
        },
        Predicate::RelativeDate {
            attribute_code,
            op,
            offset_days,
        } => {
            let Some(value) = current
                .values
                .get(attribute_code)
                .filter(|value| present(Some(value)))
            else {
                return Ok(());
            };
            let threshold = input.now + Duration::days(*offset_days);
            let holds = match value.as_str() {
                Some(text) => {
                    if let Some(date) = parse_date(text) {
                        Some(ordering_holds(*op, date.cmp(&threshold.date_naive())))
                    } else {
                        parse_datetime(text)
                            .map(|instant| ordering_holds(*op, instant.cmp(&threshold)))
                    }
                }
                None => None,
            };
            let relative = match offset_days {
                0 => "now".to_owned(),
                days if days > &0 => format!("{days} days from now"),
                days => format!("{} days ago", -days),
            };
            match holds {
                Some(true) => Ok(()),
                _ => Err((
                    format!("'{attribute_code}' must be {} {relative}", op.phrase()),
                    json!({ "attribute_code": attribute_code, "value": value, "threshold": threshold }),
                )),
            }
        }
        Predicate::Unique { attribute_codes } => {
            if attribute_codes
                .iter()
                .any(|code| !present(current.values.get(code)))
            {
                return Ok(());
            }
            match input.related.duplicates.get(attribute_codes) {
                Some(others) if !others.is_empty() => Err((
                    format!(
                        "Another record has the same {}",
                        attribute_codes
                            .iter()
                            .map(|code| format!("'{code}'"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    json!({ "attribute_codes": attribute_codes, "duplicate_entity_ids": others }),
                )),
                _ => Ok(()),
            }
        }
        Predicate::Acyclic { relationship_code } => {
            match input.related.cycles.get(relationship_code) {
                Some(CycleState::Cycle(path)) => Err((
                    format!("'{relationship_code}' forms a cycle"),
                    json!({ "relationship_code": relationship_code, "cycle": path }),
                )),
                Some(CycleState::LimitReached) => Err((
                    format!(
                        "'{relationship_code}' could not be checked for cycles within {MAX_CYCLE_VISITS} records"
                    ),
                    json!({ "relationship_code": relationship_code, "limit": MAX_CYCLE_VISITS }),
                )),
                _ => Ok(()),
            }
        }
        Predicate::Linked {
            relationship_code,
            quantifier,
            predicate,
        } => {
            let empty = RecordSet::default();
            let set = input
                .related
                .linked
                .get(relationship_code)
                .unwrap_or(&empty);
            if set.truncated {
                return Err((
                    format!(
                        "'{relationship_code}' has more than {MAX_LINKED_RECORDS} linked records to check"
                    ),
                    json!({ "relationship_code": relationship_code, "limit": MAX_LINKED_RECORDS }),
                ));
            }
            let mut matching = Vec::new();
            let mut failing = Vec::new();
            let mut first_failure = None;
            for record in &set.records {
                match evaluate_on(predicate, record, input) {
                    Ok(()) => matching.push(record.id.clone()),
                    Err((message, _)) => {
                        first_failure.get_or_insert(message);
                        failing.push(record.id.clone());
                    }
                }
            }
            let describe = || {
                first_failure
                    .clone()
                    .unwrap_or_else(|| describe_predicate(predicate))
            };
            let result = match quantifier {
                Quantifier::All if !failing.is_empty() => Some(format!(
                    "Every '{relationship_code}' record must satisfy the check: {}",
                    describe()
                )),
                Quantifier::Any if matching.is_empty() => Some(format!(
                    "At least one '{relationship_code}' record must satisfy the check: {}",
                    describe_predicate(predicate)
                )),
                Quantifier::None if !matching.is_empty() => Some(format!(
                    "No '{relationship_code}' record may satisfy: {}",
                    describe_predicate(predicate)
                )),
                _ => None,
            };
            match result {
                None => Ok(()),
                Some(message) => Err((
                    message,
                    json!({
                        "relationship_code": relationship_code,
                        "quantifier": quantifier,
                        "failing_entity_ids": if *quantifier == Quantifier::None { &matching } else { &failing },
                    }),
                )),
            }
        }
        Predicate::ReferencedBy {
            blueprint_code,
            relationship_code,
            predicate,
            min,
            max,
        } => {
            let key = (blueprint_code.clone(), relationship_code.clone());
            let empty = RecordSet::default();
            let set = input.related.referenced_by.get(&key).unwrap_or(&empty);
            if set.truncated {
                return Err((
                    format!(
                        "More than {MAX_REFERENCING_RECORDS} '{blueprint_code}' records reference this one; the check cannot be evaluated"
                    ),
                    json!({ "blueprint_code": blueprint_code, "relationship_code": relationship_code, "limit": MAX_REFERENCING_RECORDS }),
                ));
            }
            let matching: Vec<_> = set
                .records
                .iter()
                .filter(|record| {
                    predicate
                        .as_ref()
                        .is_none_or(|predicate| evaluate_on(predicate, record, input).is_ok())
                })
                .map(|record| record.id.clone())
                .collect();
            let count = matching.len() as u32;
            let what = predicate
                .as_ref()
                .map(|predicate| format!(" matching {}", describe_predicate(predicate)))
                .unwrap_or_default();
            let failure = match (min, max) {
                (_, Some(0)) if count > 0 => Some(format!(
                    "No '{blueprint_code}' record{what} may reference this record through '{relationship_code}' (found {count})"
                )),
                (_, Some(max)) if count > *max => Some(format!(
                    "At most {max} '{blueprint_code}' records{what} may reference this record through '{relationship_code}' (found {count})"
                )),
                (Some(min), _) if count < *min => Some(format!(
                    "At least {min} '{blueprint_code}' records{what} must reference this record through '{relationship_code}' (found {count})"
                )),
                _ => None,
            };
            match failure {
                None => Ok(()),
                Some(message) => Err((
                    message,
                    json!({ "blueprint_code": blueprint_code, "relationship_code": relationship_code, "count": count, "min": min, "max": max, "matching_entity_ids": matching }),
                )),
            }
        }
        Predicate::AllOf { predicates } => {
            for predicate in predicates {
                evaluate_on(predicate, current, input)?;
            }
            Ok(())
        }
        Predicate::AnyOf { predicates } => {
            let mut messages = Vec::with_capacity(predicates.len());
            for predicate in predicates {
                match evaluate_on(predicate, current, input) {
                    Ok(()) => return Ok(()),
                    Err((message, _)) => messages.push(message),
                }
            }
            Err((
                format!("None of the alternatives hold: {}", messages.join("; or ")),
                json!({ "alternatives": messages }),
            ))
        }
    }
}

/// A short description of what a predicate requires, used in messages.
pub fn describe_predicate(predicate: &Predicate) -> String {
    match predicate {
        Predicate::Required { attribute_code } => format!("'{attribute_code}' is set"),
        Predicate::Stale {
            attribute_code,
            max_age_seconds,
        } => format!("'{attribute_code}' changed within {max_age_seconds} seconds"),
        Predicate::HasTag { tag } => format!("tag '{tag}'"),
        Predicate::MissingTag { tag } => format!("no tag '{tag}'"),
        Predicate::Compare {
            attribute_code,
            op,
            other_attribute_code,
            subject_attribute_code,
            value,
        } => {
            let right = other_attribute_code
                .as_ref()
                .map(|code| format!("'{code}'"))
                .or_else(|| {
                    subject_attribute_code
                        .as_ref()
                        .map(|code| format!("'{code}' of this record"))
                })
                .or_else(|| value.as_ref().map(Value::to_string))
                .unwrap_or_default();
            format!("'{attribute_code}' {} {right}", op.phrase())
        }
        Predicate::OneOf {
            attribute_code,
            values,
        } => format!(
            "'{attribute_code}' in {}",
            values
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Predicate::RelativeDate {
            attribute_code,
            op,
            offset_days,
        } => format!(
            "'{attribute_code}' {} now {:+} days",
            op.phrase(),
            offset_days
        ),
        Predicate::Unique { attribute_codes } => format!("unique {}", attribute_codes.join(", ")),
        Predicate::Linked {
            relationship_code, ..
        } => format!("linked '{relationship_code}' records"),
        Predicate::ReferencedBy { blueprint_code, .. } => {
            format!("referencing '{blueprint_code}' records")
        }
        Predicate::Acyclic { relationship_code } => format!("no '{relationship_code}' cycle"),
        Predicate::AllOf { predicates } => predicates
            .iter()
            .map(describe_predicate)
            .collect::<Vec<_>>()
            .join(" and "),
        Predicate::AnyOf { predicates } => predicates
            .iter()
            .map(describe_predicate)
            .collect::<Vec<_>>()
            .join(" or "),
    }
}

fn ordering_holds(op: CompareOp, ordering: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        CompareOp::Eq => ordering == Equal,
        CompareOp::Ne => ordering != Equal,
        CompareOp::Lt => ordering == Less,
        CompareOp::Lte => ordering != Greater,
        CompareOp::Gt => ordering == Greater,
        CompareOp::Gte => ordering != Less,
        CompareOp::Disjoint => false,
    }
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

/// Compares two present values. `None` means the values are not comparable
/// with the operator.
fn compare(op: CompareOp, left: &Value, right: &Value) -> Option<bool> {
    match (left, right) {
        (Value::Array(left), Value::Array(right)) => {
            let left: HashSet<_> = left.iter().map(Value::to_string).collect();
            let right: HashSet<_> = right.iter().map(Value::to_string).collect();
            match op {
                CompareOp::Eq => Some(left == right),
                CompareOp::Ne => Some(left != right),
                CompareOp::Disjoint => Some(left.is_disjoint(&right)),
                _ => None,
            }
        }
        (Value::Number(left), Value::Number(right)) if op != CompareOp::Disjoint => {
            let ordering = left.as_f64()?.partial_cmp(&right.as_f64()?)?;
            Some(ordering_holds(op, ordering))
        }
        (Value::String(left), Value::String(right)) if op != CompareOp::Disjoint => {
            if let (Some(left), Some(right)) = (parse_date(left), parse_date(right)) {
                return Some(ordering_holds(op, left.cmp(&right)));
            }
            if let (Some(left), Some(right)) = (parse_datetime(left), parse_datetime(right)) {
                return Some(ordering_holds(op, left.cmp(&right)));
            }
            match op {
                CompareOp::Eq => Some(left == right),
                CompareOp::Ne => Some(left != right),
                _ => None,
            }
        }
        (left, right) => match op {
            CompareOp::Eq if std::mem::discriminant(left) == std::mem::discriminant(right) => {
                Some(left == right)
            }
            CompareOp::Ne if std::mem::discriminant(left) == std::mem::discriminant(right) => {
                Some(left != right)
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types() -> HashMap<String, String> {
        [
            ("valid_from", "date"),
            ("valid_until", "date"),
            ("lower", "number"),
            ("upper", "integer"),
            ("title", "string"),
            ("source", "relationship"),
            ("target", "relationship"),
            ("facility", "relationship"),
            ("supplier", "relationship"),
            ("status", "string"),
            ("expires_at", "datetime"),
            ("photo", "file"),
        ]
        .into_iter()
        .map(|(code, kind)| (code.to_owned(), kind.to_owned()))
        .collect()
    }

    fn predicate(source: Value) -> Predicate {
        serde_json::from_value(source).unwrap()
    }

    fn validate(source: Value, usage: Usage) -> Result<(), String> {
        let types = types();
        validate_predicate(&predicate(source), Some(&types), usage)
    }

    fn record(values: Value) -> Record {
        Record {
            id: "subject".into(),
            values: values.as_object().unwrap().clone(),
            ..Record::default()
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-10-03T12:00:00Z".parse().unwrap()
    }

    fn holds(source: Value, subject: &Record, related: &Related) -> Result<(), Failure> {
        evaluate(
            &predicate(source),
            &Evaluation {
                subject,
                related,
                now: now(),
            },
        )
    }

    #[test]
    fn existing_rule_predicates_keep_their_serialized_form() {
        let stored = json!({"type": "required", "attribute_code": "title"});
        let parsed = predicate(stored.clone());
        assert_eq!(serde_json::to_value(&parsed).unwrap(), stored);
        let toml: Predicate =
            toml::from_str("type='stale'\nattribute_code='title'\nmax_age_seconds=60").unwrap();
        assert!(matches!(toml, Predicate::Stale { .. }));
    }

    #[test]
    fn rejects_unknown_fields_and_types() {
        assert!(
            serde_json::from_value::<Predicate>(
                json!({"type":"required","attribute_code":"a","x":1})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<Predicate>(json!({"type":"sql","query":"x"})).is_err());
    }

    #[test]
    fn validates_comparisons_against_attribute_types() {
        let enforced = Usage::Enforced;
        assert!(validate(json!({"type":"compare","attribute_code":"valid_from","op":"lte","other_attribute_code":"valid_until"}), enforced).is_ok());
        assert!(validate(json!({"type":"compare","attribute_code":"lower","op":"lte","other_attribute_code":"upper"}), enforced).is_ok());
        assert!(validate(json!({"type":"compare","attribute_code":"source","op":"disjoint","other_attribute_code":"target"}), enforced).is_ok());
        assert!(
            validate(
                json!({"type":"compare","attribute_code":"title","op":"eq","value":"x"}),
                enforced
            )
            .is_ok()
        );
        for invalid in [
            json!({"type":"compare","attribute_code":"title","op":"lt","value":"x"}),
            json!({"type":"compare","attribute_code":"valid_from","op":"lt","other_attribute_code":"lower"}),
            json!({"type":"compare","attribute_code":"valid_from","op":"lt","other_attribute_code":"missing"}),
            json!({"type":"compare","attribute_code":"valid_from","op":"lt","value":"tomorrow"}),
            json!({"type":"compare","attribute_code":"valid_from","op":"lt"}),
            json!({"type":"compare","attribute_code":"valid_from","op":"lt","value":"2026-01-01","other_attribute_code":"valid_until"}),
            json!({"type":"compare","attribute_code":"title","op":"disjoint","other_attribute_code":"status"}),
            json!({"type":"compare","attribute_code":"title","op":"eq","subject_attribute_code":"status"}),
            json!({"type":"compare","attribute_code":"photo","op":"eq","value":"x"}),
            json!({"type":"compare","attribute_code":"source","op":"eq","value":"x"}),
        ] {
            assert!(validate(invalid.clone(), enforced).is_err(), "{invalid}");
        }
    }

    #[test]
    fn enforced_usage_rejects_unbounded_predicates() {
        for source in [
            json!({"type":"unique","attribute_codes":["title"]}),
            json!({"type":"acyclic","relationship_code":"source"}),
            json!({"type":"stale","attribute_code":"title","max_age_seconds":5}),
            json!({"type":"all_of","predicates":[{"type":"unique","attribute_codes":["title"]}]}),
        ] {
            assert!(validate(source.clone(), Usage::Finding).is_ok(), "{source}");
            assert!(
                validate(source.clone(), Usage::Enforced).is_err(),
                "{source}"
            );
            assert!(!predicate(source).is_sync_safe());
        }
    }

    #[test]
    fn linked_predicates_are_one_hop_and_bounded() {
        assert!(validate(json!({"type":"linked","relationship_code":"facility","predicate":{"type":"compare","attribute_code":"supplier","op":"eq","subject_attribute_code":"supplier"}}), Usage::Enforced).is_ok());
        assert!(validate(json!({"type":"linked","relationship_code":"title","predicate":{"type":"required","attribute_code":"x"}}), Usage::Enforced).is_err());
        assert!(validate(json!({"type":"linked","relationship_code":"facility","predicate":{"type":"linked","relationship_code":"x","predicate":{"type":"required","attribute_code":"x"}}}), Usage::Enforced).is_err());
        assert!(validate(json!({"type":"linked","relationship_code":"facility","predicate":{"type":"compare","attribute_code":"supplier","op":"eq","subject_attribute_code":"missing"}}), Usage::Enforced).is_err());
        assert!(
            validate(
                json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc"}),
                Usage::Enforced
            )
            .is_err()
        );
        assert!(validate(json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","min":3,"max":1}), Usage::Enforced).is_err());
        assert!(validate(json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","max":0,"predicate":{"type":"one_of","attribute_code":"status","values":["open"]}}), Usage::Enforced).is_ok());
        let deep = json!({"type":"all_of","predicates":[{"type":"all_of","predicates":[{"type":"all_of","predicates":[{"type":"all_of","predicates":[{"type":"required","attribute_code":"title"}]}]}]}]});
        assert!(validate(deep, Usage::Enforced).is_err());
    }

    #[test]
    fn compares_numbers_dates_strings_and_sets() {
        let related = Related::default();
        let subject = record(json!({
            "valid_from": "2026-01-02", "valid_until": "2026-01-01",
            "lower": 5, "upper": 10, "title": "A",
            "source": ["a", "b"], "target": ["b"], "other": ["c"]
        }));
        let range = json!({"type":"compare","attribute_code":"valid_from","op":"lte","other_attribute_code":"valid_until"});
        let failure = holds(range, &subject, &related).unwrap_err();
        assert_eq!(failure.attributes, vec!["valid_from", "valid_until"]);
        assert!(holds(json!({"type":"compare","attribute_code":"lower","op":"lt","other_attribute_code":"upper"}), &subject, &related).is_ok());
        assert!(
            holds(
                json!({"type":"compare","attribute_code":"title","op":"eq","value":"A"}),
                &subject,
                &related
            )
            .is_ok()
        );
        assert!(holds(json!({"type":"compare","attribute_code":"source","op":"disjoint","other_attribute_code":"target"}), &subject, &related).is_err());
        assert!(holds(json!({"type":"compare","attribute_code":"source","op":"disjoint","other_attribute_code":"other"}), &subject, &related).is_ok());
        assert!(holds(json!({"type":"compare","attribute_code":"source","op":"ne","other_attribute_code":"target"}), &subject, &related).is_ok());
        // Missing operands hold; requiredness is a separate predicate.
        assert!(holds(json!({"type":"compare","attribute_code":"missing","op":"lt","other_attribute_code":"upper"}), &subject, &related).is_ok());
        assert!(
            holds(
                json!({"type":"compare","attribute_code":"title","op":"lt","value":3}),
                &subject,
                &related
            )
            .is_err()
        );
    }

    #[test]
    fn relative_dates_support_expiry() {
        let related = Related::default();
        let subject =
            record(json!({"valid_until": "2026-10-20", "expires_at": "2026-10-01T00:00:00Z"}));
        let within_30_days = json!({"type":"relative_date","attribute_code":"valid_until","op":"gte","offset_days":30});
        assert!(holds(within_30_days, &subject, &related).is_err());
        assert!(holds(json!({"type":"relative_date","attribute_code":"valid_until","op":"gte","offset_days":7}), &subject, &related).is_ok());
        assert!(
            holds(
                json!({"type":"relative_date","attribute_code":"expires_at","op":"gt"}),
                &subject,
                &related
            )
            .is_err()
        );
    }

    #[test]
    fn evaluates_linked_and_referencing_records() {
        let subject = record(json!({"supplier": ["s1"], "facility": ["f1", "f2"]}));
        let linked = |supplier: &str, id: &str| Record {
            id: id.into(),
            values: json!({"supplier": [supplier], "status": "released"})
                .as_object()
                .unwrap()
                .clone(),
            ..Record::default()
        };
        let mut related = Related::default();
        related.linked.insert(
            "facility".into(),
            RecordSet {
                records: vec![linked("s1", "f1"), linked("s2", "f2")],
                truncated: false,
            },
        );
        let same_supplier = json!({"type":"linked","relationship_code":"facility","predicate":{"type":"compare","attribute_code":"supplier","op":"eq","subject_attribute_code":"supplier"}});
        let failure = holds(same_supplier, &subject, &related).unwrap_err();
        assert_eq!(failure.attributes, vec!["facility", "supplier"]);
        assert_eq!(failure.evidence["failing_entity_ids"], json!(["f2"]));
        assert!(holds(json!({"type":"linked","relationship_code":"facility","quantifier":"any","predicate":{"type":"compare","attribute_code":"supplier","op":"eq","subject_attribute_code":"supplier"}}), &subject, &related).is_ok());
        assert!(holds(json!({"type":"linked","relationship_code":"facility","predicate":{"type":"one_of","attribute_code":"status","values":["released"]}}), &subject, &related).is_ok());
        related.linked.get_mut("facility").unwrap().truncated = true;
        assert!(holds(json!({"type":"linked","relationship_code":"facility","predicate":{"type":"one_of","attribute_code":"status","values":["released"]}}), &subject, &related).is_err());

        let open = |id: &str, status: &str| Record {
            id: id.into(),
            values: json!({"status": status}).as_object().unwrap().clone(),
            ..Record::default()
        };
        related.referenced_by.insert(
            ("action".into(), "nc".into()),
            RecordSet {
                records: vec![open("a1", "closed"), open("a2", "open")],
                truncated: false,
            },
        );
        let no_open = json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","max":0,"predicate":{"type":"one_of","attribute_code":"status","values":["open"]}});
        let failure = holds(no_open, &subject, &related).unwrap_err();
        assert_eq!(failure.evidence["count"], json!(1));
        assert!(holds(json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","min":2}), &subject, &related).is_ok());
        assert!(holds(json!({"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","min":3}), &subject, &related).is_err());
    }

    #[test]
    fn reports_duplicates_cycles_and_alternatives() {
        let subject = record(json!({"title": "A"}));
        let mut related = Related::default();
        related
            .duplicates
            .insert(vec!["title".into()], vec!["other".into()]);
        related
            .cycles
            .insert("parent".into(), CycleState::Cycle(vec!["subject".into()]));
        assert!(
            holds(
                json!({"type":"unique","attribute_codes":["title"]}),
                &subject,
                &related
            )
            .is_err()
        );
        assert!(
            holds(
                json!({"type":"acyclic","relationship_code":"parent"}),
                &subject,
                &related
            )
            .is_err()
        );
        assert!(holds(json!({"type":"any_of","predicates":[{"type":"required","attribute_code":"missing"},{"type":"required","attribute_code":"title"}]}), &subject, &related).is_ok());
        let failure = holds(json!({"type":"any_of","predicates":[{"type":"required","attribute_code":"a"},{"type":"required","attribute_code":"b"}]}), &subject, &related).unwrap_err();
        assert!(
            failure
                .message
                .contains("'a' is required; or 'b' is required")
        );
    }

    #[test]
    fn requirements_name_related_data() {
        let requirements = Requirements::for_predicates([&predicate(
            json!({"type":"all_of","predicates":[
                {"type":"linked","relationship_code":"facility","predicate":{"type":"required","attribute_code":"x"}},
                {"type":"referenced_by","blueprint_code":"action","relationship_code":"nc","max":0}
            ]}),
        )]);
        assert!(requirements.linked.contains("facility"));
        assert!(
            requirements
                .referenced_by
                .contains(&("action".into(), "nc".into()))
        );
    }

    #[test]
    fn entity_checks_are_strict() {
        let schema = json!({"type":"object", CHECKS_KEY: [{"code":"range","message":"Bad range","predicate":{"type":"compare","attribute_code":"a","op":"lte","other_attribute_code":"b"}}]});
        assert_eq!(entity_checks(&schema).unwrap().len(), 1);
        let duplicate = json!({CHECKS_KEY: [
            {"code":"x","predicate":{"type":"required","attribute_code":"a"}},
            {"code":"x","predicate":{"type":"required","attribute_code":"b"}}
        ]});
        assert!(entity_checks(&duplicate).is_err());
        let unsafe_check = json!({CHECKS_KEY: [{"code":"x","predicate":{"type":"unique","attribute_codes":["a"]}}]});
        assert!(entity_checks(&unsafe_check).is_err());
        let unknown = json!({CHECKS_KEY: [{"code":"x","when":1,"predicate":{"type":"required","attribute_code":"a"}}]});
        assert!(entity_checks(&unknown).is_err());
    }
}
