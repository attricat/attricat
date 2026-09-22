use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// A random 32-byte browser credential or CSRF value. It deliberately cannot
/// be formatted, serialized, or logged; only delivery code may expose it.
pub struct SessionSecret(SecretString);

impl SessionSecret {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(SecretString::from(URL_SAFE_NO_PAD.encode(bytes)))
    }

    pub fn from_delivery_value(value: impl Into<String>) -> Result<Self, SessionSecretError> {
        let value = value.into();
        let decoded = URL_SAFE_NO_PAD
            .decode(&value)
            .map_err(|_| SessionSecretError::Invalid)?;
        if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(SessionSecretError::Invalid);
        }
        Ok(Self(SecretString::from(value)))
    }

    pub fn expose_for_delivery(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn digest(&self) -> SessionDigest {
        SessionDigest(Sha256::digest(self.0.expose_secret().as_bytes()).into())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SessionDigest([u8; 32]);

impl SessionDigest {
    pub fn from_slice(value: &[u8]) -> Result<Self, SessionSecretError> {
        Ok(Self(
            value.try_into().map_err(|_| SessionSecretError::Invalid)?,
        ))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn matches(&self, value: &SessionSecret) -> bool {
        self.0.ct_eq(value.digest().as_bytes()).into()
    }
}

impl AsRef<[u8]> for SessionDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SessionSecretError {
    #[error("session value is invalid")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_secrets_are_opaque_and_digest_only() {
        let secret = SessionSecret::generate();
        let delivery = secret.expose_for_delivery().to_owned();
        let digest = secret.digest();
        assert_ne!(delivery.as_bytes(), digest.as_bytes());
        assert!(digest.matches(&SessionSecret::from_delivery_value(delivery).unwrap()));
        assert!(SessionSecret::from_delivery_value("not-a-session").is_err());
    }
}
