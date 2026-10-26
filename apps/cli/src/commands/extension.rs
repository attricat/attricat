use std::path::PathBuf;

use clap::Subcommand;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{json_input, json_object_input};
use crate::transport::{ZSTD_MEDIA_TYPE, raw_download, raw_upload, request, segment};

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
        #[arg(long, value_parser = clap::value_parser!(i32).range(i64::from(MIN_SCHEDULE_INTERVAL_SECONDS)..=i64::from(MAX_SCHEDULE_INTERVAL_SECONDS)))]
        interval_seconds: i32,
    },
    Update {
        schedule_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long, value_parser = clap::value_parser!(i32).range(i64::from(MIN_SCHEDULE_INTERVAL_SECONDS)..=i64::from(MAX_SCHEDULE_INTERVAL_SECONDS)))]
        interval_seconds: i32,
    },
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
    client: &Client,
    server: &Url,
    command: ExtensionRegistryCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRegistryCommand::List => {
            request(client, server, Method::GET, "/extension-registries", None).await
        }
        ExtensionRegistryCommand::Add { source } => {
            request(
                client,
                server,
                Method::POST,
                "/extension-registries",
                Some(json!({"source":source})),
            )
            .await
        }
        ExtensionRegistryCommand::Remove { registry_id } => {
            request(
                client,
                server,
                Method::DELETE,
                &format!("/extension-registries/{}", segment(registry_id)),
                None,
            )
            .await
        }
        ExtensionRegistryCommand::Discover => {
            request(
                client,
                server,
                Method::GET,
                "/extension-registries/discover",
                None,
            )
            .await
        }
        ExtensionRegistryCommand::Details { owner, repository } => {
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/extension-registries/extensions/{}/{}",
                    segment(&owner),
                    segment(&repository)
                ),
                None,
            )
            .await
        }
    }
}

pub(crate) async fn extension_command(
    client: &Client,
    server: &Url,
    command: ExtensionCommand,
) -> Result<String, CliError> {
    match command {
    ExtensionCommand::List => request(client,server,Method::GET,"/extensions",None).await,
    ExtensionCommand::Detail { extension_id } => request(client,server,Method::GET,&format!("/extensions/{}",segment(&extension_id)),None).await,
    ExtensionCommand::Remove { extension_id } => request(client,server,Method::DELETE,&format!("/extensions/{}",segment(&extension_id)),None).await,
    ExtensionCommand::Runtime => request(client,server,Method::GET,"/extensions/runtime",None).await,
    ExtensionCommand::WorkspaceMode { enabled } => request(client,server,Method::PUT,"/workspace/extensions-mode",Some(json!({"enabled":enabled}))).await,
    ExtensionCommand::Install { owner, repository, release_id } => request(client,server,Method::POST,"/extensions",Some(json!({"owner":owner,"repository":repository,"release_id":release_id}))).await,
    ExtensionCommand::Sideload { file } => raw_upload(client,server,"/extensions/sideload",&file,ZSTD_MEDIA_TYPE).await,
    ExtensionCommand::Upgrade { extension_id, owner, repository, release_id } => request(client,server,Method::POST,&format!("/extensions/{}/upgrade",segment(&extension_id)),Some(json!({"owner":owner,"repository":repository,"release_id":release_id}))).await,
    ExtensionCommand::Configure { extension_id, configuration } => request(client,server,Method::PUT,&format!("/extensions/{}/configure",segment(&extension_id)),Some(json!({"configuration":json_input(&configuration,"--configuration")?}))).await,
    ExtensionCommand::Grant { extension_id, grant_kind, grant_id } => request(client,server,Method::POST,&format!("/extensions/{}/grants",segment(&extension_id)),Some(json!({"grant_kind":grant_kind,"grant_id":grant_id}))).await,
    ExtensionCommand::Revoke { extension_id, grant_kind, grant_id } => request(client,server,Method::DELETE,&format!("/extensions/{}/grants/{}/{}",segment(&extension_id),segment(&grant_kind),segment(&grant_id)),None).await,
    ExtensionCommand::Enable { extension_id } => request(client,server,Method::POST,&format!("/extensions/{}/enable",segment(&extension_id)),None).await,
    ExtensionCommand::Disable { extension_id } => request(client,server,Method::POST,&format!("/extensions/{}/disable",segment(&extension_id)),None).await,
    ExtensionCommand::Quarantine { extension_id, diagnostic_code } => request(client,server,Method::POST,&format!("/extensions/{}/quarantine",segment(&extension_id)),Some(json!({"diagnostic_code":diagnostic_code}))).await,
    ExtensionCommand::Artifact { extension_id, contribution_id, output } => raw_download(client,server,&format!("/extensions/{}/{}/artifact",segment(&extension_id),segment(&contribution_id)),&output,None).await,
    ExtensionCommand::Storage { extension_id, contribution_id, release_id, body } => request(client,server,Method::POST,&format!("/extensions/{}/{}/storage/{}",segment(&extension_id),segment(&contribution_id),segment(release_id)),Some(json_input(&body,"--body")?)).await,
    ExtensionCommand::AnnotationNamespace { extension_id, adopt } => request(client,server,if adopt { Method::POST } else { Method::GET },&format!("/extensions/{}/annotation-namespace",segment(&extension_id)),None).await,
    ExtensionCommand::RepairAnnotations { extension_id, record_id, patch } => request(client,server,Method::POST,&format!("/extensions/{}/annotation-namespace/records/{}",segment(&extension_id),segment(record_id)),Some(json_object_input(&patch, "--patch")?)).await,
    ExtensionCommand::Command { extension_id, contribution_id, release_id, command_id, payload } => request(client,server,Method::POST,&format!("/extensions/{}/{}/command",segment(&extension_id),segment(&contribution_id)),Some(json!({"release_id":release_id,"command_id":command_id,"payload":json_input(&payload,"--payload")?}))).await,
}
}

pub(crate) async fn extension_run_command(
    client: &Client,
    server: &Url,
    command: ExtensionRunCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRunCommand::List { extension_id } => {
            let path = match extension_id {
                Some(id) => format!("/extension-runs?extension_id={}", segment(id)),
                None => "/extension-runs".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        ExtensionRunCommand::Show { run_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/extension-runs/{}", segment(run_id)),
                None,
            )
            .await
        }
        ExtensionRunCommand::Cancel { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/extension-runs/{}/cancel", segment(run_id)),
                None,
            )
            .await
        }
        ExtensionRunCommand::Download {
            run_id,
            artifact_id,
            output,
        } => {
            raw_download(
                client,
                server,
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
    client: &Client,
    server: &Url,
    command: ExtensionOperationCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionOperationCommand::Start { extension_id, operation_id, input, idempotency_key, input_file_id } => {
            request(client, server, Method::POST,
                &format!("/extensions/{}/operations", segment(extension_id)),
                Some(json!({"operation_id":operation_id,"input":json_object_input(&input, "--input")?,
                    "idempotency_key":idempotency_key,"source_reference":operation_source(input_file_id),
                    "destination_reference":{}}))).await
        }
        ExtensionOperationCommand::List => request(client, server, Method::GET, "/extension-operation-runs", None).await,
        ExtensionOperationCommand::Show { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{}", segment(run_id)), None).await,
        ExtensionOperationCommand::Artifacts { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{}/artifacts", segment(run_id)), None).await,
        ExtensionOperationCommand::Deliveries { run_id } => request(client, server, Method::GET, &format!("/extension-operation-runs/{}/deliveries", segment(run_id)), None).await,
        ExtensionOperationCommand::Download { run_id, artifact_id, output } => raw_download(client, server, &format!("/extension-operation-runs/{}/artifacts/{}/download", segment(run_id), segment(artifact_id)), &output, None).await,
        ExtensionOperationCommand::Cancel { run_id } => request(client, server, Method::POST, &format!("/extension-operation-runs/{}/cancel", segment(run_id)), None).await,
        ExtensionOperationCommand::Replay { run_id } => request(client, server, Method::POST, &format!("/extension-operation-runs/{}/replay", segment(run_id)), None).await,
    }
}

pub(crate) fn operation_source(file_id: Option<Uuid>) -> Value {
    match file_id {
        Some(id) => json!({"input_file_id":id}),
        None => json!({}),
    }
}

pub(crate) async fn extension_schedule_command(
    client: &Client,
    server: &Url,
    command: ExtensionScheduleCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionScheduleCommand::List => {
            request(
                client,
                server,
                Method::GET,
                "/extension-operation-schedules",
                None,
            )
            .await
        }
        ExtensionScheduleCommand::Create {
            extension_id,
            operation_id,
            input,
            input_file_id,
            interval_seconds,
        } => request(
            client,
            server,
            Method::POST,
            &format!("/extensions/{}/operation-schedules", segment(extension_id)),
            Some(
                json!({"operation_id":operation_id,"input":json_object_input(&input, "--input")?,
                    "source_reference":operation_source(input_file_id),"destination_reference":{},
                    "interval_seconds":interval_seconds}),
            ),
        )
        .await,
        ExtensionScheduleCommand::Update {
            schedule_id,
            enabled,
            interval_seconds,
        } => {
            request(
                client,
                server,
                Method::PATCH,
                &format!("/extension-operation-schedules/{}", segment(schedule_id)),
                Some(json!({"enabled":enabled,"interval_seconds":interval_seconds})),
            )
            .await
        }
    }
}

pub(crate) async fn connector_job_command(
    client: &Client,
    server: &Url,
    command: ConnectorJobCommand,
) -> Result<String, CliError> {
    match command {
        ConnectorJobCommand::List { blueprint_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/blueprints/{}/connector-jobs", segment(blueprint_id)),
                None,
            )
            .await
        }
        ConnectorJobCommand::Run {
            job_id,
            idempotency_key,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/blueprint-connector-jobs/{}/run", segment(job_id)),
                Some(json!({"idempotency_key":idempotency_key})),
            )
            .await
        }
    }
}
