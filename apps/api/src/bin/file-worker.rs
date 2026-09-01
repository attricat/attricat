use api::{
    storage::{ObjectStore, S3ObjectStore, StorageConfig},
    telemetry::{init_metrics, init_tracing},
};

/// Storage-ready process seam for the durable file worker introduced in issue
/// #60. Keeping it separate now ensures its configuration and readiness checks
/// match the API before it begins claiming jobs.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing()?;
    init_metrics()?;

    let config = StorageConfig::from_env()
        .map_err(|error| format!("invalid object storage configuration: {error}"))?;
    let object_store = S3ObjectStore::new(config).await;
    object_store.readiness().await.map_err(|_| {
        "file worker storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials"
    })?;

    tracing::info!("file worker is storage-ready; waiting for durable job support");
    tokio::signal::ctrl_c().await?;
    tracing::info!("file worker stopped");
    Ok(())
}
