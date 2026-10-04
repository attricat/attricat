//! The released operation-batch lifecycle, shared by every operation world.
//!
//! The 1.2/1.3, 1.4, 1.5 and unified worlds all export the same
//! `catalog:host/operations` interface, but bindgen generates distinct types
//! for each world. [`OperationsExport`] abstracts over those types so that one
//! function, [`run_operation_batch`], owns the lifecycle and its validation.

use std::future::Future;

use serde_json::{Value, json};
use wasmtime::Store;

use super::{
    ExtensionRuntimeError, MAX_HOST_JSON_BYTES, host_connector::exports::catalog::host as v14,
    host_operations::exports::catalog::host as v12,
    interactive::host_interactive::exports::catalog::host as v15,
    unified::host_unified::exports::catalog::host as v16,
};
use crate::repository::ClaimedExtensionOperationRun;

/// A lifecycle export that takes the request and only reports failure.
#[derive(Clone, Copy, Debug)]
pub(super) enum LifecycleCall {
    Prepare,
    Start,
    Checkpoint,
    Finish,
    Cancel,
}

/// The raw `batch-result` a component returned, before validation.
pub(super) struct BatchOutput {
    checkpoint: String,
    progress: String,
    done: bool,
}

/// One world's `catalog:host/operations` export.
pub(super) trait OperationsExport: Sync {
    type Request: Send + Sync;

    /// Builds the world's request for one batch of `run`.
    fn request(run: &ClaimedExtensionOperationRun) -> Result<Self::Request, ExtensionRuntimeError>;

    fn set_checkpoint(request: &mut Self::Request, checkpoint: String);

    fn call<T: Send + 'static>(
        &self,
        store: &mut Store<T>,
        call: LifecycleCall,
        request: &Self::Request,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send;

    fn process_batch<T: Send + 'static>(
        &self,
        store: &mut Store<T>,
        request: &Self::Request,
    ) -> impl Future<Output = wasmtime::Result<Result<BatchOutput, String>>> + Send;
}

/// Implements [`OperationsExport`] for the `operations` export module of each
/// listed world. The bodies only forward to the bindgen-generated methods.
macro_rules! impl_operations_export {
    ($($world:ident),+ $(,)?) => {$(
        impl OperationsExport for $world::operations::Guest {
            type Request = $world::operations::OperationRequest;

            fn request(
                run: &ClaimedExtensionOperationRun,
            ) -> Result<Self::Request, ExtensionRuntimeError> {
                Ok($world::operations::OperationRequest {
                    run_id: run.id.to_string(),
                    operation_id: run.operation_handler.clone(),
                    configuration: to_json(&run.configuration)?,
                    input: to_json(&run.input)?,
                    checkpoint: to_json(&run.checkpoint)?,
                    batch_key: run.batch_key.clone(),
                })
            }

            fn set_checkpoint(request: &mut Self::Request, checkpoint: String) {
                request.checkpoint = checkpoint;
            }

            async fn call<T: Send + 'static>(
                &self,
                store: &mut Store<T>,
                call: LifecycleCall,
                request: &Self::Request,
            ) -> wasmtime::Result<Result<(), String>> {
                // `prepare` and `start` return a message the host ignores.
                Ok(match call {
                    LifecycleCall::Prepare => self.call_prepare(store, request).await?.map(drop),
                    LifecycleCall::Start => self.call_start(store, request).await?.map(drop),
                    LifecycleCall::Checkpoint => self.call_checkpoint(store, request).await?,
                    LifecycleCall::Finish => self.call_finish(store, request).await?,
                    LifecycleCall::Cancel => self.call_cancel(store, request).await?,
                })
            }

            async fn process_batch<T: Send + 'static>(
                &self,
                store: &mut Store<T>,
                request: &Self::Request,
            ) -> wasmtime::Result<Result<BatchOutput, String>> {
                Ok(self
                    .call_process_batch(store, request)
                    .await?
                    .map(|result| BatchOutput {
                        checkpoint: result.checkpoint,
                        progress: result.progress,
                        done: result.done,
                    }))
            }
        }
    )+};
}

impl_operations_export!(v12, v14, v15, v16);

fn to_json(value: &Value) -> Result<String, ExtensionRuntimeError> {
    serde_json::to_string(value).map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))
}

fn flatten<T>(result: wasmtime::Result<Result<T, String>>) -> Result<T, ExtensionRuntimeError> {
    result
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))?
        .map_err(ExtensionRuntimeError::Runtime)
}

/// Runs one batch of `run` against an instantiated operations export: cancel,
/// or prepare/start on the first batch, then process, validate, checkpoint and
/// finish when done. Returns the validated `(checkpoint, progress, done)`.
pub(super) async fn run_operation_batch<E: OperationsExport, T: Send + 'static>(
    store: &mut Store<T>,
    operations: &E,
    run: &ClaimedExtensionOperationRun,
    cancelling: bool,
) -> Result<(Value, Value, bool), ExtensionRuntimeError> {
    let mut request = E::request(run)?;
    if cancelling {
        flatten(
            operations
                .call(store, LifecycleCall::Cancel, &request)
                .await,
        )?;
        return Ok((run.checkpoint.clone(), json!({"cancelled": true}), true));
    }
    if !run.lifecycle_started {
        for call in [LifecycleCall::Prepare, LifecycleCall::Start] {
            flatten(operations.call(store, call, &request).await)?;
        }
    }
    let output = flatten(operations.process_batch(store, &request).await)?;
    let (checkpoint, progress) = validate_batch_output(&output, run.max_checkpoint_bytes)?;
    E::set_checkpoint(&mut request, output.checkpoint);
    flatten(
        operations
            .call(store, LifecycleCall::Checkpoint, &request)
            .await,
    )?;
    if output.done {
        flatten(
            operations
                .call(store, LifecycleCall::Finish, &request)
                .await,
        )?;
    }
    Ok((checkpoint, progress, output.done))
}

/// Validates the checkpoint and progress JSON a batch returned. Both must be
/// objects within the host JSON bound, and the checkpoint must also fit the
/// run's declared checkpoint budget.
fn validate_batch_output(
    output: &BatchOutput,
    max_checkpoint_bytes: u64,
) -> Result<(Value, Value), ExtensionRuntimeError> {
    let checkpoint: Value = serde_json::from_str(&output.checkpoint).map_err(|_| {
        ExtensionRuntimeError::Runtime("operation returned invalid checkpoint".into())
    })?;
    let progress: Value = serde_json::from_str(&output.progress).map_err(|_| {
        ExtensionRuntimeError::Runtime("operation returned invalid progress".into())
    })?;
    let serialized_len = |value: &Value| serde_json::to_vec(value).map_or(usize::MAX, |v| v.len());
    let checkpoint_bytes = serialized_len(&checkpoint);
    let checkpoint_limit = usize::try_from(max_checkpoint_bytes)
        .unwrap_or(usize::MAX)
        .min(MAX_HOST_JSON_BYTES);
    if !checkpoint.is_object()
        || !progress.is_object()
        || checkpoint_bytes > checkpoint_limit
        || serialized_len(&progress) > MAX_HOST_JSON_BYTES
    {
        return Err(ExtensionRuntimeError::Runtime(
            "operation returned oversized or non-object checkpoint".into(),
        ));
    }
    Ok((checkpoint, progress))
}
