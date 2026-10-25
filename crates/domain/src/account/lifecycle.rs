use std::str::FromStr;

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
}
