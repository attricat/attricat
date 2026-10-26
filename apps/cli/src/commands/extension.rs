use std::path::PathBuf;

use clap::Subcommand;
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{json_input, json_object_input};
use crate::transport::{Api, ZSTD_MEDIA_TYPE, segment, with_query};

pub(crate) const MIN_SCHEDULE_INTERVAL_SECONDS: i32 = 60;

/// Thirty days.
pub(crate) const MAX_SCHEDULE_INTERVAL_SECONDS: i32 = 30 * 24 * 60 * 60;

#[derive(Subcommand)]
pub(crate) enum ExtensionRegistryCommand {
    List,
    Add {
        #[arg(long)]
        source: String,
    },
    Remove {
        registry_id: Uuid,
    },
    Discover,
    Details {
        owner: String,
        repository: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ExtensionCommand {
    List,
    Detail {
        extension_id: String,
    },
    Remove {
        extension_id: String,
    },
    Runtime,
    WorkspaceMode {
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
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
        grant_kind: String,
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
    /// Inventory an extension's annotation namespace, or adopt existing
    /// annotations under its name with --adopt.
    AnnotationNamespace {
        extension_id: String,
        #[arg(long)]
        adopt: bool,
    },
    /// Operator repair or cleanup of one extension's annotations on a record.
    RepairAnnotations {
        extension_id: String,
        record_id: Uuid,
        /// Annotation patch JSON object or path to a JSON file.
        #[arg(long)]
        patch: String,
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
}

#[derive(Subcommand)]
pub(crate) enum ExtensionRunCommand {
    /// List your 50 most recent interactive runs.
    List {
        #[arg(long)]
        extension_id: Option<String>,
    },
    /// Show status, outcome and downloadable outputs of one run.
    Show { run_id: Uuid },
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
    },
    List,
    Show {
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

#[derive(Subcommand)]
pub(crate) enum ExtensionScheduleCommand {
    List,
    Create {
        extension_id: String,
        #[arg(long)]
        operation_id: String,
        /// JSON object or path to a JSON file.
        #[arg(long)]
        input: String,
        #[arg(long)]
        input_file_id: Option<Uuid>,
        #[arg(long, value_parser = schedule_interval_seconds())]
        interval_seconds: i32,
    },
    Update {
        schedule_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long, value_parser = schedule_interval_seconds())]
        interval_seconds: i32,
    },
}

fn schedule_interval_seconds() -> clap::builder::RangedI64ValueParser<i32> {
    clap::value_parser!(i32)
        .range(i64::from(MIN_SCHEDULE_INTERVAL_SECONDS)..=i64::from(MAX_SCHEDULE_INTERVAL_SECONDS))
}

#[derive(Subcommand)]
pub(crate) enum ConnectorJobCommand {
    List {
        blueprint_id: Uuid,
    },
    Run {
        job_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
    },
}

pub(crate) async fn extension_registry_command(
    api: &Api,
    command: ExtensionRegistryCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRegistryCommand::List => api.get("/extension-registries").await,
        ExtensionRegistryCommand::Add { source } => {
            api.post_json("/extension-registries", json!({ "source": source }))
                .await
        }
        ExtensionRegistryCommand::Remove { registry_id } => {
            api.delete(&format!("/extension-registries/{}", segment(registry_id)))
                .await
        }
        ExtensionRegistryCommand::Discover => api.get("/extension-registries/discover").await,
        ExtensionRegistryCommand::Details { owner, repository } => {
            api.get(&format!(
                "/extension-registries/extensions/{}/{}",
                segment(&owner),
                segment(&repository)
            ))
            .await
        }
    }
}

pub(crate) async fn extension_command(
    api: &Api,
    command: ExtensionCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionCommand::List => api.get("/extensions").await,
        ExtensionCommand::Detail { extension_id } => {
            api.get(&format!("/extensions/{}", segment(&extension_id)))
                .await
        }
        ExtensionCommand::Remove { extension_id } => {
            api.delete(&format!("/extensions/{}", segment(&extension_id)))
                .await
        }
        ExtensionCommand::Runtime => api.get("/extensions/runtime").await,
        ExtensionCommand::WorkspaceMode { enabled } => {
            api.put_json("/workspace/extensions-mode", json!({ "enabled": enabled }))
                .await
        }
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
                &format!("/extensions/{}/upgrade", segment(&extension_id)),
                json!({ "owner": owner, "repository": repository, "release_id": release_id }),
            )
            .await
        }
        ExtensionCommand::Configure {
            extension_id,
            configuration,
        } => {
            api.put_json(
                &format!("/extensions/{}/configure", segment(&extension_id)),
                json!({"configuration":json_input(&configuration,"--configuration")?}),
            )
            .await
        }
        ExtensionCommand::Grant {
            extension_id,
            grant_kind,
            grant_id,
        } => {
            api.post_json(
                &format!("/extensions/{}/grants", segment(&extension_id)),
                json!({ "grant_kind": grant_kind, "grant_id": grant_id }),
            )
            .await
        }
        ExtensionCommand::Revoke {
            extension_id,
            grant_kind,
            grant_id,
        } => {
            api.delete(&format!(
                "/extensions/{}/grants/{}/{}",
                segment(&extension_id),
                segment(&grant_kind),
                segment(&grant_id)
            ))
            .await
        }
        ExtensionCommand::Enable { extension_id } => {
            api.post(&format!("/extensions/{}/enable", segment(&extension_id)))
                .await
        }
        ExtensionCommand::Disable { extension_id } => {
            api.post(&format!("/extensions/{}/disable", segment(&extension_id)))
                .await
        }
        ExtensionCommand::Quarantine {
            extension_id,
            diagnostic_code,
        } => {
            api.post_json(
                &format!("/extensions/{}/quarantine", segment(&extension_id)),
                json!({ "diagnostic_code": diagnostic_code }),
            )
            .await
        }
        ExtensionCommand::Artifact {
            extension_id,
            contribution_id,
            output,
        } => {
            api.download(
                &format!(
                    "/extensions/{}/{}/artifact",
                    segment(&extension_id),
                    segment(&contribution_id)
                ),
                &output,
                None,
            )
            .await
        }
        ExtensionCommand::Storage {
            extension_id,
            contribution_id,
            release_id,
            body,
        } => {
            api.post_json(
                &format!(
                    "/extensions/{}/{}/storage/{}",
                    segment(&extension_id),
                    segment(&contribution_id),
                    segment(release_id)
                ),
                json_input(&body, "--body")?,
            )
            .await
        }
        ExtensionCommand::AnnotationNamespace {
            extension_id,
            adopt,
        } => {
            api.send(
                if adopt { Method::POST } else { Method::GET },
                &format!(
                    "/extensions/{}/annotation-namespace",
                    segment(&extension_id)
                ),
                None,
            )
            .await
        }
        ExtensionCommand::RepairAnnotations {
            extension_id,
            record_id,
            patch,
        } => {
            api.post_json(
                &format!(
                    "/extensions/{}/annotation-namespace/records/{}",
                    segment(&extension_id),
                    segment(record_id)
                ),
                json_object_input(&patch, "--patch")?,
            )
            .await
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
                &format!(
                    "/extensions/{}/{}/command",
                    segment(&extension_id),
                    segment(&contribution_id)
                ),
                body,
            )
            .await
        }
    }
}

pub(crate) async fn extension_run_command(
    api: &Api,
    command: ExtensionRunCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRunCommand::List { extension_id } => {
            api.get(&with_query(
                "/extension-runs",
                [("extension_id", extension_id)],
            ))
            .await
        }
        ExtensionRunCommand::Show { run_id } => {
            api.get(&format!("/extension-runs/{}", segment(run_id)))
                .await
        }
        ExtensionRunCommand::Cancel { run_id } => {
            api.post(&format!("/extension-runs/{}/cancel", segment(run_id)))
                .await
        }
        ExtensionRunCommand::Download {
            run_id,
            artifact_id,
            output,
        } => {
            api.download(
                &format!(
                    "/extension-runs/{}/artifacts/{}/download",
                    segment(run_id),
                    segment(artifact_id)
                ),
                &output,
                None,
            )
            .await
        }
    }
}

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
        } => {
            let body = json!({
                "operation_id": operation_id,
                "input": json_object_input(&input, "--input")?,
                "idempotency_key": idempotency_key,
                "source_reference": operation_source(input_file_id),
                "destination_reference": {},
            });
            api.post_json(
                &format!("/extensions/{}/operations", segment(extension_id)),
                body,
            )
            .await
        }
        ExtensionOperationCommand::List => api.get(OPERATION_RUNS_PATH).await,
        ExtensionOperationCommand::Show { run_id } => {
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

const OPERATION_RUNS_PATH: &str = "/extension-operation-runs";

pub(crate) fn operation_source(file_id: Option<Uuid>) -> Value {
    match file_id {
        Some(id) => json!({ "input_file_id": id }),
        None => json!({}),
    }
}

pub(crate) async fn extension_schedule_command(
    api: &Api,
    command: ExtensionScheduleCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionScheduleCommand::List => api.get("/extension-operation-schedules").await,
        ExtensionScheduleCommand::Create {
            extension_id,
            operation_id,
            input,
            input_file_id,
            interval_seconds,
        } => {
            api.post_json(
                &format!("/extensions/{}/operation-schedules", segment(extension_id)),
                json!({"operation_id":operation_id,"input":json_object_input(&input, "--input")?,
                    "source_reference":operation_source(input_file_id),"destination_reference":{},
                    "interval_seconds":interval_seconds}),
            )
            .await
        }
        ExtensionScheduleCommand::Update {
            schedule_id,
            enabled,
            interval_seconds,
        } => {
            api.patch_json(
                &format!("/extension-operation-schedules/{}", segment(schedule_id)),
                json!({ "enabled": enabled, "interval_seconds": interval_seconds }),
            )
            .await
        }
    }
}

pub(crate) async fn connector_job_command(
    api: &Api,
    command: ConnectorJobCommand,
) -> Result<String, CliError> {
    match command {
        ConnectorJobCommand::List { blueprint_id } => {
            api.get(&format!(
                "/blueprints/{}/connector-jobs",
                segment(blueprint_id)
            ))
            .await
        }
        ConnectorJobCommand::Run {
            job_id,
            idempotency_key,
        } => {
            api.post_json(
                &format!("/blueprint-connector-jobs/{}/run", segment(job_id)),
                json!({ "idempotency_key": idempotency_key }),
            )
            .await
        }
    }
}
