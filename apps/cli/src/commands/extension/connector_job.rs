use clap::{Subcommand, ValueEnum};
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment};

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
    /// Send records again: the given ones, or every record in one state.
    Resync {
        job_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
        /// Record ID; repeat for several.
        #[arg(long = "record-id", conflicts_with = "status")]
        record_ids: Vec<Uuid>,
        #[arg(long, value_enum)]
        status: Option<ExportState>,
    },
}

/// Where a record's export through a connection stands.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ExportState {
    Pending,
    Retrying,
    Uncertain,
    Rejected,
    Synced,
    Withdrawn,
}

impl ExportState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Retrying => "retrying",
            Self::Uncertain => "uncertain",
            Self::Rejected => "rejected",
            Self::Synced => "synced",
            Self::Withdrawn => "withdrawn",
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
        ConnectorJobCommand::Resync {
            job_id,
            idempotency_key,
            record_ids,
            status,
        } => {
            let mut body = json!({ "idempotency_key": idempotency_key });
            if !record_ids.is_empty() {
                body["record_ids"] = json!(record_ids);
            }
            if let Some(status) = status {
                body["status"] = json!(status.as_str());
            }
            api.post_json(
                &format!("/blueprint-connector-jobs/{}/resync", segment(job_id)),
                body,
            )
            .await
        }
    }
}
