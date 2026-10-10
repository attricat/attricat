//! Run-bound host interfaces of `attricat:host@1.0.0`, for operation runs.
//!
//! Artifact, transfer and administrative catalog calls apply to every run.
//! Interactive runs are additionally confined to their frozen selection:
//! generic page/lookup reads and connector calls are rejected, and every
//! write is checked against the selection and the initiator's live grants.

use super::*;
use crate::repository::{ExtensionAttricatBatch, InteractiveRunScope};

const SELECTION_SCOPE_ERROR: &str =
    "interactive runs read catalog data through their run-bound selection";

// Borrow only the Sync repository field: `OperationState` holds a resource
// table and is not Sync, so host futures cannot capture `&OperationState`.
async fn load_scope(
    repository: &AttricatRepository,
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
        batch: ExtensionAttricatBatch,
    ) -> Result<String, String> {
        for intent in &batch.intents {
            match intent.target_record_id() {
                Some(record_id) if scope.record_ids.contains(&record_id) => {}
                Some(_) => return Err("intent targets a record outside the run selection".into()),
                None => return Err("interactive runs cannot create or upsert records".into()),
            }
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_all_active(batch_capabilities(&batch)).await?;
        let repository = self.repository.for_interactive_run(
            &self.installation.extension_id,
            self.run_id,
            &scope,
        );
        let outcomes = AttricatMutationService::new(&repository)
            .execute_extension_attricat_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
    }
}

impl wit::selection::Host for OperationState {
    async fn describe(&mut self) -> Result<String, String> {
        let scope = load_scope(&self.repository, self.run_id)
            .await?
            .ok_or_else(|| "this run has no interactive selection".to_owned())?;
        bounded_serialize(&json!({
            "count": scope.record_ids.len(),
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
        host.require_active("attricat.read").await?;
        let page = self
            .repository
            .interactive_selection_page(&self.installation.extension_id, &scope, &cursor, limit)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&page)
    }
}

impl wit::attricat_data::Host for OperationState {
    async fn read(&mut self, request: String) -> Result<String, String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        self.attricat_data_read(request).await
    }

    async fn batch(&mut self, request: String) -> Result<String, String> {
        if request.len() > MAX_HOST_JSON_BYTES {
            return Err("request exceeds host JSON limit".into());
        }
        let Some(scope) = load_scope(&self.repository, self.run_id).await? else {
            return self.attricat_data_batch(request).await;
        };
        let input: AttricatCommandRequest = parse_storage_request(&request)?;
        let AttricatCommandRequest::Batch { batch } = input;
        require_operation_batch_key(&batch.batch_key, &self.batch_key)?;
        self.interactive_batch(scope, batch).await
    }
}

impl wit::attricat::Host for OperationState {
    async fn schema(
        &mut self,
        blueprint_id: String,
        blueprint_version: u64,
        context_id: String,
    ) -> Result<String, String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        self.attricat_schema(blueprint_id, blueprint_version, context_id)
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
        self.attricat_page(blueprint_id, blueprint_version, context_id, cursor, limit)
            .await
    }

    async fn upsert_batch(&mut self, request: String) -> Result<(), String> {
        if load_scope(&self.repository, self.run_id).await?.is_some() {
            return Err(SELECTION_SCOPE_ERROR.into());
        }
        self.attricat_upsert_batch(request).await
    }
}

impl wit::transfer::Host for OperationState {
    async fn fetch_input(&mut self, request: String) -> Result<String, String> {
        self.transfer_fetch_input(request).await
    }

    async fn deliver_output(&mut self, request: String) -> Result<String, String> {
        self.transfer_deliver_output(request).await
    }
}

impl wit::artifacts::Host for OperationState {
    async fn open_input(
        &mut self,
        artifact_id: String,
    ) -> Result<Resource<InputArtifactStream>, String> {
        self.artifacts_open_input(artifact_id).await
    }

    async fn describe_input(
        &mut self,
        handle: Resource<InputArtifactStream>,
    ) -> Result<wit::artifacts::InputMetadata, String> {
        let metadata = self.artifacts_describe_input(handle).await?;
        Ok(wit::artifacts::InputMetadata {
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
        self.artifacts_read(handle, max_bytes).await
    }

    async fn create_output(
        &mut self,
        media_type: String,
    ) -> Result<Resource<OutputArtifactStream>, String> {
        self.artifacts_create_output(media_type).await
    }

    async fn write(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.artifacts_write(handle, bytes).await
    }

    async fn complete(
        &mut self,
        handle: Resource<OutputArtifactStream>,
        checksum_sha256: String,
    ) -> Result<wit::artifacts::OutputMetadata, String> {
        let metadata = self.artifacts_complete(handle, checksum_sha256).await?;
        Ok(wit::artifacts::OutputMetadata {
            artifact_id: metadata.artifact_id,
            content_length: metadata.content_length,
            media_type: metadata.media_type,
            checksum_sha256: metadata.checksum_sha256,
        })
    }

    async fn abort(&mut self, handle: Resource<OutputArtifactStream>) -> Result<(), String> {
        self.artifacts_abort(handle).await
    }

    async fn append_output(
        &mut self,
        name: String,
        media_type: String,
        batch_key: String,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.artifacts_append_output(name, media_type, batch_key, bytes)
            .await
    }

    async fn finalize_output(&mut self, name: String) -> Result<String, String> {
        self.artifacts_finalize_output(name).await
    }
}

impl wit::artifacts::HostInputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<InputArtifactStream>) -> wasmtime::Result<()> {
        self.artifacts_drop_input(handle).await
    }
}

impl wit::artifacts::HostOutputArtifact for OperationState {
    async fn drop(&mut self, handle: Resource<OutputArtifactStream>) -> wasmtime::Result<()> {
        self.artifacts_drop_output(handle).await
    }
}
