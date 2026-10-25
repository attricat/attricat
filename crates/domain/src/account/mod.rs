//! Authentication value objects without HTTP or persistence concerns.

mod credentials;
mod lifecycle;
mod secrets;

pub use credentials::{
    MAXIMUM_PASSWORD_BYTES, MINIMUM_PASSWORD_LENGTH, Password, PasswordHash, PasswordHashError,
    PasswordPolicyError, hash_password, validate_password,
};
pub use lifecycle::{LifecycleActionPurpose, LifecycleActionPurposeError};
pub use secrets::{OpaqueSecret, SecretDigest, SecretError};

/// A browser session credential or CSRF value.
pub type SessionSecret = OpaqueSecret;
pub type SessionDigest = SecretDigest;
/// A one-time account lifecycle action token, such as a password reset link.
pub type ActionTokenSecret = OpaqueSecret;
pub type ActionTokenDigest = SecretDigest;
