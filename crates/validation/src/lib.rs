/// Returns whether a user-facing catalog identifier is safe for use as a reference.
pub fn is_valid_code(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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
