//! Order-independent JSON digests. `serde_json` keeps object keys in
//! insertion order, and PostgreSQL `jsonb` reorders them, so a digest that
//! must survive a round trip or a client's key order hashes sorted keys.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of `value` serialized with every object's keys
/// sorted.
pub fn canonical_json_sha256<T: Serialize + ?Sized>(
    value: &T,
) -> Result<String, serde_json::Error> {
    let mut value = serde_json::to_value(value)?;
    value.sort_all_objects();
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_the_digest() {
        let a = json!({"b": 1, "a": {"y": [{"q": 1, "p": 2}], "x": null}});
        let b = json!({"a": {"x": null, "y": [{"p": 2, "q": 1}]}, "b": 1});
        assert_eq!(
            canonical_json_sha256(&a).unwrap(),
            canonical_json_sha256(&b).unwrap()
        );
        assert_ne!(
            canonical_json_sha256(&a).unwrap(),
            canonical_json_sha256(&json!({"b": 2})).unwrap()
        );
    }
}
