//! Host implementation of the additive `catalog:host@1.5.0` operation world.
//!
//! Artifact, transfer and administrative catalog calls keep their released
//! 1.4 behavior. Interactive runs are additionally confined to their frozen
//! selection: generic page/lookup reads and connector calls are rejected, and
//! every write is checked against the selection and the initiator's live grants.

use super::*;
use crate::repository::{ExtensionCatalogBatch, InteractiveRunScope};

pub(super) mod host_interactive {
    wasmtime::component::bindgen!({
        path: "wit-interactive",
        world: "catalog-extension-operation",
        with: {
            "catalog:host/artifacts.input-artifact": crate::extension_runtime::InputArtifactStream,
            "catalog:host/artifacts.output-artifact": crate::extension_runtime::OutputArtifactStream,
        },
        imports: { default: async },
        exports: { default: async },
    });
}

use host_connector::catalog::host as v14;
use host_interactive::catalog::host as v15;

const SELECTION_SCOPE_ERROR: &str =
    "interactive runs read catalog data through their run-bound selection";

// Borrow only the Sync repository field: `OperationState` holds a resource
// table and is not Sync, so host futures cannot capture `&OperationState`.
async fn load_scope(
    repository: &CatalogRepository,
    run_id: Uuid,
) -> Result<Option<InteractiveRunScope>, String> {
    repository
        .interactive_run_scope(run_id)
        .await
        .map_err(|_| "operation scope could not be loaded".to_owned())
}

impl OperationState {
    /// Validates a batch against an interactive run's selection, then applies
    /// it with the initiator as the authorization actor.
    async fn interactive_batch(
        &mut self,
        scope: InteractiveRunScope,
        batch: ExtensionCatalogBatch,
    ) -> Result<String, String> {
        for intent in &batch.intents {
            match intent.target_entity_id() {
                Some(entity_id) if scope.entity_ids.contains(&entity_id) => {}
                Some(_) => return Err("intent targets an entity outside the run selection".into()),
                None => return Err("interactive runs cannot create or upsert entities".into()),
            }
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_all_active(batch_capabilities(&batch)).await?;
        let repository = self.repository.for_interactive_run(
            &self.installation.extension_id,
            self.run_id,
            &scope,
        );
        let outcomes = CatalogMutationService::new(&repository)
            .execute_extension_catalog_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
    }
}

impl v15::selection::Host for OperationState {
    async fn describe(&mut self) -> Result<String, String> {
        let scope = load_scope(&self.repository, self.run_id)
            .await?
            .ok_or_else(|| "this run has no interactive selection".to_owned())?;
        bounded_serialize(&json!({
            "count": scope.entity_ids.len(),
            "blueprint_id": scope.blueprint_id,
            "blueprint_version": scope.blueprint_version,
            "context_id": scope.context_id,
        }))
    }

    async fn page(&mut self, cursor: String, limit: u32) -> Result<String, String> {
        if cursor.len() > 32 {
            return Err("selection cursor is invalid".into());
        }
        let scope = load_scope(&self.repository, self.run_id)
            .await?
            .ok_or_else(|| "this run has no interactive selection".to_owned())?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("catalog.read").await?;
        let page = self
            .repository
            .interactive_selection_page(&self.installation.extension_id, &scope, &cursor, limit)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&page)
    }
}

impl v15::catalog_data::Host for OperationState {
    async fn read(&mut self, request: String) -> Result<String, String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        <Self as v14::catalog_data::Host>::read(self, request).await
    }

    async fn batch(&mut self, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let Some(scope) = load_scope(&self.repository, self.run_id).await? else {
            return <Self as v14::catalog_data::Host>::batch(self, request).await;
        };
        let input: CatalogCommandRequest = parse_storage_request(&request)?;
        let CatalogCommandRequest::Batch { batch } = input;
        require_operation_batch_key(&batch.batch_key, &self.batch_key)?;
        self.interactive_batch(scope, batch).await
    }
}

impl v15::catalog::Host for OperationState {
    async fn schema(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
    ) -> Result<String, String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        <Self as v14::catalog::Host>::schema(self, blueprint_id, blueprint_version, context_id)
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
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        <Self as v14::catalog::Host>::page(
            self,
            blueprint_id,
            blueprint_version,
            context_id,
            cursor,
            limit,
        )
        .await
    }

    async fn upsert_batch(&mut self, request: String) -> Result<(), String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        <Self as v14::catalog::Host>::upsert_batch(self, request).await
    }
}

impl v15::transfer::Host for OperationState {
    async fn fetch_input(&mut self, request: String) -> Result<String, String> {
        <Self as v14::transfer::Host>::fetch_input(self, request).await
    }

    async fn deliver_output(&mut self, request: String) -> Result<String, String> {
        <Self as v14::transfer::Host>::deliver_output(self, request).await
    }
}

impl v15::artifacts::Host for OperationState {
    async fn open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        <Self as v14::artifacts::Host>::open_input(self, artifact_id).await
    }

    async fn describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<v15::artifacts::InputMetadata, String> {
        let metadata = <Self as v14::artifacts::Host>::describe_input(self, handle).await?;
        Ok(v15::artifacts::InputMetadata {
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
        <Self as v14::artifacts::Host>::read(self, handle, max_bytes).await
    }

    async fn create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        <Self as v14::artifacts::Host>::create_output(self, media_type).await
    }

    async fn write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <Self as v14::artifacts::Host>::write(self, handle, bytes).await
    }

    async fn complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<v15::artifacts::OutputMetadata, String> {
        let metadata =
            <Self as v14::artifacts::Host>::complete(self, handle, checksum_sha256).await?;
        Ok(v15::artifacts::OutputMetadata {
            artifact_id: metadata.artifact_id,
            content_length: metadata.content_length,
            media_type: metadata.media_type,
            checksum_sha256: metadata.checksum_sha256,
        })
    }

    async fn abort(&mut self, handle: Resource<OutputArtifactStream>) -> Result<(), String> {
        <Self as v14::artifacts::Host>::abort(self, handle).await
    }

    async fn append_output(
        &mut self,
        name: String,
        media_type: String,
        batch_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        <Self as v14::artifacts::Host>::append_output(self, name, media_type, batch_key, bytes)
            .await
    }

    async fn finalize_output(&mut self, name: String) -> Result<String, String> {
        <Self as v14::artifacts::Host>::finalize_output(self, name).await
    }
}

impl v15::artifacts::HostInputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<InputArtifactStream>) -> wasmtime::Result<()> {
        <Self as v14::artifacts::HostInputArtifact>::drop(self, handle).await
    }
}

impl v15::artifacts::HostOutputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<OutputArtifactStream>) -> wasmtime::Result<()> {
        <Self as v14::artifacts::HostOutputArtifact>::drop(self, handle).await
    }
}

impl ExtensionRuntime {
    /// Executes one batch through the 1.5 world. The lifecycle mirrors the
    /// released 1.4 connector world exactly.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn invoke_interactive_batch(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: CatalogRepository,
        run_id: Uuid,
        operation_handler: &str,
        configuration: &Value,
        input: &Value,
        checkpoint: &Value,
        batch_key: &str,
        max_checkpoint_bytes: u64,
        lifecycle_started: bool,
        cancelling: bool,
    ) -> Result<(Value, Value, bool), ExtensionRuntimeError> {
        let runtime_error = |error: String| ExtensionRuntimeError::Runtime(error);
        let component = self.component(installation).await?;
        let request = host_interactive::exports::catalog::host::operations::OperationRequest {
            run_id: run_id.to_string(),
            operation_id: operation_handler.to_owned(),
            configuration: serde_json::to_string(configuration)
                .map_err(|e| runtime_error(e.to_string()))?,
            input: serde_json::to_string(input).map_err(|e| runtime_error(e.to_string()))?,
            checkpoint: serde_json::to_string(checkpoint)
                .map_err(|e| runtime_error(e.to_string()))?,
            batch_key: batch_key.to_owned(),
        };
        let mut store = Store::new(
            &self.engine,
            OperationState::new(
                self.config.max_memory_bytes,
                installation.clone(),
                repository,
                self.object_store.clone(),
                run_id,
            )
            .with_batch_key(batch_key),
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.config.fuel)
            .map_err(|e| runtime_error(e.to_string()))?;
        store.set_epoch_deadline(epoch_deadline(self.config.invocation_timeout));
        let bindings = host_interactive::CatalogExtensionOperation::instantiate_async(
            &mut store,
            &component,
            &self.linkers.interactive,
        )
        .await
        .map_err(|e| runtime_error(e.to_string()))?;
        run_operation_batch!(
            &mut store,
            bindings.catalog_host_operations(),
            request,
            checkpoint: checkpoint,
            max_checkpoint_bytes: max_checkpoint_bytes,
            lifecycle_started: lifecycle_started,
            cancelling: cancelling,
        )
    }
}

/// The interactive operation world's linker; built once per runtime.
pub(super) fn linker(engine: &Engine) -> Result<Linker<OperationState>, ExtensionRuntimeError> {
    let mut linker = Linker::new(engine);
    host_interactive::CatalogExtensionOperation::add_to_linker::<
        OperationState,
        HasSelf<OperationState>,
    >(&mut linker, |state| state)
    .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?;
    Ok(linker)
}
