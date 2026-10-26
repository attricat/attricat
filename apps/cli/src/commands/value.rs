use std::path::PathBuf;

use crate::error::CliError;
use crate::input::{relationships_body_from_file, values_body_from_file};
use crate::transport::{Api, segment};
use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub(crate) enum ValueCommand {
    Append {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Current {
        record_id: Uuid,
    },
    Replace {
        record_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Remove {
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
        } => {
            api.post_json(
                &format!("/records/{}/values", segment(record_id)),
                values_body_from_file(&file, context_id)?,
            )
            .await
        }
        ValueCommand::Current { record_id } => {
            api.get(&format!("/records/{}/values/current", segment(record_id)))
                .await
        }
        ValueCommand::Replace {
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
        ValueCommand::Remove {
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
