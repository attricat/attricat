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
    let permit = PASSWORD_WORK
        .acquire()
        .await
        .map_err(|_| ApiError::internal("password hashing is unavailable"))?;
    run_with_permit(permit, work).await
}

async fn run_with_permit<T: Send + 'static>(
    permit: tokio::sync::SemaphorePermit<'static>,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(move || {
        // Dropping the request does not cancel spawn_blocking. Keep the permit
        // on the blocking thread until its CPU/memory work actually ends.
        let _permit = permit;
        work()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_keeps_the_permit_until_blocking_work_finishes() {
        static PERMITS: Semaphore = Semaphore::const_new(1);
        let permit = PERMITS.acquire().await.unwrap();
        let (started, waiting) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let work = tokio::spawn(run_with_permit(permit, move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
        }));
        waiting.await.unwrap();
        work.abort();
        assert!(work.await.unwrap_err().is_cancelled());
        assert!(PERMITS.try_acquire().is_err());
        release.send(()).unwrap();
        let _permit = tokio::time::timeout(std::time::Duration::from_secs(2), PERMITS.acquire())
            .await
            .unwrap()
            .unwrap();
    }
}
