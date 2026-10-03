//! Versioned status metadata attached to a string attribute's JSON Schema.
use serde_json::Value;
use std::collections::HashSet;

pub const STATUS_KEY: &str = "x-attricat-status";

pub fn status_configuration_schema() -> Value {
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
                    "code": {"type": "string", "minLength": 1, "maxLength": 128, "pattern": super::CODE_PATTERN},
                    "label": {"type": "string", "minLength": 1, "maxLength": 200},
                    "tone": {"enum": ["default", "success", "warning", "error", "info"]}
                }
            }},
            "transitions": {"type": "array", "maxItems": 10000, "uniqueItems": true, "items": {
                "type": "object", "additionalProperties": false, "required": ["from", "to"],
                "properties": {"from": {"type": ["string", "null"]}, "to": {"type": ["string", "null"]}}
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
    if let Some(edges) = config.get("transitions").and_then(Value::as_array) {
        for edge in edges {
            for endpoint in [&edge["from"], &edge["to"]] {
                if !endpoint.is_null() && endpoint.as_str().is_none_or(|code| !codes.contains(code))
                {
                    return Err("status transition references an unknown option".into());
                }
            }
        }
    }
    Ok(())
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
