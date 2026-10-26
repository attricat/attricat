use std::path::PathBuf;

use clap::Subcommand;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum ExtensionRunCommand {
    /// List your 50 most recent interactive runs.
    List {
        #[arg(long)]
        extension_id: Option<String>,
    },
    /// Show status, outcome and downloadable outputs of one run.
    Get { run_id: Uuid },
    /// Request cancellation of a run that has not finished.
    Cancel { run_id: Uuid },
    /// Download an output of a completed run.
    Download {
        run_id: Uuid,
        artifact_id: Uuid,
        #[arg(long)]
        output: PathBuf,
    },
}

const RUNS_PATH: &str = "/extension-runs";

pub(crate) async fn extension_run_command(
    api: &Api,
    command: ExtensionRunCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRunCommand::List { extension_id } => {
            api.get(&with_query(RUNS_PATH, [("extension_id", extension_id)]))
                .await
        }
        ExtensionRunCommand::Get { run_id } => {
            api.get(&format!("{RUNS_PATH}/{}", segment(run_id))).await
        }
        ExtensionRunCommand::Cancel { run_id } => {
            api.post(&format!("{RUNS_PATH}/{}/cancel", segment(run_id)))
                .await
        }
        ExtensionRunCommand::Download {
            run_id,
            artifact_id,
            output,
        } => {
            let path = format!(
                "{RUNS_PATH}/{}/artifacts/{}/download",
                segment(run_id),
                segment(artifact_id)
            );
            api.download(&path, &output, None).await
        }
    }
}
