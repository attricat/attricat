use std::sync::Arc;

use api::{
    config::Config,
    file_worker::FileWorker,
    runtime::{ready_object_store, shutdown_signal},
    storage::ObjectStore,
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

/// A job holds a transaction while its lease heartbeat renews.
const CONNECTIONS_PER_JOB: u32 = 2;
/// Claiming and reconciliation, plus the readiness probe.
const SHARED_CONNECTIONS: u32 = 2;

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
    let object_store = ready_object_store(config.storage).await?;
    let concurrency = file_worker.concurrency;
    let pool = PgPoolOptions::new()
        .max_connections(concurrency as u32 * CONNECTIONS_PER_JOB + SHARED_CONNECTIONS)
        .connect(config.database.url.expose_secret())
        .await?;
    let worker = Arc::new(FileWorker::new(
        pool.clone(),
        object_store.clone(),
        file_worker.worker,
    ));
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
    tracing::info!(operations_address = %operations_addr, concurrency, "file worker started");
    tokio::select! {
        () = worker.run(concurrency, poll) => {}
        () = shutdown_signal() => {}
    }
    let _ = shutdown_sender.send(());
    operations_server.await??;
    tracing::info!("file worker stopped");
    Ok(())
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
