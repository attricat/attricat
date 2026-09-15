use crate::repository::CatalogRepository;
use std::{collections::HashSet, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 256;
const MAX_CONCURRENT_BATCHES: usize = 4;
const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct MigrationBatchDispatcher {
    sender: mpsc::Sender<MigrationBatchJob>,
}

struct MigrationBatchJob {
    repository: CatalogRepository,
    batch_id: Uuid,
    is_recovery: bool,
}

/// Starts the durable safe-migration worker.
///
/// Queue entries are only notifications: the batch table remains the source of
/// truth, and periodic recovery picks up notifications that could not fit in
/// the process-local queue. At most `MAX_CONCURRENT_BATCHES` batches execute
/// in one API process; repository leases prevent duplicate work across
/// processes.
pub fn start(
    recovery_repository: CatalogRepository,
    shutdown: watch::Receiver<()>,
) -> (MigrationBatchDispatcher, tokio::task::JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::channel(QUEUE_CAPACITY);
    let dispatcher = MigrationBatchDispatcher {
        sender: sender.clone(),
    };
    let handle = tokio::spawn(async move {
        let mut shutdown = shutdown;
        let mut recovery = tokio::time::interval(RECOVERY_INTERVAL);
        let mut scheduled = HashSet::new();
        let mut workers = JoinSet::new();
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    receiver.close();
                    while let Some(result) = workers.join_next().await {
                        if let Err(error) = result {
                            tracing::error!(%error, "safe blueprint migration worker task failed while shutting down");
                        }
                    }
                    return;
                }
                Some(result) = workers.join_next(), if !workers.is_empty() => {
                    match result {
                        Ok(batch_id) => {
                            scheduled.remove(&batch_id);
                        }
                        Err(error) => {
                            tracing::error!(%error, "safe blueprint migration worker task failed");
                        }
                    }
                }
                _ = recovery.tick() => {
                    enqueue_recovered_batches(&recovery_repository, &sender, &mut scheduled).await;
                }
                job = receiver.recv(), if workers.len() < MAX_CONCURRENT_BATCHES => {
                    match job {
                        Some(job) if job.is_recovery || scheduled.insert(job.batch_id) => {
                            let job_shutdown = shutdown.clone();
                            workers.spawn(run_batch(job, job_shutdown));
                        }
                        Some(job) => {
                            tracing::debug!(batch_id = %job.batch_id, "safe blueprint migration batch is already scheduled");
                        }
                        None => return,
                    }
                }
            }
        }
    });
    (dispatcher, handle)
}

impl MigrationBatchDispatcher {
    /// Schedules a durable batch for prompt execution. A full local queue is
    /// not an error for callers because periodic database recovery will retry
    /// the already-persisted queued batch.
    pub fn enqueue(&self, repository: CatalogRepository, batch_id: Uuid) {
        if let Err(error) = self.sender.try_send(MigrationBatchJob {
            repository,
            batch_id,
            is_recovery: false,
        }) {
            tracing::warn!(%batch_id, %error, "safe blueprint migration batch remains queued for recovery");
        }
    }
}

async fn enqueue_recovered_batches(
    repository: &CatalogRepository,
    sender: &mpsc::Sender<MigrationBatchJob>,
    scheduled: &mut HashSet<Uuid>,
) {
    let batches = match repository.recover_safe_blueprint_migration_batches().await {
        Ok(batches) => batches,
        Err(error) => {
            tracing::error!(%error, "could not recover safe blueprint migration batches");
            return;
        }
    };
    for (workspace_id, batch_id) in batches {
        if scheduled.contains(&batch_id) {
            continue;
        }
        let workspace_repository = match repository.for_workspace(workspace_id).await {
            Ok(repository) => repository,
            Err(error) => {
                tracing::error!(%workspace_id, %batch_id, %error, "could not scope recovered blueprint migration batch");
                continue;
            }
        };
        match sender.try_send(MigrationBatchJob {
            repository: workspace_repository,
            batch_id,
            is_recovery: true,
        }) {
            Ok(()) => {
                scheduled.insert(batch_id);
            }
            Err(error) => {
                tracing::debug!(%batch_id, %error, "safe blueprint migration recovery queue is full");
                break;
            }
        }
    }
}

async fn run_batch(job: MigrationBatchJob, mut shutdown: watch::Receiver<()>) -> Uuid {
    let batch_id = job.batch_id;
    let lease_owner = Uuid::new_v4();
    tokio::select! {
        result = job.repository.run_safe_blueprint_migration_batch_with_lease(batch_id, lease_owner) => {
            if let Err(error) = result {
                tracing::error!(%batch_id, %error, "safe blueprint migration batch failed");
            }
        }
        _ = shutdown.changed() => {
            if let Err(error) = job.repository.release_safe_blueprint_migration_batch_lease(batch_id, lease_owner).await {
                tracing::error!(%batch_id, %error, "could not release safe blueprint migration batch during shutdown");
            }
        }
    }
    batch_id
}
