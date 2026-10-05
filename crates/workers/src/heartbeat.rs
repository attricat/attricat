//! Poll job execution and lease renewal independently. Awaiting renewal inside
//! a timer branch stops polling the job, which may own the very database lock
//! or pool connection that renewal needs.

use std::future::Future;

/// A renewal loop finishes only when it loses the lease or cannot renew it.
/// Drop the losing future before returning, so lease-loss cleanup cannot wait
/// on resources still owned by the cancelled job.
pub(crate) async fn with_heartbeat<T, E>(
    operation: impl Future<Output = T>,
    renewal: impl Future<Output = E>,
) -> Result<T, E> {
    tokio::select! {
        biased;
        error = renewal => Err(error),
        result = operation => Ok(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::sync::oneshot;

    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn pending_renewal_does_not_stop_the_operation_that_unblocks_it() {
        let (started, waiting) = oneshot::channel();
        let (release, blocked) = oneshot::channel();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            with_heartbeat(
                async {
                    waiting.await.unwrap();
                    release.send(()).unwrap();
                    42
                },
                async {
                    started.send(()).unwrap();
                    blocked.await.unwrap();
                    std::future::pending::<()>().await
                },
            ),
        )
        .await
        .unwrap();
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn lease_loss_drops_the_job_before_cleanup_can_run() {
        let dropped = Arc::new(AtomicBool::new(false));
        let (started, waiting) = oneshot::channel();
        let result = with_heartbeat(
            async {
                let _guard = Dropped(dropped.clone());
                started.send(()).unwrap();
                std::future::pending::<()>().await
            },
            async {
                waiting.await.unwrap();
                "lost"
            },
        )
        .await;
        assert_eq!(result, Err("lost"));
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn completing_a_job_cancels_its_renewal() {
        let dropped = Arc::new(AtomicBool::new(false));
        let result = with_heartbeat(async { 42 }, async {
            let _guard = Dropped(dropped.clone());
            std::future::pending::<()>().await
        })
        .await;
        assert_eq!(result, Ok(42));
        assert!(dropped.load(Ordering::SeqCst));
    }
}
