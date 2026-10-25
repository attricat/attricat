use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// Random bytes in an opaque secret, before encoding.
const OPAQUE_SECRET_BYTES: usize = 32;

/// A random 32-byte value delivered to a client once, such as a browser
/// session, CSRF value or lifecycle action token. It deliberately cannot be
/// formatted, serialized, or logged; only delivery code may expose it.
pub struct OpaqueSecret(SecretString);

impl OpaqueSecret {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; OPAQUE_SECRET_BYTES];
        OsRng.fill_bytes(&mut bytes);
        Self(SecretString::from(URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// Reconstitutes a delivered value after validating its exact opaque form.
    pub fn from_delivery_value(value: impl Into<String>) -> Result<Self, SecretError> {
        let value = value.into();
        let decoded = URL_SAFE_NO_PAD
            .decode(&value)
            .map_err(|_| SecretError::Invalid)?;
        if decoded.len() != OPAQUE_SECRET_BYTES || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(SecretError::Invalid);
        }
        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_for_delivery(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn digest(&self) -> SecretDigest {
        SecretDigest(Sha256::digest(self.0.expose_secret().as_bytes()).into())
    }
}

/// The SHA-256 digest that is safe to persist instead of an opaque secret.
/// This type also intentionally has no formatting implementation.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretDigest([u8; 32]);

impl SecretDigest {
    pub fn from_slice(value: &[u8]) -> Result<Self, SecretError> {
        Ok(Self(value.try_into().map_err(|_| SecretError::Invalid)?))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn matches(&self, value: &OpaqueSecret) -> bool {
        self.0.ct_eq(value.digest().as_bytes()).into()
    }
}

impl AsRef<[u8]> for SecretDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SecretError {
    #[error("secret value is invalid")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_secrets_persist_only_a_digest() {
        let secret = OpaqueSecret::generate();
        let delivery = secret.expose_for_delivery().to_owned();
        let digest = secret.digest();
        assert_ne!(delivery.as_bytes(), digest.as_bytes());
        assert!(digest.matches(&OpaqueSecret::from_delivery_value(delivery).unwrap()));
        assert!(OpaqueSecret::from_delivery_value("not-a-secret").is_err());
        assert!(SecretDigest::from_slice(&[0; 31]).is_err());
    }
}
