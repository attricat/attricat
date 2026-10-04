//! Host implementation of the unified, evolving `catalog:host@1.6.0` ABI.
//!
//! 1.6 merges the released 1.1 event/command world and the released 1.5
//! operation world into one package. Every import is linked for every
//! invocation; the host instantiates the component once and loads only the
//! export the invocation needs, so a component may target the combined
//! `catalog-extension` world or the narrower `handler-extension` or
//! `operation-extension` world.
//!
//! The behavior of each import is the released behavior it came from. Run-bound
//! interfaces delegate to the 1.5 operation implementation (including the
//! interactive selection confinement) and fail outside an operation run.
//! Direct catalog access through `api` delegates to the 1.1 implementation and
//! fails inside an operation run, so a run can never bypass its scope.
//!
//! Export lookup is semver-aware in wasmtime, so components built against an
//! earlier unified release keep working when this host binds a later,
//! additive one.

use super::interactive::host_interactive::catalog::host as v15;
use super::*;
use host_v11::catalog::host as v11;

pub(super) mod host_unified {
    wasmtime::component::bindgen!({
        path: "wit-host",
        world: "catalog-extension",
        with: {
            "catalog:host/artifacts.input-artifact": crate::extension_runtime::InputArtifactStream,
            "catalog:host/artifacts.output-artifact": crate::extension_runtime::OutputArtifactStream,
        },
        imports: { default: async },
        exports: { default: async },
    });
}

use host_unified::catalog::host as v16;
use host_unified::exports::catalog::host::{handler as v16_handler, operations as v16_operations};

const RUN_ONLY_ERROR: &str = "this interface is available only during an operation run";
const RUN_CATALOG_ERROR: &str =
    "operation runs access catalog data through their run-scoped interfaces";

pub(super) struct UnifiedState {
    host: HostState,
    operation: Option<OperationState>,
}

impl UnifiedState {
    fn run(&mut self) -> Result<&mut OperationState, String> {
        self.operation
            .as_mut()
            .ok_or_else(|| RUN_ONLY_ERROR.to_owned())
    }

    fn deny_in_run(&self) -> Result<(), String> {
        if self.operation.is_some() {
            Err(RUN_CATALOG_ERROR.into())
        } else {
            Ok(())
        }
    }
}

fn to_v11_scope(scope: v16::api::ConfigurationScope) -> v11::api::ConfigurationScope {
    v11::api::ConfigurationScope {
        kind: match scope.kind {
            v16::api::ConfigurationScopeKind::Blueprint => {
                v11::api::ConfigurationScopeKind::Blueprint
            }
            v16::api::ConfigurationScopeKind::Attribute => {
                v11::api::ConfigurationScopeKind::Attribute
            }
        },
        blueprint_id: scope.blueprint_id,
        blueprint_version: scope.blueprint_version,
        attribute_id: scope.attribute_id,
    }
}

impl v16::api::Host for UnifiedState {
    async fn read(
        &mut self,
        request: v16::api::ReadRequest,
    ) -> Result<v16::api::ReadResponse, String> {
        self.deny_in_run()?;
        let request = match request {
            v16::api::ReadRequest::Entity(input) => {
                v11::api::ReadRequest::Entity(v11::api::EntityReference {
                    entity_id: input.entity_id,
                })
            }
            v16::api::ReadRequest::Values(input) => {
                v11::api::ReadRequest::Values(v11::api::EntityReference {
                    entity_id: input.entity_id,
                })
            }
            v16::api::ReadRequest::Resolved(input) => {
                v11::api::ReadRequest::Resolved(v11::api::ResolvedRead {
                    entity_id: input.entity_id,
                    context_id: input.context_id,
                })
            }
        };
        let response = <HostState as v11::api::Host>::read(&mut self.host, request).await?;
        Ok(v16::api::ReadResponse {
            entity: response.entity,
            blueprint: response.blueprint,
            direct_values: response.direct_values,
            resolved_values: response.resolved_values,
        })
    }

    async fn write(
        &mut self,
        request: v16::api::WriteRequest,
    ) -> Result<v16::api::WriteResponse, String> {
        self.deny_in_run()?;
        let request = v11::api::WriteRequest {
            entity_id: request.entity_id,
            values: request
                .values
                .into_iter()
                .map(|value| v11::api::ScalarWrite {
                    attribute_id: value.attribute_id,
                    attribute_code: value.attribute_code,
                    context_id: value.context_id,
                    value: value.value,
                })
                .collect(),
        };
        let response = <HostState as v11::api::Host>::write(&mut self.host, request).await?;
        Ok(v16::api::WriteResponse {
            values: response.values,
        })
    }

    async fn scoped_configuration_get(
        &mut self,
        scope: v16::api::ConfigurationScope,
    ) -> Result<Option<String>, String> {
        <HostState as v11::api::Host>::scoped_configuration_get(&mut self.host, to_v11_scope(scope))
            .await
    }

    async fn scoped_configuration_set(
        &mut self,
        request: v16::api::ScopedConfigurationUpdate,
    ) -> Result<(), String> {
        let request = v11::api::ScopedConfigurationUpdate {
            scope: to_v11_scope(request.scope),
            value: request.value,
        };
        <HostState as v11::api::Host>::scoped_configuration_set(&mut self.host, request).await
    }

    async fn call(&mut self, operation: String, request: String) -> Result<String, String> {
        if matches!(operation.as_str(), "catalog.read.v1" | "catalog.command.v1") {
            self.deny_in_run()?;
        }
        <HostState as v11::api::Host>::call(&mut self.host, operation, request).await
    }

    async fn log(&mut self, level: String, message: String) -> Result<(), String> {
        <HostState as v11::api::Host>::log(&mut self.host, level, message).await
    }
}

impl v16::selection::Host for UnifiedState {
    async fn describe(&mut self) -> Result<String, String> {
        <OperationState as v15::selection::Host>::describe(self.run()?).await
    }

    async fn page(&mut self, cursor: String, limit: u32) -> Result<String, String> {
        <OperationState as v15::selection::Host>::page(self.run()?, cursor, limit).await
    }
}

impl v16::catalog_data::Host for UnifiedState {
    async fn read(&mut self, request: String) -> Result<String, String> {
        <OperationState as v15::catalog_data::Host>::read(self.run()?, request).await
    }

    async fn batch(&mut self, request: String) -> Result<String, String> {
        <OperationState as v15::catalog_data::Host>::batch(self.run()?, request).await
    }
}

impl v16::catalog::Host for UnifiedState {
    async fn schema(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
    ) -> Result<String, String> {
        <OperationState as v15::catalog::Host>::schema(
            self.run()?,
            blueprint_id,
            blueprint_version,
            context_id,
        )
        .await
    }

    async fn page(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
        cursor: String,
        limit: u32,
    ) -> Result<String, String> {
        <OperationState as v15::catalog::Host>::page(
            self.run()?,
            blueprint_id,
            blueprint_version,
            context_id,
            cursor,
            limit,
        )
        .await
    }

    async fn upsert_batch(&mut self, request: String) -> Result<(), String> {
        <OperationState as v15::catalog::Host>::upsert_batch(self.run()?, request).await
    }
}

impl v16::transfer::Host for UnifiedState {
    async fn fetch_input(&mut self, request: String) -> Result<String, String> {
        <OperationState as v15::transfer::Host>::fetch_input(self.run()?, request).await
    }

    async fn deliver_output(&mut self, request: String) -> Result<String, String> {
        <OperationState as v15::transfer::Host>::deliver_output(self.run()?, request).await
    }
}

impl v16::artifacts::Host for UnifiedState {
    async fn open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        <OperationState as v15::artifacts::Host>::open_input(self.run()?, artifact_id).await
    }

    async fn describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<v16::artifacts::InputMetadata, String> {
        let metadata =
            <OperationState as v15::artifacts::Host>::describe_input(self.run()?, handle).await?;
        Ok(v16::artifacts::InputMetadata {
            content_length: metadata.content_length,
            media_type: metadata.media_type,
            checksum_sha256: metadata.checksum_sha256,
        })
    }

    async fn read(
        &mut self,
        handle: Resource<InputArtifactStream>,
        max_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        <OperationState as v15::artifacts::Host>::read(self.run()?, handle, max_bytes).await
    }

    async fn create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        <OperationState as v15::artifacts::Host>::create_output(self.run()?, media_type).await
    }

    async fn write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <OperationState as v15::artifacts::Host>::write(self.run()?, handle, bytes).await
    }

    async fn complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<v16::artifacts::OutputMetadata, String> {
        let metadata = <OperationState as v15::artifacts::Host>::complete(
            self.run()?,
            handle,
            checksum_sha256,
        )
        .await?;
        Ok(v16::artifacts::OutputMetadata {
            artifact_id: metadata.artifact_id,
            content_length: metadata.content_length,
            media_type: metadata.media_type,
            checksum_sha256: metadata.checksum_sha256,
        })
    }

    async fn abort(&mut self, handle: Resource<OutputArtifactStream>) -> Result<(), String> {
        <OperationState as v15::artifacts::Host>::abort(self.run()?, handle).await
    }

    async fn append_output(
        &mut self,
        name: String,
        media_type: String,
        batch_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <OperationState as v15::artifacts::Host>::append_output(
            self.run()?,
            name,
            media_type,
            batch_key,
            bytes,
        )
        .await
    }

    async fn finalize_output(&mut self, name: String) -> Result<String, String> {
        <OperationState as v15::artifacts::Host>::finalize_output(self.run()?, name).await
    }
}

// Artifact resources can only be created through a run, so outside one there
// is never a handle to drop.
impl v16::artifacts::HostInputArtifact for UnifiedState {
    async fn drop(&mut self, handle: Resource<InputArtifactStream>) -> wasmtime::Result<()> {
        match self.operation.as_mut() {
            Some(run) => {
                <OperationState as v15::artifacts::HostInputArtifact>::drop(run, handle).await
            }
            None => Ok(()),
        }
    }
}

impl v16::artifacts::HostOutputArtifact for UnifiedState {
    async fn drop(&mut self, handle: Resource<OutputArtifactStream>) -> wasmtime::Result<()> {
        match self.operation.as_mut() {
            Some(run) => {
                <OperationState as v15::artifacts::HostOutputArtifact>::drop(run, handle).await
            }
            None => Ok(()),
        }
    }
}

fn to_wit_v16_event(event: &DomainEvent) -> v16::api::Event {
    v16::api::Event {
        id: event.id.to_string(),
        event_type: event.event_type.clone(),
        aggregate_kind: event.aggregate_kind.clone(),
        aggregate_id: event.aggregate_id.to_string(),
        correlation_id: event.correlation_id.to_string(),
        causation_id: event.causation_id.map(|id| id.to_string()),
        payload: event.payload.to_string(),
    }
}

impl ExtensionRuntime {
    /// Instantiates a unified component with every import linked. `operation`
    /// is present only for an operation-run batch.
    async fn instantiate_unified(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        operation: Option<OperationState>,
    ) -> Result<
        (
            Store<UnifiedState>,
            wasmtime::component::InstancePre<UnifiedState>,
            wasmtime::component::Instance,
        ),
        ExtensionRuntimeError,
    > {
        let component = self.component(installation).await?;
        let state = UnifiedState {
            host: HostState::new(
                installation.clone(),
                repository,
                self.config.max_memory_bytes,
            ),
            operation,
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.host.limits);
        store.set_fuel(self.config.fuel).map_err(runtime_error)?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));
        let mut linker = Linker::new(&self.engine);
        host_unified::CatalogExtension::add_to_linker::<UnifiedState, HasSelf<UnifiedState>>(
            &mut linker,
            |state| state,
        )
        .map_err(runtime_error)?;
        let pre = linker.instantiate_pre(&component).map_err(runtime_error)?;
        let instance = pre
            .instantiate_async(&mut store)
            .await
            .map_err(runtime_error)?;
        Ok((store, pre, instance))
    }

    async fn unified_handler(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
    ) -> Result<(Store<UnifiedState>, v16_handler::Guest), ExtensionRuntimeError> {
        let (mut store, pre, instance) = self
            .instantiate_unified(installation, repository, None)
            .await?;
        let handler = v16_handler::GuestIndices::new(&pre)
            .and_then(|indices| indices.load(&mut store, &instance))
            .map_err(|error| {
                runtime_error(format!(
                    "component does not export catalog:host/handler: {error}"
                ))
            })?;
        Ok((store, handler))
    }

    pub(super) async fn invoke_unified_event(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
    ) -> Result<(), ExtensionRuntimeError> {
        let (mut store, guest) = self.unified_handler(installation, repository).await?;
        match guest
            .call_handle_event(&mut store, &to_wit_v16_event(event))
            .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(runtime_error(format!(
                "handler '{}' failed: {message}",
                handler.id
            ))),
            Err(error) => Err(runtime_error(error)),
        }
    }

    pub(super) async fn invoke_unified_command(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        handler: &str,
        request: &str,
        max_response_bytes: u64,
    ) -> Result<String, ExtensionRuntimeError> {
        let (mut store, guest) = self.unified_handler(installation, repository).await?;
        let command = v16_handler::CommandRequest {
            handler: handler.to_owned(),
            payload: request.to_owned(),
        };
        match guest.call_handle_command(&mut store, &command).await {
            Ok(Ok(response))
                if response.payload.len() <= MAX_HOST_JSON_BYTES
                    && response.payload.len() <= max_response_bytes as usize =>
            {
                Ok(response.payload)
            }
            Ok(Ok(_)) => Err(runtime_error(
                "command response exceeds its declared byte limit",
            )),
            Ok(Err(error)) => Err(ExtensionRuntimeError::Runtime(error)),
            Err(error) => Err(runtime_error(error)),
        }
    }

    /// Executes one batch through the unified world. The lifecycle mirrors the
    /// released 1.4/1.5 operation worlds exactly.
    pub(super) async fn invoke_unified_batch(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        run: &ClaimedExtensionOperationRun,
        cancelling: bool,
    ) -> Result<(Value, Value, bool), ExtensionRuntimeError> {
        let operation = self
            .operation_state(installation, repository.clone(), run.id)
            .with_batch_key(&run.batch_key);
        let (mut store, pre, instance) = self
            .instantiate_unified(installation, repository, Some(operation))
            .await?;
        let operations = v16_operations::GuestIndices::new(&pre)
            .and_then(|indices| indices.load(&mut store, &instance))
            .map_err(|error| {
                runtime_error(format!(
                    "component does not export catalog:host/operations: {error}"
                ))
            })?;
        run_operation_batch(&mut store, &operations, run, cancelling).await
    }
}
