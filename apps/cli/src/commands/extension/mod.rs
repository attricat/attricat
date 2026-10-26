pub(crate) mod connector_job;
pub(crate) mod operation;
pub(crate) mod registry;
pub(crate) mod run;
pub(crate) mod schedule;

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{json_input, json_object_input};
use crate::transport::{Api, ZSTD_MEDIA_TYPE, segment};
use connector_job::{ConnectorJobCommand, connector_job_command};
use operation::{ExtensionOperationCommand, extension_operation_command};
use registry::{ExtensionRegistryCommand, extension_registry_command};
use run::{ExtensionRunCommand, extension_run_command};
use schedule::{ExtensionScheduleCommand, extension_schedule_command};

#[derive(Subcommand)]
pub(crate) enum ExtensionCommand {
    /// List installed extensions.
    List,
    Get {
        extension_id: String,
    },
    Install {
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        release_id: u64,
    },
    Sideload {
        #[arg(long)]
        file: PathBuf,
    },
    Upgrade {
        extension_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        repository: String,
        #[arg(long)]
        release_id: u64,
    },
    Remove {
        extension_id: String,
    },
    /// Show the server's extension runtime and supported host ABI.
    Runtime,
    WorkspaceMode {
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    Configure {
        extension_id: String,
        #[arg(long)]
        configuration: String,
    },
    Grant {
        extension_id: String,
        #[arg(long)]
        grant_kind: String,
        #[arg(long)]
        grant_id: String,
    },
    Revoke {
        extension_id: String,
        #[arg(long)]
        grant_kind: String,
        #[arg(long)]
        grant_id: String,
    },
    Enable {
        extension_id: String,
    },
    Disable {
        extension_id: String,
    },
    Quarantine {
        extension_id: String,
        #[arg(long)]
        diagnostic_code: String,
    },
    Artifact {
        extension_id: String,
        contribution_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    Storage {
        extension_id: String,
        contribution_id: String,
        release_id: Uuid,
        #[arg(long)]
        body: String,
    },
    Command {
        extension_id: String,
        contribution_id: String,
        #[arg(long)]
        release_id: Uuid,
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        payload: String,
    },
    /// Inspect, adopt or repair the record annotations an extension owns.
    Annotations {
        #[command(subcommand)]
        command: AnnotationCommand,
    },
    /// Registries to discover and install extensions from.
    Registry {
        #[command(subcommand)]
        command: ExtensionRegistryCommand,
    },
    /// Server-side operation runs.
    Operation {
        #[command(subcommand)]
        command: ExtensionOperationCommand,
    },
    /// Your own interactive runs, started from extension actions.
    Run {
        #[command(subcommand)]
        command: ExtensionRunCommand,
    },
    /// Operations that run on an interval.
    Schedule {
        #[command(subcommand)]
        command: ExtensionScheduleCommand,
    },
    /// Blueprint connector jobs that extensions run.
    ConnectorJob {
        #[command(subcommand)]
        command: ConnectorJobCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum AnnotationCommand {
    /// Inventory an extension's annotation namespace.
    Namespace { extension_id: String },
    /// Adopt existing annotations under the extension's name.
    Adopt { extension_id: String },
    /// Operator repair or cleanup of one extension's annotations on a record.
    Repair {
        extension_id: String,
        record_id: Uuid,
        /// Annotation patch JSON object or path to a JSON file.
        #[arg(long)]
        patch: String,
    },
}

pub(crate) async fn extension_command(
    api: &Api,
    command: ExtensionCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionCommand::List => api.get("/extensions").await,
        ExtensionCommand::Get { extension_id } => api.get(&extension_path(&extension_id, "")).await,
        ExtensionCommand::Install {
            owner,
            repository,
            release_id,
        } => {
            api.post_json(
                "/extensions",
                json!({ "owner": owner, "repository": repository, "release_id": release_id }),
            )
            .await
        }
        ExtensionCommand::Sideload { file } => {
            api.upload("/extensions/sideload", &file, ZSTD_MEDIA_TYPE)
                .await
        }
        ExtensionCommand::Upgrade {
            extension_id,
            owner,
            repository,
            release_id,
        } => {
            api.post_json(
                &extension_path(&extension_id, "/upgrade"),
                json!({ "owner": owner, "repository": repository, "release_id": release_id }),
            )
            .await
        }
        ExtensionCommand::Remove { extension_id } => {
            api.delete(&extension_path(&extension_id, "")).await
        }
        ExtensionCommand::Runtime => api.get("/extensions/runtime").await,
        ExtensionCommand::WorkspaceMode { enabled } => {
            api.put_json("/workspace/extensions-mode", json!({ "enabled": enabled }))
                .await
        }
        ExtensionCommand::Configure {
            extension_id,
            configuration,
        } => {
            let configuration = json_input(&configuration, "--configuration")?;
            api.put_json(
                &extension_path(&extension_id, "/configure"),
                json!({ "configuration": configuration }),
            )
            .await
        }
        ExtensionCommand::Grant {
            extension_id,
            grant_kind,
            grant_id,
        } => {
            api.post_json(
                &extension_path(&extension_id, "/grants"),
                json!({ "grant_kind": grant_kind, "grant_id": grant_id }),
            )
            .await
        }
        ExtensionCommand::Revoke {
            extension_id,
            grant_kind,
            grant_id,
        } => {
            api.delete(&extension_path(
                &extension_id,
                &format!("/grants/{}/{}", segment(&grant_kind), segment(&grant_id)),
            ))
            .await
        }
        ExtensionCommand::Enable { extension_id } => {
            api.post(&extension_path(&extension_id, "/enable")).await
        }
        ExtensionCommand::Disable { extension_id } => {
            api.post(&extension_path(&extension_id, "/disable")).await
        }
        ExtensionCommand::Quarantine {
            extension_id,
            diagnostic_code,
        } => {
            api.post_json(
                &extension_path(&extension_id, "/quarantine"),
                json!({ "diagnostic_code": diagnostic_code }),
            )
            .await
        }
        ExtensionCommand::Artifact {
            extension_id,
            contribution_id,
            output,
        } => {
            let path = contribution_path(&extension_id, &contribution_id, "/artifact");
            api.download(&path, &output, None).await
        }
        ExtensionCommand::Storage {
            extension_id,
            contribution_id,
            release_id,
            body,
        } => {
            let path = contribution_path(
                &extension_id,
                &contribution_id,
                &format!("/storage/{}", segment(release_id)),
            );
            api.post_json(&path, json_input(&body, "--body")?).await
        }
        ExtensionCommand::Command {
            extension_id,
            contribution_id,
            release_id,
            command_id,
            payload,
        } => {
            let body = json!({
                "release_id": release_id,
                "command_id": command_id,
                "payload": json_input(&payload, "--payload")?,
            });
            api.post_json(
                &contribution_path(&extension_id, &contribution_id, "/command"),
                body,
            )
            .await
        }
        ExtensionCommand::Annotations { command } => annotation_command(api, command).await,
        ExtensionCommand::Registry { command } => extension_registry_command(api, command).await,
        ExtensionCommand::Operation { command } => extension_operation_command(api, command).await,
        ExtensionCommand::Run { command } => extension_run_command(api, command).await,
        ExtensionCommand::Schedule { command } => extension_schedule_command(api, command).await,
        ExtensionCommand::ConnectorJob { command } => connector_job_command(api, command).await,
    }
}

async fn annotation_command(api: &Api, command: AnnotationCommand) -> Result<String, CliError> {
    match command {
        AnnotationCommand::Namespace { extension_id } => {
            api.get(&extension_path(&extension_id, "/annotation-namespace"))
                .await
        }
        AnnotationCommand::Adopt { extension_id } => {
            api.post(&extension_path(&extension_id, "/annotation-namespace"))
                .await
        }
        AnnotationCommand::Repair {
            extension_id,
            record_id,
            patch,
        } => {
            api.post_json(
                &extension_path(
                    &extension_id,
                    &format!("/annotation-namespace/records/{}", segment(record_id)),
                ),
                json_object_input(&patch, "--patch")?,
            )
            .await
        }
    }
}

/// `/extensions/{extension_id}` followed by `rest`.
pub(crate) fn extension_path(extension_id: &str, rest: &str) -> String {
    format!("/extensions/{}{rest}", segment(extension_id))
}

/// `/extensions/{extension_id}/{contribution_id}` followed by `rest`.
fn contribution_path(extension_id: &str, contribution_id: &str, rest: &str) -> String {
    extension_path(
        extension_id,
        &format!("/{}{rest}", segment(contribution_id)),
    )
}
