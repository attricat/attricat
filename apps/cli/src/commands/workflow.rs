use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::cli::{PageArgs, SourceInput};
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum WorkflowCommand {
    List,
    Validate(SourceInput),
    Create(SourceInput),
    Get {
        workflow_id: Uuid,
    },
    ListVersions {
        workflow_id: Uuid,
    },
    GetVersion {
        workflow_id: Uuid,
        version: i64,
    },
    CreateVersion {
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
    RunList {
        #[command(flatten)]
        page: PageArgs,
    },
    /// Per-target outcomes of `referencing_records_update` actions in a run.
    RunTargets {
        run_id: Uuid,
    },
    RunReplay {
        run_id: Uuid,
    },
}

pub(crate) async fn workflow_command(
    api: &Api,
    command: WorkflowCommand,
) -> Result<String, CliError> {
    match command {
        WorkflowCommand::List => api.get("/workflows").await,
        WorkflowCommand::Validate(source) => {
            api.post_json(
                "/workflows/validate",
                json!({"definition":read_source(source)?}),
            )
            .await
        }
        WorkflowCommand::Create(source) => {
            api.post_json("/workflows", json!({"definition":read_source(source)?}))
                .await
        }
        WorkflowCommand::Get { workflow_id } => {
            api.get(&format!("/workflows/{}", segment(workflow_id)))
                .await
        }
        WorkflowCommand::ListVersions { workflow_id } => {
            api.get(&format!("/workflows/{}/versions", segment(workflow_id)))
                .await
        }
        WorkflowCommand::GetVersion {
            workflow_id,
            version,
        } => {
            api.get(&format!(
                "/workflows/{}/versions/{version}",
                segment(workflow_id)
            ))
            .await
        }
        WorkflowCommand::CreateVersion {
            workflow_id,
            source,
        } => {
            api.post_json(
                &format!("/workflows/{}/versions", segment(workflow_id)),
                json!({"definition":read_source(source)?}),
            )
            .await
        }
        WorkflowCommand::Publish {
            workflow_id,
            version,
        } => {
            api.post(&format!(
                "/workflows/{}/versions/{version}/publish",
                segment(workflow_id)
            ))
            .await
        }
        WorkflowCommand::Enable {
            workflow_id,
            version,
        } => {
            api.post(&format!(
                "/workflows/{}/versions/{version}/enable",
                segment(workflow_id)
            ))
            .await
        }
        WorkflowCommand::Disable { workflow_id } => {
            api.post(&format!("/workflows/{}/disable", segment(workflow_id)))
                .await
        }
        WorkflowCommand::RunNow {
            workflow_id,
            record_id,
            idempotency_key,
        } => {
            api.post_json(
                &format!("/workflows/{}/run-now", segment(workflow_id)),
                json!({"record_id":record_id,"idempotency_key":idempotency_key}),
            )
            .await
        }
        WorkflowCommand::RunList { page } => {
            api.get(&with_query("/workflow-runs", page.query())).await
        }
        WorkflowCommand::RunTargets { run_id } => {
            api.get(&format!("/workflow-runs/{}/targets", segment(run_id)))
                .await
        }
        WorkflowCommand::RunReplay { run_id } => {
            api.post(&format!("/workflow-runs/{}/replay", segment(run_id)))
                .await
        }
    }
}
