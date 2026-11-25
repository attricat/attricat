use clap::Subcommand;
use serde_json::json;

use crate::commands::extension::extension_path;
use crate::error::CliError;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum WebhookCommand {
    /// List an extension's webhook endpoints and how each last answered.
    List { extension_id: String },
    /// Create a public endpoint of a webhook the extension declares (requires
    /// `extensions.manage`). The result's URL is shown only once.
    Create {
        extension_id: String,
        /// The webhook ID from the extension's manifest.
        #[arg(long)]
        webhook: String,
        /// A name unique within the extension, such as the sending account.
        #[arg(long)]
        endpoint: String,
    },
    /// Issue a new URL for an endpoint; the previous URL stops working.
    Rotate {
        extension_id: String,
        endpoint: String,
    },
    /// Delete an endpoint; payloads it already kept are still delivered.
    Delete {
        extension_id: String,
        endpoint: String,
    },
}

fn endpoint_path(extension_id: &str, endpoint: &str, rest: &str) -> String {
    extension_path(
        extension_id,
        &format!("/webhook-endpoints/{}{rest}", segment(endpoint)),
    )
}

pub(crate) async fn webhook_command(
    api: &Api,
    command: WebhookCommand,
) -> Result<String, CliError> {
    match command {
        WebhookCommand::List { extension_id } => {
            api.get(&extension_path(&extension_id, "/webhook-endpoints"))
                .await
        }
        WebhookCommand::Create {
            extension_id,
            webhook,
            endpoint,
        } => {
            api.post_json(
                &extension_path(&extension_id, "/webhook-endpoints"),
                json!({"webhook": webhook, "endpoint": endpoint}),
            )
            .await
        }
        WebhookCommand::Rotate {
            extension_id,
            endpoint,
        } => {
            api.post(&endpoint_path(&extension_id, &endpoint, "/rotate"))
                .await
        }
        WebhookCommand::Delete {
            extension_id,
            endpoint,
        } => {
            api.delete(&endpoint_path(&extension_id, &endpoint, ""))
                .await
        }
    }
}
