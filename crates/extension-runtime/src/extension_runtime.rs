//! Sandboxed execution of installed server extension components.
//!
//! Components get no WASI context, filesystem, environment, clocks, sockets, or
//! pre-opened descriptors. The only imports are the versioned WIT functions in
//! `wit/catalog-extension.wit`; every call is checked against the immutable
//! release manifest and invocation-time grants.

use std::{
    collections::{HashMap, VecDeque},
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use semver::{Version, VersionReq};
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
    catalog_service::CatalogMutationService,
    constants::DEFAULT_LIST_PAGE_SIZE,
    domain_events::DomainEvent,
    extension_installer::installed_artifact_key,
    extensions::{
        ArtifactKind, EventHandler as ManifestEventHandler, MAX_EXTENSION_IDENTIFIER_BYTES,
    },
    model::{AppendAttributeValues, NewAttributeValue},
    repository::{
        CatalogRepository, ClaimedTask, ExtensionCatalogPageRequest, ExtensionConfigurationScope,
        ExtensionOperationArtifact, ExtensionRuntimeInstallation,
    },
    storage::{ObjectStore, ObjectStoreError},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};

wasmtime::component::bindgen!({
    path: "wit",
    world: "catalog-extension",
    imports: { default: async },
    exports: { default: async },
});
mod host_operations {
    wasmtime::component::bindgen!({
        path: "wit-artifacts",
        world: "catalog-extension-operation",
        with: {
            "catalog:host/artifacts.input-artifact": crate::extension_runtime::InputArtifactStream,
            "catalog:host/artifacts.output-artifact": crate::extension_runtime::OutputArtifactStream,
        },
        imports: { default: async },
        exports: { default: async },
    });
}
mod host_v11 {
    wasmtime::component::bindgen!({
        path: "wit-next",
        world: "catalog-extension",
        imports: { default: async },
        exports: { default: async },
    });
}

const MAX_HOST_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_HOST_JSON_BYTES: usize = 64 * 1024;
const MAX_CACHED_COMPONENTS: usize = 64;
const MAX_WRITE_VALUES: usize = 100;
const EPOCH_TICK_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug)]
pub struct ExtensionRuntimeConfig {
    pub fuel: u64,
    pub max_memory_bytes: usize,
    pub invocation_timeout: Duration,
}

impl Default for ExtensionRuntimeConfig {
    fn default() -> Self {
        Self {
            fuel: 10_000_000,
            max_memory_bytes: 32 * 1024 * 1024,
            invocation_timeout: Duration::from_secs(10),
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
        if let Some(component) = self
            .components
            .lock()
            .expect("component cache is not poisoned")
            .get(&key)
        {
            metrics::counter!("catalog_extension_component_cache_total", "outcome" => "hit")
                .increment(1);
            return Ok(component);
        }
        let bytes = self
            .object_store
            .get(&installed_artifact_key(
                key.installed_release_id,
                &key.artifact_id,
            ))
            .await?
            .bytes;
        let component = Arc::new(Component::new(&self.engine, &bytes).map_err(|error| {
            ExtensionRuntimeError::Runtime(format!("invalid component: {error}"))
        })?);
        let component = self
            .components
            .lock()
            .expect("component cache is not poisoned")
            .insert(key, component);
        metrics::counter!("catalog_extension_component_cache_total", "outcome" => "miss")
            .increment(1);
        Ok(component)
    }

    /// Invokes a v1.1 component command after its HTTP broker has resolved a
    /// locked enabled-release snapshot.
    /// Executes one bounded v1.2 operation batch. The component is selected by
    /// the run's immutable release snapshot; no current-installation lookup can
    /// substitute upgraded code.
    async fn invoke_operation_batch(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        run_id: uuid::Uuid,
        operation_handler: &str,
        configuration: &Value,
        input: &Value,
        checkpoint: &Value,
        batch_key: &str,
        max_checkpoint_bytes: u64,
        lifecycle_started: bool,
        cancelling: bool,
    ) -> Result<(Value, Value, bool), ExtensionRuntimeError> {
        let component = self.component(installation).await?;
        let request = host_operations::exports::catalog::host::operations::OperationRequest {
            run_id: run_id.to_string(),
            operation_id: operation_handler.to_owned(),
            configuration: serde_json::to_string(configuration)
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?,
            input: serde_json::to_string(input)
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?,
            checkpoint: serde_json::to_string(checkpoint)
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?,
            batch_key: batch_key.to_owned(),
        };
        let mut store = Store::new(
            &self.engine,
            OperationState::new(
                self.config.max_memory_bytes,
                installation.clone(),
                repository.clone(),
                self.object_store.clone(),
                run_id,
            ),
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.config.fuel)
            .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?;
        store.set_epoch_deadline(
            (self.config.invocation_timeout.as_millis() / EPOCH_TICK_INTERVAL.as_millis()).max(1)
                as u64,
        );
        let mut linker = Linker::new(&self.engine);
        host_operations::CatalogExtensionOperation::add_to_linker::<
            OperationState,
            HasSelf<OperationState>,
        >(&mut linker, |state| state)
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let bindings = host_operations::CatalogExtensionOperation::instantiate_async(
            &mut store, &component, &linker,
        )
        .await
        .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?;
        let operations = bindings.catalog_host_operations();
        if cancelling {
            operations
                .call_cancel(&mut store, &request)
                .await
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
                .map_err(ExtensionRuntimeError::Runtime)?;
            return Ok((checkpoint.clone(), json!({"cancelled":true}), true));
        }
        if !lifecycle_started {
            operations
                .call_prepare(&mut store, &request)
                .await
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
                .map_err(ExtensionRuntimeError::Runtime)?;
            operations
                .call_start(&mut store, &request)
                .await
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
                .map_err(ExtensionRuntimeError::Runtime)?;
        }
        let result = operations
            .call_process_batch(&mut store, &request)
            .await
            .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
            .map_err(ExtensionRuntimeError::Runtime)?;
        let next: Value = serde_json::from_str(&result.checkpoint).map_err(|_| {
            ExtensionRuntimeError::Runtime("operation returned invalid checkpoint".into())
        })?;
        let progress: Value = serde_json::from_str(&result.progress).map_err(|_| {
            ExtensionRuntimeError::Runtime("operation returned invalid progress".into())
        })?;
        if !next.is_object()
            || !progress.is_object()
            || serde_json::to_vec(&next).map_or(true, |v| v.len() > MAX_HOST_JSON_BYTES)
            || serde_json::to_vec(&next).map_or(true, |v| v.len() > max_checkpoint_bytes as usize)
            || serde_json::to_vec(&progress).map_or(true, |v| v.len() > MAX_HOST_JSON_BYTES)
        {
            return Err(ExtensionRuntimeError::Runtime(
                "operation returned oversized or non-object checkpoint".into(),
            ));
        }
        let checkpoint_request =
            host_operations::exports::catalog::host::operations::OperationRequest {
                checkpoint: result.checkpoint,
                ..request
            };
        operations
            .call_checkpoint(&mut store, &checkpoint_request)
            .await
            .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
            .map_err(ExtensionRuntimeError::Runtime)?;
        if result.done {
            operations
                .call_finish(&mut store, &checkpoint_request)
                .await
                .map_err(|e| ExtensionRuntimeError::Runtime(e.to_string()))?
                .map_err(ExtensionRuntimeError::Runtime)?;
        }
        Ok((next, progress, result.done))
    }

    pub async fn invoke_command(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &str,
        request: &str,
        max_response_bytes: u64,
    ) -> Result<String, ExtensionRuntimeError> {
        let component = self.component(installation).await?;
        let state = HostState::new(
            installation.clone(),
            repository,
            self.config.max_memory_bytes,
        );
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.config.fuel)
            .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));
        let mut linker = Linker::new(&self.engine);
        host_v11::CatalogExtension::add_to_linker::<HostState, HasSelf<HostState>>(
            &mut linker,
            |state| state,
        )
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let instance =
            host_v11::CatalogExtension::instantiate_async(&mut store, &component, &linker)
                .await
                .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let command = host_v11::exports::catalog::host::handler::CommandRequest {
            handler: handler.to_owned(),
            payload: request.to_owned(),
        };
        let result = instance
            .catalog_host_handler()
            .call_handle_command(&mut store, &command)
            .await;
        match result {
            Ok(Ok(response))
                if response.payload.len() <= MAX_HOST_JSON_BYTES
                    && response.payload.len() <= max_response_bytes as usize =>
            {
                Ok(response.payload)
            }
            Ok(Ok(_)) => Err(ExtensionRuntimeError::Runtime(
                "command response exceeds its declared byte limit".into(),
            )),
            Ok(Err(error)) => Err(ExtensionRuntimeError::Runtime(error)),
            Err(error) => Err(ExtensionRuntimeError::Runtime(error.to_string())),
        }
    }

    async fn invoke_v11_event(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
    ) -> Result<(), ExtensionRuntimeError> {
        let component = self.component(installation).await?;
        let state = HostState::new(
            installation.clone(),
            repository,
            self.config.max_memory_bytes,
        );
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.config.fuel)
            .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));
        let mut linker = Linker::new(&self.engine);
        host_v11::CatalogExtension::add_to_linker::<HostState, HasSelf<HostState>>(
            &mut linker,
            |state| state,
        )
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let instance =
            host_v11::CatalogExtension::instantiate_async(&mut store, &component, &linker)
                .await
                .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let result = instance
            .catalog_host_handler()
            .call_handle_event(&mut store, &to_wit_v11_event(event))
            .await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(ExtensionRuntimeError::Runtime(format!(
                "handler '{}' failed: {message}",
                handler.id
            ))),
            Err(error) => Err(ExtensionRuntimeError::Runtime(error.to_string())),
        }
    }

    async fn invoke(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
    ) -> Result<(), ExtensionRuntimeError> {
        if uses_v11(&installation.manifest.catalog.host_api) {
            return self
                .invoke_v11_event(installation, repository, handler, event)
                .await;
        }
        let component = self.component(installation).await?;

        let state = HostState::new(
            installation.clone(),
            repository,
            self.config.max_memory_bytes,
        );
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.config.fuel)
            .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));

        let mut linker = Linker::new(&self.engine);
        CatalogExtension::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state| {
            state
        })
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let instance = CatalogExtension::instantiate_async(&mut store, &component, &linker)
            .await
            .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
        let result = instance
            .catalog_host_handler()
            .call_handle_event(&mut store, &to_wit_event(event))
            .await;
        match result {
            Ok(Ok(())) => {
                metrics::counter!("catalog_extension_invocations_total", "outcome" => "completed")
                    .increment(1);
                Ok(())
            }
            Ok(Err(message)) => Err(ExtensionRuntimeError::Runtime(format!(
                "handler '{}' failed: {message}",
                handler.id
            ))),
            Err(error) => Err(ExtensionRuntimeError::Runtime(error.to_string())),
        }
    }
}

fn epoch_deadline(timeout: Duration) -> u64 {
    timeout
        .as_millis()
        .div_ceil(EPOCH_TICK_INTERVAL.as_millis())
        .max(1) as u64
}

fn uses_v11(range: &str) -> bool {
    let Ok(range) = VersionReq::parse(range) else {
        return false;
    };
    range.matches(&Version::new(1, 1, 0)) && !range.matches(&Version::new(1, 0, 0))
}

fn to_wit_v11_event(event: &DomainEvent) -> host_v11::catalog::host::api::Event {
    host_v11::catalog::host::api::Event {
        id: event.id.to_string(),
        event_type: event.event_type.clone(),
        aggregate_kind: event.aggregate_kind.clone(),
        aggregate_id: event.aggregate_id.to_string(),
        correlation_id: event.correlation_id.to_string(),
        causation_id: event.causation_id.map(|id| id.to_string()),
        payload: event.payload.to_string(),
    }
}

fn to_wit_event(event: &DomainEvent) -> catalog::host::api::Event {
    catalog::host::api::Event {
        id: event.id.to_string(),
        event_type: event.event_type.clone(),
        aggregate_kind: event.aggregate_kind.clone(),
        aggregate_id: event.aggregate_id.to_string(),
        correlation_id: event.correlation_id.to_string(),
        causation_id: event.causation_id.map(|id| id.to_string()),
        payload: event.payload.to_string(),
    }
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
    limits: StoreLimits,
    installation: ExtensionRuntimeInstallation,
    repository: CatalogRepository,
    object_store: Arc<dyn ObjectStore>,
    run_id: Uuid,
    artifacts: ResourceTable,
}

impl OperationState {
    fn new(
        max_memory_bytes: usize,
        installation: ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        object_store: Arc<dyn ObjectStore>,
        run_id: Uuid,
    ) -> Self {
        Self {
            limits: StoreLimitsBuilder::new()
                .memory_size(max_memory_bytes)
                .build(),
            installation,
            repository,
            object_store,
            run_id,
            artifacts: ResourceTable::new(),
        }
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
    ) -> Result<(CatalogRepository, String, Uuid), String> {
        self.require(capability)?;
        Ok((
            self.repository.clone(),
            self.installation.extension_id.clone(),
            self.installation.installed_release_id,
        ))
    }
}

async fn ensure_operation_artifact_access(
    repository: CatalogRepository,
    extension_id: String,
    release_id: Uuid,
) -> Result<(), String> {
    repository
        .runtime_extension_installation(&extension_id, release_id)
        .await
        .map_err(|_| "extension authorization could not be checked")?
        .ok_or_else(|| "extension invocation is no longer authorized".to_owned())?;
    Ok(())
}

#[derive(Clone)]
struct HostState {
    installation: ExtensionRuntimeInstallation,
    repository: CatalogRepository,
    limits: StoreLimits,
}

impl HostState {
    fn new(
        installation: ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        max_memory_bytes: usize,
    ) -> Self {
        Self {
            installation,
            repository,
            limits: StoreLimitsBuilder::new()
                .memory_size(max_memory_bytes)
                .build(),
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

impl catalog::host::api::Host for HostState {
    async fn call(&mut self, operation: String, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let required = match operation.as_str() {
            "configuration.get.v1" => "configuration.read",
            "secrets.get.v1" => "secrets.read",
            "storage.get.v1" | "storage.set.v1" | "storage.put.v1" | "storage.delete.v1"
            | "storage.list.v1" => "storage.extension",
            "catalog.read.v1" => "catalog.read",
            "catalog.command.v1" => "catalog.write",
            "events.emit.v1" => "events.emit",
            "network.request.v1" => "network.request",
            _ => return Err("unknown host operation".into()),
        };
        if let Err(error) = self.require(required) {
            metrics::counter!("catalog_extension_host_calls_total", "outcome" => "denied", "operation" => operation).increment(1);
            return Err(error.to_string());
        }
        if self
            .repository
            .runtime_extension_installation(
                &self.installation.extension_id,
                self.installation.installed_release_id,
            )
            .await
            .map_err(|_| "extension authorization could not be checked")?
            .is_none()
        {
            return Err("extension invocation is no longer authorized".into());
        }
        if operation == "configuration.get.v1" {
            return Ok(self.installation.configuration.to_string());
        }
        if operation == "events.emit.v1" {
            let input: EventEmit = parse_event_emit_request(&request)?;
            self.repository
                .emit_extension_event(
                    &self.installation.extension_id,
                    self.installation.installed_release_id,
                    &input.contract_id,
                    &input.aggregate_kind,
                    parse_uuid(&input.aggregate_id, "event aggregate ID")?,
                    input.payload,
                )
                .await
                .map_err(|error| error.to_string())?;
            return Ok("null".to_owned());
        }
        if operation == "catalog.read.v1" {
            return self.catalog_read_call(&request).await;
        }
        if operation == "catalog.command.v1" {
            return self.catalog_command_call(&request).await;
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

impl host_v11::catalog::host::api::Host for HostState {
    async fn read(
        &mut self,
        request: host_v11::catalog::host::api::ReadRequest,
    ) -> Result<host_v11::catalog::host::api::ReadResponse, String> {
        self.require_active("catalog.read").await?;
        let (entity_id, context_id) = match request {
            host_v11::catalog::host::api::ReadRequest::Entity(input)
            | host_v11::catalog::host::api::ReadRequest::Values(input) => {
                (parse_uuid(&input.entity_id, "entity ID")?, None)
            }
            host_v11::catalog::host::api::ReadRequest::Resolved(input) => (
                parse_uuid(&input.entity_id, "entity ID")?,
                Some(parse_uuid(&input.context_id, "context ID")?),
            ),
        };
        let entity = self
            .repository
            .get_entity(entity_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "entity not found".to_owned())?;
        let blueprint = self
            .repository
            .get_blueprint_revision(entity.blueprint_id, entity.blueprint_version)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "blueprint revision not found".to_owned())?;
        let direct_values = self
            .repository
            .current_values(entity_id)
            .await
            .map_err(|error| error.to_string())?;
        let resolved_values = match context_id {
            Some(context_id) => Some(
                self.repository
                    .resolved_preview(entity_id, context_id, 0)
                    .await
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "entity not found".to_owned())?,
            ),
            None => None,
        };
        Ok(host_v11::catalog::host::api::ReadResponse {
            entity: bounded_serialize(&entity)?,
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
        request: host_v11::catalog::host::api::WriteRequest,
    ) -> Result<host_v11::catalog::host::api::WriteResponse, String> {
        self.require_active("catalog.write").await?;
        if request.values.is_empty() || request.values.len() > MAX_WRITE_VALUES {
            return Err("writes require 1-100 scalar values".into());
        }
        let entity_id = parse_uuid(&request.entity_id, "entity ID")?;
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
        let values = CatalogMutationService::new(&repository)
            .append_values(entity_id, AppendAttributeValues { values })
            .await
            .map_err(|error| error.to_string())?;
        Ok(host_v11::catalog::host::api::WriteResponse {
            values: bounded_serialize(&values)?,
        })
    }

    async fn scoped_configuration_get(
        &mut self,
        scope: host_v11::catalog::host::api::ConfigurationScope,
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
        request: host_v11::catalog::host::api::ScopedConfigurationUpdate,
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

    async fn log(&mut self, level: String, message: String) -> Result<(), String> {
        <Self as catalog::host::api::Host>::log(self, level, message).await
    }
}

impl HostState {
    async fn require_active(&self, capability: &str) -> Result<(), String> {
        self.require(capability)
            .map_err(|error| error.to_string())?;
        self.repository
            .runtime_extension_installation(
                &self.installation.extension_id,
                self.installation.installed_release_id,
            )
            .await
            .map_err(|_| "extension authorization could not be checked")?
            .ok_or_else(|| "extension invocation is no longer authorized".to_owned())?;
        Ok(())
    }

    /// The v1 JSON host call is retained for older components. Unlike generic
    /// search it exposes only the two bounded extension-data operations below.
    async fn catalog_read_call(&self, request: &str) -> Result<String, String> {
        let input: CatalogReadRequest = parse_storage_request(request)?;
        match input {
            CatalogReadRequest::Page {
                blueprint_id,
                blueprint_version,
                context_id,
                publication_context_id,
                cursor,
                limit,
            } => {
                let page = self
                    .repository
                    .extension_catalog_page(ExtensionCatalogPageRequest {
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
            CatalogReadRequest::Changes {
                blueprint_id,
                blueprint_version,
                cursor,
                limit,
            } => {
                let page = self
                    .repository
                    .extension_catalog_changes(
                        parse_uuid(&blueprint_id, "blueprint ID")?,
                        blueprint_version,
                        cursor,
                        limit,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&page)
            }
            CatalogReadRequest::Lookup {
                blueprint_id,
                blueprint_version,
                attribute_id,
                value,
            } => {
                let entity = self
                    .repository
                    .extension_catalog_lookup(
                        parse_uuid(&blueprint_id, "blueprint ID")?,
                        blueprint_version,
                        parse_uuid(&attribute_id, "attribute ID")?,
                        &value,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&entity)
            }
        }
    }

    async fn catalog_command_call(&self, request: &str) -> Result<String, String> {
        let input: CatalogCommandRequest = parse_storage_request(request)?;
        let CatalogCommandRequest::Batch { batch } = input;
        let repository = self
            .repository
            .for_extension(&self.installation.extension_id);
        let outcomes = CatalogMutationService::new(&repository)
            .execute_extension_catalog_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
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
    scope: host_v11::catalog::host::api::ConfigurationScope,
) -> Result<ExtensionConfigurationScope, String> {
    let kind = match scope.kind {
        host_v11::catalog::host::api::ConfigurationScopeKind::Blueprint => {
            crate::extensions::ConfigurationScope::Blueprint
        }
        host_v11::catalog::host::api::ConfigurationScopeKind::Attribute => {
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
enum CatalogReadRequest {
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
enum CatalogCommandRequest {
    Batch {
        batch: crate::repository::ExtensionCatalogBatch,
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

/// Shared-task executor for enabled extension event handlers. A component can
/// finish before its task acknowledgement, so extensions remain at-least-once
/// and must stay idempotent.
/// Shared-task executor for release-pinned checkpointed operations. The
/// checkpoint is committed under the task lease only after WASM returns; a
/// crash before that transaction repeats the batch, so operation components
/// must make each batch idempotent using the durable checkpoint.
pub struct ExtensionOperationTaskHandler {
    repository: CatalogRepository,
    runtime: ExtensionRuntime,
}
impl ExtensionOperationTaskHandler {
    pub fn new(repository: CatalogRepository, runtime: ExtensionRuntime) -> Self {
        Self {
            repository,
            runtime,
        }
    }
}
#[async_trait]
impl TaskHandler for ExtensionOperationTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::ExtensionOperationRunV1
    }
    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let repository = self
            .repository
            .for_workspace(task.workspace_id)
            .await
            .map_err(|e| TaskHandlerError {
                code: "workspace",
                message: e.to_string(),
            })?;
        // Abandoned streams are temporary only. Completed artifacts are never
        // selected by this cleanup path and therefore remain immutable.
        for key in repository
            .abort_stale_extension_operation_artifacts()
            .await
            .map_err(|error| TaskHandlerError {
                code: "operation",
                message: error.to_string(),
            })?
        {
            let _ = self.runtime.object_store.delete(&key).await;
        }
        let Some(run) = repository
            .begin_extension_operation_task(&task)
            .await
            .map_err(|e| TaskHandlerError {
                code: "operation",
                message: e.to_string(),
            })?
        else {
            return Ok(TaskOutcome::Complete);
        };
        let Some(installation) = repository
            .runtime_extension_installation(&run.extension_id, run.installed_release_id)
            .await
            .map_err(|e| TaskHandlerError {
                code: "operation",
                message: e.to_string(),
            })?
        else {
            // Disable, upgrade, grant change, and quarantine never switch an
            // in-flight run to a different release. It remains resumable only
            // when its exact pinned release becomes authorized again.
            repository
                .pause_extension_operation_task(&task)
                .await
                .map_err(|error| TaskHandlerError {
                    code: "operation",
                    message: error.to_string(),
                })?;
            return Ok(TaskOutcome::Reschedule {
                at: chrono::Utc::now() + chrono::Duration::seconds(30),
            });
        };
        let cancellation_requested = repository
            .extension_operation_cancellation_requested(&task)
            .await
            .map_err(|error| TaskHandlerError {
                code: "operation",
                message: error.to_string(),
            })?;
        let (checkpoint, progress, done) = match self
            .runtime
            .invoke_operation_batch(
                &installation,
                repository.clone(),
                run.id,
                &run.operation_handler,
                &run.configuration,
                &run.input,
                &run.checkpoint,
                &run.batch_key,
                run.max_checkpoint_bytes,
                run.lifecycle_started,
                cancellation_requested,
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                repository
                    .fail_extension_operation_task(&task, &error.to_string())
                    .await
                    .map_err(|failure| TaskHandlerError {
                        code: "operation",
                        message: failure.to_string(),
                    })?;
                return Ok(TaskOutcome::DeadLettered);
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
                .map_err(|error| TaskHandlerError {
                    code: "operation",
                    message: error.to_string(),
                })?
        };
        if cancellation_delivered {
            if !cancellation_requested {
                self.runtime
                    .invoke_operation_batch(
                        &installation,
                        repository.clone(),
                        run.id,
                        &run.operation_handler,
                        &run.configuration,
                        &run.input,
                        &run.checkpoint,
                        &run.batch_key,
                        run.max_checkpoint_bytes,
                        run.lifecycle_started,
                        true,
                    )
                    .await
                    .map_err(|error| TaskHandlerError {
                        code: "operation",
                        message: error.to_string(),
                    })?;
            }
            repository
                .mark_extension_operation_cancellation_delivered(&task)
                .await
                .map_err(|error| TaskHandlerError {
                    code: "operation",
                    message: error.to_string(),
                })?;
        }
        let terminal = repository
            .checkpoint_extension_operation_task(&task, &run, checkpoint, progress, done)
            .await
            .map_err(|e| TaskHandlerError {
                code: "operation",
                message: e.to_string(),
            })?;
        if terminal {
            Ok(TaskOutcome::Complete)
        } else {
            Ok(TaskOutcome::Reschedule {
                at: chrono::Utc::now(),
            })
        }
    }
}

pub struct WasmExtensionTaskHandler {
    repository: CatalogRepository,
    runtime: ExtensionRuntime,
}

impl WasmExtensionTaskHandler {
    pub fn new(repository: CatalogRepository, runtime: ExtensionRuntime) -> Self {
        Self {
            repository,
            runtime,
        }
    }

    async fn invoke_event(
        &self,
        event: DomainEvent,
        repository: CatalogRepository,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let installations = repository.enabled_extension_handlers(&event).await?;
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
                if let Err(error) = self
                    .runtime
                    .invoke(
                        &installation,
                        repository.for_extension(&installation.extension_id),
                        handler,
                        &event,
                    )
                    .await
                {
                    metrics::counter!("catalog_extension_invocations_total", "outcome" => "failed")
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
                        error_debug = ?error,
                        "extension handler failed; quarantining extension"
                    );
                    repository
                        .quarantine_extension(&installation.extension_id, "runtime_failure")
                        .await?;
                    return Err(Box::new(error));
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TaskHandler for WasmExtensionTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::EventDeliveryV1
    }

    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let repository = self
            .repository
            .for_workspace(task.workspace_id)
            .await
            .map_err(|error| TaskHandlerError {
                code: "workspace",
                message: error.to_string(),
            })?;
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
            repository.for_event_delivery_task(&delivery.event, "catalog.extensions.wasm", &task);
        self.invoke_event(delivery.event, context)
            .await
            .map_err(|error| TaskHandlerError {
                code: "extension",
                message: error.to_string(),
            })?;
        repository
            .complete_task_event_delivery(&task)
            .await
            .map_err(|error| TaskHandlerError {
                code: "receipt",
                message: error.to_string(),
            })?;
        Ok(TaskOutcome::Complete)
    }
}

/// Intake is only a producer/coordinator. It registers at the high-water mark
/// and atomically creates delivery/task pairs for subsequently supported event
/// versions; execution is exclusively owned by `WasmExtensionTaskHandler`.
pub fn start_event_delivery_coordinator(
    repository: CatalogRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            for workspace_id in repository.active_workspace_ids().await.unwrap_or_default() {
                let result = async {
                    let scoped = repository.for_workspace(workspace_id).await?;
                    // Incomplete operation output has no committed object key;
                    // this periodic lifecycle sweep makes interrupted streams
                    // unavailable even when no subsequent operation task runs.
                    scoped.abort_stale_extension_operation_artifacts().await?;
                    scoped
                        .ensure_event_consumer("catalog.extensions.wasm", &[])
                        .await?;
                    let event_types = scoped.enabled_extension_event_types().await?;
                    scoped
                        .materialize_event_delivery_tasks("catalog.extensions.wasm", &event_types)
                        .await
                }
                .await;
                if let Err(error) = result {
                    tracing::error!(%error, "extension event-delivery intake failed");
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(250)) => {},
                _ = shutdown.changed() => return,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::{
        MAX_ARTIFACT_CHUNK_BYTES, host_v11, parse_bounded_json, to_configuration_scope, uses_v11,
    };

    #[test]
    fn selects_an_explicit_abi_without_upgrading_v1_ranges() {
        assert!(!uses_v11("^1.0"));
        assert!(uses_v11(">=1.1.0, <2.0.0"));
        assert!(!uses_v11(">=1.0.0, <2.0.0"));
    }

    #[test]
    fn artifact_stream_contract_keeps_chunks_and_checksums_bounded() {
        assert_eq!(MAX_ARTIFACT_CHUNK_BYTES, 64 * 1024);
        assert!(MAX_ARTIFACT_CHUNK_BYTES <= super::MAX_HOST_JSON_BYTES);
        assert_eq!(
            format!("{:x}", Sha256::digest(b"bounded output")),
            "d047501029296dac4c1be5e22f05ff7229244184357ac63424d75339009a77e3"
        );
    }

    #[test]
    fn typed_v11_inputs_reject_oversized_json_and_invalid_scopes() {
        assert!(parse_bounded_json(&"x".repeat(65_537), "value").is_err());
        assert!(parse_bounded_json("{", "value").is_err());
        assert!(
            to_configuration_scope(host_v11::catalog::host::api::ConfigurationScope {
                kind: host_v11::catalog::host::api::ConfigurationScopeKind::Attribute,
                blueprint_id: "not-a-uuid".into(),
                blueprint_version: 1,
                attribute_id: Some("also-not-a-uuid".into()),
            })
            .is_err()
        );
    }
}

impl host_operations::catalog::host::artifacts::Host for OperationState {
    async fn open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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

    async fn describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<host_operations::catalog::host::artifacts::InputMetadata, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
        let artifact = self
            .artifacts
            .get(&handle)
            .map_err(|_| "invalid artifact handle".to_owned())?;
        Ok(host_operations::catalog::host::artifacts::InputMetadata {
            content_length: artifact.artifact.content_length as u64,
            media_type: artifact.artifact.media_type.clone(),
            checksum_sha256: artifact
                .artifact
                .checksum_sha256
                .clone()
                .unwrap_or_default(),
        })
    }

    async fn read(
        &mut self,
        handle: Resource<InputArtifactStream>,
        max_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.read")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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

    async fn create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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
        let path = std::env::temp_dir().join(format!("catalog-operation-artifact-{}", artifact.id));
        File::create(&path).map_err(|_| "temporary artifact storage is unavailable".to_owned())?;
        self.artifacts
            .push(OutputArtifactStream {
                artifact,
                path,
                hasher: Sha256::new(),
            })
            .map_err(|_| "artifact handle limit reached".to_owned())
    }

    async fn write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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

    async fn complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<host_operations::catalog::host::artifacts::OutputMetadata, String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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
        let key = format!("extension-operation-artifacts/{}", output.artifact.id);
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
        Ok(host_operations::catalog::host::artifacts::OutputMetadata {
            artifact_id: artifact.id.to_string(),
            content_length: artifact.content_length as u64,
            media_type: artifact.media_type,
            checksum_sha256: actual,
        })
    }

    async fn abort(&mut self, handle: Resource<OutputArtifactStream>) -> Result<(), String> {
        let (repository, extension_id, release_id) = self.artifact_access("artifacts.write")?;
        ensure_operation_artifact_access(repository, extension_id, release_id).await?;
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

impl host_operations::catalog::host::artifacts::HostInputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<InputArtifactStream>) -> wasmtime::Result<()> {
        self.artifacts.delete(handle)?;
        Ok(())
    }
}

impl host_operations::catalog::host::artifacts::HostOutputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<OutputArtifactStream>) -> wasmtime::Result<()> {
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
