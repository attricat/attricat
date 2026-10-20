use std::{sync::Arc, time::Duration};

use api::{
    config::Config,
    file_worker::FileWorker,
    storage::{ObjectStore, S3ObjectStore},
    telemetry::{PROMETHEUS_CONTENT_TYPE, init_metrics, init_tracing},
};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use secrecy::ExposeSecret;
use sqlx::{PgPool, postgres::PgPoolOptions};
use subtle::ConstantTimeEq;

const POOL_CONNECTIONS: u32 = 4;
/// Back-off after a failed iteration, so a persistent fault does not spin.
const FAILED_ITERATION_BACKOFF: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct OperationsState {
    pool: PgPool,
    store: Arc<dyn ObjectStore>,
    metrics: PrometheusHandle,
    token: Option<Arc<str>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    init_tracing(
        "attricat-file-worker",
        config.telemetry.log_filter.as_deref(),
        config.telemetry.otlp_traces_endpoint.as_deref(),
    )?;
    let metrics = init_metrics()?;
    let file_worker = config.file_worker;
    let object_store = Arc::new(S3ObjectStore::new(config.storage).await);
    object_store.readiness().await.map_err(|_| "file worker storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials")?;
    let pool = PgPoolOptions::new()
        .max_connections(POOL_CONNECTIONS)
        .connect(config.database.url.expose_secret())
        .await?;
    let worker = FileWorker::new(pool.clone(), object_store.clone(), file_worker.worker);
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(job_id) = args
        .windows(2)
        .find_map(|args| (args[0] == "--retry").then_some(&args[1]))
    {
        let retried = worker.retry_job(job_id.parse()?).await?;
        tracing::info!(%job_id, retried, "operator retry requested");
        return Ok(());
    }

    let operations_addr = file_worker.operations_bind_addr;
    let operations_token = file_worker
        .metrics_token
        .map(|token| Arc::<str>::from(token.expose_secret()));
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

    let poll = file_worker.poll_interval;
    tracing::info!(operations_address = %operations_addr, "file worker started");
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = worker.run_once() => {
                if let Err(error) = result {
                    tracing::error!(%error, "file worker iteration failed");
                    tokio::time::sleep(FAILED_ITERATION_BACKOFF).await;
                }
            }
            _ = &mut shutdown => break,
        }
        tokio::select! {
            _ = &mut shutdown => break,
            _ = tokio::time::sleep(poll) => {}
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
    metrics::gauge!("attricat_file_worker_database_ready").set(if database.is_ok() {
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
            .get(header::AUTHORIZATION)
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
        [(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)],
        state.metrics.render(),
    )
        .into_response()
}
