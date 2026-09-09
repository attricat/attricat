//! Sandboxed execution of installed server extension components.
//!
//! Components get no WASI context, filesystem, environment, clocks, sockets, or
//! pre-opened descriptors. The only imports are the versioned WIT functions in
//! `wit/catalog-extension.wit`; every call is checked against the immutable
//! release manifest and invocation-time grants.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use semver::{Version, VersionReq};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, HasSelf, Linker},
};

use crate::{
    catalog_service::CatalogMutationService,
    constants::DEFAULT_LIST_PAGE_SIZE,
    domain_events::DomainEvent,
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    extension_installer::installed_artifact_key,
    extensions::{
        ArtifactKind, EventHandler as ManifestEventHandler, MAX_EXTENSION_IDENTIFIER_BYTES,
    },
    model::{AppendAttributeValues, NewAttributeValue},
    repository::{CatalogRepository, ExtensionConfigurationScope, ExtensionRuntimeInstallation},
    storage::{ObjectStore, ObjectStoreError},
};

wasmtime::component::bindgen!({
    path: "wit",
    world: "catalog-extension",
    imports: { default: async },
    exports: { default: async },
});
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

#[derive(Clone)]
pub struct ExtensionRuntime {
    object_store: Arc<dyn ObjectStore>,
    config: ExtensionRuntimeConfig,
    engine: Arc<Engine>,
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
        Ok(Self {
            object_store,
            config,
            engine: Arc::new(engine),
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
        store.set_epoch_deadline(1);
        let timeout_engine = self.engine.clone();
        let timeout = self.config.invocation_timeout;
        let epoch = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            timeout_engine.increment_epoch();
        });
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
        epoch.abort();
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
        store.set_epoch_deadline(1);
        let timeout_engine = self.engine.clone();
        let timeout = self.config.invocation_timeout;
        let epoch = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            timeout_engine.increment_epoch();
        });
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
        epoch.abort();
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
        store.set_epoch_deadline(1);
        let timeout_engine = self.engine.clone();
        let timeout = self.config.invocation_timeout;
        let epoch = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            timeout_engine.increment_epoch();
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
            .enabled_extension_handlers(&event)
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
                    .invoke(
                        &installation,
                        context
                            .repository()
                            .for_extension(&installation.extension_id),
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

#[cfg(test)]
mod tests {
    use super::{host_v11, parse_bounded_json, to_configuration_scope, uses_v11};

    #[test]
    fn selects_an_explicit_abi_without_upgrading_v1_ranges() {
        assert!(!uses_v11("^1.0"));
        assert!(uses_v11(">=1.1.0, <2.0.0"));
        assert!(!uses_v11(">=1.0.0, <2.0.0"));
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
