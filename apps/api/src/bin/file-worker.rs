use std::{sync::Arc, time::Duration};

use api::{
    file_worker::{FileWorker, WorkerConfig},
    storage::{ObjectStore, S3ObjectStore, StorageConfig},
    telemetry::{init_metrics, init_tracing},
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing("attricat-file-worker")?;
    init_metrics()?;
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must be set to start the file worker")?;
    let config = StorageConfig::from_env()
        .map_err(|error| format!("invalid object storage configuration: {error}"))?;
    let object_store = Arc::new(S3ObjectStore::new(config).await);
    object_store.readiness().await.map_err(|_| "file worker storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials")?;
    let worker = FileWorker::new(
        PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await?,
        object_store,
        WorkerConfig::from_env()?,
    );
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(job_id) = args
        .windows(2)
        .find_map(|args| (args[0] == "--retry").then_some(&args[1]))
    {
        let retried = worker.retry_job(job_id.parse()?).await?;
        tracing::info!(%job_id, retried, "operator retry requested");
        return Ok(());
    }
    let poll = std::env::var("FILE_WORKER_POLL_MILLISECONDS")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(500_u64);
    tracing::info!("file worker started");
    loop {
        tokio::select! {
            result = worker.run_once() => { if let Err(error) = result { tracing::error!(%error, "file worker iteration failed"); tokio::time::sleep(Duration::from_secs(1)).await; } }
            _ = tokio::signal::ctrl_c() => break,
        }
        tokio::time::sleep(Duration::from_millis(poll)).await;
    }
    tracing::info!("file worker stopped");
    Ok(())
}
