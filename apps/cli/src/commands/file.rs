use std::path::PathBuf;

use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum RetentionHoldCommand {
    List {
        file_id: Uuid,
    },
    /// Hold a file for a number of days (requires `files.hold`).
    Create {
        file_id: Uuid,
        #[arg(long)]
        days: u32,
        #[arg(long)]
        reason: String,
    },
    /// Release an explicit hold before it expires (requires `files.hold`).
    Release {
        file_id: Uuid,
        hold_id: Uuid,
    },
}

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
    /// Upload files for a record not created yet; pass the returned file
    /// IDs to `record create --files`.
    StageUpload {
        blueprint_id: Uuid,
        attribute_code: String,
        #[arg(long = "file", required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    /// Replace the files a record's file attribute references, in order.
    SetReferences {
        record_id: Uuid,
        attribute_code: String,
        /// File ID; repeat for several. Omit to remove every file.
        #[arg(long = "file-id")]
        file_ids: Vec<Uuid>,
        /// A file the attribute currently references; repeat for each. The
        /// change fails if the current references differ.
        #[arg(long = "expected-file-id")]
        expected_file_ids: Vec<Uuid>,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Get {
        file_id: Uuid,
    },
    /// Keep a file from being purged.
    RetentionHold {
        #[command(subcommand)]
        command: RetentionHoldCommand,
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
        FileCommand::StageUpload {
            blueprint_id,
            attribute_code,
            files,
            context_id,
        } => {
            let path = format!(
                "/blueprints/{}/file-attributes/{}/staged-uploads",
                segment(blueprint_id),
                segment(&attribute_code)
            );
            api.multipart(&path, files, context_id).await
        }
        FileCommand::SetReferences {
            record_id,
            attribute_code,
            file_ids,
            expected_file_ids,
            context_id,
        } => {
            let path = format!(
                "/records/{}/file-attributes/{}/references",
                segment(record_id),
                segment(&attribute_code)
            );
            let body = json!({
                "context_id": context_id,
                "expected_file_ids": expected_file_ids,
                "file_ids": file_ids,
            });
            api.put_json(&path, body).await
        }
        FileCommand::Get { file_id } => api.get(&format!("/files/{}", segment(file_id))).await,
        FileCommand::RetentionHold { command } => retention_hold_command(api, command).await,
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

async fn retention_hold_command(
    api: &Api,
    command: RetentionHoldCommand,
) -> Result<String, CliError> {
    match command {
        RetentionHoldCommand::List { file_id } => api.get(&holds_path(file_id)).await,
        RetentionHoldCommand::Create {
            file_id,
            days,
            reason,
        } => {
            api.post_json(
                &holds_path(file_id),
                json!({ "days": days, "reason": reason }),
            )
            .await
        }
        RetentionHoldCommand::Release { file_id, hold_id } => {
            api.post(&format!(
                "{}/{}/release",
                holds_path(file_id),
                segment(hold_id)
            ))
            .await
        }
    }
}

fn holds_path(file_id: Uuid) -> String {
    format!("/files/{}/retention-holds", segment(file_id))
}
