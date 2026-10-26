pub(crate) mod connection;
pub(crate) mod connector_job;
pub(crate) mod export;
pub(crate) mod operation;
pub(crate) mod registry;
pub(crate) mod run;
pub(crate) mod schedule;

use std::path::PathBuf;

use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{json_input, json_object_input, read_secret_stdin};
use crate::transport::{Api, ZSTD_MEDIA_TYPE, segment};
use connection::{ConnectionCommand, connection_command};
use connector_job::{ConnectorJobCommand, connector_job_command};
use export::{ExportCommand, export_command};
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
    /// Credentials an extension uses to reach an external system.
    Connection {
        #[command(subcommand)]
        command: ConnectionCommand,
    },
    /// Where connector exports sent records, and resending them.
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    /// Request budgets for an extension's outbound hosts, lowered below what
    /// it declares.
    Budget {
        #[command(subcommand)]
        command: BudgetCommand,
    },
    /// Workspace secrets that extension configuration can reference.
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    /// The order and visibility of extension contributions in each outlet.
    Layout {
        #[command(subcommand)]
        command: LayoutCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum BudgetCommand {
    List {
        extension_id: String,
    },
    /// Allow at most `--requests` per `--per-seconds` to one host.
    Set {
        extension_id: String,
        host_permission_id: String,
        #[arg(long)]
        requests: i32,
        #[arg(long)]
        per_seconds: i32,
    },
    /// Return a host to the budget the extension declares.
    Reset {
        extension_id: String,
        host_permission_id: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum SecretCommand {
    /// List secret names; values are never returned.
    List,
    /// Create or replace a secret with the value on standard input.
    Set {
        name: String,
        #[arg(long, required = true)]
        value_stdin: bool,
    },
    Delete {
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum LayoutCommand {
    Get,
    /// Replace the layout with a JSON document or a file containing it.
    Set {
        #[arg(long)]
        layout: String,
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
        ExtensionCommand::Connection { command } => connection_command(api, command).await,
        ExtensionCommand::Export { command } => export_command(api, command).await,
        ExtensionCommand::Budget { command } => budget_command(api, command).await,
        ExtensionCommand::Secret { command } => match command {
            SecretCommand::List => api.get(SECRETS_PATH).await,
            SecretCommand::Set { name, .. } => {
                let value = read_secret_stdin("secret value")?;
                api.put_json(
                    &format!("{SECRETS_PATH}/{}", segment(&name)),
                    json!({ "value": value }),
                )
                .await
            }
            SecretCommand::Delete { name } => {
                api.delete(&format!("{SECRETS_PATH}/{}", segment(&name)))
                    .await
            }
        },
        ExtensionCommand::Layout { command } => match command {
            LayoutCommand::Get => api.get(LAYOUT_PATH).await,
            LayoutCommand::Set { layout } => {
                api.put_json(LAYOUT_PATH, json_input(&layout, "--layout")?)
                    .await
            }
        },
    }
}

const SECRETS_PATH: &str = "/workspace/extension-secrets";
const LAYOUT_PATH: &str = "/workspace/extension-layout";

async fn budget_command(api: &Api, command: BudgetCommand) -> Result<String, CliError> {
    match command {
        BudgetCommand::List { extension_id } => {
            api.get(&extension_path(&extension_id, "/host-permission-budgets"))
                .await
        }
        BudgetCommand::Set {
            extension_id,
            host_permission_id,
            requests,
            per_seconds,
        } => {
            api.put_json(
                &budget_path(&extension_id, &host_permission_id),
                json!({ "requests": requests, "per_seconds": per_seconds }),
            )
            .await
        }
        BudgetCommand::Reset {
            extension_id,
            host_permission_id,
        } => {
            api.delete(&budget_path(&extension_id, &host_permission_id))
                .await
        }
    }
}

fn budget_path(extension_id: &str, host_permission_id: &str) -> String {
    extension_path(
        extension_id,
        &format!("/host-permission-budgets/{}", segment(host_permission_id)),
    )
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
pub(crate) fn contribution_path(extension_id: &str, contribution_id: &str, rest: &str) -> String {
    extension_path(
        extension_id,
        &format!("/{}{rest}", segment(contribution_id)),
    )
}
