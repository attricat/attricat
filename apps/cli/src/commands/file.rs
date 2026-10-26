use std::path::PathBuf;

use clap::Subcommand;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum FileCommand {
    Upload {
        record_id: Uuid,
        attribute_code: String,
        #[arg(long = "file", required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Get {
        file_id: Uuid,
    },
    DownloadOriginal {
        file_id: Uuid,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        range: Option<String>,
    },
    DownloadVariant {
        file_id: Uuid,
        kind: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        range: Option<String>,
    },
}

pub(crate) async fn file_command(api: &Api, command: FileCommand) -> Result<String, CliError> {
    match command {
        FileCommand::Upload {
            record_id,
            attribute_code,
            files,
            context_id,
        } => {
            api.multipart(
                &format!(
                    "/records/{}/file-attributes/{}/uploads",
                    segment(record_id),
                    segment(&attribute_code)
                ),
                files,
                context_id,
            )
            .await
        }
        FileCommand::Get { file_id } => api.get(&format!("/files/{}", segment(file_id))).await,
        FileCommand::DownloadOriginal {
            file_id,
            output,
            range,
        } => {
            api.download(
                &format!("/files/{}/download", segment(file_id)),
                &output,
                range.as_deref(),
            )
            .await
        }
        FileCommand::DownloadVariant {
            file_id,
            kind,
            output,
            range,
        } => {
            api.download(
                &format!(
                    "/files/{}/variants/{}/download",
                    segment(file_id),
                    segment(&kind)
                ),
                &output,
                range.as_deref(),
            )
            .await
        }
    }
}
