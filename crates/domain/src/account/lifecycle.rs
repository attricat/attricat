use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// A stable, persisted reason for a one-time account lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleActionPurpose {
    EmailVerification,
    PasswordSetup,
    PasswordReset,
}

impl LifecycleActionPurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmailVerification => "email_verification",
            Self::PasswordSetup => "password_setup",
            Self::PasswordReset => "password_reset",
        }
    }
}

impl FromStr for LifecycleActionPurpose {
    type Err = LifecycleActionPurposeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "email_verification" => Ok(Self::EmailVerification),
            "password_setup" => Ok(Self::PasswordSetup),
            "password_reset" => Ok(Self::PasswordReset),
            _ => Err(LifecycleActionPurposeError::UnknownPurpose),
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum LifecycleActionPurposeError {
    #[error("unknown lifecycle action purpose")]
    UnknownPurpose,
}

/// Random bytes in an action-token secret, before encoding.
const ACTION_TOKEN_SECRET_BYTES: usize = 32;

/// A randomly generated, opaque 32-byte action-token secret.
///
/// The value is encoded only for delivery to the action recipient. It does not
/// implement `Debug`, `Display`, or serialization traits.
pub struct ActionTokenSecret(SecretString);

impl ActionTokenSecret {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; ACTION_TOKEN_SECRET_BYTES];
        OsRng.fill_bytes(&mut bytes);
        Self(SecretString::from(URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// Reconstitutes a delivered token after validating its exact opaque form.
    pub fn from_delivery_value(value: impl Into<String>) -> Result<Self, ActionTokenSecretError> {
        let value = value.into();
        let decoded = URL_SAFE_NO_PAD
            .decode(&value)
            .map_err(|_| ActionTokenSecretError::InvalidToken)?;
        if decoded.len() != ACTION_TOKEN_SECRET_BYTES || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(ActionTokenSecretError::InvalidToken);
        }
        Ok(Self(SecretString::from(value)))
    }

    /// Explicitly exposes the token for the delivery link.
    pub fn expose_for_delivery(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn digest(&self) -> ActionTokenDigest {
        ActionTokenDigest(Sha256::digest(self.0.expose_secret().as_bytes()).into())
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ActionTokenSecretError {
    #[error("action token is invalid")]
    InvalidToken,
}

/// The SHA-256 digest that is safe to persist instead of an action-token
/// secret. This type also intentionally has no formatting implementation.
#[derive(Clone, Eq, PartialEq)]
pub struct ActionTokenDigest([u8; 32]);

impl AsRef<[u8]> for ActionTokenDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{Password, hash_password};

    #[test]
    fn password_hash_is_argon2id_and_never_retains_plaintext() {
        let password = Password::new("correct horse battery staple");
        let hash = hash_password(&password).unwrap();

        assert!(hash.as_phc().starts_with("$argon2id$"));
        assert!(!hash.as_phc().contains("correct horse battery staple"));
        assert!(hash.verify(&password).unwrap());
        assert!(!hash.verify(&Password::new("not the password")).unwrap());
    }

    #[test]
    fn lifecycle_purposes_have_stable_persisted_names() {
        assert_eq!(
            LifecycleActionPurpose::EmailVerification.as_str(),
            "email_verification"
        );
        assert_eq!(
            "password_setup".parse(),
            Ok(LifecycleActionPurpose::PasswordSetup)
        );
        assert_eq!(
            "password_reset".parse(),
            Ok(LifecycleActionPurpose::PasswordReset)
        );
        assert_eq!(
            "unknown".parse::<LifecycleActionPurpose>(),
            Err(LifecycleActionPurposeError::UnknownPurpose)
        );
    }

    #[test]
    fn action_token_persists_a_digest_not_the_delivery_secret() {
        let secret = ActionTokenSecret::generate();

        assert_ne!(
            secret.expose_for_delivery().as_bytes(),
            secret.digest().as_ref()
        );
        assert!(
            ActionTokenSecret::from_delivery_value(secret.expose_for_delivery())
                .unwrap()
                .digest()
                == secret.digest()
        );
        assert!(ActionTokenSecret::from_delivery_value("not-a-token").is_err());
    }
}
