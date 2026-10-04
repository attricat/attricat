//! Versioned status metadata attached to a string attribute's JSON Schema.
use crate::predicate::{Check, MAX_TRANSITION_CONDITIONS, Usage, validate_checks};
use serde_json::Value;
use std::collections::HashSet;

pub const STATUS_KEY: &str = "x-attricat-status";
/// Attribute selectors in lock and approval lists: a blueprint attribute code
/// or a qualified `namespace:code` reusable attribute.
const ATTRIBUTE_SELECTOR_PATTERN: &str = "^[A-Za-z0-9_-]+(:[A-Za-z0-9_-]+)?$";
/// About one hundred years.
pub const MAX_RETENTION_DAYS: i64 = 36_600;

/// Attributes made read-only by a status, or covered by an approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusCoverage {
    /// Every attribute, relationship and file of the record except the status
    /// attribute that declares the coverage.
    All,
    Attributes(Vec<String>),
}

impl StatusCoverage {
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(all) if all == "all" => Some(Self::All),
            Value::Array(codes) => Some(Self::Attributes(
                codes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            )),
            _ => None,
        }
    }

    /// Whether `code` is covered. `status_code` is the declaring attribute,
    /// which `All` never covers: its changes are governed by transitions.
    pub fn covers(&self, code: &str, status_code: &str) -> bool {
        match self {
            Self::All => code != status_code,
            Self::Attributes(codes) => codes.iter().any(|item| item == code),
        }
    }
}

/// The approval declared by a status option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusApproval {
    pub covers: StatusCoverage,
    pub void_to: String,
}

/// Restrictions declared on one transition edge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransitionRequirements {
    pub code: Option<String>,
    pub permission: Option<String>,
    pub roles: Vec<String>,
    pub separate_from: Vec<String>,
}

impl TransitionRequirements {
    pub fn is_restricted(&self) -> bool {
        self.permission.is_some() || !self.roles.is_empty() || !self.separate_from.is_empty()
    }
}

fn status_option<'a>(schema: &'a Value, code: &Value) -> Option<&'a Value> {
    schema
        .get(STATUS_KEY)?
        .get("options")?
        .as_array()?
        .iter()
        .find(|option| &option["code"] == code)
}

/// The lock declared by the option `code`; absent and unknown codes lock nothing.
pub fn status_lock(schema: &Value, code: &Value) -> Option<StatusCoverage> {
    StatusCoverage::parse(status_option(schema, code)?.get("lock")?)
}

pub fn status_approval(schema: &Value, code: &Value) -> Option<StatusApproval> {
    let approval = status_option(schema, code)?.get("approval")?;
    Some(StatusApproval {
        covers: StatusCoverage::parse(&approval["covers"])?,
        void_to: approval["void_to"].as_str()?.to_owned(),
    })
}

pub fn status_retention_days(schema: &Value, code: &Value) -> Option<i64> {
    status_option(schema, code)?.get("retention_days")?.as_i64()
}

/// Whether any option declares a lock, an approval or a retention period.
pub fn has_record_controls(schema: &Value) -> bool {
    schema
        .get(STATUS_KEY)
        .and_then(|config| config.get("options"))
        .and_then(Value::as_array)
        .is_some_and(|options| {
            options.iter().any(|option| {
                option.get("lock").is_some()
                    || option.get("approval").is_some()
                    || option.get("retention_days").is_some()
            })
        })
}

/// The restrictions of the declared edge for a change. Unchanged values,
/// undeclared edges and unrestricted graphs have none.
pub fn transition_requirements(
    schema: &Value,
    before: &Value,
    after: &Value,
) -> TransitionRequirements {
    let edge = schema
        .get(STATUS_KEY)
        .and_then(|config| config.get("transitions"))
        .and_then(Value::as_array)
        .and_then(|edges| {
            edges
                .iter()
                .find(|edge| &edge["from"] == before && &edge["to"] == after)
        });
    let Some(edge) = edge.filter(|_| before != after) else {
        return TransitionRequirements::default();
    };
    let strings = |key: &str| {
        edge.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    TransitionRequirements {
        code: edge.get("code").and_then(Value::as_str).map(str::to_owned),
        permission: edge
            .get("permission")
            .and_then(Value::as_str)
            .map(str::to_owned),
        roles: strings("roles"),
        separate_from: strings("separate_from"),
    }
}

pub fn status_configuration_schema() -> Value {
    let code = serde_json::json!({"type": "string", "minLength": 1, "maxLength": 128, "pattern": super::CODE_PATTERN});
    let coverage = serde_json::json!({"oneOf": [
        {"const": "all"},
        {"type": "array", "minItems": 1, "maxItems": 500, "uniqueItems": true, "items": {
            "type": "string", "minLength": 1, "maxLength": 257, "pattern": ATTRIBUTE_SELECTOR_PATTERN
        }}
    ]});
    let conditions = serde_json::json!({"type": "array", "maxItems": MAX_TRANSITION_CONDITIONS, "items": {
        "type": "object", "additionalProperties": false, "required": ["code", "predicate"],
        "properties": {
            "code": code,
            "message": {"type": "string", "minLength": 1, "maxLength": crate::predicate::MAX_MESSAGE_LENGTH},
            "predicate": {"type": "object", "required": ["type"], "properties": {"type": {"type": "string"}}}
        }
    }});
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Attricat status control v1",
        "type": "object", "additionalProperties": false,
        "required": ["version", "options"],
        "properties": {
            "version": {"const": 1},
            "options": {"type": "array", "minItems": 1, "maxItems": 100, "items": {
                "type": "object", "additionalProperties": false, "required": ["code", "label"],
                "properties": {
                    "code": code,
                    "label": {"type": "string", "minLength": 1, "maxLength": 200},
                    "tone": {"enum": ["default", "success", "warning", "error", "info"]},
                    "lock": coverage,
                    "approval": {
                        "type": "object", "additionalProperties": false, "required": ["covers", "void_to"],
                        "properties": {"covers": coverage, "void_to": code}
                    },
                    "retention_days": {"type": "integer", "minimum": 1, "maximum": MAX_RETENTION_DAYS}
                }
            }},
            "transitions": {"type": "array", "maxItems": 10000, "uniqueItems": true, "items": {
                "type": "object", "additionalProperties": false, "required": ["from", "to"],
                "properties": {
                    "from": {"type": ["string", "null"]},
                    "to": {"type": ["string", "null"]},
                    "code": code,
                    "permission": {"type": "string", "maxLength": 128, "pattern": "^[a-z_]+\\.[a-z_]+$"},
                    "roles": {"type": "array", "minItems": 1, "maxItems": 20, "uniqueItems": true, "items": {
                        "type": "string", "minLength": 1, "maxLength": 64, "pattern": "^[a-z][a-z0-9_-]*$"
                    }},
                    "separate_from": {"type": "array", "minItems": 1, "maxItems": 20, "uniqueItems": true, "items": code},
                    "conditions": conditions
                }
            }}
        }
    })
}

pub fn validate_status_definition(schema: &Value) -> Result<(), String> {
    let Some(config) = schema.get(STATUS_KEY) else {
        return Ok(());
    };
    if !super::validate_json_schema(&status_configuration_schema(), config)?.is_empty() {
        return Err("invalid x-attricat-status configuration".into());
    }
    if schema.get("type").and_then(Value::as_str) != Some("string") {
        return Err("status requires a string schema".into());
    }
    let options = config["options"].as_array().unwrap();
    let codes: HashSet<_> = options
        .iter()
        .map(|option| option["code"].as_str().unwrap())
        .collect();
    let enumeration = schema
        .get("enum")
        .and_then(Value::as_array)
        .ok_or("status requires an enum matching its options")?;
    if codes.len() != options.len()
        || enumeration.len() != codes.len()
        || enumeration
            .iter()
            .any(|value| value.as_str().is_none_or(|code| !codes.contains(code)))
    {
        return Err("status enum and unique option codes must match".into());
    }
    for option in options {
        if option["label"].as_str().unwrap().trim().is_empty()
            || !super::validate_json_schema(schema, &option["code"])?.is_empty()
        {
            return Err(
                "status labels must not be blank and all options must satisfy the schema".into(),
            );
        }
    }
    let edges = config.get("transitions").and_then(Value::as_array);
    if let Some(edges) = edges {
        let mut pairs = HashSet::new();
        for edge in edges {
            for endpoint in [&edge["from"], &edge["to"]] {
                if !endpoint.is_null() && endpoint.as_str().is_none_or(|code| !codes.contains(code))
                {
                    return Err("status transition references an unknown option".into());
                }
            }
            if !pairs.insert((edge["from"].to_string(), edge["to"].to_string())) {
                return Err("status transitions must declare each from/to pair once".into());
            }
            edge_conditions(edge)?;
        }
        let edge_codes: HashSet<_> = edges
            .iter()
            .filter_map(|edge| edge.get("code").and_then(Value::as_str))
            .collect();
        if edges
            .iter()
            .filter_map(|edge| edge.get("separate_from").and_then(Value::as_array))
            .flatten()
            .any(|code| code.as_str().is_none_or(|code| !edge_codes.contains(code)))
        {
            return Err("separate_from must name the code of a declared transition".into());
        }
    }
    for option in options {
        let code = option["code"].as_str().unwrap();
        // Leaving a locked status must always be an explicit, governable edge.
        if option.get("lock").is_some() && edges.is_none() {
            return Err(format!(
                "status option '{code}' declares a lock, so transitions must be declared"
            ));
        }
        if option.get("retention_days").is_some() && option.get("lock").is_none() {
            return Err(format!(
                "status option '{code}' declares retention_days without a lock"
            ));
        }
        if let Some(void_to) = option
            .get("approval")
            .and_then(|approval| approval["void_to"].as_str())
            && (void_to == code || !codes.contains(void_to))
        {
            return Err(format!(
                "approval void_to of status option '{code}' must name another option"
            ));
        }
    }
    Ok(())
}

/// One declared edge with its optional conditions.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionEdge {
    pub from: Option<String>,
    pub to: Option<String>,
    pub conditions: Vec<Check>,
}

fn edge_conditions(edge: &Value) -> Result<Vec<Check>, String> {
    let Some(conditions) = edge.get("conditions") else {
        return Ok(Vec::new());
    };
    let conditions: Vec<Check> = serde_json::from_value(conditions.clone())
        .map_err(|error| format!("invalid status transition conditions: {error}"))?;
    validate_checks(
        &conditions,
        None,
        Usage::Enforced,
        MAX_TRANSITION_CONDITIONS,
    )
    .map_err(|message| format!("invalid status transition conditions: {message}"))?;
    Ok(conditions)
}

/// Declared transition edges, or `None` when transitions are unrestricted.
pub fn transition_edges(schema: &Value) -> Option<Vec<TransitionEdge>> {
    let edges = schema.get(STATUS_KEY)?.get("transitions")?.as_array()?;
    Some(
        edges
            .iter()
            .map(|edge| TransitionEdge {
                from: edge["from"].as_str().map(str::to_owned),
                to: edge["to"].as_str().map(str::to_owned),
                conditions: edge_conditions(edge).unwrap_or_default(),
            })
            .collect(),
    )
}

/// Conditions of the edge taken from `before` to `after`. Unchanged values
/// and unrestricted graphs have none.
pub fn transition_conditions(schema: &Value, before: &Value, after: &Value) -> Vec<Check> {
    if before == after {
        return Vec::new();
    }
    transition_edges(schema)
        .unwrap_or_default()
        .into_iter()
        .find(|edge| {
            edge.from.as_deref() == before.as_str() && edge.to.as_deref() == after.as_str()
        })
        .map(|edge| edge.conditions)
        .unwrap_or_default()
}

/// The `(code, label)` pairs of a status annotation, in option order. Labels
/// may reference the workspace lexicon (`{{key}}`), like other catalog labels.
/// Malformed configurations, which definition validation rejects, yield none.
pub fn status_option_labels(schema: &Value) -> Vec<(&str, &str)> {
    schema
        .get(STATUS_KEY)
        .and_then(|config| config.get("options"))
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(|option| {
                    Some((
                        option.get("code")?.as_str()?,
                        option.get("label")?.as_str()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Missing values are represented by null; clearing requires an explicit edge.
/// Compare the transaction's original and final effective values, never its
/// intermediate writes, so a batch cannot traverse several edges in one save.
pub fn validate_status_transition(
    schema: &Value,
    before: &Value,
    after: &Value,
) -> Result<(), String> {
    validate_status_definition(schema)?;
    let Some(config) = schema.get(STATUS_KEY) else {
        return Ok(());
    };
    if !after.is_null() && !schema["enum"].as_array().unwrap().contains(after) {
        return Err("unknown status option".into());
    }
    if before == after {
        return Ok(());
    }
    if let Some(edges) = config.get("transitions").and_then(Value::as_array) {
        if edges
            .iter()
            .any(|edge| &edge["from"] == before && &edge["to"] == after)
        {
            return Ok(());
        }
        return Err("status transition is not allowed".into());
    }
    if after.is_null() {
        return Err("clearing status requires an explicit transition".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn status_contract_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/status-control-v1.schema.json");
        let generated = format!(
            "{}\n",
            serde_json::to_string_pretty(&status_configuration_schema()).unwrap()
        );
        if std::env::var_os("UPDATE_CONTRACTS").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), generated);
    }
    fn schema() -> Value {
        json!({"type":"string", "enum":["draft","live","done"], STATUS_KEY: {
            "version":1, "options":[{"code":"draft","label":"Draft"},{"code":"live","label":"Live"},{"code":"done","label":"Done"}],
            "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"live"},{"from":"live","to":"done"}]
        }})
    }
    #[test]
    fn validates_edges_initial_terminal_and_unchanged_states() {
        let s = schema();
        assert!(validate_status_transition(&s, &Value::Null, &json!("draft")).is_ok());
        assert!(validate_status_transition(&s, &json!("draft"), &json!("live")).is_ok());
        assert!(validate_status_transition(&s, &json!("draft"), &json!("done")).is_err());
        assert!(validate_status_transition(&s, &json!("done"), &json!("done")).is_ok());
        assert!(validate_status_transition(&s, &json!("done"), &json!("draft")).is_err());
        assert!(validate_status_transition(&s, &json!("live"), &Value::Null).is_err());
        assert!(validate_status_transition(&s, &json!("live"), &json!("unknown")).is_err());
    }
    #[test]
    fn unrestricted_does_not_allow_implicit_clearing() {
        let mut s = schema();
        s[STATUS_KEY].as_object_mut().unwrap().remove("transitions");
        assert!(validate_status_transition(&s, &Value::Null, &json!("done")).is_ok());
        assert!(validate_status_transition(&s, &json!("done"), &json!("draft")).is_ok());
        assert!(validate_status_transition(&s, &json!("done"), &Value::Null).is_err());
    }
    fn controlled() -> Value {
        let mut s = schema();
        s[STATUS_KEY]["options"][1]["approval"] = json!({"covers":"all","void_to":"draft"});
        s[STATUS_KEY]["options"][2]["lock"] = json!(["title", "acme:weight"]);
        s[STATUS_KEY]["options"][2]["retention_days"] = json!(3650);
        s[STATUS_KEY]["transitions"] = json!([
            {"from":null,"to":"draft"},
            {"from":"draft","to":"live","code":"submit"},
            {"from":"live","to":"done","code":"release","permission":"entities.publish","roles":["reviewer"],"separate_from":["submit"]},
            {"from":"done","to":"draft","code":"correct","roles":["owner"]}
        ]);
        s
    }

    #[test]
    fn reads_record_controls_and_edge_requirements() {
        let s = controlled();
        assert!(validate_status_definition(&s).is_ok());
        assert!(has_record_controls(&s));
        assert!(!has_record_controls(&schema()));
        assert_eq!(
            status_lock(&s, &json!("done")),
            Some(StatusCoverage::Attributes(vec![
                "title".into(),
                "acme:weight".into()
            ]))
        );
        assert_eq!(status_lock(&s, &json!("draft")), None);
        assert_eq!(status_lock(&s, &Value::Null), None);
        assert_eq!(status_retention_days(&s, &json!("done")), Some(3650));
        let approval = status_approval(&s, &json!("live")).unwrap();
        assert_eq!(approval.void_to, "draft");
        assert!(approval.covers.covers("title", "status"));
        assert!(!approval.covers.covers("status", "status"));
        let release = transition_requirements(&s, &json!("live"), &json!("done"));
        assert_eq!(release.code.as_deref(), Some("release"));
        assert_eq!(release.permission.as_deref(), Some("entities.publish"));
        assert_eq!(release.roles, ["reviewer"]);
        assert_eq!(release.separate_from, ["submit"]);
        assert!(release.is_restricted());
        assert!(!transition_requirements(&s, &json!("draft"), &json!("live")).is_restricted());
        assert_eq!(
            transition_requirements(&s, &json!("done"), &json!("done")),
            TransitionRequirements::default()
        );
    }

    #[test]
    fn rejects_inconsistent_record_controls() {
        let mut unrestricted = controlled();
        unrestricted[STATUS_KEY]
            .as_object_mut()
            .unwrap()
            .remove("transitions");
        let mut retention = controlled();
        retention[STATUS_KEY]["options"][1]["retention_days"] = json!(1);
        let mut void_self = controlled();
        void_self[STATUS_KEY]["options"][1]["approval"]["void_to"] = json!("live");
        let mut void_unknown = controlled();
        void_unknown[STATUS_KEY]["options"][1]["approval"]["void_to"] = json!("gone");
        let mut separation = controlled();
        separation[STATUS_KEY]["transitions"][2]["separate_from"] = json!(["review"]);
        let mut duplicate = controlled();
        duplicate[STATUS_KEY]["transitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"from":"draft","to":"live","code":"other"}));
        let mut permission = controlled();
        permission[STATUS_KEY]["transitions"][2]["permission"] = json!("Entities publish");
        let mut lock = controlled();
        lock[STATUS_KEY]["options"][2]["lock"] = json!("everything");
        for (name, s) in [
            ("unrestricted", unrestricted),
            ("retention", retention),
            ("void_self", void_self),
            ("void_unknown", void_unknown),
            ("separation", separation),
            ("duplicate", duplicate),
            ("permission", permission),
            ("lock", lock),
        ] {
            assert!(validate_status_definition(&s).is_err(), "{name}");
        }
    }

    #[test]
    fn edges_carry_strict_conditions() {
        let mut s = schema();
        s[STATUS_KEY]["transitions"][2]["conditions"] = json!([
            {"code": "has-owner", "message": "Assign an owner", "predicate": {"type": "required", "attribute_code": "owner"}}
        ]);
        assert!(validate_status_definition(&s).is_ok());
        assert_eq!(
            transition_conditions(&s, &json!("live"), &json!("done"))[0].code,
            "has-owner"
        );
        assert!(transition_conditions(&s, &json!("draft"), &json!("live")).is_empty());
        assert!(transition_conditions(&s, &json!("done"), &json!("done")).is_empty());
        for invalid in [
            json!([{"code": "x", "predicate": {"type": "unique", "attribute_codes": ["a"]}}]),
            json!([{"code": "x", "predicate": {"type": "required"}}]),
            json!([{"code": "x", "when": 1, "predicate": {"type": "required", "attribute_code": "a"}}]),
        ] {
            s[STATUS_KEY]["transitions"][2]["conditions"] = invalid.clone();
            assert!(validate_status_definition(&s).is_err(), "{invalid}");
        }
        let mut duplicate = schema();
        duplicate[STATUS_KEY]["transitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"from": "live", "to": "done", "conditions": []}));
        assert!(validate_status_definition(&duplicate).is_err());
    }

    #[test]
    fn lists_option_labels() {
        assert_eq!(
            status_option_labels(&schema()),
            [("draft", "Draft"), ("live", "Live"), ("done", "Done")]
        );
        assert!(status_option_labels(&json!({"type":"string"})).is_empty());
    }
    #[test]
    fn rejects_invalid_configuration() {
        for (pointer, value) in [
            ("/type", json!("number")),
            ("/enum", json!(["draft"])),
            ("/x-attricat-status/version", json!(2)),
            ("/x-attricat-status/options/0/tone", json!("#ffffff")),
            ("/x-attricat-status/options/0/code", json!("live")),
            ("/x-attricat-status/options/0/label", json!(" ")),
            ("/x-attricat-status/transitions/0/to", json!("missing")),
        ] {
            let mut s = schema();
            if pointer.ends_with("/tone") {
                s[STATUS_KEY]["options"][0]["tone"] = value;
            } else {
                *s.pointer_mut(pointer).unwrap() = value;
            }
            assert!(validate_status_definition(&s).is_err(), "{pointer}");
        }
    }
}
