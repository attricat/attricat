use clap::Subcommand;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::commands::extension::connector_job::ExportState;
use crate::commands::extension::extension_path;
use crate::error::CliError;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum ExportCommand {
    /// Export counts by state across the workspace.
    Summary,
    /// One extension's export mappings, a page at a time.
    Status {
        extension_id: String,
        #[arg(long)]
        connection: Option<String>,
        #[arg(long, value_enum)]
        status: Option<ExportState>,
        /// Continue after a page: its `next` value.
        #[arg(long)]
        after: Option<Uuid>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Mark records for export again, and optionally run the jobs now.
    Retry {
        extension_id: String,
        #[arg(long)]
        connection: Option<String>,
        /// Record ID; repeat for several.
        #[arg(long = "record-id")]
        record_ids: Vec<Uuid>,
        #[arg(long, value_enum)]
        status: Option<ExportState>,
        /// Start the connector jobs now instead of on their next trigger.
        #[arg(long, requires = "idempotency_key")]
        run: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Show what an export would send for one record, without sending it.
    Preview {
        extension_id: String,
        export_id: String,
        #[arg(long)]
        record_id: Uuid,
        #[arg(long)]
        connection: Option<String>,
    },
}

pub(crate) async fn export_command(api: &Api, command: ExportCommand) -> Result<String, CliError> {
    match command {
        ExportCommand::Summary => api.get("/export-status").await,
        ExportCommand::Status {
            extension_id,
            connection,
            status,
            after,
            limit,
        } => {
            api.get(&with_query(
                &extension_path(&extension_id, "/export-status"),
                [
                    ("connection", connection),
                    ("status", status.map(|status| status.as_str().to_owned())),
                    ("after", after.map(|after| after.to_string())),
                    ("limit", limit.map(|limit| limit.to_string())),
                ],
            ))
            .await
        }
        ExportCommand::Retry {
            extension_id,
            connection,
            record_ids,
            status,
            run,
            idempotency_key,
        } => {
            let mut body = Map::new();
            body.insert("run".into(), json!(run));
            if let Some(connection) = connection {
                body.insert("connection".into(), json!(connection));
            }
            if !record_ids.is_empty() {
                body.insert("record_ids".into(), json!(record_ids));
            }
            if let Some(status) = status {
                body.insert("status".into(), json!(status.as_str()));
            }
            if let Some(key) = idempotency_key {
                body.insert("idempotency_key".into(), json!(key));
            }
            api.post_json(
                &extension_path(&extension_id, "/export-status/retry"),
                Value::Object(body),
            )
            .await
        }
        ExportCommand::Preview {
            extension_id,
            export_id,
            record_id,
            connection,
        } => {
            api.post_json(
                &extension_path(
                    &extension_id,
                    &format!("/exports/{}/preview", segment(&export_id)),
                ),
                json!({ "record_id": record_id, "connection": connection }),
            )
            .await
        }
    }
}
