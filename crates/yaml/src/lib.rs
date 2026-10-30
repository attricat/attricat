//! Reads and writes Attricat definitions as YAML with one shared policy:
//! duplicate keys are errors, only `true` and `false` are booleans, `<<` is an
//! ordinary key rather than a YAML 1.1 merge, and alias expansion stays within
//! the parser's default budget. The web editor parses with the same rules.

mod layout;

pub use layout::rewrite_preserving_layout;
use serde::{Serialize, de::DeserializeOwned};
use serde_saphyr::{DuplicateKeyPolicy, MergeKeyPolicy, Options};
use thiserror::Error;

/// Advice the parser appends to duplicate-key errors; it names a Rust option
/// that authors cannot set.
const DUPLICATE_KEY_ADVICE: &str = ", set DuplicateKeyPolicy in Options if acceptable";

/// A YAML syntax or shape error, with its line and column when known.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct Error(String);

fn options() -> Options {
    let mut options = Options::default();
    options.duplicate_keys = DuplicateKeyPolicy::Error;
    options.merge_keys = MergeKeyPolicy::AsOrdinary;
    options.strict_booleans = true;
    options.with_snippet = false;
    options
}

pub fn from_str<T: DeserializeOwned>(source: &str) -> Result<T, Error> {
    serde_saphyr::from_str_with_options(source, options()).map_err(|error| {
        Error(
            error
                .without_snippet()
                .to_string()
                .replace(DUPLICATE_KEY_ADVICE, ""),
        )
    })
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String, Error> {
    serde_saphyr::to_string(value).map_err(|error| Error(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn rejects_duplicate_keys_with_their_location() {
        let error = from_str::<Value>("code: a\ncode: b\n").unwrap_err();
        assert_eq!(
            error.to_string(),
            "duplicate mapping key: code at line 2, column 1"
        );
    }

    #[test]
    fn reads_yes_and_no_as_strings() {
        let value: Value = from_str("required: true\nlabel: no\n").unwrap();
        assert_eq!(value, json!({ "required": true, "label": "no" }));
    }

    #[test]
    fn reads_merge_keys_as_ordinary_keys() {
        let value: Value = from_str("base: &base {a: 1}\nitem:\n  <<: *base\n").unwrap();
        assert_eq!(value["item"], json!({ "<<": { "a": 1 } }));
    }

    #[test]
    fn round_trips_values() {
        let value = json!({ "code": "sku", "label": "SKU: code", "max": 1.5, "tags": ["a"] });
        assert_eq!(
            from_str::<Value>(&to_string(&value).unwrap()).unwrap(),
            value
        );
    }
}
