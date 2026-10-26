use clap::Subcommand;
use reqwest::{Client, Method};
use serde_json::json;
use url::Url;
use uuid::Uuid;

use crate::cli::SourceInput;
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{request, segment};

#[derive(Subcommand)]
pub(crate) enum WorkflowCommand {
    List,
    Validate(SourceInput),
    Create(SourceInput),
    Get {
        workflow_id: Uuid,
    },
    RevisionList {
        workflow_id: Uuid,
    },
    VersionGet {
        workflow_id: Uuid,
        version: i64,
    },
    Revision {
        workflow_id: Uuid,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        workflow_id: Uuid,
        version: i64,
    },
    Enable {
        workflow_id: Uuid,
        version: i64,
    },
    Disable {
        workflow_id: Uuid,
    },
    RunNow {
        workflow_id: Uuid,
        #[arg(long)]
        record_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
    },
    RunList,
    /// Per-target outcomes of `referencing_records_update` actions in a run.
    RunTargets {
        run_id: Uuid,
    },
    RunReplay {
        run_id: Uuid,
    },
}

pub(crate) async fn workflow_command(
    client: &Client,
    server: &Url,
    command: WorkflowCommand,
) -> Result<String, CliError> {
    match command {
        WorkflowCommand::List => request(client, server, Method::GET, "/workflows", None).await,
        WorkflowCommand::Validate(source) => {
            request(
                client,
                server,
                Method::POST,
                "/workflows/validate",
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Create(source) => {
            request(
                client,
                server,
                Method::POST,
                "/workflows",
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Get { workflow_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RevisionList { workflow_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}/versions", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::VersionGet {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflows/{}/versions/{version}", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::Revision {
            workflow_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/versions", segment(workflow_id)),
                Some(json!({"definition":read_source(source)?})),
            )
            .await
        }
        WorkflowCommand::Publish {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/workflows/{}/versions/{version}/publish",
                    segment(workflow_id)
                ),
                None,
            )
            .await
        }
        WorkflowCommand::Enable {
            workflow_id,
            version,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/workflows/{}/versions/{version}/enable",
                    segment(workflow_id)
                ),
                None,
            )
            .await
        }
        WorkflowCommand::Disable { workflow_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/disable", segment(workflow_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RunNow {
            workflow_id,
            record_id,
            idempotency_key,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflows/{}/run-now", segment(workflow_id)),
                Some(json!({"record_id":record_id,"idempotency_key":idempotency_key})),
            )
            .await
        }
        WorkflowCommand::RunList => {
            request(client, server, Method::GET, "/workflow-runs", None).await
        }
        WorkflowCommand::RunTargets { run_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/workflow-runs/{}/targets", segment(run_id)),
                None,
            )
            .await
        }
        WorkflowCommand::RunReplay { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/workflow-runs/{}/replay", segment(run_id)),
                None,
            )
            .await
        }
    }
}
