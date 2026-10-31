//! Reads and writes Attricat definitions as YAML with one shared policy:
//! duplicate keys are errors, only `true` and `false` are booleans, `<<` is an
//! ordinary key rather than a YAML 1.1 merge, alias expansion stays within
//! the parser's default budget, decimal integers must be exact (no leading
//! zero, within 64 bits), and only YAML core-schema tags are accepted. The
//! web editor parses with the same rules.

mod layout;
mod policy;

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
    options.reject_unsupported_tags = true;
    options.with_snippet = false;
    options
}

pub fn from_str<T: DeserializeOwned>(source: &str) -> Result<T, Error> {
    policy::check(source)?;
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

    #[test]
    fn rejects_an_integer_key_beside_the_same_quoted_key() {
        for source in [
            "1: a\n\"1\": b\n",
            "'1': a\n1: b\n",
            "x:\n  - {1: a, '1': b}\n",
        ] {
            let error = from_str::<Value>(source).unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with("duplicate mapping key: 1 at line"),
                "{source:?}: {error}"
            );
            assert!(from_str::<std::collections::HashMap<String, Value>>(source).is_err());
        }
        let error = from_str::<Value>("1: a\n\"1\": b\n").unwrap_err();
        assert_eq!(
            error.to_string(),
            "duplicate mapping key: 1 at line 2, column 1"
        );
    }

    #[test]
    fn keeps_equal_keys_in_different_mappings() {
        let value: Value = from_str("a: {1: x}\nb: {'1': y}\n").unwrap();
        assert_eq!(value, json!({ "a": { "1": "x" }, "b": { "1": "y" } }));
    }

    #[test]
    fn rejects_leading_zero_integers() {
        for source in [
            "x: 012\n",
            "x: -012\n",
            "x: 08\n",
            "x: 00\n",
            "x: !!int 012\n",
        ] {
            let error = from_str::<Value>(source).unwrap_err();
            assert!(
                error.to_string().contains("leading zero"),
                "{source:?}: {error}"
            );
        }
        assert_eq!(
            from_str::<Value>("x: 012\n").unwrap_err().to_string(),
            "integer `012` has a leading zero; remove it, or quote the value to keep it as text at line 1, column 4"
        );
    }

    #[test]
    fn rejects_integers_beyond_64_bits() {
        for source in [
            "x: 123456789012345678901234567890\n",
            "x: 18446744073709551616\n",
            "x: -9223372036854775809\n",
        ] {
            let error = from_str::<Value>(source).unwrap_err();
            assert!(
                error.to_string().contains("64-bit range"),
                "{source:?}: {error}"
            );
        }
    }

    #[test]
    fn reads_exact_integers_and_quoted_digits() {
        let value: Value = from_str(
            "zero: 0\nneg: -0\nmax: 18446744073709551615\nmin: -9223372036854775808\n\
             hex: 0x10\nfloat: 0012.5\ntext: '012'\nbig: \"123456789012345678901234567890\"\n\
             012: key\n",
        )
        .unwrap();
        assert_eq!(
            value,
            json!({
                "zero": 0, "neg": 0, "max": u64::MAX, "min": i64::MIN, "hex": 16,
                "float": 12.5, "text": "012", "big": "123456789012345678901234567890",
                "012": "key"
            })
        );
    }

    #[test]
    fn rejects_unsupported_tags() {
        for (source, tag) in [
            ("x: !foo bar\n", "!foo"),
            ("x: !foo {a: 1}\n", "!foo"),
            ("x: !!binary aGVsbG8=\n", "!!binary"),
            ("x: !binary aGVsbG8=\n", "!binary"),
            ("x: !!timestamp 2001-12-14\n", "!!timestamp"),
            ("x: !!set {a: null}\n", "!!set"),
        ] {
            let error = from_str::<Value>(source).unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("unsupported tag `{tag}`")),
                "{source:?}: {error}"
            );
        }
    }

    #[test]
    fn reads_core_and_non_specific_tags() {
        let value: Value =
            from_str("a: !!str 1\nb: !!int 2\nc: ! 3\nd: !!seq [x]\ne: !!map {k: v}\n").unwrap();
        assert_eq!(
            value,
            json!({ "a": "1", "b": 2, "c": "3", "d": ["x"], "e": { "k": "v" } })
        );
    }

    #[test]
    fn leaves_syntax_errors_to_the_parser() {
        assert!(from_str::<Value>("a: [1\n").is_err());
    }
}
