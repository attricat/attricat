use std::{error::Error, sync::Mutex};

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{EnvFilter, fmt};

static METRICS: Mutex<Option<PrometheusHandle>> = Mutex::new(None);

pub fn init_tracing() -> Result<(), Box<dyn Error + Send + Sync>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()?;
    Ok(())
}

pub fn init_metrics() -> Result<PrometheusHandle, Box<dyn Error + Send + Sync>> {
    let mut metrics = METRICS
        .lock()
        .expect("metrics initialization lock is not poisoned");
    if let Some(handle) = metrics.as_ref() {
        return Ok(handle.clone());
    }
    let handle = PrometheusBuilder::new().install_recorder()?;
    *metrics = Some(handle.clone());
    Ok(handle)
}
