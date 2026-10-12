use std::sync::{Arc, LazyLock};

pub mod choices;
pub mod files;
pub mod predicate;
pub mod principal;
pub mod saved_search;
pub mod status;
pub mod unique_key;

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
    choices::validate_enum_labels_definition(schema)?;
    predicate::record_checks(schema)?;
    principal::validate_principal_definition(schema)?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Compiled validators by schema text. Building one compiles every `pattern`
/// and `format` regex, and record writes and migrations validate many values
/// against the same few published schemas, so each schema compiles once per
/// process. Keys are the schema's full JSON text, so equal keys are equal
/// schemas.
static VALIDATORS: LazyLock<moka::sync::Cache<Arc<str>, Arc<jsonschema::Validator>>> =
    LazyLock::new(|| moka::sync::Cache::new(VALIDATOR_CACHE_ENTRIES));

const VALIDATOR_CACHE_ENTRIES: u64 = 4_096;

fn validator(schema: &serde_json::Value) -> Result<Arc<jsonschema::Validator>, String> {
    let key: Arc<str> = schema.to_string().into();
    if let Some(validator) = VALIDATORS.get(&key) {
        metrics::counter!("attricat_json_schema_validator_cache_total", "outcome" => "hit")
            .increment(1);
        return Ok(validator);
    }
    // Concurrent misses may compile twice; the result is the same.
    let validator = Arc::new(
        jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(schema)
            .map_err(|error| error.to_string())?,
    );
    metrics::counter!("attricat_json_schema_validator_cache_total", "outcome" => "miss")
        .increment(1);
    VALIDATORS.insert(key, validator.clone());
    Ok(validator)
}

pub fn validate_json_schema(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<Vec<JsonSchemaViolation>, String> {
    let validator = validator(schema)?;
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
    use std::sync::Arc;

    use super::{CODE_PATTERN, is_valid_code, validate_json_schema, validator};

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

    #[test]
    fn equal_schemas_share_one_compiled_validator() {
        let schema = serde_json::json!({ "type": "string", "pattern": "^cached-[0-9]+$" });
        let first = validator(&schema).unwrap();
        assert!(Arc::ptr_eq(&first, &validator(&schema.clone()).unwrap()));
        let other = serde_json::json!({ "type": "string", "pattern": "^cached-[a-z]+$" });
        assert!(!Arc::ptr_eq(&first, &validator(&other).unwrap()));
        assert!(
            validate_json_schema(&schema, &serde_json::json!("cached-1"))
                .unwrap()
                .is_empty()
        );
        assert!(
            !validate_json_schema(&schema, &serde_json::json!("cached-x"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn invalid_schemas_are_reported_on_every_call() {
        let schema = serde_json::json!({ "type": "string", "pattern": "(" });
        assert!(validate_json_schema(&schema, &serde_json::json!("x")).is_err());
        assert!(validate_json_schema(&schema, &serde_json::json!("x")).is_err());
    }
}
