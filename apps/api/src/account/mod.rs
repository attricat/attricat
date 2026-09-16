//! Account credential and lifecycle security primitives.
//!
//! This module intentionally has no HTTP, mail, session, or persistence
//! dependencies. Persistence adapters store only the returned hash and token
//! digest values, never the secret wrappers.

mod credentials;
mod lifecycle;
mod sessions;

pub use credentials::{
    MINIMUM_PASSWORD_LENGTH, Password, PasswordHash, PasswordHashError, PasswordPolicyError,
    hash_password, validate_password,
};
pub use lifecycle::{
    ActionTokenDigest, ActionTokenDigestError, ActionTokenSecret, ActionTokenSecretError,
    ActionVerification, CredentialVersion, IssuedLifecycleAction, LifecycleAction,
    LifecycleActionError, LifecycleActionPurpose, LifecycleActionPurposeError, SecurityVersion,
};
pub use sessions::{SessionDigest, SessionSecret, SessionSecretError};
