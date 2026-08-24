use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{
        PasswordHash as ParsedPasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
};
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// A plaintext password that cannot be formatted or serialized accidentally.
pub struct Password(SecretString);

impl Password {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::from(value.into()))
    }
}

/// An Argon2id PHC value suitable for persistence.
///
/// This wrapper deliberately exposes only the PHC representation; it never
/// retains the corresponding plaintext password.
#[derive(Clone, Eq, PartialEq)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Validates a persisted PHC value before it is used for verification.
    pub fn from_phc(value: impl Into<String>) -> Result<Self, PasswordHashError> {
        let value = value.into();
        validate_argon2id_phc(&value)?;
        Ok(Self(value))
    }

    /// Returns the PHC value to be stored by a persistence adapter.
    pub fn as_phc(&self) -> &str {
        &self.0
    }

    /// Verifies a candidate password against this Argon2id PHC value.
    pub fn verify(&self, password: &Password) -> Result<bool, PasswordHashError> {
        let parsed = validate_argon2id_phc(&self.0)?;
        Ok(argon2id()
            .verify_password(password.0.expose_secret().as_bytes(), &parsed)
            .is_ok())
    }
}

/// Errors are intentionally generic so neither passwords nor malformed stored
/// values are included in logs.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum PasswordHashError {
    #[error("password hash must be a valid Argon2id PHC value")]
    InvalidPhc,
}

/// Creates a salted Argon2id PHC password hash.
pub fn hash_password(password: &Password) -> Result<PasswordHash, PasswordHashError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2id()
        .hash_password(password.0.expose_secret().as_bytes(), &salt)
        .map_err(|_| PasswordHashError::InvalidPhc)?;
    PasswordHash::from_phc(hash.to_string())
}

fn argon2id() -> Argon2<'static> {
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
}

fn validate_argon2id_phc(value: &str) -> Result<ParsedPasswordHash<'_>, PasswordHashError> {
    let parsed = ParsedPasswordHash::new(value).map_err(|_| PasswordHashError::InvalidPhc)?;
    if parsed.algorithm.as_str() != "argon2id" {
        return Err(PasswordHashError::InvalidPhc);
    }
    Ok(parsed)
}
