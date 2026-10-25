//! Process startup and shutdown shared by the API server and file worker.
use std::sync::Arc;

use crate::storage::{ObjectStore, S3ObjectStore, StorageConfig};

/// Connects to the configured S3 bucket and fails startup when it cannot be
/// reached, rather than at the first upload.
pub async fn ready_object_store(
    storage: StorageConfig,
) -> Result<Arc<S3ObjectStore>, &'static str> {
    let store = Arc::new(S3ObjectStore::new(storage).await);
    store.readiness().await.map_err(|_| {
        "object storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials"
    })?;
    Ok(store)
}

/// Resolves on Ctrl-C, or on SIGTERM on Unix.
pub async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            result = ctrl_c => result.expect("install Ctrl-C handler"),
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await.expect("install Ctrl-C handler");
}
