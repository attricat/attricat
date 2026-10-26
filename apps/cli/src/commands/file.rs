use std::path::PathBuf;

use clap::Subcommand;
use reqwest::{Client, Method};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{multipart_upload, raw_download, request, segment};

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
    Metadata {
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

pub(crate) async fn file_command(
    client: &Client,
    server: &Url,
    command: FileCommand,
) -> Result<String, CliError> {
    match command {
        FileCommand::Upload {
            record_id,
            attribute_code,
            files,
            context_id,
        } => {
            multipart_upload(
                client,
                server,
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
        FileCommand::Metadata { file_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/files/{}", segment(file_id)),
                None,
            )
            .await
        }
        FileCommand::DownloadOriginal {
            file_id,
            output,
            range,
        } => {
            raw_download(
                client,
                server,
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
            raw_download(
                client,
                server,
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
