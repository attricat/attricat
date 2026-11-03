//! Process startup and shutdown shared by the API server and file worker.
use std::sync::Arc;

use crate::storage::{ObjectStore, S3ObjectStore, StorageConfig};

/// Connects to the configured S3 bucket and fails startup when it cannot be
/// reached, rather than at the first upload.
pub async fn ready_object_store(
    storage: StorageConfig,
) -> Result<Arc<S3ObjectStore>, &'static str> {
    let store = Arc::new(
        S3ObjectStore::new(storage).map_err(|_| "object storage client could not be created")?,
    );
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

/// Temp files of a process that crashed are left behind on a persistent volume.
/// A live request's file is younger than this, even with replicas sharing the
/// directory.
const STALE_TEMP_FILE_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Removes this application's old files from the temp directory, at startup.
pub async fn clear_stale_temp_files() {
    let directory = std::env::temp_dir();
    let removed = tokio::task::spawn_blocking(move || {
        remove_stale_temp_files(&directory, STALE_TEMP_FILE_AGE)
    })
    .await
    .unwrap_or(0);
    if removed > 0 {
        tracing::info!(removed, "removed temp files left by an earlier run");
    }
}

fn remove_stale_temp_files(directory: &std::path::Path, age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("attricat-"))
        .filter(|entry| {
            entry.metadata().is_ok_and(|metadata| {
                metadata.is_file()
                    && metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|elapsed| elapsed >= age)
            })
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_old_files_of_this_application_are_removed() {
        let directory =
            std::env::temp_dir().join(format!("stale-temp-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let old = std::time::SystemTime::now() - 2 * STALE_TEMP_FILE_AGE;
        for name in ["attricat-upload-old", "other-old"] {
            let file = std::fs::File::create(directory.join(name)).unwrap();
            file.set_modified(old).unwrap();
        }
        std::fs::File::create(directory.join("attricat-upload-new")).unwrap();

        assert_eq!(remove_stale_temp_files(&directory, STALE_TEMP_FILE_AGE), 1);

        assert!(!directory.join("attricat-upload-old").exists());
        assert!(directory.join("other-old").exists());
        assert!(directory.join("attricat-upload-new").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
