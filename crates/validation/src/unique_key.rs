//! Normalization of business-key values, shared by blueprint `[[unique_keys]]`
//! and the rules-only `unique` predicate so both decide "same value" alike.
use serde_json::Value;

/// Whether the `unique` predicate compares strings case-sensitively. It uses
/// the `[[unique_keys]]` default (`case_sensitive = false`).
pub const UNIQUE_PREDICATE_CASE_SENSITIVE: bool = false;

/// Normalizes one key component, or returns `None` when the value counts as
/// missing.
///
/// - Strings are trimmed, internal whitespace runs become one space and,
///   unless `case_sensitive`, they are lowercased; a blank string is missing.
/// - Numbers compare by value (`1.50` equals `1.5`) and normalize to their
///   shortest decimal text. A JSON string holding the exact decimal is
///   accepted so hosts can avoid binary floating point.
/// - Relationships compare by target record: the value is the sorted array of
///   target ID strings and the smallest ID is the component.
/// - Date-times are expected in one canonical form (UTC), so they compare by
///   instant; other types compare by their JSON value. `null` is missing.
pub fn normalize_key_component(
    value_type: &str,
    value: &Value,
    case_sensitive: bool,
) -> Option<Value> {
    match (value_type, value) {
        (_, Value::Null) => None,
        ("relationship", Value::Array(targets)) => targets
            .iter()
            .filter_map(Value::as_str)
            .min()
            .map(|target| Value::String(target.to_owned())),
        ("relationship", Value::String(target)) => Some(Value::String(target.clone())),
        ("string", Value::String(text)) => {
            let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() {
                return None;
            }
            Some(Value::String(if case_sensitive {
                normalized
            } else {
                normalized.to_lowercase()
            }))
        }
        ("number", value) => decimal_text(value)
            .and_then(|text| normalize_decimal(&text))
            .map(Value::String),
        (_, Value::Array(items)) if items.is_empty() => None,
        (_, value) => Some(value.clone()),
    }
}

fn decimal_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Some(integer.to_string())
            } else if let Some(integer) = number.as_u64() {
                Some(integer.to_string())
            } else {
                // `f64` display never uses an exponent and keeps the shortest
                // digits that round-trip.
                number
                    .as_f64()
                    .filter(|float| float.is_finite())
                    .map(|float| float.to_string())
            }
        }
        Value::String(text) => Some(text.trim().to_owned()),
        _ => None,
    }
}

/// `-001.500` → `-1.5`, `0.0` → `0`: the text form of a normalized decimal.
fn normalize_decimal(text: &str) -> Option<String> {
    let (negative, unsigned) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if (integer.is_empty() && fraction.is_empty())
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let fraction = fraction.trim_end_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    if integer == "0" && fraction.is_empty() {
        return Some("0".to_owned());
    }
    let mut normalized = String::with_capacity(text.len());
    if negative {
        normalized.push('-');
    }
    normalized.push_str(integer);
    if !fraction.is_empty() {
        normalized.push('.');
        normalized.push_str(fraction);
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_are_trimmed_collapsed_and_case_folded_unless_case_sensitive() {
        assert_eq!(
            normalize_key_component("string", &json!("  ab-1\t  X "), false),
            Some(json!("ab-1 x"))
        );
        assert_eq!(
            normalize_key_component("string", &json!("AB-1  x"), true),
            Some(json!("AB-1 x"))
        );
        assert_eq!(
            normalize_key_component("string", &json!("   "), false),
            None
        );
        assert_eq!(normalize_key_component("string", &Value::Null, false), None);
    }

    #[test]
    fn numbers_compare_by_value_in_decimal_text() {
        for (input, expected) in [
            (json!(1.5), "1.5"),
            (json!("1.50"), "1.5"),
            (json!("-001.500"), "-1.5"),
            (json!("100"), "100"),
            (json!(100), "100"),
            (json!("0.000"), "0"),
            (json!("-0.0"), "0"),
            (json!(1e21), "1000000000000000000000"),
            (
                json!("12345678901234567890.123456789"),
                "12345678901234567890.123456789",
            ),
        ] {
            assert_eq!(
                normalize_key_component("number", &input, false),
                Some(json!(expected)),
                "{input}"
            );
        }
        assert_eq!(
            normalize_key_component("number", &json!("1e3"), false),
            None
        );
    }

    #[test]
    fn relationships_use_the_smallest_target_and_other_types_their_value() {
        assert_eq!(
            normalize_key_component("relationship", &json!(["b", "a"]), false),
            Some(json!("a"))
        );
        assert_eq!(
            normalize_key_component("relationship", &json!([]), false),
            None
        );
        assert_eq!(
            normalize_key_component("integer", &json!(7), false),
            Some(json!(7))
        );
        assert_eq!(
            normalize_key_component("date", &json!("2026-01-31"), false),
            Some(json!("2026-01-31"))
        );
        assert_eq!(
            normalize_key_component("boolean", &json!(false), false),
            Some(json!(false))
        );
    }
}
