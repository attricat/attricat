pub mod predicate;
pub mod principal;
pub mod status;

/// The JSON Schema `pattern` equivalent of [`is_valid_code`], published in
/// definition contracts so editors can flag invalid codes before saving.
pub const CODE_PATTERN: &str = "^[A-Za-z0-9_-]+$";

/// Returns whether a user-facing catalog identifier is safe for use as a reference.
pub fn is_valid_code(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[derive(Debug, PartialEq)]
pub struct JsonSchemaViolation {
    pub instance_path: String,
    pub message: String,
}

pub fn validate_json_schema_definition(schema: &serde_json::Value) -> Result<(), String> {
    status::validate_status_definition(schema)?;
    predicate::entity_checks(schema)?;
    principal::validate_principal_definition(schema)?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn validate_json_schema(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<Vec<JsonSchemaViolation>, String> {
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|error| error.to_string())?;
    Ok(validator
        .iter_errors(instance)
        .map(|error| JsonSchemaViolation {
            instance_path: error.instance_path().as_str().to_owned(),
            message: error.masked().to_string(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{CODE_PATTERN, is_valid_code, validate_json_schema};

    const VALID_CODES: [&str; 4] = ["product", "en_GB", "en-GB", "v2_item"];
    const INVALID_CODES: [&str; 5] = ["", "product name", "product.name", "produit-été", "café"];

    #[test]
    fn accepts_ascii_reference_codes() {
        for value in VALID_CODES {
            assert!(is_valid_code(value));
        }
    }

    #[test]
    fn rejects_unsafe_reference_codes() {
        for value in INVALID_CODES {
            assert!(!is_valid_code(value));
        }
    }

    #[test]
    fn code_pattern_matches_runtime_code_validation() {
        let schema = serde_json::json!({ "type": "string", "pattern": CODE_PATTERN });
        for value in VALID_CODES.into_iter().chain(INVALID_CODES) {
            let pattern_accepts = validate_json_schema(&schema, &serde_json::json!(value))
                .unwrap()
                .is_empty();
            assert_eq!(pattern_accepts, is_valid_code(value), "{value:?}");
        }
    }
}
