//! Sandboxed execution of installed server extension components.
//!
//! Components get no WASI context, filesystem, environment, clocks, sockets, or
//! pre-opened descriptors. The only imports are the versioned WIT functions in
//! `wit/catalog-extension.wit`; every call is checked against the immutable
//! release manifest and invocation-time grants.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, HasSelf, Linker},
};

use crate::{
    domain_events::DomainEvent,
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    extension_installer::installed_artifact_key,
    extensions::{ArtifactKind, EventHandler as ManifestEventHandler},
    repository::{CatalogRepository, ExtensionRuntimeInstallation},
    storage::{ObjectStore, ObjectStoreError},
};

wasmtime::component::bindgen!({
    path: "wit",
    world: "catalog-extension",
    imports: { default: async },
    exports: { default: async },
});

const MAX_HOST_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_HOST_JSON_BYTES: usize = 64 * 1024;

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

#[derive(Clone)]
pub struct ExtensionRuntime {
    engine: Engine,
    object_store: Arc<dyn ObjectStore>,
    config: ExtensionRuntimeConfig,
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
        Ok(Self {
            engine,
            object_store,
            config,
        })
    }

    async fn invoke(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
    ) -> Result<(), ExtensionRuntimeError> {
        let artifact = installation
            .manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == ArtifactKind::ServerWasm)
            .ok_or_else(|| {
                ExtensionRuntimeError::Runtime("release has no server_wasm artifact".into())
            })?;
        let key = installed_artifact_key(installation.installed_release_id, &artifact.id);
        let bytes = self.object_store.get(&key).await?.bytes;
        let component = Component::new(&self.engine, &bytes).map_err(|error| {
            ExtensionRuntimeError::Runtime(format!("invalid component: {error}"))
        })?;

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
        store.set_epoch_deadline(1);
        let engine = self.engine.clone();
        let timeout = self.config.invocation_timeout;
        let epoch = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            engine.increment_epoch();
        });

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
        epoch.abort();
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
        if operation == "configuration.get.v1" {
            return Ok(self.installation.configuration.to_string());
        }
        self.storage_call(&operation, &request).await
    }

    async fn log(&mut self, level: String, message: String) -> Result<(), String> {
        if let Err(error) = self.require("logging.write") {
            return Err(error.to_string());
        }
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
                        input.limit.unwrap_or(50),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(json!({"entries": page.entries.into_iter().map(|entry| json!({"key": entry.key, "value": entry.value, "revision": entry.revision})).collect::<Vec<_>>(), "cursor": page.cursor}).to_string())
            }
            _ => Err("host operation is not enabled by this deployment".into()),
        }
    }
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
fn parse_storage_request<T: for<'de> Deserialize<'de>>(request: &str) -> Result<T, String> {
    serde_json::from_str(request).map_err(|_| "invalid storage request".to_owned())
}

/// One durable dispatcher consumer runs enabled extension handlers. Individual
/// handlers must be idempotent because a component can finish before delivery
/// acknowledgement, and a failure retries the entire event fan-out.
pub struct WasmExtensionHandler {
    runtime: ExtensionRuntime,
}

impl WasmExtensionHandler {
    pub fn new(runtime: ExtensionRuntime) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl EventHandler for WasmExtensionHandler {
    fn name(&self) -> &'static str {
        "catalog.extensions.wasm"
    }

    fn event_types(&self) -> &'static [&'static str] {
        crate::domain_events::ALL_EVENT_TYPES_V1
    }

    async fn handle(
        &self,
        event: DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let installations = context
            .repository()
            .enabled_extension_handlers(&event.event_type)
            .await?;
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
                let Some(installation) = context
                    .repository()
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
                    .invoke(&installation, context.repository().clone(), handler, &event)
                    .await
                {
                    metrics::counter!("catalog_extension_invocations_total", "outcome" => "failed")
                        .increment(1);
                    tracing::warn!(extension = %installation.extension_id, handler = %handler.id, event_id = %event.id, %error, "extension handler failed");
                    context
                        .repository()
                        .quarantine_extension(&installation.extension_id, "runtime_failure")
                        .await?;
                    return Err(Box::new(error));
                }
            }
        }
        Ok(())
    }
}

pub fn registry_with_wasm(
    runtime: ExtensionRuntime,
) -> crate::event_dispatcher::EventHandlerRegistry {
    crate::event_dispatcher::EventHandlerRegistry::default_handlers()
        .with_handler(Arc::new(WasmExtensionHandler::new(runtime)))
        .expect("WASM extension handler is valid")
}
