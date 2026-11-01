use clap::{Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::error::CliError;
use crate::input::read_secret_stdin;
use crate::transport::Api;

/// A workspace's single sign-on provider. Both the workspace and the operator
/// commands manage it through these subcommands.
#[derive(Subcommand)]
pub(crate) enum IdentityProviderCommand {
    Get,
    /// Create or replace the provider. Changing the issuer or client ID makes
    /// it unverified and off again.
    Set {
        #[arg(long)]
        issuer: String,
        #[arg(long)]
        client_id: String,
        /// The name on the sign-in button.
        #[arg(long)]
        display_name: String,
        /// Read the client secret from standard input. Required for a new
        /// provider; otherwise leaving it out keeps the stored secret.
        #[arg(long)]
        client_secret_stdin: bool,
    },
    /// Turn the provider on or off, or set who may use a password.
    Update {
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long, value_enum)]
        password_sign_in: Option<PasswordSignInArg>,
    },
    Delete,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum PasswordSignInArg {
    All,
    Owners,
}

impl PasswordSignInArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Owners => "owners",
        }
    }
}

pub(crate) async fn identity_provider_command(
    api: &Api,
    path: &str,
    command: IdentityProviderCommand,
) -> Result<String, CliError> {
    match command {
        IdentityProviderCommand::Get => api.get(path).await,
        IdentityProviderCommand::Set {
            issuer,
            client_id,
            display_name,
            client_secret_stdin,
        } => {
            let mut body = json!({
                "issuer": issuer,
                "client_id": client_id,
                "display_name": display_name,
            });
            if client_secret_stdin {
                body["client_secret"] = Value::String(read_secret_stdin("client secret")?);
            }
            api.put_json(path, body).await
        }
        IdentityProviderCommand::Update {
            enabled,
            password_sign_in,
        } => {
            let mut body = Map::new();
            if let Some(enabled) = enabled {
                body.insert("enabled".to_owned(), Value::Bool(enabled));
            }
            if let Some(policy) = password_sign_in {
                body.insert(
                    "password_sign_in".to_owned(),
                    Value::String(policy.as_str().to_owned()),
                );
            }
            api.patch_json(path, Value::Object(body)).await
        }
        IdentityProviderCommand::Delete => api.delete(path).await,
    }
}
