//! Optional display labels for the values of a string `enum`.

use serde_json::Value;

pub const ENUM_LABELS_KEY: &str = "x-attricat-enum-labels";
pub const MAX_LABEL_CHARS: usize = 200;

/// Checks an `x-attricat-enum-labels` map: it labels some or all string values
/// of the schema's `enum`, and nothing else. Statuses label their options in
/// `x-attricat-status` instead.
pub fn validate_enum_labels_definition(schema: &Value) -> Result<(), String> {
    let Some(labels) = schema.get(ENUM_LABELS_KEY) else {
        return Ok(());
    };
    if schema.get(super::status::STATUS_KEY).is_some() {
        return Err(format!(
            "a status labels its options in x-attricat-status; remove {ENUM_LABELS_KEY}"
        ));
    }
    let values = schema
        .get("enum")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{ENUM_LABELS_KEY} requires an enum"))?;
    let labels = labels
        .as_object()
        .ok_or_else(|| format!("{ENUM_LABELS_KEY} must map enum values to labels"))?;
    for (code, label) in labels {
        if !values.iter().any(|value| value.as_str() == Some(code)) {
            return Err(format!(
                "{ENUM_LABELS_KEY} labels '{code}', which is not in the enum"
            ));
        }
        let label = label.as_str().map(str::trim).unwrap_or_default();
        if label.is_empty() || label.chars().count() > MAX_LABEL_CHARS {
            return Err(format!(
                "{ENUM_LABELS_KEY} label for '{code}' must be 1 to {MAX_LABEL_CHARS} characters"
            ));
        }
    }
    Ok(())
}

/// The `(value, label)` pairs of an `x-attricat-enum-labels` map. Labels may
/// reference the workspace lexicon (`{{key}}`), like other catalog labels.
/// Malformed maps, which definition validation rejects, yield none.
pub fn enum_value_labels(schema: &Value) -> Vec<(&str, &str)> {
    schema
        .get(ENUM_LABELS_KEY)
        .and_then(Value::as_object)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|(code, label)| Some((code.as_str(), label.as_str()?)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(labels: Value) -> Value {
        json!({"type": "string", "enum": ["small", "large"], ENUM_LABELS_KEY: labels})
    }

    #[test]
    fn accepts_labels_for_some_or_all_values() {
        assert!(validate_enum_labels_definition(&json!({"type": "string"})).is_ok());
        assert!(
            validate_enum_labels_definition(
                &json!({"enum": ["s"], ENUM_LABELS_KEY: {"s": "Small"}})
            )
            .is_ok()
        );
        assert!(validate_enum_labels_definition(&schema(json!({"small": "Small"}))).is_ok());
        assert!(
            validate_enum_labels_definition(&schema(
                json!({"small": "{{Small}}", "large": "Large"})
            ))
            .is_ok()
        );
        assert_eq!(
            enum_value_labels(&schema(json!({"small": "Small"}))),
            vec![("small", "Small")]
        );
    }

    #[test]
    fn rejects_invalid_labels() {
        for invalid in [
            schema(json!(["Small"])),
            schema(json!({"medium": "Medium"})),
            schema(json!({"small": " "})),
            schema(json!({"small": 1})),
            schema(json!({"small": "x".repeat(MAX_LABEL_CHARS + 1)})),
            json!({"type": "string", ENUM_LABELS_KEY: {"small": "Small"}}),
            json!({"type": "number", "enum": [1], ENUM_LABELS_KEY: {"1": "One"}}),
            json!({
                "type": "string",
                "enum": ["small"],
                ENUM_LABELS_KEY: {"small": "Small"},
                "x-attricat-status": {"version": 1, "options": [{"code": "small", "label": "Small"}]}
            }),
        ] {
            assert!(
                validate_enum_labels_definition(&invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
