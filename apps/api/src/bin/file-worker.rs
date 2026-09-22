use std::{net::SocketAddr, sync::Arc, time::Duration};

use api::{
    file_worker::{FileWorker, WorkerConfig},
    storage::{ObjectStore, S3ObjectStore, StorageConfig},
    telemetry::{init_metrics, init_tracing},
};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::{PgPool, postgres::PgPoolOptions};
use subtle::ConstantTimeEq;

#[derive(Clone)]
struct OperationsState {
    pool: PgPool,
    store: Arc<dyn ObjectStore>,
    metrics: PrometheusHandle,
    token: Option<Arc<str>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing("attricat-file-worker")?;
    let metrics = init_metrics()?;
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must be set to start the file worker")?;
    let config = StorageConfig::from_env()
        .map_err(|error| format!("invalid object storage configuration: {error}"))?;
    let object_store = Arc::new(S3ObjectStore::new(config).await);
    object_store.readiness().await.map_err(|_| "file worker storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let worker = FileWorker::new(
        pool.clone(),
        object_store.clone(),
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

    let operations_addr: SocketAddr = std::env::var("FILE_WORKER_OPERATIONS_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3001".to_owned())
        .parse()?;
    let operations_token = std::env::var("FILE_WORKER_METRICS_TOKEN")
        .ok()
        .filter(|value| !value.is_empty())
        .map(Arc::<str>::from);
    if !operations_addr.ip().is_loopback() && operations_token.is_none() {
        return Err("FILE_WORKER_METRICS_TOKEN is required when FILE_WORKER_OPERATIONS_BIND_ADDR is not loopback".into());
    }
    let operations_listener = tokio::net::TcpListener::bind(operations_addr).await?;
    let operations_state = OperationsState {
        pool,
        store: object_store,
        metrics,
        token: operations_token,
    };
    let operations_router = Router::new()
        .route("/health/live", get(worker_liveness))
        .route("/health/ready", get(worker_readiness))
        .route("/metrics", get(worker_metrics))
        .with_state(operations_state);
    let (shutdown_sender, mut operations_shutdown) = tokio::sync::watch::channel(());
    let operations_server = tokio::spawn(async move {
        axum::serve(operations_listener, operations_router)
            .with_graceful_shutdown(async move {
                let _ = operations_shutdown.changed().await;
            })
            .await
    });

    let poll = std::env::var("FILE_WORKER_POLL_MILLISECONDS")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(500_u64);
    tracing::info!(operations_address = %operations_addr, "file worker started");
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = worker.run_once() => { if let Err(error) = result { tracing::error!(%error, "file worker iteration failed"); tokio::time::sleep(Duration::from_secs(1)).await; } }
            _ = &mut shutdown => break,
        }
        tokio::select! {
            _ = &mut shutdown => break,
            _ = tokio::time::sleep(Duration::from_millis(poll)) => {}
        }
    }
    let _ = shutdown_sender.send(());
    operations_server.await??;
    tracing::info!("file worker stopped");
    Ok(())
}

async fn shutdown_signal() {
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

async fn worker_liveness() -> impl IntoResponse {
    (StatusCode::OK, "live\n")
}

async fn worker_readiness(State(state): State<OperationsState>) -> Response {
    let database = sqlx::query("SELECT 1").execute(&state.pool).await;
    let storage = state.store.readiness().await;
    metrics::gauge!("catalog_file_worker_database_ready").set(if database.is_ok() {
        1.0
    } else {
        0.0
    });
    if database.is_ok() && storage.is_ok() {
        (StatusCode::OK, "ready\n").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready\n").into_response()
    }
}

async fn worker_metrics(State(state): State<OperationsState>, headers: HeaderMap) -> Response {
    if let Some(expected) = state.token.as_deref() {
        let supplied = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        let authorized = supplied.len() == expected.len()
            && bool::from(supplied.as_bytes().ct_eq(expected.as_bytes()));
        if !authorized {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics.render(),
    )
        .into_response()
}
