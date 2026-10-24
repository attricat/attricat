//! The operation-batch lifecycle.
//!
//! [`OperationsExport`] wraps the bindgen types of the `attricat:host/operations`
//! (1.0) and `attricat:host/deferrable-operations` (1.1) exports so that one
//! function, [`run_operation_batch`], owns the lifecycle and its validation.

use std::{future::Future, time::Duration};

use serde_json::{Value, json};
use wasmtime::Store;

use super::{
    ExtensionRuntimeError, MAX_HOST_JSON_BYTES,
    outcomes::{InvocationFailure, component_delay},
    unified::host_unified::exports::attricat::host as v16,
};
use crate::repository::ClaimedExtensionOperationRun;

/// A validated, committed-to-be batch result.
#[derive(Debug)]
pub(super) struct BatchResult {
    pub checkpoint: Value,
    pub progress: Value,
    pub done: bool,
    /// Present when the batch asked to be resumed later rather than at once.
    pub resume_after: Option<Duration>,
}

/// Why a batch did not produce a result.
#[derive(Debug)]
pub(super) enum BatchError {
    /// The host, a trap, invalid batch output or a 1.0 `process-batch` error.
    Runtime(ExtensionRuntimeError),
    /// An error a lifecycle export (`prepare`, `start`, `checkpoint`,
    /// `finish` or `cancel`) returned. It fails the run but is not a fault.
    Lifecycle(String),
    /// A typed 1.1 `process-batch` failure.
    Failure(InvocationFailure),
}

impl BatchError {
    /// Whether the component misbehaved, which quarantines a 1.1 component.
    pub(super) fn is_fault(&self) -> bool {
        match self {
            Self::Runtime(_) | Self::Failure(InvocationFailure::Fault(_)) => true,
            Self::Lifecycle(_) | Self::Failure(_) => false,
        }
    }
}

impl From<ExtensionRuntimeError> for BatchError {
    fn from(error: ExtensionRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Runtime(error) => error.fmt(formatter),
            Self::Lifecycle(message) => formatter.write_str(message),
            Self::Failure(failure) => formatter.write_str(failure.message()),
        }
    }
}

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
    resume_after_ms: Option<u64>,
}

/// The `attricat:host/operations` export.
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
    ) -> impl Future<Output = wasmtime::Result<Result<BatchOutput, BatchError>>> + Send;
}

/// Implements [`OperationsExport`] for the `operations` export module of each
/// listed world. The bodies only forward to the bindgen-generated methods.
macro_rules! impl_operations_export {
    ($($world:ident :: $interface:ident => $map:expr),+ $(,)?) => {$(
        impl OperationsExport for $world::$interface::Guest {
            type Request = $world::$interface::OperationRequest;

            fn request(
                run: &ClaimedExtensionOperationRun,
            ) -> Result<Self::Request, ExtensionRuntimeError> {
                Ok($world::$interface::OperationRequest {
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
            ) -> wasmtime::Result<Result<BatchOutput, BatchError>> {
                Ok(($map)(self.call_process_batch(store, request).await?))
            }
        }
    )+};
}

impl_operations_export!(
    v16::operations => |result: Result<v16::operations::BatchResult, String>| result
        .map(|result| BatchOutput {
            checkpoint: result.checkpoint,
            progress: result.progress,
            done: result.done,
            resume_after_ms: None,
        })
        .map_err(|error| BatchError::Runtime(ExtensionRuntimeError::Runtime(error))),
    v16::deferrable_operations => |result: Result<
        v16::deferrable_operations::BatchOutcome,
        super::unified::host_unified::attricat::host::outcomes::Failure,
    >| result
        .map(|result| BatchOutput {
            checkpoint: result.checkpoint,
            progress: result.progress,
            done: result.done,
            resume_after_ms: result.resume_after_ms,
        })
        .map_err(|failure| BatchError::Failure(failure.into())),
);

fn to_json(value: &Value) -> Result<String, ExtensionRuntimeError> {
    serde_json::to_string(value).map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))
}

fn flatten<T>(result: wasmtime::Result<Result<T, String>>) -> Result<T, BatchError> {
    result
        .map_err(|error| BatchError::Runtime(ExtensionRuntimeError::Runtime(error.to_string())))?
        .map_err(BatchError::Lifecycle)
}

/// Runs one batch of `run` against an instantiated operations export: cancel,
/// or prepare/start on the first batch, then process, validate, checkpoint and
/// finish when done.
pub(super) async fn run_operation_batch<E: OperationsExport, T: Send + 'static>(
    store: &mut Store<T>,
    operations: &E,
    run: &ClaimedExtensionOperationRun,
    cancelling: bool,
) -> Result<BatchResult, BatchError> {
    let mut request = E::request(run)?;
    if cancelling {
        flatten(
            operations
                .call(store, LifecycleCall::Cancel, &request)
                .await,
        )?;
        return Ok(BatchResult {
            checkpoint: run.checkpoint.clone(),
            progress: json!({"cancelled": true}),
            done: true,
            resume_after: None,
        });
    }
    if !run.lifecycle_started {
        for call in [LifecycleCall::Prepare, LifecycleCall::Start] {
            flatten(operations.call(store, call, &request).await)?;
        }
    }
    let output = operations
        .process_batch(store, &request)
        .await
        .map_err(|error| ExtensionRuntimeError::Runtime(error.to_string()))??;
    let (checkpoint, progress) = validate_batch_output(&output, run.max_checkpoint_bytes)?;
    // A finished run has nothing left to resume.
    let resume_after = output
        .resume_after_ms
        .filter(|_| !output.done)
        .map(component_delay);
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
    Ok(BatchResult {
        checkpoint,
        progress,
        done: output.done,
        resume_after,
    })
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
