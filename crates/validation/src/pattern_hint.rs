//! An optional plain-language description of a string `pattern`.

use serde_json::Value;

pub(crate) const PATTERN_HINT_KEY: &str = "x-attricat-pattern-hint";
pub(crate) const MAX_HINT_CHARS: usize = 200;

/// Checks an `x-attricat-pattern-hint`: 1 to [`MAX_HINT_CHARS`] characters of
/// text describing the `pattern` beside it, which forms show in place of the
/// regular expression.
pub(crate) fn validate_pattern_hint_definition(schema: &Value) -> Result<(), String> {
    let Some(hint) = schema.get(PATTERN_HINT_KEY) else {
        return Ok(());
    };
    if schema.get("pattern").is_none() {
        return Err(format!("{PATTERN_HINT_KEY} requires a pattern"));
    }
    let hint = hint.as_str().map(str::trim).unwrap_or_default();
    if hint.is_empty() || hint.chars().count() > MAX_HINT_CHARS {
        return Err(format!(
            "{PATTERN_HINT_KEY} must be 1 to {MAX_HINT_CHARS} characters"
        ));
    }
    Ok(())
}

/// The pattern hint of a value schema. It may reference the workspace lexicon
/// (`{{key}}`), like other catalog labels. A malformed hint, which definition
/// validation rejects, yields none.
pub fn pattern_hint(schema: &Value) -> Option<&str> {
    schema.get(PATTERN_HINT_KEY).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_a_hint_beside_a_pattern() {
        assert!(validate_pattern_hint_definition(&json!({"type": "string"})).is_ok());
        let schema = json!({"pattern": "^[A-Z]{3}$", PATTERN_HINT_KEY: "Three capital letters"});
        assert!(validate_pattern_hint_definition(&schema).is_ok());
        assert_eq!(pattern_hint(&schema), Some("Three capital letters"));
    }

    #[test]
    fn rejects_a_hint_without_a_pattern_or_text() {
        assert!(validate_pattern_hint_definition(&json!({PATTERN_HINT_KEY: "Hint"})).is_err());
        for hint in [
            json!(""),
            json!("  "),
            json!(3),
            json!("x".repeat(MAX_HINT_CHARS + 1)),
        ] {
            assert!(
                validate_pattern_hint_definition(&json!({"pattern": "a", PATTERN_HINT_KEY: hint}))
                    .is_err()
            );
        }
    }
}
