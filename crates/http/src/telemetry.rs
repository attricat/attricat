use std::{
    collections::HashMap,
    error::Error,
    sync::{Mutex, OnceLock},
};

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{resource::Resource, trace::SdkTracerProvider};
use tracing::{Event, Id, Subscriber};
use tracing_subscriber::{
    EnvFilter, Layer, fmt,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
};

use crate::http::RequestTiming;

static METRICS: Mutex<Option<PrometheusHandle>> = Mutex::new(None);
static REQUEST_TIMINGS: OnceLock<Mutex<HashMap<Id, RequestTiming>>> = OnceLock::new();
static SQL_OPERATIONS: OnceLock<Mutex<HashMap<Id, String>>> = OnceLock::new();

fn request_timings() -> &'static Mutex<HashMap<Id, RequestTiming>> {
    REQUEST_TIMINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sql_operations() -> &'static Mutex<HashMap<Id, String>> {
    SQL_OPERATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn register_request_timing(id: Id, timing: RequestTiming) {
    request_timings()
        .lock()
        .expect("request timing lock is not poisoned")
        .insert(id, timing);
}

pub(crate) fn unregister_request_timing(id: Id) {
    request_timings()
        .lock()
        .expect("request timing lock is not poisoned")
        .remove(&id);
}

/// Collects SQLx's measured execution durations without retaining or exposing
/// statements, bind values, rows, or identifiers.
struct SqlTimingLayer;

impl<S> Layer<S> for SqlTimingLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attributes: &tracing::span::Attributes<'_>, id: &Id, _: Context<'_, S>) {
        if attributes.metadata().name() != "sql.operation" {
            return;
        }
        let mut visitor = SqlOperationLabel::default();
        attributes.record(&mut visitor);
        if let Some(label) = visitor.label {
            sql_operations()
                .lock()
                .expect("SQL operation lock is not poisoned")
                .insert(id.clone(), label);
        }
    }

    fn on_close(&self, id: Id, _: Context<'_, S>) {
        sql_operations()
            .lock()
            .expect("SQL operation lock is not poisoned")
            .remove(&id);
        // The middleware unregisters on completion, but a cancelled request
        // (client disconnect, timeout) never reaches that point. Span IDs are
        // reused, so a leaked entry would also misattribute later timings.
        unregister_request_timing(id);
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        if event.metadata().target() != "sqlx::query" {
            return;
        }
        let mut visitor = ElapsedSeconds::default();
        event.record(&mut visitor);
        let Some(elapsed_seconds) = visitor.elapsed_seconds else {
            return;
        };
        catalog_repository::round_trips::record_round_trip();
        let Some(scope) = context.event_scope(event) else {
            return;
        };
        let mut label = None;
        let mut timing = None;
        for span in scope.from_root() {
            if label.is_none() {
                label = sql_operations()
                    .lock()
                    .expect("SQL operation lock is not poisoned")
                    .get(&span.id())
                    .cloned();
            }
            if timing.is_none() {
                timing = request_timings()
                    .lock()
                    .expect("request timing lock is not poisoned")
                    .get(&span.id())
                    .cloned();
            }
        }
        if let Some(timing) = timing {
            timing.record_sql(label.as_deref(), elapsed_seconds * 1_000.0);
        }
    }
}

#[derive(Default)]
struct ElapsedSeconds {
    elapsed_seconds: Option<f64>,
}

#[derive(Default)]
struct SqlOperationLabel {
    label: Option<String>,
}

impl tracing::field::Visit for SqlOperationLabel {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "label" {
            self.label = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

impl tracing::field::Visit for ElapsedSeconds {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == "elapsed_secs" {
            self.elapsed_seconds = Some(value);
        }
    }

    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

pub fn init_tracing(service_name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).with_filter(filter))
        .with(SqlTimingLayer);

    if std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .is_ok_and(|endpoint| !endpoint.trim().is_empty())
    {
        let exporter = SpanExporter::builder().with_tonic().build()?;
        let provider = SdkTracerProvider::builder()
            .with_resource(
                Resource::builder()
                    .with_attribute(KeyValue::new("service.name", service_name.to_owned()))
                    .build(),
            )
            .with_batch_exporter(exporter)
            .build();
        let tracer = provider.tracer(service_name.to_owned());
        global::set_tracer_provider(provider);
        subscriber
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()?;
    } else {
        subscriber.try_init()?;
    }
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
