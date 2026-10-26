use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::json;
use uuid::Uuid;

use crate::commands::extension::contribution_path;
use crate::error::CliError;
use crate::input::json_input;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum ExtensionRunCommand {
    /// Start an action's operation on selected records, as the web app does.
    Start {
        extension_id: String,
        contribution_id: String,
        #[arg(long)]
        release_id: Uuid,
        #[arg(long)]
        operation_id: String,
        /// Operation input: JSON or a file containing it.
        #[arg(long)]
        input: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// Selected record ID; repeat for several.
        #[arg(long = "record-id", required = true)]
        record_ids: Vec<Uuid>,
    },
    /// List your 50 most recent interactive runs.
    List {
        #[arg(long)]
        extension_id: Option<String>,
    },
    /// Show status, outcome and downloadable outputs of one run.
    Get {
        run_id: Uuid,
        #[command(flatten)]
        scope: RunScope,
    },
    /// Request cancellation of a run that has not finished.
    Cancel {
        run_id: Uuid,
        #[command(flatten)]
        scope: RunScope,
    },
    /// Download an output of a completed run.
    Download {
        run_id: Uuid,
        artifact_id: Uuid,
        #[arg(long)]
        output: PathBuf,
        #[command(flatten)]
        scope: RunScope,
    },
}

/// Which runs a lookup considers: every run you may inspect, or only yours.
#[derive(Args, Clone, Copy)]
pub(crate) struct RunScope {
    /// Only consider runs you started.
    #[arg(long)]
    own: bool,
}

impl RunScope {
    fn query(self) -> [(&'static str, Option<String>); 1] {
        [("scope", self.own.then(|| "own".to_owned()))]
    }
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
        ExtensionRunCommand::Start {
            extension_id,
            contribution_id,
            release_id,
            operation_id,
            input,
            idempotency_key,
            blueprint_id,
            blueprint_version,
            context_id,
            record_ids,
        } => {
            let body = json!({
                "release_id": release_id,
                "operation_id": operation_id,
                "input": json_input(&input, "--input")?,
                "idempotency_key": idempotency_key,
                "selection": {
                    "blueprint_id": blueprint_id,
                    "blueprint_version": blueprint_version,
                    "context_id": context_id,
                    "record_ids": record_ids,
                },
            });
            api.post_json(
                &contribution_path(&extension_id, &contribution_id, "/operations"),
                body,
            )
            .await
        }
        ExtensionRunCommand::Get { run_id, scope } => {
            api.get(&with_query(
                &format!("{RUNS_PATH}/{}", segment(run_id)),
                scope.query(),
            ))
            .await
        }
        ExtensionRunCommand::Cancel { run_id, scope } => {
            api.post(&with_query(
                &format!("{RUNS_PATH}/{}/cancel", segment(run_id)),
                scope.query(),
            ))
            .await
        }
        ExtensionRunCommand::Download {
            run_id,
            artifact_id,
            output,
            scope,
        } => {
            let path = with_query(
                &format!(
                    "{RUNS_PATH}/{}/artifacts/{}/download",
                    segment(run_id),
                    segment(artifact_id)
                ),
                scope.query(),
            );
            api.download(&path, &output, None).await
        }
    }
}
