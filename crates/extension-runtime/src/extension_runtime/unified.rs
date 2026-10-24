//! Host implementation of the `attricat:host` ABI, currently 1.1.0.
//!
//! Every import is linked for every invocation; the host instantiates the
//! component once and loads only the export the invocation needs, so a
//! component may target the combined `attricat-extension` world, the
//! narrower `handler-extension` or `operation-extension` world, or the 1.1
//! `outcome-extension` world. When a component exports the 1.1 interfaces
//! (`event-handler`, `command-handler`, `deferrable-operations`) the host
//! calls them instead of the 1.0 `handler` and `operations`.
//!
//! Run-bound interfaces are implemented by the operation state (including
//! the interactive selection confinement) and fail outside an operation run.
//! Direct catalog access through `api` fails inside an operation run, so a
//! run can never bypass its scope.
//!
//! Export lookup is semver-aware in wasmtime, so components built against an
//! earlier 1.x release keep working when this host binds a later, additive one.

use super::*;

pub(super) mod host_unified {
    // A host-only world that combines every export of the package, so one
    // binding covers components of every released world.
    wasmtime::component::bindgen!({
        path: "wit-host",
        inline: "
            package attricat:host-bindings;
            world host {
                include attricat:host/attricat-extension@1.1.0;
                include attricat:host/outcome-extension@1.1.0;
            }
        ",
        world: "attricat:host-bindings/host",
        with: {
            "attricat:host/artifacts.input-artifact": crate::extension_runtime::InputArtifactStream,
            "attricat:host/artifacts.output-artifact": crate::extension_runtime::OutputArtifactStream,
        },
        imports: { default: async },
        exports: { default: async },
    });
}

use host_unified::attricat::host as v16;
use host_unified::exports::attricat::host::{
    command_handler as v11_command_handler, deferrable_operations as v11_operations,
    event_handler as v11_event_handler, handler as v16_handler, operations as v16_operations,
};

const RUN_ONLY_ERROR: &str = "this interface is available only during an operation run";
const RUN_ATTRICAT_ERROR: &str =
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
            Err(RUN_ATTRICAT_ERROR.into())
        } else {
            Ok(())
        }
    }
}

impl v16::api::Host for UnifiedState {
    async fn read(
        &mut self,
        request: v16::api::ReadRequest,
    ) -> Result<v16::api::ReadResponse, String> {
        self.deny_in_run()?;
        self.host.read(request).await
    }

    async fn write(
        &mut self,
        request: v16::api::WriteRequest,
    ) -> Result<v16::api::WriteResponse, String> {
        self.deny_in_run()?;
        self.host.write(request).await
    }

    async fn scoped_configuration_get(
        &mut self,
        scope: v16::api::ConfigurationScope,
    ) -> Result<Option<String>, String> {
        self.host.scoped_configuration_get(scope).await
    }

    async fn scoped_configuration_set(
        &mut self,
        request: v16::api::ScopedConfigurationUpdate,
    ) -> Result<(), String> {
        self.host.scoped_configuration_set(request).await
    }

    async fn call(&mut self, operation: String, request: String) -> Result<String, String> {
        if matches!(
            operation.as_str(),
            "attricat.read.v1" | "attricat.command.v1"
        ) {
            self.deny_in_run()?;
        }
        if operation == "connector.scope.v1" {
            return match self.operation.as_mut() {
                Some(run) => run.connector_scope().await,
                None => Ok("null".into()),
            };
        }
        self.host.call(operation, request).await
    }

    async fn log(&mut self, level: String, message: String) -> Result<(), String> {
        self.host.log(level, message).await
    }
}

impl v16::selection::Host for UnifiedState {
    async fn describe(&mut self) -> Result<String, String> {
        <OperationState as v16::selection::Host>::describe(self.run()?).await
    }

    async fn page(&mut self, cursor: String, limit: u32) -> Result<String, String> {
        <OperationState as v16::selection::Host>::page(self.run()?, cursor, limit).await
    }
}

impl v16::attricat_data::Host for UnifiedState {
    async fn read(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::attricat_data::Host>::read(self.run()?, request).await
    }

    async fn batch(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::attricat_data::Host>::batch(self.run()?, request).await
    }
}

impl v16::attricat::Host for UnifiedState {
    async fn schema(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
    ) -> Result<String, String> {
        <OperationState as v16::attricat::Host>::schema(
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
        <OperationState as v16::attricat::Host>::page(
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
        <OperationState as v16::attricat::Host>::upsert_batch(self.run()?, request).await
    }

    async fn page_v2(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::attricat::Host>::page_v2(self.run()?, request).await
    }

    async fn withdrawn(&mut self, cursor: String, limit: u32) -> Result<String, String> {
        <OperationState as v16::attricat::Host>::withdrawn(self.run()?, cursor, limit).await
    }
}

impl v16::transfer::Host for UnifiedState {
    async fn fetch_input(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::transfer::Host>::fetch_input(self.run()?, request).await
    }

    async fn deliver_output(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::transfer::Host>::deliver_output(self.run()?, request).await
    }

    async fn fetch_input_v2(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::transfer::Host>::fetch_input_v2(self.run()?, request).await
    }

    async fn deliver_output_v2(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::transfer::Host>::deliver_output_v2(self.run()?, request).await
    }

    async fn open_record_file(&mut self, request: String) -> Result<String, String> {
        <OperationState as v16::transfer::Host>::open_record_file(self.run()?, request).await
    }
}

impl v16::clock::Host for UnifiedState {
    async fn now(&mut self) -> v16::clock::Instant {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        v16::clock::Instant {
            seconds: elapsed.as_secs(),
            nanoseconds: elapsed.subsec_nanos(),
        }
    }
}

// Types-only interfaces still get a (method-less) host trait.
impl v16::outcomes::Host for UnifiedState {}

impl v16::artifacts::Host for UnifiedState {
    async fn open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        <OperationState as v16::artifacts::Host>::open_input(self.run()?, artifact_id).await
    }

    async fn describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<v16::artifacts::InputMetadata, String> {
        let metadata =
            <OperationState as v16::artifacts::Host>::describe_input(self.run()?, handle).await?;
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
        <OperationState as v16::artifacts::Host>::read(self.run()?, handle, max_bytes).await
    }

    async fn create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        <OperationState as v16::artifacts::Host>::create_output(self.run()?, media_type).await
    }

    async fn write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <OperationState as v16::artifacts::Host>::write(self.run()?, handle, bytes).await
    }

    async fn complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<v16::artifacts::OutputMetadata, String> {
        let metadata = <OperationState as v16::artifacts::Host>::complete(
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
        <OperationState as v16::artifacts::Host>::abort(self.run()?, handle).await
    }

    async fn append_output(
        &mut self,
        name: String,
        media_type: String,
        batch_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <OperationState as v16::artifacts::Host>::append_output(
            self.run()?,
            name,
            media_type,
            batch_key,
            bytes,
        )
        .await
    }

    async fn finalize_output(&mut self, name: String) -> Result<String, String> {
        <OperationState as v16::artifacts::Host>::finalize_output(self.run()?, name).await
    }
}

// Artifact resources can only be created through a run, so outside one there
// is never a handle to drop.
impl v16::artifacts::HostInputArtifact for UnifiedState {
    async fn drop(&mut self, handle: Resource<InputArtifactStream>) -> wasmtime::Result<()> {
        match self.operation.as_mut() {
            Some(run) => {
                <OperationState as v16::artifacts::HostInputArtifact>::drop(run, handle).await
            }
            None => Ok(()),
        }
    }
}

impl v16::artifacts::HostOutputArtifact for UnifiedState {
    async fn drop(&mut self, handle: Resource<OutputArtifactStream>) -> wasmtime::Result<()> {
        match self.operation.as_mut() {
            Some(run) => {
                <OperationState as v16::artifacts::HostOutputArtifact>::drop(run, handle).await
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
    /// is present only for an operation-run batch; `lease_holder` names the
    /// invocation for the export leases it takes.
    async fn instantiate_unified(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        operation: Option<OperationState>,
        lease_holder: Option<String>,
    ) -> Result<
        (
            Store<UnifiedState>,
            wasmtime::component::InstancePre<UnifiedState>,
            wasmtime::component::Instance,
        ),
        ExtensionRuntimeError,
    > {
        let component = self.component(installation).await?;
        let mut host = HostState::new(
            installation.clone(),
            repository,
            self.config.max_memory_bytes,
        );
        host.lease_holder = lease_holder;
        host.run_id = operation.as_ref().map(|operation| operation.run_id);
        let state = UnifiedState { host, operation };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.host.limits);
        store.set_fuel(self.config.fuel).map_err(runtime_error)?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));
        let pre = self.unified_instance_pre(installation, &component)?;
        let instance = pre
            .instantiate_async(&mut store)
            .await
            .map_err(runtime_error)?;
        Ok((store, pre, instance))
    }

    /// The release's unified component, linked and pre-instantiated once.
    fn unified_instance_pre(
        &self,
        installation: &ExtensionRuntimeInstallation,
        component: &Component,
    ) -> Result<wasmtime::component::InstancePre<UnifiedState>, ExtensionRuntimeError> {
        let release = installation.installed_release_id;
        if let Some(pre) = self
            .unified_instances
            .lock()
            .expect("unified instance cache is not poisoned")
            .get(&release)
        {
            return Ok(pre.clone());
        }
        let pre = self
            .linkers
            .unified
            .instantiate_pre(component)
            .map_err(runtime_error)?;
        let mut instances = self
            .unified_instances
            .lock()
            .expect("unified instance cache is not poisoned");
        if instances.len() >= MAX_CACHED_COMPONENTS {
            instances.clear();
        }
        instances.insert(release, pre.clone());
        Ok(pre)
    }

    /// Loads the 1.1 export `load_v11` when the component has it, otherwise
    /// the 1.0 export `load_v10`.
    fn load_export<V11, V10>(
        store: &mut Store<UnifiedState>,
        pre: &wasmtime::component::InstancePre<UnifiedState>,
        instance: &wasmtime::component::Instance,
        load_v11: impl FnOnce(
            &wasmtime::component::InstancePre<UnifiedState>,
            &mut Store<UnifiedState>,
            &wasmtime::component::Instance,
        ) -> wasmtime::Result<V11>,
        load_v10: impl FnOnce(
            &wasmtime::component::InstancePre<UnifiedState>,
            &mut Store<UnifiedState>,
            &wasmtime::component::Instance,
        ) -> wasmtime::Result<V10>,
        missing: &str,
    ) -> Result<Export<V11, V10>, ExtensionRuntimeError> {
        if let Ok(export) = load_v11(pre, store, instance) {
            return Ok(Export::V11(export));
        }
        load_v10(pre, store, instance)
            .map(Export::V10)
            .map_err(|error| runtime_error(format!("component does not export {missing}: {error}")))
    }

    /// Invokes an event handler. `Ok` carries the handler's outcome; a 1.0
    /// handler's error, a trap, fuel or memory exhaustion and a timeout are
    /// all faults.
    pub(super) async fn invoke_unified_event(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        handler: &ManifestEventHandler,
        event: &DomainEvent,
        delivery: &EventDeliveryAttempt,
    ) -> Result<Result<(), InvocationFailure>, ExtensionRuntimeError> {
        let (mut store, pre, instance) = self
            .instantiate_unified(
                installation,
                repository,
                None,
                Some(super::handler_lease_holder(event.id, &handler.id)),
            )
            .await?;
        let export = Self::load_export(
            &mut store,
            &pre,
            &instance,
            |pre, store, instance| v11_event_handler::GuestIndices::new(pre)?.load(store, instance),
            |pre, store, instance| v16_handler::GuestIndices::new(pre)?.load(store, instance),
            "attricat:host/event-handler or attricat:host/handler",
        )?;
        let wit_event = to_wit_v16_event(event);
        Ok(match export {
            Export::V11(guest) => {
                let delivery = v11_event_handler::Delivery {
                    sequence: delivery.sequence,
                    record_revision: delivery.record_revision,
                    attempt: delivery.attempt,
                    source_kind: event.source_kind.clone(),
                    source_name: event.source_name.clone(),
                };
                match guest
                    .call_handle_event(&mut store, &wit_event, &delivery)
                    .await
                {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(failure)) => Err(failure.into()),
                    Err(error) => Err(InvocationFailure::Fault(error.to_string())),
                }
            }
            Export::V10(guest) => match guest.call_handle_event(&mut store, &wit_event).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(message)) => Err(InvocationFailure::Fault(format!(
                    "handler '{}' failed: {message}",
                    handler.id
                ))),
                Err(error) => Err(InvocationFailure::Fault(error.to_string())),
            },
        })
    }

    pub(super) async fn invoke_unified_command(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        handler: &str,
        request: &str,
        max_response_bytes: u64,
    ) -> Result<String, ExtensionRuntimeError> {
        let (mut store, pre, instance) = self
            .instantiate_unified(installation, repository, None, None)
            .await?;
        let export = Self::load_export(
            &mut store,
            &pre,
            &instance,
            |pre, store, instance| {
                v11_command_handler::GuestIndices::new(pre)?.load(store, instance)
            },
            |pre, store, instance| v16_handler::GuestIndices::new(pre)?.load(store, instance),
            "attricat:host/command-handler or attricat:host/handler",
        )?;
        let response = match export {
            Export::V11(guest) => guest
                .call_handle_command(
                    &mut store,
                    &v11_command_handler::CommandRequest {
                        handler: handler.to_owned(),
                        payload: request.to_owned(),
                    },
                )
                .await
                .map(|result| result.map(|response| response.payload)),
            Export::V10(guest) => guest
                .call_handle_command(
                    &mut store,
                    &v16_handler::CommandRequest {
                        handler: handler.to_owned(),
                        payload: request.to_owned(),
                    },
                )
                .await
                .map(|result| result.map(|response| response.payload)),
        };
        match response {
            Ok(Ok(payload))
                if payload.len() <= MAX_HOST_JSON_BYTES
                    && payload.len() <= max_response_bytes as usize =>
            {
                Ok(payload)
            }
            Ok(Ok(_)) => Err(runtime_error(
                "command response exceeds its declared byte limit",
            )),
            Ok(Err(error)) => Err(ExtensionRuntimeError::Runtime(error)),
            Err(error) => Err(runtime_error(error)),
        }
    }

    /// Executes one batch through the 1.1 `deferrable-operations` export when
    /// the component has it, otherwise through the 1.0 `operations` export.
    /// Returns whether the 1.1 export ran, because only its faults quarantine.
    pub(super) async fn invoke_unified_batch(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        run: &ClaimedExtensionOperationRun,
        cancelling: bool,
    ) -> (bool, Result<BatchResult, BatchError>) {
        let operation = self
            .operation_state(installation, repository.clone(), run.id)
            .with_batch_key(&run.batch_key);
        let (mut store, pre, instance) = match self
            .instantiate_unified(
                installation,
                repository,
                Some(operation),
                Some(super::run_lease_holder(run.id)),
            )
            .await
        {
            Ok(instance) => instance,
            Err(error) => return (false, Err(error.into())),
        };
        let export = match Self::load_export(
            &mut store,
            &pre,
            &instance,
            |pre, store, instance| v11_operations::GuestIndices::new(pre)?.load(store, instance),
            |pre, store, instance| v16_operations::GuestIndices::new(pre)?.load(store, instance),
            "attricat:host/deferrable-operations or attricat:host/operations",
        ) {
            Ok(export) => export,
            Err(error) => return (false, Err(error.into())),
        };
        match export {
            Export::V11(operations) => (
                true,
                run_operation_batch(&mut store, &operations, run, cancelling).await,
            ),
            Export::V10(operations) => (
                false,
                run_operation_batch(&mut store, &operations, run, cancelling).await,
            ),
        }
    }
}

/// The export a component provides: the 1.1 interface or its 1.0 predecessor.
enum Export<V11, V10> {
    V11(V11),
    V10(V10),
}

/// The unified world's linker; built once per runtime.
pub(super) fn linker(engine: &Engine) -> Result<Linker<UnifiedState>, ExtensionRuntimeError> {
    let mut linker = Linker::new(engine);
    host_unified::Host_::add_to_linker::<UnifiedState, HasSelf<UnifiedState>>(
        &mut linker,
        |state| state,
    )
    .map_err(runtime_error)?;
    Ok(linker)
}
