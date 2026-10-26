use std::path::PathBuf;

use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{context_body_from_file, json_input};
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum ContextCommand {
    List,
    Create {
        #[arg(long, conflicts_with_all = ["code", "data", "parent_id"])]
        file: Option<PathBuf>,
        #[arg(long, requires = "data")]
        code: Option<String>,
        #[arg(long, requires = "code")]
        data: Option<String>,
        #[arg(long)]
        parent_id: Option<Uuid>,
    },
    Get {
        code: String,
    },
    Update {
        context_id: Uuid,
        #[arg(long)]
        parent_id: Uuid,
        #[arg(long)]
        data: String,
    },
    Delete {
        context_id: Uuid,
    },
}

pub(crate) async fn context_command(
    api: &Api,
    command: ContextCommand,
) -> Result<String, CliError> {
    match command {
        ContextCommand::List => api.get("/contexts").await,
        ContextCommand::Create {
            file,
            code,
            data,
            parent_id,
        } => {
            let body = match (file, code, data, parent_id) {
                (Some(file), None, None, None) => context_body_from_file(&file)?,
                (None, Some(code), Some(data), parent_id) => json!({
                    "code": code,
                    "data": json_input(&data, "--data")?,
                    "parent_id": parent_id,
                }),
                _ => {
                    return Err(CliError::Input(
                        "provide --file or both --code and --data".to_owned(),
                    ));
                }
            };
            api.post_json("/contexts", body).await
        }
        ContextCommand::Get { code } => api.get(&format!("/contexts/{}", segment(&code))).await,
        ContextCommand::Update {
            context_id,
            parent_id,
            data,
        } => {
            api.put_json(
                &format!("/contexts/id/{}", segment(context_id)),
                json!({ "parent_id": parent_id, "data": json_input(&data, "--data")? }),
            )
            .await
        }
        ContextCommand::Delete { context_id } => {
            api.delete(&format!("/contexts/id/{}", segment(context_id)))
                .await
        }
    }
}
