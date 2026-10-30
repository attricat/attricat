use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Map, Value, json};

use crate::commands::extension::extension_path;
use crate::error::CliError;
use crate::input::{json_object_input, read_secret_stdin, read_stdin_lines};
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum ConnectionCommand {
    List {
        extension_id: String,
    },
    /// Create a connection to an external system (requires `extensions.manage`).
    Create {
        extension_id: String,
        /// Name connector jobs use with `connection: …`.
        #[arg(long)]
        connection: String,
        /// Connection kind the extension declares.
        #[arg(long)]
        kind: String,
        #[arg(long)]
        display_name: String,
        #[arg(long, value_enum)]
        grant_type: GrantType,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        client_id: String,
        #[command(flatten)]
        endpoints: ConnectionEndpoints,
        #[command(flatten)]
        secrets: ConnectionSecrets,
    },
    /// Change the given fields and keep the others; an empty
    /// `--authorization-url` or `--scope` clears it.
    Update {
        extension_id: String,
        connection: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        token_url: Option<String>,
        #[command(flatten)]
        endpoints: ConnectionEndpoints,
        #[command(flatten)]
        secrets: ConnectionSecrets,
    },
    Delete {
        extension_id: String,
        connection: String,
    },
    /// Start a device-code authorization; the result shows the user code.
    DeviceAuthorization {
        extension_id: String,
        connection: String,
    },
    /// Check whether a started device authorization was approved.
    Poll {
        extension_id: String,
        connection: String,
    },
    /// Start an authorization-code sign-in; needs a browser session.
    Authorize {
        extension_id: String,
        connection: String,
    },
    /// Exchange an authorization code for tokens.
    ExchangeCode {
        extension_id: String,
        connection: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        redirect_uri: String,
        #[arg(long)]
        code_verifier: Option<String>,
    },
    /// Request a token to check the connection works.
    Test {
        extension_id: String,
        connection: String,
    },
}

/// OAuth endpoints and options shared by create and update.
#[derive(Args)]
pub(crate) struct ConnectionEndpoints {
    #[arg(long)]
    device_authorization_url: Option<String>,
    /// Only for the authorization_code grant.
    #[arg(long)]
    authorization_url: Option<String>,
    #[arg(long)]
    scope: Option<String>,
    /// Kind-specific parameters: a JSON object or a file containing it.
    #[arg(long)]
    parameters: Option<String>,
}

/// Secrets are read from standard input, the client secret first, so they
/// stay out of process arguments.
#[derive(Args)]
pub(crate) struct ConnectionSecrets {
    #[arg(long)]
    client_secret_stdin: bool,
    #[arg(long)]
    refresh_token_stdin: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum GrantType {
    RefreshToken,
    ClientCredentials,
    DeviceCode,
    AuthorizationCode,
}

impl GrantType {
    fn as_str(self) -> &'static str {
        match self {
            Self::RefreshToken => "refresh_token",
            Self::ClientCredentials => "client_credentials",
            Self::DeviceCode => "device_code",
            Self::AuthorizationCode => "authorization_code",
        }
    }
}

pub(crate) async fn connection_command(
    api: &Api,
    command: ConnectionCommand,
) -> Result<String, CliError> {
    match command {
        ConnectionCommand::List { extension_id } => {
            api.get(&extension_path(&extension_id, "/connections"))
                .await
        }
        ConnectionCommand::Create {
            extension_id,
            connection,
            kind,
            display_name,
            grant_type,
            token_url,
            client_id,
            endpoints,
            secrets,
        } => {
            let mut body = Map::new();
            body.insert("connection".into(), json!(connection));
            body.insert("kind".into(), json!(kind));
            body.insert("display_name".into(), json!(display_name));
            body.insert("grant_type".into(), json!(grant_type.as_str()));
            body.insert("token_url".into(), json!(token_url));
            body.insert("client_id".into(), json!(client_id));
            add_connection_options(&mut body, endpoints, secrets)?;
            api.post_json(
                &extension_path(&extension_id, "/connections"),
                Value::Object(body),
            )
            .await
        }
        ConnectionCommand::Update {
            extension_id,
            connection,
            display_name,
            token_url,
            endpoints,
            secrets,
        } => {
            let mut body = Map::new();
            if let Some(name) = display_name {
                body.insert("display_name".into(), json!(name));
            }
            if let Some(url) = token_url {
                body.insert("token_url".into(), json!(url));
            }
            add_connection_options(&mut body, endpoints, secrets)?;
            api.patch_json(
                &connection_path(&extension_id, &connection, ""),
                Value::Object(body),
            )
            .await
        }
        ConnectionCommand::Delete {
            extension_id,
            connection,
        } => {
            api.delete(&connection_path(&extension_id, &connection, ""))
                .await
        }
        ConnectionCommand::DeviceAuthorization {
            extension_id,
            connection,
        } => {
            api.post(&connection_path(
                &extension_id,
                &connection,
                "/device-authorization",
            ))
            .await
        }
        ConnectionCommand::Poll {
            extension_id,
            connection,
        } => {
            api.post(&connection_path(
                &extension_id,
                &connection,
                "/device-authorization/poll",
            ))
            .await
        }
        ConnectionCommand::Authorize {
            extension_id,
            connection,
        } => {
            api.post(&connection_path(
                &extension_id,
                &connection,
                "/authorization",
            ))
            .await
        }
        ConnectionCommand::ExchangeCode {
            extension_id,
            connection,
            code,
            redirect_uri,
            code_verifier,
        } => {
            let body = json!({
                "code": code,
                "redirect_uri": redirect_uri,
                "code_verifier": code_verifier,
            });
            api.post_json(
                &connection_path(&extension_id, &connection, "/authorization-code"),
                body,
            )
            .await
        }
        ConnectionCommand::Test {
            extension_id,
            connection,
        } => {
            api.post(&connection_path(&extension_id, &connection, "/test"))
                .await
        }
    }
}

fn add_connection_options(
    body: &mut Map<String, Value>,
    endpoints: ConnectionEndpoints,
    secrets: ConnectionSecrets,
) -> Result<(), CliError> {
    for (name, value) in [
        (
            "device_authorization_url",
            endpoints.device_authorization_url,
        ),
        ("authorization_url", endpoints.authorization_url),
        ("scope", endpoints.scope),
    ] {
        if let Some(value) = value {
            body.insert(name.into(), json!(value));
        }
    }
    if let Some(parameters) = endpoints.parameters {
        body.insert(
            "parameters".into(),
            json_object_input(&parameters, "--parameters")?,
        );
    }
    let (client_secret, refresh_token) =
        match (secrets.client_secret_stdin, secrets.refresh_token_stdin) {
            (true, true) => {
                let [secret, token] = read_stdin_lines(["client secret", "refresh token"])?;
                (Some(secret), Some(token))
            }
            (true, false) => (Some(read_secret_stdin("client secret")?), None),
            (false, true) => (None, Some(read_secret_stdin("refresh token")?)),
            (false, false) => (None, None),
        };
    if let Some(secret) = client_secret {
        body.insert("client_secret".into(), json!(secret));
    }
    if let Some(token) = refresh_token {
        body.insert("refresh_token".into(), json!(token));
    }
    Ok(())
}

fn connection_path(extension_id: &str, connection: &str, rest: &str) -> String {
    extension_path(
        extension_id,
        &format!("/connections/{}{rest}", segment(connection)),
    )
}
