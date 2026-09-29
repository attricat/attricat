use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{
        PasswordHash as ParsedPasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
};
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

pub const MINIMUM_PASSWORD_LENGTH: usize = 5;
/// Bounds the input to the deliberately expensive hash. Measured in bytes so
/// the limit tracks the work actually performed.
pub const MAXIMUM_PASSWORD_BYTES: usize = 1024;

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

/// Validates the password policy shared by every password-setting flow.
pub fn validate_password(value: &str) -> Result<(), PasswordPolicyError> {
    if value.chars().count() < MINIMUM_PASSWORD_LENGTH {
        return Err(PasswordPolicyError::TooShort);
    }
    if value.len() > MAXIMUM_PASSWORD_BYTES {
        return Err(PasswordPolicyError::TooLong);
    }
    if !value.chars().any(char::is_alphabetic) {
        return Err(PasswordPolicyError::MissingLetter);
    }
    if !value.chars().any(char::is_numeric) {
        return Err(PasswordPolicyError::MissingNumber);
    }
    if !value
        .chars()
        .any(|character| !character.is_alphanumeric() && !character.is_whitespace())
    {
        return Err(PasswordPolicyError::MissingSymbol);
    }
    Ok(())
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PasswordPolicyError {
    #[error("password must be at least {MINIMUM_PASSWORD_LENGTH} characters")]
    TooShort,
    #[error("password must be at most {MAXIMUM_PASSWORD_BYTES} bytes")]
    TooLong,
    #[error("password must include at least one letter")]
    MissingLetter,
    #[error("password must include at least one number")]
    MissingNumber,
    #[error("password must include at least one symbol")]
    MissingSymbol,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_policy_requires_length_and_character_classes() {
        assert_eq!(validate_password("a1!"), Err(PasswordPolicyError::TooShort));
        assert_eq!(
            validate_password(&format!("a1!{}", "x".repeat(MAXIMUM_PASSWORD_BYTES))),
            Err(PasswordPolicyError::TooLong)
        );
        assert_eq!(
            validate_password("1234!"),
            Err(PasswordPolicyError::MissingLetter)
        );
        assert_eq!(
            validate_password("abcde!"),
            Err(PasswordPolicyError::MissingNumber)
        );
        assert_eq!(
            validate_password("abcde1"),
            Err(PasswordPolicyError::MissingSymbol)
        );
        assert!(validate_password("a1!bc").is_ok());
    }
}
