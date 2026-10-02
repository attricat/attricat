//! Bounded periodic retention and cancellation-safe object cleanup.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use catalog_repository::repository::{
    RepositoryError, SystemRepository, ValueHistoryRetentionDays,
};
use catalog_storage::{ObjectStore, ObjectStoreError};
use tokio::sync::watch;

const INTERVAL: Duration = Duration::from_secs(60);
const SWEEP_BUDGET: Duration = Duration::from_secs(10);

pub fn start(
    repository: SystemRepository,
    store: Arc<dyn ObjectStore>,
    retention: ValueHistoryRetentionDays,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = tick.tick() => {
                    tokio::select! {
                        _ = shutdown.changed() => break,
                        _ = run_once(&repository, store.as_ref(), retention) => {}
                    }
                }
            }
        }
    })
}

async fn run_once(
    repository: &SystemRepository,
    store: &dyn ObjectStore,
    retention: ValueHistoryRetentionDays,
) {
    let started = Instant::now();
    let history = tokio::time::timeout(SWEEP_BUDGET, async {
        loop {
            let deleted = repository.purge_value_history_batch(retention).await?;
            metrics::counter!("catalog_value_history_entries_purged_total").increment(deleted);
            if deleted < 1000 {
                return Ok::<(), RepositoryError>(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    let outcome = match history {
        Ok(Ok(())) => "success",
        Ok(Err(error)) => {
            tracing::error!(%error, "history retention batch failed; will retry");
            "failed"
        }
        Err(_) => "budget_exhausted",
    };
    metrics::counter!("catalog_value_history_cleanup_total", "outcome" => outcome).increment(1);
    metrics::histogram!("catalog_value_history_cleanup_duration_seconds")
        .record(started.elapsed().as_secs_f64());

    let _ = tokio::time::timeout(SWEEP_BUDGET, async {
        for _ in 0..100 {
            match cleanup_upload_once(repository, store).await {
                Ok(true) => {},
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(%error, "upload cleanup failed; durable intent retained for retry");
                    metrics::counter!("catalog_upload_cleanup_total", "outcome" => "failed").increment(1);
                    break;
                }
            }
        }
    }).await;
}

/// One idempotent external deletion. A failure or process crash leaves the
/// claimed intent reclaimable after five minutes. No DB lock spans S3 I/O.
pub async fn cleanup_upload_once(
    repository: &SystemRepository,
    store: &dyn ObjectStore,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(upload) = repository.claim_abandoned_upload().await? else {
        return Ok(false);
    };
    match store.delete(&upload.object_key).await {
        Ok(()) | Err(ObjectStoreError::NotFound) => {}
        Err(error) => return Err(error.into()),
    }
    repository.complete_abandoned_upload(&upload).await?;
    metrics::counter!("catalog_upload_cleanup_total", "outcome" => "success").increment(1);
    Ok(true)
}
