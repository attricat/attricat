use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum ExtensionRegistryCommand {
    List,
    Add {
        #[arg(long)]
        source: String,
    },
    Remove {
        registry_id: Uuid,
    },
    /// List the extensions every added registry offers.
    Discover,
    /// Show one registry extension and its releases.
    Get {
        owner: String,
        repository: String,
    },
}

pub(crate) async fn extension_registry_command(
    api: &Api,
    command: ExtensionRegistryCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionRegistryCommand::List => api.get("/extension-registries").await,
        ExtensionRegistryCommand::Add { source } => {
            api.post_json("/extension-registries", json!({ "source": source }))
                .await
        }
        ExtensionRegistryCommand::Remove { registry_id } => {
            api.delete(&format!("/extension-registries/{}", segment(registry_id)))
                .await
        }
        ExtensionRegistryCommand::Discover => api.get("/extension-registries/discover").await,
        ExtensionRegistryCommand::Get { owner, repository } => {
            api.get(&format!(
                "/extension-registries/extensions/{}/{}",
                segment(&owner),
                segment(&repository)
            ))
            .await
        }
    }
}
