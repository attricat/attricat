use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::CliError;
use crate::input::json_object_input;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum ExtensionOperationCommand {
    Start {
        extension_id: String,
        #[arg(long)]
        operation_id: String,
        /// JSON object or path to a JSON file.
        #[arg(long)]
        input: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        input_file_id: Option<Uuid>,
        /// Where the operation delivers its output: a JSON object or a file
        /// containing it, interpreted by the extension.
        #[arg(long)]
        destination: Option<String>,
    },
    List,
    Get {
        run_id: Uuid,
    },
    Artifacts {
        run_id: Uuid,
    },
    Deliveries {
        run_id: Uuid,
    },
    Download {
        run_id: Uuid,
        artifact_id: Uuid,
        #[arg(long)]
        output: PathBuf,
    },
    Cancel {
        run_id: Uuid,
    },
    Replay {
        run_id: Uuid,
    },
}

const OPERATION_RUNS_PATH: &str = "/extension-operation-runs";

pub(crate) async fn extension_operation_command(
    api: &Api,
    command: ExtensionOperationCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionOperationCommand::Start {
            extension_id,
            operation_id,
            input,
            idempotency_key,
            input_file_id,
            destination,
        } => {
            let body = json!({
                "operation_id": operation_id,
                "input": json_object_input(&input, "--input")?,
                "idempotency_key": idempotency_key,
                "source_reference": operation_source(input_file_id),
                "destination_reference": destination_reference(destination.as_deref())?,
            });
            api.post_json(
                &format!("/extensions/{}/operations", segment(extension_id)),
                body,
            )
            .await
        }
        ExtensionOperationCommand::List => api.get(OPERATION_RUNS_PATH).await,
        ExtensionOperationCommand::Get { run_id } => {
            api.get(&format!("{OPERATION_RUNS_PATH}/{}", segment(run_id)))
                .await
        }
        ExtensionOperationCommand::Artifacts { run_id } => {
            api.get(&format!(
                "{OPERATION_RUNS_PATH}/{}/artifacts",
                segment(run_id)
            ))
            .await
        }
        ExtensionOperationCommand::Deliveries { run_id } => {
            api.get(&format!(
                "{OPERATION_RUNS_PATH}/{}/deliveries",
                segment(run_id)
            ))
            .await
        }
        ExtensionOperationCommand::Download {
            run_id,
            artifact_id,
            output,
        } => {
            let path = format!(
                "{OPERATION_RUNS_PATH}/{}/artifacts/{}/download",
                segment(run_id),
                segment(artifact_id)
            );
            api.download(&path, &output, None).await
        }
        ExtensionOperationCommand::Cancel { run_id } => {
            api.post(&format!("{OPERATION_RUNS_PATH}/{}/cancel", segment(run_id)))
                .await
        }
        ExtensionOperationCommand::Replay { run_id } => {
            api.post(&format!("{OPERATION_RUNS_PATH}/{}/replay", segment(run_id)))
                .await
        }
    }
}

/// The operation's `destination_reference`: `--destination`, or none.
pub(crate) fn destination_reference(destination: Option<&str>) -> Result<Value, CliError> {
    destination.map_or_else(
        || Ok(json!({})),
        |destination| json_object_input(destination, "--destination"),
    )
}

/// The operation's `source_reference`: the uploaded input file, if any.
pub(crate) fn operation_source(file_id: Option<Uuid>) -> Value {
    match file_id {
        Some(id) => json!({ "input_file_id": id }),
        None => json!({}),
    }
}
