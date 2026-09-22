use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use rand::{RngCore, rngs::OsRng};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq;
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

/// A password-change version captured when an action is issued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialVersion(i64);

impl CredentialVersion {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i64 {
        self.0
    }
}

/// A version that invalidates security-sensitive actions when it changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityVersion(i64);

impl SecurityVersion {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i64 {
        self.0
    }
}

/// A randomly generated, opaque 32-byte action-token secret.
///
/// The value is encoded only for delivery to the action recipient. It does not
/// implement `Debug`, `Display`, or serialization traits.
pub struct ActionTokenSecret(SecretString);

impl ActionTokenSecret {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(SecretString::from(URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// Reconstitutes a delivered token after validating its exact opaque form.
    pub fn from_delivery_value(value: impl Into<String>) -> Result<Self, ActionTokenSecretError> {
        let value = value.into();
        let decoded = URL_SAFE_NO_PAD
            .decode(&value)
            .map_err(|_| ActionTokenSecretError::InvalidToken)?;
        if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(ActionTokenSecretError::InvalidToken);
        }
        Ok(Self(SecretString::from(value)))
    }

    /// Explicitly exposes the token for a future delivery adapter.
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

impl ActionTokenDigest {
    pub fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub fn from_slice(value: &[u8]) -> Result<Self, ActionTokenDigestError> {
        let bytes: [u8; 32] = value
            .try_into()
            .map_err(|_| ActionTokenDigestError::InvalidDigest)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn matches(&self, secret: &ActionTokenSecret) -> bool {
        self.0.ct_eq(secret.digest().as_bytes()).into()
    }
}

impl AsRef<[u8]> for ActionTokenDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ActionTokenDigestError {
    #[error("action token digest must be 32 bytes")]
    InvalidDigest,
}

/// The stored fields needed to validate a lifecycle action. This contains a
/// digest, never the delivered token secret.
#[derive(Clone, Eq, PartialEq)]
pub struct LifecycleAction {
    purpose: LifecycleActionPurpose,
    token_digest: ActionTokenDigest,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
    security_version: SecurityVersion,
    credential_version: Option<CredentialVersion>,
}

impl LifecycleAction {
    /// Rehydrates a lifecycle action from fields returned by persistence.
    pub fn from_persisted(
        purpose: LifecycleActionPurpose,
        token_digest: ActionTokenDigest,
        expires_at: DateTime<Utc>,
        consumed_at: Option<DateTime<Utc>>,
        security_version: SecurityVersion,
        credential_version: Option<CredentialVersion>,
    ) -> Self {
        Self {
            purpose,
            token_digest,
            expires_at,
            consumed_at,
            security_version,
            credential_version,
        }
    }

    pub fn purpose(&self) -> LifecycleActionPurpose {
        self.purpose
    }

    pub fn token_digest(&self) -> &ActionTokenDigest {
        &self.token_digest
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn consumed_at(&self) -> Option<DateTime<Utc>> {
        self.consumed_at
    }

    pub fn security_version(&self) -> SecurityVersion {
        self.security_version
    }

    pub fn credential_version(&self) -> Option<CredentialVersion> {
        self.credential_version
    }

    /// Verifies all conditions before a persistence layer consumes the action.
    pub fn verify(
        &self,
        secret: &ActionTokenSecret,
        expected: ActionVerification,
    ) -> Result<(), LifecycleActionError> {
        if !self.token_digest.matches(secret) {
            return Err(LifecycleActionError::InvalidToken);
        }
        if self.purpose != expected.purpose {
            return Err(LifecycleActionError::WrongPurpose);
        }
        if self.consumed_at.is_some() {
            return Err(LifecycleActionError::AlreadyConsumed);
        }
        if self.expires_at <= expected.now {
            return Err(LifecycleActionError::Expired);
        }
        if self.security_version != expected.security_version {
            return Err(LifecycleActionError::SecurityVersionMismatch);
        }
        if self.credential_version != expected.credential_version {
            return Err(LifecycleActionError::CredentialVersionMismatch);
        }
        Ok(())
    }

    /// Marks an already verified action consumed for an in-memory workflow.
    ///
    /// A persistence implementation must perform the equivalent state change
    /// atomically with `consumed_at IS NULL` so concurrent requests cannot
    /// consume the same action twice.
    pub fn consume(
        &mut self,
        secret: &ActionTokenSecret,
        expected: ActionVerification,
    ) -> Result<(), LifecycleActionError> {
        self.verify(secret, expected)?;
        self.consumed_at = Some(expected.now);
        Ok(())
    }
}

/// Context supplied by the current account state when an action is checked.
#[derive(Clone, Copy, Debug)]
pub struct ActionVerification {
    pub purpose: LifecycleActionPurpose,
    pub now: DateTime<Utc>,
    pub security_version: SecurityVersion,
    pub credential_version: Option<CredentialVersion>,
}

/// An action record plus its one-time delivery secret. Only the action record
/// belongs in persistence.
pub struct IssuedLifecycleAction {
    pub action: LifecycleAction,
    pub secret: ActionTokenSecret,
}

impl IssuedLifecycleAction {
    pub fn issue(
        purpose: LifecycleActionPurpose,
        expires_at: DateTime<Utc>,
        security_version: SecurityVersion,
        credential_version: Option<CredentialVersion>,
    ) -> Self {
        let secret = ActionTokenSecret::generate();
        let action = LifecycleAction {
            purpose,
            token_digest: secret.digest(),
            expires_at,
            consumed_at: None,
            security_version,
            credential_version,
        };
        Self { action, secret }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum LifecycleActionError {
    #[error("action token is invalid")]
    InvalidToken,
    #[error("action purpose does not match")]
    WrongPurpose,
    #[error("action has already been consumed")]
    AlreadyConsumed,
    #[error("action has expired")]
    Expired,
    #[error("action security version no longer matches")]
    SecurityVersionMismatch,
    #[error("action credential version no longer matches")]
    CredentialVersionMismatch,
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;
    use crate::account::{Password, hash_password};

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 22, 12, 0, 0).unwrap()
    }

    fn verification(purpose: LifecycleActionPurpose) -> ActionVerification {
        ActionVerification {
            purpose,
            now: now(),
            security_version: SecurityVersion::new(4),
            credential_version: Some(CredentialVersion::new(7)),
        }
    }

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
    fn lifecycle_action_persists_a_digest_not_the_delivery_secret() {
        let issued = IssuedLifecycleAction::issue(
            LifecycleActionPurpose::PasswordReset,
            now() + Duration::minutes(30),
            SecurityVersion::new(4),
            Some(CredentialVersion::new(7)),
        );

        assert_ne!(
            issued.secret.expose_for_delivery().as_bytes(),
            issued.action.token_digest().as_bytes()
        );
        assert!(issued.action.token_digest() == &issued.secret.digest());
        assert!(
            ActionTokenSecret::from_delivery_value(issued.secret.expose_for_delivery())
                .unwrap()
                .digest()
                == issued.secret.digest()
        );
    }

    #[test]
    fn lifecycle_action_rejects_expired_replayed_and_stale_security_conditions() {
        let mut issued = IssuedLifecycleAction::issue(
            LifecycleActionPurpose::PasswordReset,
            now() + Duration::minutes(30),
            SecurityVersion::new(4),
            Some(CredentialVersion::new(7)),
        );

        assert_eq!(
            issued.action.verify(
                &issued.secret,
                ActionVerification {
                    now: now() + Duration::minutes(31),
                    ..verification(LifecycleActionPurpose::PasswordReset)
                }
            ),
            Err(LifecycleActionError::Expired)
        );
        assert_eq!(
            issued.action.verify(
                &issued.secret,
                ActionVerification {
                    security_version: SecurityVersion::new(5),
                    ..verification(LifecycleActionPurpose::PasswordReset)
                }
            ),
            Err(LifecycleActionError::SecurityVersionMismatch)
        );
        assert_eq!(
            issued.action.verify(
                &issued.secret,
                ActionVerification {
                    credential_version: Some(CredentialVersion::new(8)),
                    ..verification(LifecycleActionPurpose::PasswordReset)
                }
            ),
            Err(LifecycleActionError::CredentialVersionMismatch)
        );

        issued
            .action
            .consume(
                &issued.secret,
                verification(LifecycleActionPurpose::PasswordReset),
            )
            .unwrap();
        assert_eq!(
            issued.action.verify(
                &issued.secret,
                verification(LifecycleActionPurpose::PasswordReset)
            ),
            Err(LifecycleActionError::AlreadyConsumed)
        );
        assert_eq!(
            issued.action.verify(
                &ActionTokenSecret::generate(),
                verification(LifecycleActionPurpose::PasswordReset)
            ),
            Err(LifecycleActionError::InvalidToken)
        );
    }
}
