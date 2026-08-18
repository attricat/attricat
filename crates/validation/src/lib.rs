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
    use super::is_valid_code;

    #[test]
    fn accepts_ascii_reference_codes() {
        for value in ["product", "en_GB", "en-GB", "v2_item"] {
            assert!(is_valid_code(value));
        }
    }

    #[test]
    fn rejects_unsafe_reference_codes() {
        for value in ["", "product name", "product.name", "produit-été", "café"] {
            assert!(!is_valid_code(value));
        }
    }
}
