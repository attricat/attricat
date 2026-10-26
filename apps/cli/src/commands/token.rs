use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::json_string_array_input;
use crate::transport::{Api, segment};

pub(crate) const GENERATOR_TOKEN_LABEL: &str = "catalog-generator";

pub(crate) const GENERATOR_TOKEN_PERMISSIONS: &[&str] = &[
    "blueprints.read",
    "blueprints.write",
    "blueprints.publish",
    "contexts.read",
    "contexts.write",
    "records.read",
    "records.write",
    "records.publish",
];

#[derive(Subcommand)]
pub(crate) enum TokenCommand {
    List,
    Create {
        /// Create a least-privilege token for the demo catalog generator.
        #[arg(long, conflicts_with_all = ["label", "permissions"])]
        generator: bool,
        #[arg(
            long,
            required_unless_present = "generator",
            conflicts_with = "generator"
        )]
        label: Option<String>,
        #[arg(
            long,
            required_unless_present = "generator",
            conflicts_with = "generator"
        )]
        permissions: Option<String>,
        #[arg(long)]
        expires_at: Option<String>,
    },
    Revoke {
        token_id: Uuid,
    },
}

pub(crate) async fn token_command(api: &Api, command: TokenCommand) -> Result<String, CliError> {
    match command {
        TokenCommand::List => api.get("/personal-access-tokens").await,
        TokenCommand::Create {
            generator,
            label,
            permissions,
            expires_at,
        } => {
            let (label, permissions) = if generator {
                (
                    GENERATOR_TOKEN_LABEL.to_owned(),
                    json!(GENERATOR_TOKEN_PERMISSIONS),
                )
            } else {
                let label = label.ok_or_else(|| {
                    CliError::Input("--label is required unless --generator is used".to_owned())
                })?;
                let permissions = permissions.ok_or_else(|| {
                    CliError::Input(
                        "--permissions is required unless --generator is used".to_owned(),
                    )
                })?;
                (
                    label,
                    json_string_array_input(&permissions, "--permissions")?,
                )
            };
            api.post_json(
                "/personal-access-tokens",
                json!({ "label": label, "permissions": permissions, "expires_at": expires_at }),
            )
            .await
        }
        TokenCommand::Revoke { token_id } => {
            api.delete(&format!("/personal-access-tokens/{}", segment(token_id)))
                .await
        }
    }
}
