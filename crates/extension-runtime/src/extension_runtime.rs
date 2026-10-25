//! Sandboxed execution of installed server extension components.
//!
//! Components get no WASI context, filesystem, environment, clocks, sockets, or
//! pre-opened descriptors. The only imports are the versioned WIT functions in
//! the `wit*/attricat-extension.wit` packages (`wit-host` is the unified,
//! evolving ABI; the others are frozen legacy worlds). Every call is checked
//! against the immutable release manifest and invocation-time grants.

use std::{
    collections::{HashMap, VecDeque},
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::StreamExt;
use metrics::{counter, histogram};
use reqwest::{Client, Method, redirect::Policy};
use url::Url;

use async_trait::async_trait;
use attricat_cache::{LocalRateLimiter, RateLimiter};
use attricat_repository::round_trips::measure;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::watch;
use uuid::Uuid;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, HasSelf, Linker, Resource, ResourceTable},
};

use crate::{
    attricat_service::AttricatMutationService,
    constants::DEFAULT_LIST_PAGE_SIZE,
    domain_events::DomainEvent,
    extension_installer::installed_artifact_key,
    extensions::{
        ArtifactKind, EventHandler as ManifestEventHandler, MAX_EXTENSION_IDENTIFIER_BYTES,
    },
    model::{AppendAttributeValues, NewAttributeValue},
    repository::{
        AttricatRepository, ClaimedExtensionOperationRun, ClaimedTask, CoordinatorLeadership,
        DeliveryState, ExtensionAttricatBatch, ExtensionAttricatIntent,
        ExtensionAttricatPageRequest, ExtensionConfigurationScope, ExtensionOperationArtifact,
        ExtensionRuntimeInstallation, HandlerAttemptOutcome, MAX_EXTENSION_BATCH_INTENTS,
        MAX_EXTENSION_LOOKUP_VALUE_BYTES, MaterializeScope, RejectionRecord, RepositoryError,
        SystemRepository,
    },
    storage::{ObjectStore, ObjectStoreError, StoredObject},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use attricat_workers::coordinator;

#[cfg(test)]
mod abi_evolution_tests;
pub mod connections;
mod connector_v2;
mod export_host;
mod interactive;
mod network_v2;
mod operation_batch;
mod outcomes;
mod transfer;
mod unified;

use unified::host_unified::attricat::host as wit;

pub use export_host::use_public_file_link_base;
pub use network_v2::forget_lowered_budget;
use operation_batch::{BatchError, BatchResult, run_operation_batch};
use outcomes::retry_backoff;
pub use outcomes::{InvocationFailure, Rejection};

const MAX_HOST_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_HOST_JSON_BYTES: usize = 64 * 1024;
const MAX_CACHED_COMPONENTS: usize = 64;
const MAX_WRITE_VALUES: usize = 100;
/// Values one connector upsert row may write.
const MAX_CONNECTOR_UPSERT_VALUES: usize = 128;
/// Rows one connector catalog page may return; keeps results within the WIT
/// JSON bound even for wide blueprints.
const MAX_CONNECTOR_PAGE_LIMIT: u32 = 16;
const DEFAULT_FUEL: u64 = 10_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_INVOCATION_TIMEOUT: Duration = Duration::from_secs(10);
/// The event consumer that feeds enabled extension event handlers.
const WASM_EVENT_CONSUMER: &str = "attricat.extensions.wasm";
/// How long a paused operation run waits before checking whether its pinned
/// release is authorized again.
const PAUSED_OPERATION_RECHECK_DELAY: chrono::TimeDelta = chrono::TimeDelta::seconds(30);
/// How many times in a row a run's batch may ask to wait with a typed retry
/// delay before further waits spend the run's failure budget.
const MAX_RETRY_DEFERRALS: i32 = 100;
/// How soon a batch replays after it rejected one record.
const RECORD_REJECTION_REPLAY_DELAY: chrono::TimeDelta = chrono::TimeDelta::seconds(1);
/// Headers a component may never set on an outbound request.
const RESERVED_REQUEST_HEADERS: [&str; 4] =
    ["host", "content-length", "connection", "transfer-encoding"];
/// Headers the host sets itself on bulk transfers, in addition to
/// [`RESERVED_REQUEST_HEADERS`].
const RESERVED_TRANSFER_HEADERS: [&str; 3] = ["range", "if-match", "idempotency-key"];
const MAX_REQUEST_HEADER_VALUE_BYTES: usize = 8192;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 512;
const MAX_SECRET_HEADER_PREFIX_BYTES: usize = 512;
const MAX_TRANSFER_SECRET_HEADERS: usize = 4;
const MAX_TRANSFER_KEY_BYTES: usize = 128;
/// The largest byte range one transfer fetch may request.
const MAX_TRANSFER_RANGE_BYTES: u32 = 16 * 1024 * 1024;
const MAX_ETAG_BYTES: usize = 256;
const MAX_MEDIA_TYPE_BYTES: usize = 255;
const EPOCH_TICK_INTERVAL: Duration = Duration::from_millis(10);
const NETWORK_RATE_WINDOW: Duration = Duration::from_secs(60);
const NETWORK_RATE_LIMIT: usize = 60;
/// The network request limiter: per process by default, shared by every
/// replica when the composition root installs a Redis-backed limiter.
static NETWORK_RATE_LIMITER: OnceLock<Arc<dyn RateLimiter>> = OnceLock::new();

/// Installs the process-wide network rate limiter. Call before the runtime
/// serves requests; later calls are ignored.
pub fn use_network_rate_limiter(limiter: Arc<dyn RateLimiter>) {
    let _ = NETWORK_RATE_LIMITER.set(limiter);
}

/// Admits one network request of a release's extension.
async fn allow_network_request(release_id: Uuid, extension_id: &str) -> bool {
    NETWORK_RATE_LIMITER
        .get_or_init(|| Arc::new(LocalRateLimiter::default()))
        .allow(
            &format!("network:{release_id}:{extension_id}"),
            NETWORK_RATE_LIMIT as u32,
            NETWORK_RATE_WINDOW,
        )
        .await
}

#[derive(Clone, Debug)]
pub struct ExtensionRuntimeConfig {
    pub fuel: u64,
    pub max_memory_bytes: usize,
    pub invocation_timeout: Duration,
}

impl Default for ExtensionRuntimeConfig {
    fn default() -> Self {
        Self {
            fuel: DEFAULT_FUEL,
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            invocation_timeout: DEFAULT_INVOCATION_TIMEOUT,
        }
    }
}

#[derive(Debug, Error)]
pub enum ExtensionRuntimeError {
    #[error("extension artifact storage failed: {0}")]
    Storage(#[from] ObjectStoreError),
    #[error("extension runtime failed: {0}")]
    Runtime(String),
    #[error("extension host operation denied: {0}")]
    Denied(String),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ComponentCacheKey {
    installed_release_id: Uuid,
    artifact_id: String,
}

#[derive(Default)]
struct ComponentCache {
    components: HashMap<ComponentCacheKey, Arc<Component>>,
    lru: VecDeque<ComponentCacheKey>,
    /// Compilations in progress, so concurrent misses share one compile.
    compiling: HashMap<ComponentCacheKey, Arc<tokio::sync::OnceCell<Arc<Component>>>>,
}

impl ComponentCache {
    fn get(&mut self, key: &ComponentCacheKey) -> Option<Arc<Component>> {
        let component = self.components.get(key)?.clone();
        self.lru.retain(|cached| cached != key);
        self.lru.push_back(key.clone());
        Some(component)
    }

    fn insert(&mut self, key: ComponentCacheKey, component: Arc<Component>) -> Arc<Component> {
        if let Some(component) = self.get(&key) {
            return component;
        }
        self.components.insert(key.clone(), component.clone());
        self.lru.push_back(key);
        if self.components.len() > MAX_CACHED_COMPONENTS
            && let Some(evicted) = self.lru.pop_front()
        {
            self.components.remove(&evicted);
        }
        component
    }
}

/// One linker per host world. Linking host functions depends only on the
/// engine and the world, so it is done once per runtime, not per invocation.
struct Linkers {
    unified: Linker<unified::UnifiedState>,
}

impl Linkers {
    fn new(engine: &Engine) -> Result<Self, ExtensionRuntimeError> {
        Ok(Self {
            unified: unified::linker(engine)?,
        })
    }
}

struct ExtensionEpochTicker {
    shutdown: watch::Sender<()>,
}

impl Drop for ExtensionEpochTicker {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
    }
}

#[derive(Clone)]
pub struct ExtensionRuntime {
    object_store: Arc<dyn ObjectStore>,
    config: ExtensionRuntimeConfig,
    engine: Arc<Engine>,
    _epoch_ticker: Arc<ExtensionEpochTicker>,
    components: Arc<Mutex<ComponentCache>>,
    linkers: Arc<Linkers>,
    /// Pre-instantiated unified components by installed release; linking and
    /// type-checking imports is done once per release.
    unified_instances:
        Arc<Mutex<HashMap<Uuid, wasmtime::component::InstancePre<unified::UnifiedState>>>>,
}

impl ExtensionRuntime {
    pub fn new(
        object_store: Arc<dyn ObjectStore>,
        config: ExtensionRuntimeConfig,
    ) -> Result<Self, ExtensionRuntimeError> {
        let mut wasmtime = Config::new();
        wasmtime.wasm_component_model(true);
        wasmtime.consume_fuel(true);
        wasmtime.epoch_interruption(true);
        let engine = Engine::new(&wasmtime)
            .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let linkers = Arc::new(Linkers::new(&engine)?);
        let engine = Arc::new(engine);
        let (shutdown, mut shutdown_receiver) = watch::channel(());
        let tick_engine = engine.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(EPOCH_TICK_INTERVAL) => tick_engine.increment_epoch(),
                    _ = shutdown_receiver.changed() => return,
                }
            }
        });
        Ok(Self {
            object_store,
            config,
            engine,
            _epoch_ticker: Arc::new(ExtensionEpochTicker { shutdown }),
            components: Arc::new(Mutex::new(ComponentCache::default())),
            linkers,
            unified_instances: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn component(
        &self,
        installation: &ExtensionRuntimeInstallation,
    ) -> Result<Arc<Component>, ExtensionRuntimeError> {
        let artifact = installation
            .manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == ArtifactKind::ServerWasm)
            .ok_or_else(|| {
                ExtensionRuntimeError::Runtime("release has no server_wasm artifact".into())
            })?;
        let key = ComponentCacheKey {
            installed_release_id: installation.installed_release_id,
            artifact_id: artifact.id.clone(),
        };
        self.cached_component(key, || async {
            Ok(self
                .object_store
                .get(&installed_artifact_key(
                    installation.installed_release_id,
                    &artifact.id,
                ))
                .await?
                .bytes)
        })
        .await
    }

    /// Returns the compiled component for `key`, compiling the bytes from
    /// `load` on a miss. Concurrent misses share one compilation.
    async fn cached_component<Load, Loading, Bytes>(
        &self,
        key: ComponentCacheKey,
        load: Load,
    ) -> Result<Arc<Component>, ExtensionRuntimeError>
    where
        Load: FnOnce() -> Loading,
        Loading: std::future::Future<Output = Result<Bytes, ExtensionRuntimeError>>,
        Bytes: AsRef<[u8]> + Send + 'static,
    {
        let compiling = {
            let mut cache = self
                .components
                .lock()
                .expect("component cache is not poisoned");
            if let Some(component) = cache.get(&key) {
                metrics::counter!("attricat_extension_component_cache_total", "outcome" => "hit")
                    .increment(1);
                return Ok(component);
            }
            cache.compiling.entry(key.clone()).or_default().clone()
        };
        let compiled = compiling
            .get_or_try_init(|| async {
                let bytes = load().await?;
                // Compiling takes seconds of CPU; keep it off the async workers
                // so it cannot stall unrelated requests.
                let engine = self.engine.clone();
                let component =
                    tokio::task::spawn_blocking(move || Component::new(&engine, &bytes))
                        .await
                        .map_err(|error| {
                            ExtensionRuntimeError::Runtime(format!(
                                "component compilation failed: {error}"
                            ))
                        })?
                        .map_err(|error| {
                            ExtensionRuntimeError::Runtime(format!("invalid component: {error}"))
                        })?;
                metrics::counter!("attricat_extension_component_cache_total", "outcome" => "miss")
                    .increment(1);
                Ok::<_, ExtensionRuntimeError>(Arc::new(component))
            })
            .await
            .cloned();
        let mut cache = self
            .components
            .lock()
            .expect("component cache is not poisoned");
        // A failed compile leaves the cell empty; dropping it lets a later
        // call retry. Only the caller's own cell is removed.
        if cache
            .compiling
            .get(&key)
            .is_some_and(|cell| Arc::ptr_eq(cell, &compiling))
        {
            cache.compiling.remove(&key);
        }
        Ok(cache.insert(key, compiled?))
    }

    /// Executes one bounded operation batch. The component and its ABI come
    /// from the run's immutable release snapshot, so no current-installation
    /// lookup can substitute upgraded code. The flag tells whether the 1.1
    /// `deferrable-operations` export ran.
    async fn invoke_operation_batch(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        run: &ClaimedExtensionOperationRun,
        cancelling: bool,
    ) -> (bool, Result<BatchResult, BatchError>) {
        self.invoke_unified_batch(installation, repository, run, cancelling)
            .await
    }

    fn operation_state(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        run_id: Uuid,
    ) -> OperationState {
        OperationState::new(
            installation.clone(),
            repository,
            self.object_store.clone(),
            run_id,
        )
    }

    /// Invokes a v1.1 component command after its HTTP broker has resolved a
    /// locked enabled-release snapshot.
    pub async fn invoke_command(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        handler: &str,
        request: &str,
        max_response_bytes: u64,
    ) -> Result<String, ExtensionRuntimeError> {
        self.invoke_unified_command(
            installation,
            repository,
            handler,
            request,
            max_response_bytes,
        )
        .await
    }

    async fn invoke(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
        delivery: &EventDeliveryAttempt,
    ) -> Result<Result<(), InvocationFailure>, ExtensionRuntimeError> {
        self.invoke_unified_event(installation, repository, handler, event, delivery)
            .await
    }
}

/// Quarantines an installation after a component fault. A concurrent
/// invocation may have quarantined it first; that is not an error.
async fn quarantine_after_fault(
    repository: &AttricatRepository,
    extension_id: &str,
) -> Result<(), RepositoryError> {
    match repository
        .quarantine_extension(extension_id, "runtime_failure")
        .await
    {
        Ok(_) | Err(RepositoryError::InvalidExtensionTransition(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

/// What a 1.1 handler learns about the delivery it handles.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EventDeliveryAttempt {
    pub sequence: i64,
    pub record_revision: Option<i64>,
    pub attempt: u32,
}

/// The most attempts of one handler for one event before its retries are
/// exhausted and the delivery is recorded as failed.
const MAX_HANDLER_ATTEMPTS: i32 = 10;

fn operation_task_error(error: impl ToString) -> TaskHandlerError {
    TaskHandlerError {
        code: "operation",
        message: error.to_string(),
    }
}

fn runtime_error(error: impl ToString) -> ExtensionRuntimeError {
    ExtensionRuntimeError::Runtime(error.to_string())
}

fn epoch_deadline(timeout: Duration) -> u64 {
    timeout
        .as_millis()
        .div_ceil(EPOCH_TICK_INTERVAL.as_millis())
        .max(1) as u64
}

const MAX_ARTIFACT_CHUNK_BYTES: usize = 64 * 1024;

pub struct InputArtifactStream {
    artifact: ExtensionOperationArtifact,
    offset: i64,
}

pub struct OutputArtifactStream {
    artifact: ExtensionOperationArtifact,
    path: PathBuf,
    hasher: Sha256,
}

struct OperationState {
    installation: ExtensionRuntimeInstallation,
    repository: AttricatRepository,
    object_store: Arc<dyn ObjectStore>,
    run_id: Uuid,
    batch_key: String,
    artifacts: ResourceTable,
    /// The run's scope, loaded on first use.
    run_facts: Arc<tokio::sync::OnceCell<transfer::RunFacts>>,
}

impl OperationState {
    fn new(
        installation: ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        object_store: Arc<dyn ObjectStore>,
        run_id: Uuid,
    ) -> Self {
        Self {
            installation,
            repository,
            object_store,
            run_id,
            batch_key: String::new(),
            artifacts: ResourceTable::new(),
            run_facts: Arc::default(),
        }
    }

    fn with_batch_key(mut self, batch_key: &str) -> Self {
        self.batch_key = batch_key.to_owned();
        self
    }

    fn require(&self, capability: &str) -> Result<(), String> {
        if self.installation.capability_grants.contains(capability) {
            Ok(())
        } else {
            Err(format!("missing capability '{capability}'"))
        }
    }

    fn artifact_access(
        &self,
        capability: &str,
    ) -> Result<(AttricatRepository, String, Uuid), String> {
        self.require(capability)?;
        Ok((
            self.repository.clone(),
            self.installation.extension_id.clone(),
            self.installation.installed_release_id,
        ))
    }
}

async fn ensure_operation_artifact_access(
    repository: AttricatRepository,
    extension_id: String,
    release_id: Uuid,
    capability: &str,
) -> Result<(), String> {
    let installation = repository
        .runtime_extension_installation(&extension_id, release_id)
        .await
        .map_err(|_| "extension authorization could not be checked")?
        .ok_or_else(|| "extension invocation is no longer authorized".to_owned())?;
    if !installation.capability_grants.contains(capability) {
        return Err(format!("missing capability '{capability}'"));
    }
    Ok(())
}

#[derive(Clone)]
struct HostState {
    installation: ExtensionRuntimeInstallation,
    repository: AttricatRepository,
    limits: StoreLimits,
    /// The invocation that holds the export leases it takes, so its own
    /// rejection of a record settles the revision it leased.
    lease_holder: Option<String>,
    /// The operation run of a batch invocation, whose scope bounds the
    /// records it may reach.
    run_id: Option<Uuid>,
    /// The run's scope, loaded on first use.
    run_facts: Arc<tokio::sync::OnceCell<transfer::RunFacts>>,
}

/// The export lease holder of an operation run.
fn run_lease_holder(run_id: Uuid) -> String {
    format!("run:{run_id}")
}

/// The export lease holder of one handler's delivery of an event.
fn handler_lease_holder(event_id: Uuid, handler_id: &str) -> String {
    format!("handler:{event_id}:{handler_id}")
}

impl HostState {
    fn new(
        installation: ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        max_memory_bytes: usize,
    ) -> Self {
        Self {
            installation,
            repository,
            limits: StoreLimitsBuilder::new()
                .memory_size(max_memory_bytes)
                .build(),
            lease_holder: None,
            run_id: None,
            run_facts: Arc::default(),
        }
    }

    fn require(&self, capability: &str) -> Result<(), ExtensionRuntimeError> {
        if self
            .installation
            .capability_grants
            .iter()
            .any(|grant| grant == capability)
        {
            Ok(())
        } else {
            Err(ExtensionRuntimeError::Denied(format!(
                "missing capability '{capability}'"
            )))
        }
    }
}

impl HostState {
    async fn call(&mut self, operation: String, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let required = match operation.as_str() {
            "configuration.get.v1" => "configuration.read",
            "secrets.get.v1" => "secrets.read",
            "storage.get.v1" | "storage.set.v1" | "storage.put.v1" | "storage.delete.v1"
            | "storage.list.v1" => "storage.extension",
            "attricat.read.v1" => "attricat.read",
            // Annotation-only batches need only the separately granted
            // annotation capability; any value intent still needs attricat.write.
            "attricat.command.v1" => {
                match parse_storage_request::<AttricatCommandRequest>(&request) {
                    Ok(AttricatCommandRequest::Batch { batch })
                        if !batch.intents.is_empty()
                            && batch
                                .intents
                                .iter()
                                .all(ExtensionAttricatIntent::is_annotation) =>
                    {
                        "attricat.annotations.write"
                    }
                    _ => "attricat.write",
                }
            }
            "events.emit.v1" => "events.emit",
            "network.request.v1" | "network.request.v2" => "network.request",
            "mappings.get.v1"
            | "mappings.list.v1"
            | "mappings.begin.v1"
            | "mappings.commit.v1"
            | "mappings.release.v1"
            | "mappings.delete.v1" => "exports.mappings",
            "connections.get.v1" | "connections.list.v1" => "connections.use",
            "files.link.v1" => "attricat.files.public_links",
            _ => return Err("unknown host operation".into()),
        };
        // The invocation was selected from a snapshot, but grants, configuration,
        // containment, lifecycle state, or the installed release can change while
        // untrusted component code is running. Refresh the entire snapshot at
        // every host call; merely checking that one still exists would retain
        // revoked grants in `self.installation`.
        self.refresh_installation().await?;
        if let Err(error) = self.require(required) {
            metrics::counter!("attricat_extension_host_calls_total", "outcome" => "denied", "operation" => operation).increment(1);
            return Err(error.to_string());
        }
        if operation == "configuration.get.v1" {
            return Ok(self.installation.configuration.to_string());
        }
        if operation == "events.emit.v1" {
            let input: EventEmit = parse_event_emit_request(&request)?;
            // The snapshot was refreshed at the start of this host call.
            self.repository
                .emit_extension_event_with(
                    &self.installation,
                    &input.contract_id,
                    &input.aggregate_kind,
                    parse_uuid(&input.aggregate_id, "event aggregate ID")?,
                    input.payload,
                )
                .await
                .map_err(|error| error.to_string())?;
            return Ok("null".to_owned());
        }
        if operation == "attricat.read.v1" {
            return self.attricat_read_call(&request).await;
        }
        if operation == "attricat.command.v1" {
            return self.attricat_command_call(&request).await;
        }
        if operation == "secrets.get.v1" {
            let input: SecretGet = parse_host_request(&request, "secret request")?;
            valid_secret_name(&input.name)?;
            let value = self
                .repository
                .workspace_extension_secret_value(&input.name)
                .await
                .map_err(|_| "secret lookup failed")?
                .ok_or_else(|| "secret reference is unavailable".to_owned())?;
            // A secret is returned only to the invoking component; it is never
            // logged, traced, persisted, or included in a management response.
            return Ok(json!({"value": value}).to_string());
        }
        if operation == "network.request.v1" {
            return self.network_request(&request).await;
        }
        if operation == "network.request.v2" {
            return self.network_request_v2(&request).await;
        }
        if operation.starts_with("mappings.") {
            return self.mappings_call(&operation, &request).await;
        }
        if operation.starts_with("connections.") {
            return self.connections_call(&operation, &request).await;
        }
        if operation == "files.link.v1" {
            return self.file_link_call(&request).await;
        }
        self.storage_call(&operation, &request).await
    }

    async fn log(&mut self, level: String, message: String) -> Result<(), String> {
        self.require_active("logging.write").await?;
        if message.len() > MAX_HOST_MESSAGE_BYTES
            || !matches!(
                level.as_str(),
                "trace" | "debug" | "info" | "warn" | "error"
            )
        {
            return Err("invalid bounded log entry".into());
        }
        tracing::info!(extension = %self.installation.extension_id, %level, message = %message, "extension host log");
        Ok(())
    }
}

impl HostState {
    async fn read(
        &mut self,
        request: wit::api::ReadRequest,
    ) -> Result<wit::api::ReadResponse, String> {
        self.require_active("attricat.read").await?;
        let (record_id, context_id) = match request {
            wit::api::ReadRequest::Record(input) | wit::api::ReadRequest::Values(input) => {
                (parse_uuid(&input.record_id, "record ID")?, None)
            }
            wit::api::ReadRequest::Resolved(input) => (
                parse_uuid(&input.record_id, "record ID")?,
                Some(parse_uuid(&input.context_id, "context ID")?),
            ),
        };
        let record = self
            .repository
            .get_record_with_revision(record_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "record not found".to_owned())?;
        let blueprint = self
            .repository
            .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "blueprint revision not found".to_owned())?;
        let direct_values = self
            .repository
            .current_values(record_id)
            .await
            .map_err(|error| error.to_string())?;
        let resolved_values = match context_id {
            Some(context_id) => Some(
                self.repository
                    .resolved_preview(record_id, context_id, 0)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "record not found".to_owned())?,
            ),
            None => None,
        };
        Ok(wit::api::ReadResponse {
            record: bounded_serialize(&record)?,
            blueprint: bounded_serialize(&blueprint)?,
            direct_values: bounded_serialize(&direct_values)?,
            resolved_values: resolved_values
                .as_ref()
                .map(bounded_serialize)
                .transpose()?,
        })
    }

    async fn write(
        &mut self,
        request: wit::api::WriteRequest,
    ) -> Result<wit::api::WriteResponse, String> {
        self.require_active("attricat.write").await?;
        if request.values.is_empty() || request.values.len() > MAX_WRITE_VALUES {
            return Err(format!("writes require 1-{MAX_WRITE_VALUES} scalar values"));
        }
        let record_id = parse_uuid(&request.record_id, "record ID")?;
        let values = request
            .values
            .into_iter()
            .map(|value| {
                let attribute_id = value
                    .attribute_id
                    .as_deref()
                    .map(|id| parse_uuid(id, "attribute ID"))
                    .transpose()?;
                if (attribute_id.is_some()) == (value.attribute_code.is_some()) {
                    return Err("each write requires exactly one attribute selector".into());
                }
                Ok(NewAttributeValue::Scalar {
                    attribute_id,
                    attribute_code: value.attribute_code,
                    context_id: Some(parse_uuid(&value.context_id, "context ID")?),
                    value: parse_bounded_json(&value.value, "attribute value")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let repository = self
            .repository
            .for_extension(&self.installation.extension_id);
        let values = AttricatMutationService::new(&repository)
            .append_values(
                record_id,
                AppendAttributeValues {
                    values,
                    expected_updated_at: None,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(wit::api::WriteResponse {
            values: bounded_serialize(&values)?,
        })
    }

    async fn scoped_configuration_get(
        &mut self,
        scope: wit::api::ConfigurationScope,
    ) -> Result<Option<String>, String> {
        self.require_active("configuration.write").await?;
        let value = self
            .repository
            .extension_scoped_configuration_get(
                &self.installation.extension_id,
                self.installation.installed_release_id,
                &to_configuration_scope(scope)?,
            )
            .await
            .map_err(|error| error.to_string())?;
        value.as_ref().map(bounded_serialize).transpose()
    }

    async fn scoped_configuration_set(
        &mut self,
        request: wit::api::ScopedConfigurationUpdate,
    ) -> Result<(), String> {
        self.require_active("configuration.write").await?;
        let value = parse_bounded_json(&request.value, "scoped configuration value")?;
        self.repository
            .extension_scoped_configuration_set(
                &self.installation.extension_id,
                self.installation.installed_release_id,
                &to_configuration_scope(request.scope)?,
                value,
            )
            .await
            .map_err(|error| error.to_string())
    }
}

impl HostState {
    /// Replaces the installation snapshot with the current authorized one.
    async fn refresh_installation(&mut self) -> Result<(), String> {
        let installation = self
            .repository
            .runtime_extension_installation(
                &self.installation.extension_id,
                self.installation.installed_release_id,
            )
            .await
            .map_err(|_| "extension authorization could not be checked")?
            .ok_or_else(|| "extension invocation is no longer authorized".to_owned())?;
        self.installation = installation;
        Ok(())
    }

    /// [`Self::require_active`] for several capabilities with one refresh.
    async fn require_all_active<'a>(
        &mut self,
        capabilities: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), String> {
        let capabilities: Vec<&str> = capabilities.into_iter().collect();
        if capabilities.is_empty() {
            return Ok(());
        }
        self.refresh_installation().await?;
        for capability in capabilities {
            self.require(capability)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    async fn require_active(&mut self, capability: &str) -> Result<(), String> {
        self.refresh_installation().await?;
        self.require(capability).map_err(|error| error.to_string())
    }

    /// The v1 JSON host call is retained for older components. Unlike generic
    /// search it exposes only the two bounded extension-data operations below.
    async fn attricat_read_call(&self, request: &str) -> Result<String, String> {
        let input: AttricatReadRequest = parse_storage_request(request)?;
        match input {
            AttricatReadRequest::Page {
                blueprint_id,
                blueprint_version,
                context_id,
                publication_context_id,
                cursor,
                limit,
            } => {
                let page = self
                    .repository
                    .extension_attricat_page(ExtensionAttricatPageRequest {
                        blueprint_id: parse_uuid(&blueprint_id, "blueprint ID")?,
                        blueprint_version,
                        context_id: context_id
                            .as_deref()
                            .map(|value| parse_uuid(value, "context ID"))
                            .transpose()?,
                        publication_context_id: publication_context_id
                            .as_deref()
                            .map(|value| parse_uuid(value, "publication context ID"))
                            .transpose()?,
                        cursor,
                        limit,
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&page)
            }
            AttricatReadRequest::Changes {
                blueprint_id,
                blueprint_version,
                cursor,
                limit,
            } => {
                let page = self
                    .repository
                    .extension_attricat_changes(
                        parse_uuid(&blueprint_id, "blueprint ID")?,
                        blueprint_version,
                        cursor,
                        limit,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&page)
            }
            AttricatReadRequest::Lookup {
                blueprint_id,
                blueprint_version,
                attribute_id,
                value,
            } => {
                let record = self
                    .repository
                    .extension_attricat_lookup(
                        parse_uuid(&blueprint_id, "blueprint ID")?,
                        blueprint_version,
                        parse_uuid(&attribute_id, "attribute ID")?,
                        &value,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&record)
            }
        }
    }

    async fn attricat_command_call(&self, request: &str) -> Result<String, String> {
        let input: AttricatCommandRequest = parse_storage_request(request)?;
        let AttricatCommandRequest::Batch { batch } = input;
        for capability in batch_capabilities(&batch) {
            self.require(capability)
                .map_err(|error| error.to_string())?;
        }
        let repository = self
            .repository
            .for_extension(&self.installation.extension_id);
        let outcomes = AttricatMutationService::new(&repository)
            .execute_extension_attricat_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
    }

    /// Executes a single brokered HTTPS request. There are intentionally no
    /// redirects or ambient sockets: DNS is resolved and checked before the
    /// rustls client is pinned to those public addresses.
    async fn network_request(&self, request: &str) -> Result<String, String> {
        let input: NetworkRequest = parse_host_request(request, "network request")?;
        let method =
            Method::from_bytes(input.method.as_bytes()).map_err(|_| "invalid HTTP method")?;
        let url = Url::parse(&input.url).map_err(|_| "invalid HTTPS URL")?;
        if url.scheme() != "https"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.username() != ""
            || url.password().is_some()
        {
            return Err("only query-free HTTPS destinations are allowed".into());
        }
        let rule = self
            .installation
            .manifest
            .host_permissions
            .iter()
            .chain(self.installation.manifest.optional_host_permissions.iter())
            .find(|rule| {
                rule.id == input.host_permission_id
                    && self.installation.host_permission_grants.contains(&rule.id)
                    && rule.allows_request(&url, method.as_str())
            })
            .ok_or_else(|| "destination is not granted by a host permission".to_owned())?;
        let body = input.body.unwrap_or_default();
        if body.len() as u64 > rule.max_request_bytes {
            return Err("request body exceeds host permission limit".into());
        }
        let host = url
            .host_str()
            .ok_or_else(|| "HTTPS URL needs a host".to_owned())?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "HTTPS URL needs a port".to_owned())?;
        // Admit the call before DNS so failed and slow lookups also consume the
        // extension's network budget. A reqwest timeout does not cover this lookup.
        if network_v2::admit_request(&self.repository, &self.installation, rule)
            .await
            .is_err()
        {
            return Err("network request rate limit exceeded".into());
        }
        let timeout = Duration::from_millis(rule.timeout_ms);
        let addresses = resolve_public_destination(host, port, timeout).await?;
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in input.headers.unwrap_or_default() {
            let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "invalid request header")?;
            if RESERVED_REQUEST_HEADERS.contains(&header_name.as_str())
                || value.len() > MAX_REQUEST_HEADER_VALUE_BYTES
            {
                return Err("unsafe request header".into());
            }
            headers.insert(
                header_name,
                reqwest::header::HeaderValue::from_str(&value)
                    .map_err(|_| "invalid request header")?,
            );
        }
        if let Some(key) = &input.idempotency_key {
            if key.is_empty()
                || key.len() > MAX_IDEMPOTENCY_KEY_BYTES
                || headers.contains_key("idempotency-key")
            {
                return Err("invalid idempotency key".into());
            }
            headers.insert(
                "idempotency-key",
                reqwest::header::HeaderValue::from_str(key)
                    .map_err(|_| "invalid idempotency key")?,
            );
        }
        for item in input.secret_headers.unwrap_or_default() {
            valid_secret_name(&item.secret)?;
            let name = reqwest::header::HeaderName::from_bytes(item.header.as_bytes())
                .map_err(|_| "invalid secret header")?;
            if RESERVED_REQUEST_HEADERS.contains(&name.as_str())
                || item.prefix.len() > MAX_SECRET_HEADER_PREFIX_BYTES
            {
                return Err("unsafe secret header".into());
            }
            let secret = self
                .repository
                .workspace_extension_secret_value(&item.secret)
                .await
                .map_err(|_| "secret lookup failed")?
                .ok_or_else(|| "secret reference is unavailable".to_owned())?;
            headers.insert(
                name,
                reqwest::header::HeaderValue::from_str(&(item.prefix + &secret))
                    .map_err(|_| "secret cannot be used as a header")?,
            );
        }
        let client = pinned_client(host, &addresses, timeout)?;
        let started = Instant::now();
        let response = client
            .request(method, url)
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|_| {
                // No automatic retries: a timeout or transport failure may have
                // reached the peer. Callers retry only with their own stable
                // idempotency key understood by the destination.
                "network request has an uncertain external outcome"
            })?;
        if response.status().is_redirection() {
            return Err("redirect responses are denied".into());
        }
        let status = response.status().as_u16();
        let response_headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                matches!(name.as_str(), "content-type" | "etag" | "retry-after")
                    .then(|| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.to_string(), value.to_string()))
                    })
                    .flatten()
            })
            .collect::<HashMap<_, _>>();
        if response
            .content_length()
            .is_some_and(|length| length > rule.max_response_bytes)
        {
            return Err("response exceeds host permission limit".into());
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "network response failed")?;
            if bytes.len().saturating_add(chunk.len()) > rule.max_response_bytes as usize {
                return Err("response exceeds host permission limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        metrics::histogram!("attricat_extension_network_duration_seconds", "extension" => self.installation.extension_id.clone(), "host_permission" => rule.id.clone()).record(started.elapsed());
        Ok(
            json!({"status":status,"headers":response_headers,"body_base64":BASE64.encode(bytes)})
                .to_string(),
        )
    }

    async fn storage_call(&self, operation: &str, request: &str) -> Result<String, String> {
        let release_id = self.installation.installed_release_id;
        let extension_id = &self.installation.extension_id;
        match operation {
            "storage.get.v1" => {
                let input: StorageGet = parse_storage_request(request)?;
                let result = self
                    .repository
                    .extension_storage_get(extension_id, release_id, &input.key)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(serde_json::to_string(
                    &result.map(|entry| json!({"value": entry.value, "revision": entry.revision})),
                )
                .expect("storage result serializes"))
            }
            "storage.set.v1" | "storage.put.v1" => {
                let input: StorageSet = parse_storage_request(request)?;
                let revision = self
                    .repository
                    .extension_storage_set(
                        extension_id,
                        release_id,
                        &input.key,
                        input.value,
                        input.expected_revision,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(json!({"revision": revision}).to_string())
            }
            "storage.delete.v1" => {
                let input: StorageDelete = parse_storage_request(request)?;
                self.repository
                    .extension_storage_delete(
                        extension_id,
                        release_id,
                        &input.key,
                        input.expected_revision,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                Ok("null".to_owned())
            }
            "storage.list.v1" => {
                let input: StorageList = parse_storage_request(request)?;
                let page = self
                    .repository
                    .extension_storage_list(
                        extension_id,
                        release_id,
                        input.prefix.as_deref(),
                        input.cursor.as_deref(),
                        input.limit.unwrap_or(DEFAULT_LIST_PAGE_SIZE),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(json!({"entries": page.entries.into_iter().map(|entry| json!({"key": entry.key, "value": entry.value, "revision": entry.revision})).collect::<Vec<_>>(), "cursor": page.cursor}).to_string())
            }
            _ => Err("host operation is not enabled by this deployment".into()),
        }
    }
}

/// Capabilities required by a catalog batch: annotation intents are governed
/// by `attricat.annotations.write`, value intents by `attricat.write`.
fn batch_capabilities(batch: &ExtensionAttricatBatch) -> Vec<&'static str> {
    let mut capabilities = Vec::new();
    if batch
        .intents
        .iter()
        .any(ExtensionAttricatIntent::is_annotation)
    {
        capabilities.push("attricat.annotations.write");
    }
    if batch.intents.iter().any(|intent| !intent.is_annotation()) {
        capabilities.push("attricat.write");
    }
    capabilities
}

fn bounded_serialize(value: &impl serde::Serialize) -> Result<String, String> {
    let serialized =
        serde_json::to_string(value).map_err(|_| "response serialization failed".to_owned())?;
    if serialized.len() > MAX_HOST_JSON_BYTES {
        Err("response exceeds host JSON limit".into())
    } else {
        Ok(serialized)
    }
}

fn parse_bounded_json(value: &str, label: &str) -> Result<Value, String> {
    if value.len() > MAX_HOST_JSON_BYTES {
        return Err(format!("{label} exceeds host JSON limit"));
    }
    serde_json::from_str(value).map_err(|_| format!("invalid {label}"))
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid, String> {
    if value.len() > MAX_EXTENSION_IDENTIFIER_BYTES {
        return Err(format!("invalid {label}"));
    }
    value.parse().map_err(|_| format!("invalid {label}"))
}

fn to_configuration_scope(
    scope: wit::api::ConfigurationScope,
) -> Result<ExtensionConfigurationScope, String> {
    let kind = match scope.kind {
        wit::api::ConfigurationScopeKind::Blueprint => {
            crate::extensions::ConfigurationScope::Blueprint
        }
        wit::api::ConfigurationScopeKind::Attribute => {
            crate::extensions::ConfigurationScope::Attribute
        }
    };
    Ok(ExtensionConfigurationScope {
        kind,
        blueprint_id: parse_uuid(&scope.blueprint_id, "blueprint ID")?,
        blueprint_version: scope.blueprint_version,
        attribute_id: scope
            .attribute_id
            .as_deref()
            .map(|id| parse_uuid(id, "attribute ID"))
            .transpose()?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventEmit {
    contract_id: String,
    aggregate_kind: String,
    aggregate_id: String,
    payload: Value,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum AttricatReadRequest {
    Page {
        blueprint_id: String,
        blueprint_version: i64,
        context_id: Option<String>,
        publication_context_id: Option<String>,
        cursor: Option<String>,
        limit: u32,
    },
    Changes {
        blueprint_id: String,
        blueprint_version: i64,
        cursor: Option<String>,
        limit: u32,
    },
    Lookup {
        blueprint_id: String,
        blueprint_version: i64,
        attribute_id: String,
        value: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum AttricatCommandRequest {
    Batch {
        batch: crate::repository::ExtensionAttricatBatch,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageGet {
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageSet {
    key: String,
    value: Value,
    expected_revision: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageDelete {
    key: String,
    expected_revision: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageList {
    prefix: Option<String>,
    cursor: Option<String>,
    limit: Option<u32>,
}
fn parse_event_emit_request(request: &str) -> Result<EventEmit, String> {
    let input: EventEmit =
        serde_json::from_str(request).map_err(|_| "invalid event emission request".to_owned())?;
    if input.contract_id.is_empty()
        || input.contract_id.len() > MAX_EXTENSION_IDENTIFIER_BYTES
        || input.aggregate_kind.is_empty()
        || input.aggregate_kind.len() > MAX_EXTENSION_IDENTIFIER_BYTES
        || !input.payload.is_object()
    {
        return Err("invalid event emission request".into());
    }
    Ok(input)
}

fn parse_storage_request<T: for<'de> Deserialize<'de>>(request: &str) -> Result<T, String> {
    serde_json::from_str(request).map_err(|_| "invalid storage request".to_owned())
}
fn parse_host_request<T: for<'de> Deserialize<'de>>(
    request: &str,
    label: &str,
) -> Result<T, String> {
    if request.len() > MAX_HOST_JSON_BYTES {
        return Err(format!("{label} exceeds host JSON limit"));
    }
    serde_json::from_str(request).map_err(|_| format!("invalid {label}"))
}
fn valid_secret_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > MAX_EXTENSION_IDENTIFIER_BYTES
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        Err("invalid secret reference".into())
    } else {
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretGet {
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretHeader {
    secret: String,
    header: String,
    #[serde(default)]
    prefix: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkRequest {
    host_permission_id: String,
    method: String,
    url: String,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    secret_headers: Option<Vec<SecretHeader>>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// Whether an extension keeps export mappings, so a rejection it reports for
/// a record is that record's export status. Other extensions' rejections stay
/// handler outcomes only.
fn exports_mappings(installation: &ExtensionRuntimeInstallation) -> bool {
    installation.capability_grants.contains("exports.mappings")
}

/// Shared-task executor for release-pinned checkpointed operations. The
/// checkpoint is committed under the task lease only after WASM returns; a
/// crash before that transaction repeats the batch, so operation components
/// must make each batch idempotent using the durable checkpoint.
pub struct ExtensionOperationTaskHandler {
    repository: SystemRepository,
    runtime: ExtensionRuntime,
}
impl ExtensionOperationTaskHandler {
    pub fn new(repository: impl Into<SystemRepository>, runtime: ExtensionRuntime) -> Self {
        Self {
            repository: repository.into(),
            runtime,
        }
    }

    async fn fail_for_revoked_initiator(
        &self,
        repository: &AttricatRepository,
        task: &ClaimedTask,
    ) -> Result<TaskOutcome, TaskHandlerError> {
        repository
            .fail_extension_operation_for_revoked_initiator(task)
            .await
            .map_err(operation_task_error)?;
        counter!("attricat_extension_operations_total", "outcome" => "initiator_revoked")
            .increment(1);
        Ok(TaskOutcome::DeadLettered)
    }
}
#[async_trait]
impl TaskHandler for ExtensionOperationTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::ExtensionOperationRunV1
    }
    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let repository = self.repository.for_workspace(task.workspace_id);
        let repository = repository.for_extension_operation_task(&task);
        // Abandoned streams are aborted by the workspace coordinator's
        // periodic sweep; a run does not repeat that workspace-wide sweep.
        let Some(run) = repository
            .begin_extension_operation_task(&task)
            .await
            .map_err(operation_task_error)?
        else {
            return Ok(TaskOutcome::Complete);
        };
        counter!("attricat_extension_operations_total", "outcome" => "started").increment(1);
        let operation_started = Instant::now();
        let Some(installation) = repository
            .runtime_extension_installation(&run.extension_id, run.installed_release_id)
            .await
            .map_err(operation_task_error)?
        else {
            // Disable, upgrade, grant change, and quarantine never switch an
            // in-flight run to a different release. It remains resumable only
            // when its exact pinned release becomes authorized again.
            repository
                .pause_extension_operation_task(&task)
                .await
                .map_err(operation_task_error)?;
            return Ok(TaskOutcome::Reschedule {
                at: chrono::Utc::now() + PAUSED_OPERATION_RECHECK_DELAY,
            });
        };
        // Read under the run's row lock when the batch began.
        let cancellation_requested = run.cancelling;
        // An interactive run acts only while its initiator remains an active
        // member; it never continues under the installer's grants alone. A
        // cancellation the initiator requested is still delivered so the
        // extension can clean up; its host calls recheck access and fail.
        let initiator_revoked = match repository
            .interactive_run_scope(run.id)
            .await
            .map_err(operation_task_error)?
        {
            Some(scope) => !repository
                .interactive_actor_active(scope.actor)
                .await
                .map_err(operation_task_error)?,
            None => false,
        };
        if initiator_revoked && !cancellation_requested {
            return self.fail_for_revoked_initiator(&repository, &task).await;
        }
        let (deferrable, result) = self
            .runtime
            .invoke_operation_batch(
                &installation,
                repository.clone(),
                &run,
                cancellation_requested,
            )
            .await;
        let batch = match result {
            Ok(batch) => batch,
            Err(_) if initiator_revoked => {
                return self.fail_for_revoked_initiator(&repository, &task).await;
            }
            Err(error) => {
                return self
                    .handle_batch_failure(
                        &repository,
                        &task,
                        &run,
                        deferrable,
                        error,
                        operation_started,
                    )
                    .await;
            }
        };
        // Cancellation may arrive while a batch runs. The database never
        // terminalizes a requested cancellation until this callback succeeds
        // and this task lease records delivery. If it races the checkpoint,
        // the run stays pending and a later lease invokes `cancel`.
        let cancellation_delivered = if cancellation_requested {
            true
        } else {
            repository
                .extension_operation_cancellation_requested(&task)
                .await
                .map_err(operation_task_error)?
        };
        if cancellation_delivered {
            if !cancellation_requested {
                self.runtime
                    .invoke_operation_batch(&installation, repository.clone(), &run, true)
                    .await
                    .1
                    .map_err(operation_task_error)?;
            }
            repository
                .mark_extension_operation_cancellation_delivered(&task)
                .await
                .map_err(operation_task_error)?;
        }
        // A deferred batch resumes later; cancellation resumes at once.
        let resume_at = batch
            .resume_after
            .filter(|_| !cancellation_delivered)
            .and_then(|delay| chrono::TimeDelta::from_std(delay).ok())
            .map(|delay| chrono::Utc::now() + delay);
        let terminal = repository
            .checkpoint_extension_operation_task_until(
                &task,
                &run,
                batch.checkpoint,
                batch.progress,
                batch.done,
                resume_at,
            )
            .await
            .map_err(operation_task_error)?;
        let outcome = if terminal {
            "completed"
        } else if resume_at.is_some() {
            "deferred"
        } else {
            "rescheduled"
        };
        counter!("attricat_extension_operations_total", "outcome" => outcome).increment(1);
        histogram!("attricat_extension_operation_duration_seconds", "outcome" => outcome)
            .record(operation_started.elapsed().as_secs_f64());
        if terminal {
            Ok(TaskOutcome::Complete)
        } else {
            Ok(TaskOutcome::Reschedule {
                at: resume_at.unwrap_or_else(chrono::Utc::now),
            })
        }
    }
}

impl ExtensionOperationTaskHandler {
    /// Applies a failed batch: a typed retry re-runs the same batch after a
    /// backoff, a rejection dead-letters the run at once, and anything else
    /// fails it through the envelope's failure budget. Only a 1.1 component's
    /// fault (a trap, invalid output or a typed `fault`) also quarantines its
    /// installation; an error a lifecycle export returns only fails the run.
    async fn handle_batch_failure(
        &self,
        repository: &AttricatRepository,
        task: &ClaimedTask,
        run: &ClaimedExtensionOperationRun,
        deferrable: bool,
        error: BatchError,
        operation_started: Instant,
    ) -> Result<TaskOutcome, TaskHandlerError> {
        // A run whose release lost its authorization mid-batch (it was
        // disabled, quarantined or lost a grant) pauses like one that finds
        // it unauthorized before the batch, whatever the batch reported.
        let Some(installation) = repository
            .runtime_extension_installation(&run.extension_id, run.installed_release_id)
            .await
            .map_err(operation_task_error)?
        else {
            repository
                .pause_extension_operation_task(task)
                .await
                .map_err(operation_task_error)?;
            return Ok(TaskOutcome::Reschedule {
                at: chrono::Utc::now() + PAUSED_OPERATION_RECHECK_DELAY,
            });
        };
        let outcome = match error {
            // A retry with an explicit delay is the batch asking to wait,
            // like a deferral, so it does not spend the failure budget until
            // the run has waited `MAX_RETRY_DEFERRALS` times in a row.
            BatchError::Failure(InvocationFailure::Retry { message, after }) => {
                let delay = after.unwrap_or_else(|| retry_backoff(task.failures.max(0) as u32 + 1));
                let at = chrono::Utc::now()
                    + chrono::TimeDelta::from_std(delay).unwrap_or(chrono::TimeDelta::zero());
                let deferred = after.is_some()
                    && repository
                        .defer_extension_operation_task(
                            task,
                            &message,
                            at,
                            Some(MAX_RETRY_DEFERRALS),
                        )
                        .await
                        .map_err(operation_task_error)?;
                if deferred {
                    "deferred"
                } else {
                    let terminal = repository
                        .retry_extension_operation_task(task, &message, at)
                        .await
                        .map_err(operation_task_error)?;
                    if terminal { "failed" } else { "retrying" }
                }
            }
            BatchError::Failure(InvocationFailure::Rejected(rejection)) => {
                let exports = exports_mappings(&installation);
                let mut recorded = None;
                if let Some(record_id) = rejection.record_id
                    && exports
                {
                    // A run's rejection names no revision. When the run holds
                    // the record's lease, the revision it leased is settled;
                    // otherwise it is chosen below so that a newer result
                    // committed meanwhile is not overwritten.
                    let details = repository
                        .connector_run_details(run.id)
                        .await
                        .map_err(operation_task_error)?;
                    let channel = details.as_ref().and_then(|details| details.channel_id);
                    let revision = repository
                        .record_content_revision_at(record_id, run.created_at, channel)
                        .await
                        .map_err(operation_task_error)?;
                    // A connector run's rejection belongs to the run's
                    // connection unless it names another; any other run's
                    // without one to the default destination.
                    // A connector run writes only its own connection's
                    // mappings, so a rejection naming another is refused.
                    let own_connection = details.map(|details| details.connection);
                    let foreign = matches!(
                        (&own_connection, &rejection.connection),
                        (Some(own), Some(named)) if own.as_deref() != Some(named.as_str())
                    );
                    let connection = match rejection.connection {
                        Some(connection) => Some(connection),
                        None => own_connection.flatten(),
                    };
                    // A run settles only records it may map, like its
                    // mappings calls.
                    recorded = Some(RejectionRecord::Refused);
                    if !foreign
                        && transfer::ensure_run_reaches_record(
                            repository,
                            run.id,
                            record_id,
                            transfer::RunReach::Map,
                        )
                        .await
                        .is_ok()
                    {
                        let mapping = repository
                            .export_mapping_get(
                                &run.extension_id,
                                connection.as_deref(),
                                crate::repository::MappingKey::Record(record_id),
                            )
                            .await
                            .ok()
                            .flatten();
                        // The revision at the run's start is the oldest the
                        // batch can have read, so an edit made since is never
                        // taken for rejected and stays to be pushed. A result
                        // committed before the run started is one the run
                        // saw, even when its revision counts more than the
                        // record's own (an expanded related record), so the
                        // rejection settles past it.
                        let committed_before = mapping
                            .as_ref()
                            .filter(|mapping| {
                                mapping.settled_at.is_some_and(|at| at <= run.created_at)
                            })
                            .map_or(0, |mapping| mapping.record_revision);
                        let revision = revision.max(committed_before);
                        recorded = Some(
                            repository
                                .record_export_rejection(
                                    &run.extension_id,
                                    connection.as_deref(),
                                    record_id,
                                    Some(revision),
                                    Some(&run_lease_holder(run.id)),
                                    &rejection.code,
                                    &rejection.message,
                                )
                                .await
                                .map_err(operation_task_error)?,
                        );
                    }
                }
                let message = format!("{}: {}", rejection.code, rejection.message);
                // A rejection of one record settles that record, so the
                // batch replays and the component moves past it (its mapping
                // now reads as settled); the rest of the run continues. A
                // newly settled record is progress and is not counted; a
                // repeat or a record leased elsewhere waits like a deferral,
                // capped, so a component that keeps rejecting the same batch
                // is still dead-lettered. One that cannot be recorded would
                // only be rejected again, so it dead-letters the run.
                // A record another worker holds is replayed once its lease
                // ends, so a long push is waited for rather than counted down.
                let replay_at = chrono::Utc::now() + RECORD_REJECTION_REPLAY_DELAY;
                let deferral = match recorded {
                    Some(RejectionRecord::Settled) => Some((None, replay_at)),
                    Some(RejectionRecord::Unchanged) => {
                        Some((Some(MAX_RETRY_DEFERRALS), replay_at))
                    }
                    Some(RejectionRecord::Busy { until }) => {
                        Some((Some(MAX_RETRY_DEFERRALS), until.max(replay_at)))
                    }
                    Some(RejectionRecord::Refused) | None => None,
                };
                let settled_record = match deferral {
                    Some((limit, at)) => repository
                        .defer_extension_operation_task(task, &message, at, limit)
                        .await
                        .map_err(operation_task_error)?,
                    None => false,
                };
                if settled_record {
                    "record_rejected"
                } else {
                    repository
                        .reject_extension_operation_task(task, &message)
                        .await
                        .map_err(operation_task_error)?;
                    "rejected"
                }
            }
            error => {
                if deferrable && error.is_fault() {
                    tracing::warn!(
                        extension = %run.extension_id,
                        installed_release_id = %run.installed_release_id,
                        error = %error,
                        "extension operation faulted; quarantining extension"
                    );
                    quarantine_after_fault(repository, &run.extension_id)
                        .await
                        .map_err(operation_task_error)?;
                }
                let terminal = repository
                    .fail_extension_operation_task(task, &error.to_string())
                    .await
                    .map_err(operation_task_error)?;
                if terminal { "failed" } else { "retrying" }
            }
        };
        // A run that ends without finishing its batch leaves the records it
        // leased showing as retrying, as a handler that gives up does.
        if matches!(outcome, "failed" | "rejected") && exports_mappings(&installation) {
            repository
                .fail_export_leases(
                    &run.extension_id,
                    &run_lease_holder(run.id),
                    if outcome == "failed" {
                        "run_failed"
                    } else {
                        "run_rejected"
                    },
                    "the run ended before it settled this record",
                )
                .await
                .map_err(operation_task_error)?;
        }
        counter!("attricat_extension_operations_total", "outcome" => outcome).increment(1);
        histogram!("attricat_extension_operation_duration_seconds", "outcome" => outcome)
            .record(operation_started.elapsed().as_secs_f64());
        // Only a failed or rejected run is dead-lettered; one that waits
        // resumes, so the task metric must not count it as a dead letter.
        Ok(match outcome {
            "deferred" | "record_rejected" => TaskOutcome::Settled {
                outcome: "rescheduled",
            },
            "retrying" => TaskOutcome::Settled { outcome: "retry" },
            _ => TaskOutcome::DeadLettered,
        })
    }
}

/// Shared-task executor for enabled extension event handlers. A component can
/// finish before its task acknowledgement, so extensions remain at-least-once
/// and must stay idempotent.
pub struct WasmExtensionTaskHandler {
    repository: SystemRepository,
    runtime: ExtensionRuntime,
}

impl WasmExtensionTaskHandler {
    pub fn new(repository: impl Into<SystemRepository>, runtime: ExtensionRuntime) -> Self {
        Self {
            repository: repository.into(),
            runtime,
        }
    }

    /// Delivers `event` to every subscribed handler that has not settled it
    /// yet. Returns when the task should run again for handlers that are
    /// retrying or debouncing, or `None` once every handler has settled.
    async fn invoke_event(
        &self,
        event: &DomainEvent,
        repository: AttricatRepository,
    ) -> Result<EventDispatch, Box<dyn std::error::Error + Send + Sync>> {
        let mut dispatch = EventDispatch::default();
        let mut completed: Vec<(String, String, Uuid)> = Vec::new();
        let installations = repository.enabled_extension_handlers(event).await?;
        dispatch.handled = !installations.is_empty();
        let mut attempts = if installations.is_empty() {
            Default::default()
        } else {
            repository.extension_handler_attempts(event.id).await?
        };
        let delivered: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
            for candidate in installations {
                let handler_ids: Vec<String> = candidate
                    .manifest
                    .server
                    .as_ref()
                    .into_iter()
                    .flat_map(|server| server.event_handlers.iter())
                    .filter(|handler| {
                        handler
                            .event_types
                            .iter()
                            .any(|event_type| event_type == &event.event_type)
                    })
                    .map(|handler| handler.id.clone())
                    .collect();
                for handler_id in handler_ids {
                    // The list above is only a dispatch hint. Re-read enabled state,
                    // exact installed release, configuration, and required grants
                    // immediately before every component invocation.
                    let Some(installation) = repository
                        .runtime_extension_installation(
                            &candidate.extension_id,
                            candidate.installed_release_id,
                        )
                        .await?
                    else {
                        continue;
                    };
                    let Some(handler) = installation.manifest.server.as_ref().and_then(|server| {
                        server.event_handlers.iter().find(|handler| {
                            handler.id == handler_id
                                && handler
                                    .event_types
                                    .iter()
                                    .any(|event_type| event_type == &event.event_type)
                        })
                    }) else {
                        continue;
                    };
                    match self
                        .deliver_to_handler(
                            &repository,
                            &installation,
                            handler,
                            event,
                            attempts
                                .remove(&(installation.extension_id.clone(), handler.id.clone())),
                            &mut dispatch,
                        )
                        .await?
                    {
                        HandlerStep::Completed => completed.push((
                            installation.extension_id.clone(),
                            handler.id.clone(),
                            installation.installed_release_id,
                        )),
                        HandlerStep::Settled => {}
                        HandlerStep::Faulted(error) => return Err(error),
                    }
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = delivered {
            // Completed handlers are not invoked again when this delivery is
            // retried, whether a component faulted or the repository failed.
            if let Err(remember) = Self::remember_completed(&repository, event, &completed).await {
                tracing::warn!(event = %event.id, error = %remember, "completed handlers could not be recorded");
            }
            return Err(error);
        }
        if dispatch.resume_at.is_some() {
            Self::remember_completed(&repository, event, &completed).await?;
        }
        Ok(dispatch)
    }

    async fn remember_completed(
        repository: &AttricatRepository,
        event: &DomainEvent,
        completed: &[(String, String, Uuid)],
    ) -> Result<(), RepositoryError> {
        for (extension_id, handler_id, release) in completed {
            repository
                .record_extension_handler_attempt(HandlerAttemptOutcome {
                    event_id: event.id,
                    extension_id,
                    handler_id,
                    installed_release_id: *release,
                    status: "completed",
                    attempts: 1,
                    next_attempt_at: None,
                    error_code: None,
                    error_message: None,
                    record_id: None,
                })
                .await?;
        }
        Ok(())
    }

    /// Delivers one event to one handler, honoring its stored outcome and
    /// debounce window, and records the new outcome.
    async fn deliver_to_handler(
        &self,
        repository: &AttricatRepository,
        installation: &ExtensionRuntimeInstallation,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
        previous: Option<crate::repository::HandlerAttempt>,
        dispatch: &mut EventDispatch,
    ) -> Result<HandlerStep, RepositoryError> {
        let now = chrono::Utc::now();
        let record = |status: &'static str,
                      attempts: i32,
                      next_attempt_at: Option<chrono::DateTime<chrono::Utc>>| {
            HandlerAttemptOutcome {
                event_id: event.id,
                extension_id: &installation.extension_id,
                handler_id: &handler.id,
                installed_release_id: installation.installed_release_id,
                status,
                attempts,
                next_attempt_at,
                error_code: None,
                error_message: None,
                record_id: None,
            }
        };
        let attempts = previous.as_ref().map_or(0, |attempt| attempt.attempts);
        match previous.as_ref().map(|attempt| attempt.status.as_str()) {
            Some("completed" | "superseded" | "rejected" | "exhausted") => {
                return Ok(HandlerStep::Settled);
            }
            Some("retrying" | "debouncing") => {
                if let Some(due) = previous
                    .as_ref()
                    .and_then(|attempt| attempt.next_attempt_at)
                    .filter(|due| *due > now)
                {
                    dispatch.resume_no_later_than(due);
                    return Ok(HandlerStep::Settled);
                }
            }
            _ => {}
        }
        // A debounced handler sees a record's event only after the record has
        // been quiet for the window; a newer event of the same record
        // supersedes it, unless the event has waited its longest.
        if let Some((debounce, max_wait)) = handler.debounce()
            && event.aggregate_kind == "record"
        {
            let delay = |duration: Duration| {
                chrono::TimeDelta::from_std(duration).unwrap_or(chrono::TimeDelta::zero())
            };
            let due = event.occurred_at + delay(debounce);
            if previous.is_none() && due > now {
                repository
                    .record_extension_handler_attempt(record("debouncing", 0, Some(due)))
                    .await?;
                dispatch.resume_no_later_than(due);
                return Ok(HandlerStep::Settled);
            }
            if now < event.occurred_at + delay(max_wait)
                && repository
                    .newer_record_event_exists(
                        &installation.extension_id,
                        event,
                        &handler.event_types,
                    )
                    .await?
            {
                // The newer event's delivery pushes the record, so leases an
                // earlier attempt took end rather than hold it off.
                if exports_mappings(installation) {
                    repository
                        .release_export_leases(
                            &installation.extension_id,
                            &handler_lease_holder(event.id, &handler.id),
                        )
                        .await?;
                }
                repository
                    .record_extension_handler_attempt(record("superseded", attempts, None))
                    .await?;
                counter!("attricat_extension_invocations_total", "outcome" => "superseded")
                    .increment(1);
                return Ok(HandlerStep::Settled);
            }
        }
        let attempt = attempts + 1;
        // A publication event concerns one channel, which a handler does not
        // export to by itself, so its delivery carries the record's revision
        // without publication events, on the scale of typed reads. Mapping
        // revisions then never run ahead of what a channel export reads.
        let record_revision = match event.record_content_revision() {
            Some(_)
                if [
                    attricat_events::RECORD_PUBLISHED_V1,
                    attricat_events::RECORD_UNPUBLISHED_V1,
                ]
                .contains(&event.event_type.as_str()) =>
            {
                Some(
                    repository
                        .record_content_revision_at(event.aggregate_id, event.occurred_at, None)
                        .await?,
                )
            }
            revision => revision,
        };
        let delivery = EventDeliveryAttempt {
            sequence: event.sequence,
            record_revision,
            attempt: attempt as u32,
        };
        let outcome = self
            .runtime
            .invoke(
                installation,
                repository.for_extension(&installation.extension_id),
                handler,
                event,
                &delivery,
            )
            .await;
        let failure = match outcome {
            Ok(Ok(())) => {
                counter!("attricat_extension_invocations_total", "outcome" => "completed")
                    .increment(1);
                return Ok(HandlerStep::Completed);
            }
            Ok(Err(failure)) => failure,
            Err(error) => InvocationFailure::Fault(error.to_string()),
        };
        match failure {
            InvocationFailure::Rejected(rejection) => {
                counter!("attricat_extension_invocations_total", "outcome" => "rejected")
                    .increment(1);
                // A rejection that names no connection belongs to the
                // default destination. The record's status is written before
                // the handler's outcome, so a retried delivery that fails in
                // between writes it again rather than finding it settled.
                if let Some(record_id) = rejection.record_id
                    && exports_mappings(installation)
                {
                    let connection = rejection.connection;
                    // The delivery's revision is the event record's; another
                    // record's rejection concerns that record's revision when
                    // the event happened.
                    let revision = if record_id == event.aggregate_id {
                        delivery.record_revision
                    } else {
                        Some(
                            repository
                                .record_content_revision_at(record_id, event.occurred_at, None)
                                .await?,
                        )
                    };
                    repository
                        .record_export_rejection(
                            &installation.extension_id,
                            connection.as_deref(),
                            record_id,
                            revision,
                            Some(&handler_lease_holder(event.id, &handler.id)),
                            &rejection.code,
                            &rejection.message,
                        )
                        .await?;
                }
                // Other records the handler leased before it rejected were
                // not pushed either.
                if exports_mappings(installation) {
                    repository
                        .fail_export_leases(
                            &installation.extension_id,
                            &handler_lease_holder(event.id, &handler.id),
                            "handler_rejected",
                            &rejection.message,
                        )
                        .await?;
                }
                repository
                    .record_extension_handler_attempt(HandlerAttemptOutcome {
                        error_code: Some(&rejection.code),
                        error_message: Some(&rejection.message),
                        record_id: rejection.record_id,
                        ..record("rejected", attempt, None)
                    })
                    .await?;
                Ok(HandlerStep::Settled)
            }
            InvocationFailure::Retry { message, .. } if attempt >= MAX_HANDLER_ATTEMPTS => {
                counter!("attricat_extension_invocations_total", "outcome" => "exhausted")
                    .increment(1);
                // Records the handler still leases show as retrying, so an
                // operator finds and retries them, and its leases end.
                if exports_mappings(installation) {
                    repository
                        .fail_export_leases(
                            &installation.extension_id,
                            &handler_lease_holder(event.id, &handler.id),
                            "retries_exhausted",
                            &message,
                        )
                        .await?;
                }
                repository
                    .record_extension_handler_attempt(HandlerAttemptOutcome {
                        error_code: Some("retries_exhausted"),
                        error_message: Some(&message),
                        ..record("exhausted", attempt, None)
                    })
                    .await?;
                Ok(HandlerStep::Settled)
            }
            InvocationFailure::Retry { message, after } => {
                counter!("attricat_extension_invocations_total", "outcome" => "retrying")
                    .increment(1);
                let delay = after.unwrap_or_else(|| retry_backoff(attempt as u32));
                let due =
                    now + chrono::TimeDelta::from_std(delay).unwrap_or(chrono::TimeDelta::zero());
                repository
                    .record_extension_handler_attempt(HandlerAttemptOutcome {
                        error_code: Some("retry"),
                        error_message: Some(&message),
                        ..record("retrying", attempt, Some(due))
                    })
                    .await?;
                dispatch.resume_no_later_than(due);
                Ok(HandlerStep::Settled)
            }
            InvocationFailure::Fault(error) => {
                metrics::counter!("attricat_extension_invocations_total", "outcome" => "failed")
                    .increment(1);
                tracing::warn!(
                    extension = %installation.extension_id,
                    installed_release_id = %installation.installed_release_id,
                    handler = %handler.id,
                    event_id = %event.id,
                    event_sequence = event.sequence,
                    event_type = %event.event_type,
                    aggregate_kind = %event.aggregate_kind,
                    aggregate_id = %event.aggregate_id,
                    correlation_id = %event.correlation_id,
                    error = %error,
                    "extension handler failed; quarantining extension"
                );
                // Records it was pushing show as retrying rather than keep a
                // lease and an older status.
                if exports_mappings(installation) {
                    repository
                        .fail_export_leases(
                            &installation.extension_id,
                            &handler_lease_holder(event.id, &handler.id),
                            "handler_fault",
                            &error,
                        )
                        .await?;
                }
                quarantine_after_fault(repository, &installation.extension_id).await?;
                Ok(HandlerStep::Faulted(Box::new(
                    ExtensionRuntimeError::Runtime(error),
                )))
            }
        }
    }
}

/// The result of delivering one event to its handlers.
#[derive(Default)]
struct EventDispatch {
    /// The earliest time a retrying or debouncing handler is due.
    resume_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether any handler took the event, so outcomes may be stored.
    handled: bool,
}

impl EventDispatch {
    fn resume_no_later_than(&mut self, at: chrono::DateTime<chrono::Utc>) {
        self.resume_at = Some(self.resume_at.map_or(at, |current| current.min(at)));
    }
}

/// How one handler's delivery ended.
enum HandlerStep {
    /// Invoked successfully just now.
    Completed,
    /// Already settled, rejected, retrying later, debouncing or superseded.
    Settled,
    /// The component faulted and its installation was quarantined.
    Faulted(Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
impl TaskHandler for WasmExtensionTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::EventDeliveryV1
    }

    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let repository = self.repository.for_workspace(task.workspace_id);
        let Some(delivery) =
            repository
                .begin_task_event_delivery(&task)
                .await
                .map_err(|error| TaskHandlerError {
                    code: "receipt",
                    message: error.to_string(),
                })?
        else {
            return Ok(TaskOutcome::Complete);
        };
        let context =
            repository.for_event_delivery_task(&delivery.event, WASM_EVENT_CONSUMER, &task);
        let dispatch = self
            .invoke_event(&delivery.event, context)
            .await
            .map_err(|error| TaskHandlerError {
                code: "extension",
                message: error.to_string(),
            })?;
        // Retrying and debouncing handlers keep the delivery open; it runs
        // again when the earliest of them is due.
        if let Some(at) = dispatch.resume_at {
            return Ok(TaskOutcome::Reschedule { at });
        }
        repository
            .complete_task_event_delivery(&task)
            .await
            .map_err(|error| TaskHandlerError {
                code: "receipt",
                message: error.to_string(),
            })?;
        // Cleared when a handler took the event, which covers every outcome
        // this delivery stored. One stored by an earlier run for a handler
        // whose extension has since been disabled is left to the retention
        // sweep, so events no handler takes cost no write.
        if dispatch.handled {
            repository
                .clear_extension_handler_attempts(delivery.event.id)
                .await
                .map_err(|error| TaskHandlerError {
                    code: "receipt",
                    message: error.to_string(),
                })?;
        }
        Ok(TaskOutcome::Complete)
    }
}

/// How often the coordinator runs its operation storage, schedule and
/// connector sweeps per workspace.
const COORDINATOR_SWEEP_INTERVAL: Duration = Duration::from_secs(5);

/// What the coordinator remembers about one workspace between ticks.
#[derive(Default)]
struct CoordinatedWorkspace {
    /// The consumer row exists; it is never removed afterwards.
    registered: bool,
    /// The eligible event types of the last materialization; a change, or
    /// the first pass, also scans legacy deliveries and the historical gap.
    event_types: Option<Vec<String>>,
    /// A historical batch was enqueued and more may remain.
    history_pending: bool,
    last_sweep: Option<Instant>,
}

/// Intake is only a producer/coordinator. It registers at the high-water mark
/// and atomically creates delivery/task pairs for subsequently supported event
/// versions; execution is exclusively owned by `WasmExtensionTaskHandler`.
pub fn start_event_delivery_coordinator(
    repository: SystemRepository,
    object_store: Arc<dyn ObjectStore>,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut workspaces: HashMap<Uuid, CoordinatedWorkspace> = HashMap::new();
        // Only one replica runs intake and sweeps; the others stand by.
        let mut leadership = CoordinatorLeadership::new("extension_intake");
        loop {
            if !leadership.is_leader(&repository).await {
                tokio::select! {
                    _ = tokio::time::sleep(coordinator::STANDBY_POLL) => continue,
                    _ = shutdown.changed() => return,
                }
            }
            measure("worker:extension_coordinator", async {
                for workspace_id in repository
                    .polled_workspace_ids()
                    .await
                    .map(|workspaces| workspaces.as_ref().clone())
                    .unwrap_or_default()
                {
                    let state = workspaces.entry(workspace_id).or_default();
                    let result =
                        coordinate_workspace(&repository, &object_store, workspace_id, state).await;
                    if let Err(error) = result {
                        tracing::error!(%error, "extension event-delivery intake failed");
                    }
                }
            })
            .await;
            tokio::select! {
                _ = tokio::time::sleep(coordinator::IDLE_POLL) => {},
                _ = shutdown.changed() => return,
            }
        }
    })
}

async fn coordinate_workspace(
    repository: &SystemRepository,
    object_store: &Arc<dyn ObjectStore>,
    workspace_id: Uuid,
    state: &mut CoordinatedWorkspace,
) -> Result<(), RepositoryError> {
    let scoped = repository.for_workspace(workspace_id);
    if state
        .last_sweep
        .is_none_or(|last| last.elapsed() >= COORDINATOR_SWEEP_INTERVAL)
    {
        state.last_sweep = Some(Instant::now());
        // Incomplete operation output has no committed object key; this
        // periodic lifecycle sweep makes interrupted streams unavailable even
        // when no subsequent operation task runs.
        scoped.expire_extension_operation_storage().await?;
        for key in scoped.abort_stale_extension_operation_artifacts().await? {
            if object_store.delete(&key).await.is_ok() {
                scoped
                    .confirm_extension_operation_object_deletion(&key)
                    .await?;
            }
        }
        scoped.produce_due_extension_operation_schedules().await?;
        scoped.produce_due_blueprint_connector_jobs().await?;
    }
    if !state.registered {
        scoped
            .ensure_event_consumer(WASM_EVENT_CONSUMER, &[])
            .await?;
        state.registered = true;
    }
    let event_types = scoped.enabled_extension_event_types().await?;
    let scope = if state.history_pending || state.event_types.as_ref() != Some(&event_types) {
        MaterializeScope::Full
    } else {
        MaterializeScope::NewEvents
    };
    state.history_pending = scoped
        .materialize_event_delivery_tasks_with(WASM_EVENT_CONSUMER, &event_types, scope)
        .await?;
    state.event_types = Some(event_types);
    Ok(())
}

const _: () = assert!(MAX_ARTIFACT_CHUNK_BYTES <= MAX_HOST_JSON_BYTES);

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::{
        MAX_ARTIFACT_CHUNK_BYTES, NETWORK_RATE_LIMIT, allow_network_request, parse_bounded_json,
        require_operation_batch_key, to_configuration_scope, transfer_url, valid_secret_name, wit,
    };

    /// The smallest valid component: the component-model binary header.
    const EMPTY_COMPONENT: &[u8] = b"\0asm\x0d\x00\x01\x00";

    fn runtime() -> super::ExtensionRuntime {
        super::ExtensionRuntime::new(
            std::sync::Arc::new(crate::storage::FakeObjectStore::available()),
            super::ExtensionRuntimeConfig::default(),
        )
        .unwrap()
    }

    fn cache_key() -> super::ComponentCacheKey {
        super::ComponentCacheKey {
            installed_release_id: Uuid::new_v4(),
            artifact_id: "server".into(),
        }
    }

    #[tokio::test]
    async fn concurrent_component_misses_share_one_compilation() {
        let runtime = runtime();
        let key = cache_key();
        let loads = std::sync::atomic::AtomicUsize::new(0);
        let load = || async {
            loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(EMPTY_COMPONENT)
        };
        let (first, second) = tokio::join!(
            runtime.cached_component(key.clone(), load),
            runtime.cached_component(key.clone(), load),
        );
        assert!(std::sync::Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Later calls are cache hits and leave no compilation behind.
        runtime.cached_component(key, load).await.unwrap();
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(runtime.components.lock().unwrap().compiling.is_empty());
    }

    #[tokio::test]
    async fn a_failed_component_compilation_is_retried() {
        let runtime = runtime();
        let key = cache_key();
        let invalid: &[u8] = b"not a component";
        assert!(
            runtime
                .cached_component(key.clone(), || async { Ok(invalid) })
                .await
                .is_err()
        );
        assert!(runtime.components.lock().unwrap().compiling.is_empty());
        runtime
            .cached_component(key, || async { Ok(EMPTY_COMPONENT) })
            .await
            .unwrap();
    }

    #[test]
    fn bulk_transfer_denies_plaintext_urls_credentials_and_queries() {
        assert!(transfer_url("https://example.com/v1/data").is_ok());
        for url in [
            "http://example.com/v1/data",
            "https://user:pass@example.com/v1/",
            "https://example.com/v1/?secret=x",
            "https://example.com/v1/#fragment",
        ] {
            assert!(transfer_url(url).is_err());
        }
    }

    #[test]
    fn connector_batches_must_use_the_current_durable_key() {
        assert!(require_operation_batch_key("run:0", "run:0").is_ok());
        assert!(require_operation_batch_key("run:1", "run:0").is_err());
        assert!(require_operation_batch_key("other:0", "run:0").is_err());
        assert!(require_operation_batch_key("", "").is_err());
    }

    #[test]
    fn artifact_stream_contract_keeps_chunks_and_checksums_bounded() {
        assert_eq!(MAX_ARTIFACT_CHUNK_BYTES, 64 * 1024);
        assert_eq!(
            format!("{:x}", Sha256::digest(b"bounded output")),
            "d047501029296dac4c1be5e22f05ff7229244184357ac63424d75339009a77e3"
        );
    }

    #[tokio::test]
    async fn network_rate_limit_is_release_scoped_and_failed_calls_do_not_share_a_bucket() {
        let release = Uuid::new_v4();
        for _ in 0..NETWORK_RATE_LIMIT {
            assert!(allow_network_request(release, "acme.extension").await);
        }
        assert!(!allow_network_request(release, "acme.extension").await);
        assert!(allow_network_request(Uuid::new_v4(), "acme.extension").await);
        assert!(allow_network_request(release, "acme.other").await);
    }

    #[test]
    fn secret_names_are_bounded_stable_identifiers() {
        assert!(valid_secret_name("destination-token_1").is_ok());
        assert!(valid_secret_name("").is_err());
        assert!(valid_secret_name("contains space").is_err());
        assert!(valid_secret_name(&"x".repeat(129)).is_err());
    }

    #[test]
    fn typed_api_inputs_reject_oversized_json_and_invalid_scopes() {
        assert!(parse_bounded_json(&"x".repeat(65_537), "value").is_err());
        assert!(parse_bounded_json("{", "value").is_err());
        assert!(
            to_configuration_scope(wit::api::ConfigurationScope {
                kind: wit::api::ConfigurationScopeKind::Attribute,
                blueprint_id: "not-a-uuid".into(),
                blueprint_version: 1,
                attribute_id: Some("also-not-a-uuid".into()),
            })
            .is_err()
        );
    }
}

fn require_operation_batch_key(supplied: &str, current: &str) -> Result<(), String> {
    if supplied != current || current.is_empty() {
        return Err("batch key must match the current operation batch".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectorUpsertBatch {
    blueprint_id: Uuid,
    blueprint_version: u64,
    context_id: Uuid,
    run_id: Uuid,
    batch_key: String,
    intents: Vec<ConnectorUpsertIntent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectorUpsertIntent {
    row: u64,
    business_key: String,
    key: String,
    values: HashMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferFetch {
    host_permission_id: String,
    url: String,
    transfer_key: String,
    offset: u64,
    max_bytes: u32,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    secret_headers: Vec<SecretHeader>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferDelivery {
    host_permission_id: String,
    url: String,
    method: String,
    artifact_id: Uuid,
    delivery_key: String,
    #[serde(default)]
    secret_headers: Vec<SecretHeader>,
}

fn transfer_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "invalid HTTPS URL")?;
    if url.scheme() != "https"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.username() != ""
        || url.password().is_some()
    {
        return Err("only query-free HTTPS destinations are allowed".into());
    }
    Ok(url)
}

fn transfer_key_valid(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_TRANSFER_KEY_BYTES && key.is_ascii()
}

/// Development-only egress overrides, so the broker can reach a local mock of
/// a marketplace: hostnames resolved to fixed (possibly private) addresses
/// and an extra trusted root certificate. Never configured in production;
/// the composition root installs it only with `ATTRICAT_DEVTOOLS`.
#[derive(Clone, Debug, Default)]
pub struct DevelopmentEgress {
    /// `(host pattern, address)`; a pattern is an exact host or `*.suffix`.
    pub resolve: Vec<(String, std::net::SocketAddr)>,
    /// PEM certificates trusted in addition to the system roots.
    pub extra_root_certificates_pem: Vec<u8>,
}

static DEVELOPMENT_EGRESS: OnceLock<DevelopmentEgress> = OnceLock::new();

/// Installs development egress overrides. Call before the runtime serves
/// requests; later calls are ignored.
pub fn use_development_egress(egress: DevelopmentEgress) {
    let _ = DEVELOPMENT_EGRESS.set(egress);
}

fn development_address(host: &str, port: u16) -> Option<std::net::SocketAddr> {
    DEVELOPMENT_EGRESS
        .get()?
        .resolve
        .iter()
        .find_map(|(pattern, address)| {
            let matches = match pattern.strip_prefix("*.") {
                Some(suffix) => host
                    .strip_suffix(suffix)
                    .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1),
                None => host.eq_ignore_ascii_case(pattern),
            };
            // An override keeps the requested port unless it names its own.
            matches.then(|| {
                let mut address = *address;
                if address.port() == 0 {
                    address.set_port(port);
                }
                address
            })
        })
}

/// Resolves an outbound destination within `timeout` and accepts it only when
/// every resolved address is public.
async fn resolve_public_destination(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Vec<std::net::SocketAddr>, String> {
    if let Some(address) = development_address(host, port) {
        return Ok(vec![address]);
    }
    let addresses: Vec<std::net::SocketAddr> =
        tokio::time::timeout(timeout, tokio::net::lookup_host((host, port)))
            .await
            .map_err(|_| "destination DNS lookup timed out")?
            .map_err(|_| "destination DNS lookup failed")?
            .collect();
    if addresses.is_empty()
        || addresses.iter().any(|address| {
            !attricat_extension_manifest::extensions::is_public_destination(address.ip())
        })
    {
        return Err("destination resolves to an unsafe address".into());
    }
    Ok(addresses)
}

/// An HTTPS-only client pinned to the checked `addresses`, without redirects.
fn pinned_client(
    host: &str,
    addresses: &[std::net::SocketAddr],
    timeout: Duration,
) -> Result<Client, String> {
    let mut builder = Client::builder()
        // Environment proxy settings would bypass the checked/pinned target
        // address and turn this broker into an SSRF proxy.
        .no_proxy()
        .redirect(Policy::none())
        .https_only(true)
        .resolve_to_addrs(host, addresses)
        .connect_timeout(timeout)
        .timeout(timeout);
    if let Some(egress) = DEVELOPMENT_EGRESS.get()
        && !egress.extra_root_certificates_pem.is_empty()
    {
        for certificate in
            reqwest::Certificate::from_pem_bundle(&egress.extra_root_certificates_pem)
                .map_err(|_| "development root certificate is invalid")?
        {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder
        .build()
        .map_err(|_| "network client unavailable".into())
}

impl OperationState {
    async fn attricat_schema(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
    ) -> Result<String, String> {
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.read").await?;
        let blueprint = parse_uuid(&blueprint_id, "blueprint ID")?;
        let version = i64::try_from(blueprint_version).map_err(|_| "invalid blueprint version")?;
        let context = parse_uuid(&context_id, "context ID")?;
        if let Some((selected_blueprint, selected_context, selected_version, _, _)) = self
            .repository
            .connector_run_scope(self.run_id)
            .await
            .map_err(|error| error.to_string())?
            && (blueprint, context, version)
                != (selected_blueprint, selected_context, selected_version)
        {
            return Err("connector schema is outside the host-selected job scope".into());
        }
        host.repository
            .get_context_by_id(context)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "context not found".to_owned())?;
        host.repository
            .get_blueprint_revision(blueprint, version)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "blueprint revision not found".to_owned())?;
        let attrs = host
            .repository
            .list_attributes(blueprint, version)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(
            &json!({"attributes": attrs.iter().map(|a| json!({"id":a.code,"kind":a.value_type})).collect::<Vec<_>>()}),
        )
    }

    async fn attricat_page(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
        cursor: String,
        limit: u32,
    ) -> Result<String, String> {
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.read").await?;
        if limit == 0 || limit > MAX_CONNECTOR_PAGE_LIMIT {
            return Err(format!(
                "connector page limit must be 1-{MAX_CONNECTOR_PAGE_LIMIT}"
            ));
        }
        let blueprint = parse_uuid(&blueprint_id, "blueprint ID")?;
        let version = i64::try_from(blueprint_version).map_err(|_| "invalid blueprint version")?;
        let context = parse_uuid(&context_id, "context ID")?;
        let publication_context_id = if let Some((
            selected_blueprint,
            selected_context,
            selected_version,
            channel,
            direction,
        )) = self
            .repository
            .connector_run_scope(self.run_id)
            .await
            .map_err(|error| error.to_string())?
        {
            if direction != "export"
                || (blueprint, context, version)
                    != (selected_blueprint, selected_context, selected_version)
            {
                return Err("connector page is outside the host-selected export scope".into());
            }
            channel
        } else {
            None
        };
        let page = host
            .repository
            .extension_attricat_page(ExtensionAttricatPageRequest {
                blueprint_id: blueprint,
                blueprint_version: version,
                context_id: Some(context),
                publication_context_id,
                cursor: (!cursor.is_empty()).then_some(cursor),
                limit,
            })
            .await
            .map_err(|error| error.to_string())?;
        let attributes = host
            .repository
            .list_attributes(blueprint, version)
            .await
            .map_err(|error| error.to_string())?;
        let mut rows = Vec::with_capacity(page.records.len());
        for record in page.records {
            let mut row = serde_json::Map::new();
            for value in host
                .repository
                .extension_attricat_values_at(record.id, page.snapshot_at)
                .await
                .map_err(|error| error.to_string())?
            {
                if value.context_id == Some(context)
                    && let Some(attribute) = attributes
                        .iter()
                        .find(|attribute| attribute.id == value.attribute_id)
                {
                    row.insert(attribute.code.clone(), value.value);
                }
            }
            rows.push(row);
        }
        bounded_serialize(&json!({"rows":rows,"next_cursor":page.next_cursor}))
    }

    async fn attricat_upsert_batch(&mut self, request: String) -> Result<(), String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let input: ConnectorUpsertBatch = parse_storage_request(&request)?;
        require_operation_batch_key(&input.batch_key, &self.batch_key)?;
        if input.run_id != self.run_id {
            return Err("batch run ID does not match".into());
        }
        if input.intents.is_empty() || input.intents.len() > MAX_EXTENSION_BATCH_INTENTS {
            return Err(format!(
                "batch must contain 1-{MAX_EXTENSION_BATCH_INTENTS} intents"
            ));
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.write").await?;
        let version =
            i64::try_from(input.blueprint_version).map_err(|_| "invalid blueprint version")?;
        if let Some((selected_blueprint, selected_context, selected_version, _, direction)) = self
            .repository
            .connector_run_scope(self.run_id)
            .await
            .map_err(|error| error.to_string())?
            && (direction != "import"
                || (input.blueprint_id, input.context_id, version)
                    != (selected_blueprint, selected_context, selected_version))
        {
            return Err("connector upsert is outside the host-selected import scope".into());
        }
        let attrs = host
            .repository
            .list_attributes(input.blueprint_id, version)
            .await
            .map_err(|error| error.to_string())?;
        let mut intents = Vec::with_capacity(input.intents.len());
        for row in input.intents {
            let lookup = attrs
                .iter()
                .find(|a| a.code == row.business_key && !a.readonly)
                .ok_or_else(|| "business key attribute is not writable".to_owned())?;
            if row.values.get(&row.business_key) != Some(&Value::String(row.key.clone())) {
                return Err("business key value does not match the lookup value".into());
            }
            if row.key.is_empty()
                || row.key.len() > MAX_EXTENSION_LOOKUP_VALUE_BYTES
                || row.values.len() > MAX_CONNECTOR_UPSERT_VALUES
            {
                return Err("invalid bounded upsert row".into());
            }
            let mut values = Vec::with_capacity(row.values.len());
            for (code, value) in row.values {
                if !attrs.iter().any(|a| a.code == code && !a.readonly) {
                    return Err("upsert attribute is not writable".into());
                }
                values.push(NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some(code),
                    context_id: Some(input.context_id),
                    value,
                });
            }
            intents.push(ExtensionAttricatIntent::Upsert {
                intent_key: row.row.to_string(),
                blueprint_id: input.blueprint_id,
                blueprint_version: version,
                lookup_attribute_id: lookup.id,
                lookup_value: row.key,
                values,
                relationships: Vec::new(),
                system_tags: Vec::new(),
                system_metadata: json!({}),
            });
        }
        let repository = host
            .repository
            .for_extension(&host.installation.extension_id);
        let outcomes = AttricatMutationService::new(&repository)
            .execute_extension_attricat_batch(ExtensionAttricatBatch {
                batch_key: input.batch_key,
                dry_run: false,
                intents,
            })
            .await
            .map_err(|error| error.to_string())?;
        if outcomes.iter().any(|outcome| outcome.error.is_some()) {
            return Err("one or more catalog intents were rejected".into());
        }
        Ok(())
    }
}

impl OperationState {
    async fn attricat_data_read(&mut self, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        if self
            .repository
            .connector_run_scope(self.run_id)
            .await
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("scoped jobs must use host-filtered connector catalog calls".into());
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.read").await?;
        host.attricat_read_call(&request).await
    }

    async fn attricat_data_batch(&mut self, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let scope = self
            .repository
            .connector_run_scope(self.run_id)
            .await
            .map_err(|error| error.to_string())?;
        let input: AttricatCommandRequest = parse_storage_request(&request)?;
        let AttricatCommandRequest::Batch { batch } = input;
        require_operation_batch_key(&batch.batch_key, &self.batch_key)?;
        if let Some((blueprint_id, ..)) = scope {
            return self.scoped_annotation_batch(blueprint_id, batch).await;
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_all_active(batch_capabilities(&batch)).await?;
        let repository = host
            .repository
            .for_extension(&host.installation.extension_id);
        let outcomes = AttricatMutationService::new(&repository)
            .execute_extension_attricat_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
    }
}

impl OperationState {
    async fn artifacts_append_output(
        &mut self,
        name: String,
        media_type: String,
        batch_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        require_operation_batch_key(&batch_key, &self.batch_key)?;
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let chunk = self
            .repository
            .reserve_extension_output_chunk(
                self.run_id,
                &name,
                &media_type,
                &batch_key,
                bytes.len(),
                &hash,
            )
            .await
            .map_err(|error| error.to_string())?;
        if !chunk.uploaded {
            self.object_store
                .put(
                    &chunk.key,
                    StoredObject {
                        bytes: bytes.into(),
                        content_type: Some(media_type),
                    },
                )
                .await
                .map_err(|_| "output staging storage is unavailable".to_owned())?;
            self.repository
                .confirm_extension_output_chunk(chunk.id, self.run_id)
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    async fn artifacts_finalize_output(&mut self, name: String) -> Result<String, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        let (media_type, existing, chunks) = self
            .repository
            .extension_output_chunks(self.run_id, &name)
            .await
            .map_err(|error| error.to_string())?;
        if chunks.is_empty() {
            return Err("output contains no chunks".into());
        }
        let artifact = match existing {
            Some(id) => self
                .repository
                .extension_output_artifact(self.run_id, id)
                .await
                .map_err(|error| error.to_string())?,
            None => {
                let artifact = self
                    .repository
                    .create_extension_operation_output_artifact(
                        self.run_id,
                        &self.installation.extension_id,
                        self.installation.installed_release_id,
                        &media_type,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                let assigned = self
                    .repository
                    .set_extension_output_artifact(self.run_id, &name, artifact.id)
                    .await
                    .map_err(|error| error.to_string())?;
                if assigned != artifact.id {
                    let _ = self
                        .repository
                        .abort_extension_operation_artifact(artifact.id, self.run_id)
                        .await;
                    self.repository
                        .extension_output_artifact(self.run_id, assigned)
                        .await
                        .map_err(|error| error.to_string())?
                } else {
                    artifact
                }
            }
        };
        if artifact.state == "completed" {
            return Ok(artifact.id.to_string());
        }
        let path =
            std::env::temp_dir().join(format!("attricat-operation-finalize-{}", artifact.id));
        let result = async {
            let mut file =
                File::create(&path).map_err(|_| "temporary output unavailable".to_owned())?;
            let mut hasher = Sha256::new();
            let mut length: i64 = 0;
            for chunk in chunks {
                let object = self
                    .object_store
                    .get(&chunk.object_key)
                    .await
                    .map_err(|_| "staged output unavailable".to_owned())?;
                if object.bytes.len() != chunk.content_length as usize
                    || format!("{:x}", Sha256::digest(&object.bytes)) != chunk.checksum_sha256
                {
                    return Err("staged output checksum mismatch".into());
                }
                length += chunk.content_length as i64;
                if length > crate::repository::MAX_OPERATION_ARTIFACT_BYTES {
                    return Err("output quota exhausted".into());
                }
                file.write_all(&object.bytes)
                    .map_err(|_| "temporary output unavailable".to_owned())?;
                hasher.update(&object.bytes);
            }
            drop(file);
            if artifact.content_length == 0 {
                self.repository
                    .reserve_extension_operation_artifact_bytes(artifact.id, self.run_id, length)
                    .await
                    .map_err(|error| error.to_string())?;
            } else if artifact.content_length != length {
                return Err("output length changed across finalize retry".into());
            }
            let key = format!("extension-operation-artifacts/v1/{}", artifact.id);
            self.object_store
                .put_file(&key, &path, Some(&media_type))
                .await
                .map_err(|_| "output storage unavailable".to_owned())?;
            let checksum = format!("{:x}", hasher.finalize());
            self.repository
                .complete_extension_operation_artifact(artifact.id, self.run_id, &checksum, &key)
                .await
                .map_err(|error| error.to_string())?;
            Ok(artifact.id.to_string())
        }
        .await;
        let _ = std::fs::remove_file(&path);
        result
    }

    async fn artifacts_open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.read")
            .await?;
        let artifact = self
            .repository
            .extension_operation_input_artifact(
                self.run_id,
                &self.installation.extension_id,
                self.installation.installed_release_id,
                &artifact_id,
            )
            .await
            .map_err(|_| "operation input artifact is not authorized".to_owned())?;
        self.artifacts
            .push(InputArtifactStream {
                artifact,
                offset: 0,
            })
            .map_err(|_| "artifact handle limit reached".to_owned())
    }

    async fn artifacts_describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<wit::artifacts::InputMetadata, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.read")
            .await?;
        let artifact = self
            .artifacts
            .get(&handle)
            .map_err(|_| "invalid artifact handle".to_owned())?;
        Ok(wit::artifacts::InputMetadata {
            content_length: artifact.artifact.content_length as u64,
            media_type: artifact.artifact.media_type.clone(),
            checksum_sha256: artifact
                .artifact
                .checksum_sha256
                .clone()
                .unwrap_or_default(),
        })
    }

    async fn artifacts_read(
        &mut self,
        handle: Resource<InputArtifactStream>,
        max_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.read")
            .await?;
        let max_bytes =
            usize::try_from(max_bytes).map_err(|_| "invalid artifact chunk".to_owned())?;
        if max_bytes == 0 || max_bytes > MAX_ARTIFACT_CHUNK_BYTES {
            return Err("artifact read chunk exceeds limit".into());
        }
        let (key, offset, length) = {
            let stream = self
                .artifacts
                .get(&handle)
                .map_err(|_| "invalid artifact handle".to_owned())?;
            (
                stream
                    .artifact
                    .object_key
                    .clone()
                    .ok_or_else(|| "artifact is unavailable".to_owned())?,
                stream.offset,
                stream.artifact.content_length,
            )
        };
        if offset >= length {
            return Ok(Vec::new());
        }
        let end = (offset + max_bytes as i64 - 1).min(length - 1);
        let object = self
            .object_store
            .get_range(&key, Some(&format!("bytes={offset}-{end}")))
            .await
            .map_err(|_| "artifact storage is unavailable".to_owned())?;
        if object.bytes.len() > max_bytes {
            return Err("artifact storage returned oversized chunk".into());
        }
        let bytes = object.bytes.to_vec();
        self.artifacts
            .get_mut(&handle)
            .map_err(|_| "invalid artifact handle".to_owned())?
            .offset += bytes.len() as i64;
        Ok(bytes)
    }

    async fn artifacts_create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        let artifact = self
            .repository
            .create_extension_operation_output_artifact(
                self.run_id,
                &self.installation.extension_id,
                self.installation.installed_release_id,
                &media_type,
            )
            .await
            .map_err(|_| "operation artifact quota exhausted".to_owned())?;
        let path =
            std::env::temp_dir().join(format!("attricat-operation-artifact-{}", artifact.id));
        File::create(&path).map_err(|_| "temporary artifact storage is unavailable".to_owned())?;
        self.artifacts
            .push(OutputArtifactStream {
                artifact,
                path,
                hasher: Sha256::new(),
            })
            .map_err(|_| "artifact handle limit reached".to_owned())
    }

    async fn artifacts_write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        if bytes.is_empty() || bytes.len() > MAX_ARTIFACT_CHUNK_BYTES {
            return Err("artifact write chunk exceeds limit".into());
        }
        let artifact_id = self
            .artifacts
            .get(&handle)
            .map_err(|_| "invalid artifact handle".to_owned())?
            .artifact
            .id;
        self.repository
            .reserve_extension_operation_artifact_bytes(
                artifact_id,
                self.run_id,
                bytes.len() as i64,
            )
            .await
            .map_err(|_| "operation artifact quota exhausted".to_owned())?;
        let output = self
            .artifacts
            .get_mut(&handle)
            .map_err(|_| "invalid artifact handle".to_owned())?;
        OpenOptions::new()
            .append(true)
            .open(&output.path)
            .and_then(|mut file| file.write_all(&bytes))
            .map_err(|_| "temporary artifact storage is unavailable".to_owned())?;
        output.hasher.update(&bytes);
        Ok(())
    }

    async fn artifacts_complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<wit::artifacts::OutputMetadata, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        let output = self
            .artifacts
            .delete(handle)
            .map_err(|_| "invalid artifact handle".to_owned())?;
        let actual = format!("{:x}", output.hasher.finalize());
        if checksum_sha256 != actual {
            let _ = self
                .repository
                .abort_extension_operation_artifact(output.artifact.id, self.run_id)
                .await;
            let _ = std::fs::remove_file(&output.path);
            return Err("artifact checksum mismatch".into());
        }
        // The prefix is an immutable storage format contract. Future artifact
        // layouts get a new version rather than changing how v1 objects read.
        let key = format!("extension-operation-artifacts/v1/{}", output.artifact.id);
        if self
            .object_store
            .put_file(&key, &output.path, Some(&output.artifact.media_type))
            .await
            .is_err()
        {
            let _ = self
                .repository
                .abort_extension_operation_artifact(output.artifact.id, self.run_id)
                .await;
            let _ = std::fs::remove_file(&output.path);
            return Err("artifact storage is unavailable".into());
        }
        let artifact = match self
            .repository
            .complete_extension_operation_artifact(output.artifact.id, self.run_id, &actual, &key)
            .await
        {
            Ok(artifact) => artifact,
            Err(_) => {
                // An object write can succeed before its database completion
                // transaction. Delete that orphan before returning; the
                // incomplete row is also terminalized so retry cannot expose it.
                let _ = self.object_store.delete(&key).await;
                let _ = self
                    .repository
                    .abort_extension_operation_artifact(output.artifact.id, self.run_id)
                    .await;
                let _ = std::fs::remove_file(&output.path);
                return Err("operation output artifact is not completable".into());
            }
        };
        let _ = std::fs::remove_file(&output.path);
        Ok(wit::artifacts::OutputMetadata {
            artifact_id: artifact.id.to_string(),
            content_length: artifact.content_length as u64,
            media_type: artifact.media_type,
            checksum_sha256: actual,
        })
    }

    async fn artifacts_abort(
        &mut self,
        handle: Resource<OutputArtifactStream>,
    ) -> Result<(), String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id, "artifacts.write")
            .await?;
        let output = self
            .artifacts
            .delete(handle)
            .map_err(|_| "invalid artifact handle".to_owned())?;
        self.repository
            .abort_extension_operation_artifact(output.artifact.id, self.run_id)
            .await
            .map_err(|_| "operation output artifact could not be aborted".to_owned())?;
        let _ = std::fs::remove_file(&output.path);
        Ok(())
    }
}

impl OperationState {
    async fn artifacts_drop_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> wasmtime::Result<()> {
        self.artifacts.delete(handle)?;
        Ok(())
    }
}

impl OperationState {
    async fn artifacts_drop_output(
        &mut self,
        handle: Resource<OutputArtifactStream>,
    ) -> wasmtime::Result<()> {
        if let Ok(output) = self.artifacts.delete(handle) {
            // Resource destruction is the normal trap/unwind cleanup path.
            // It must terminalize the durable record immediately rather than
            // relying solely on the periodic abandoned-stream sweep.
            let _ = self
                .repository
                .abort_extension_operation_artifact(output.artifact.id, self.run_id)
                .await;
            let _ = std::fs::remove_file(&output.path);
        }
        Ok(())
    }
}
