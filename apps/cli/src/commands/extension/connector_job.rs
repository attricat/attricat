use clap::Subcommand;
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
