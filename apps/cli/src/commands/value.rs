use std::path::PathBuf;

use crate::error::CliError;
use crate::input::{relationships_body_from_file, values_body_from_file};
use crate::transport::{Api, segment};
use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum ValueCommand {
    Append {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// Fail instead of overwriting when the record changed after this
        /// `updated_at` (RFC 3339).
        #[arg(long)]
        expected_updated_at: Option<String>,
    },
    Current {
        record_id: Uuid,
    },
    /// Replace a record\'s targets for the given relationship attributes.
    ReplaceRelationships {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    /// Remove the given targets from relationship attributes.
    RemoveRelationships {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
}

pub(crate) async fn value_command(api: &Api, command: ValueCommand) -> Result<String, CliError> {
    match command {
        ValueCommand::Append {
            record_id,
            file,
            context_id,
            expected_updated_at,
        } => {
            let mut body = values_body_from_file(&file, context_id)?;
            if let Some(expected_updated_at) = expected_updated_at {
                body["expected_updated_at"] = json!(expected_updated_at);
            }
            api.post_json(&format!("/records/{}/values", segment(record_id)), body)
                .await
        }
        ValueCommand::Current { record_id } => {
            api.get(&format!("/records/{}/values/current", segment(record_id)))
                .await
        }
        ValueCommand::ReplaceRelationships {
            record_id,
            file,
            context_id,
        } => {
            api.post_json(
                &format!("/records/{}/relationships/replace", segment(record_id)),
                relationships_body_from_file(&file, context_id)?,
            )
            .await
        }
        ValueCommand::RemoveRelationships {
            record_id,
            file,
            context_id,
        } => {
            api.post_json(
                &format!("/records/{}/relationships/remove", segment(record_id)),
                relationships_body_from_file(&file, context_id)?,
            )
            .await
        }
    }
}
