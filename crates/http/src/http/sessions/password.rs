//! Argon2id work for request handlers.
//!
//! Each hash deliberately costs ~19 MiB and tens of milliseconds of CPU. It
//! runs on the blocking pool so it never stalls async workers, and a process
//! wide permit bounds concurrency so unauthenticated routes (login, reset,
//! onboarding) cannot exhaust memory or the blocking pool.

use std::{num::NonZeroUsize, sync::LazyLock};

use tokio::sync::Semaphore;

use super::super::error::ApiError;
use crate::account::{
    MAXIMUM_PASSWORD_BYTES, Password, PasswordHash, hash_password as hash_password_blocking,
};

static PASSWORD_WORK: LazyLock<Semaphore> = LazyLock::new(|| {
    Semaphore::new(std::thread::available_parallelism().map_or(4, NonZeroUsize::get))
});

/// Verified in place of a missing credential so a login takes the same time
/// whether or not the account exists.
static TIMING_EQUALIZER: LazyLock<Option<PasswordHash>> =
    LazyLock::new(|| hash_password_blocking(&Password::new("timing-equalizer")).ok());

async fn run<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    let _permit = PASSWORD_WORK
        .acquire()
        .await
        .map_err(|_| ApiError::internal("password hashing is unavailable"))?;
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ApiError::internal("password hashing failed"))
}

pub(in super::super) async fn hash_password(password: String) -> Result<PasswordHash, ApiError> {
    run(move || hash_password_blocking(&Password::new(password)))
        .await?
        .map_err(|_| ApiError::internal("password could not be set"))
}

/// Returns whether `password` matches `hash`. A missing hash is verified
/// against a fixed dummy value and always fails.
pub(super) async fn verify_password(
    hash: Option<PasswordHash>,
    password: String,
) -> Result<bool, ApiError> {
    // Length is not secret, so oversized input can be rejected without work.
    if password.len() > MAXIMUM_PASSWORD_BYTES {
        return Ok(false);
    }
    run(move || {
        let password = Password::new(password);
        match hash {
            Some(hash) => hash.verify(&password).unwrap_or(false),
            None => {
                if let Some(dummy) = TIMING_EQUALIZER.as_ref() {
                    let _ = dummy.verify(&password);
                }
                false
            }
        }
    })
    .await
}
