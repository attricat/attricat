use std::{
    fs,
    io::{self},
    path::PathBuf,
};

use clap::Subcommand;
use reqwest::{Client, Method};
use serde_json::json;
use url::Url;

use crate::error::CliError;
use crate::input::{read_secret_stdin, read_two_secrets_stdin};
use crate::session::save_session_response;
use crate::transport::{REQUEST_TIMEOUT, endpoint, raw_response, request};

#[derive(Subcommand)]
pub(crate) enum AuthCommand {
    Discover {
        /// Workspace login identifier, not a user ID, workspace UUID, or slug.
        #[arg(value_name = "WORKSPACE_LOGIN_IDENTIFIER")]
        login_identifier: String,
    },
    Login {
        /// Workspace login identifier, not a user ID, workspace UUID, or slug.
        #[arg(value_name = "WORKSPACE_LOGIN_IDENTIFIER")]
        login_identifier: String,
        #[arg(long)]
        email: String,
        #[arg(long)]
        password_stdin: bool,
    },
    PasswordReset {
        #[arg(long)]
        email: String,
    },
    PasswordResetConfirm {
        #[arg(long)]
        reset_token_stdin: bool,
        #[arg(long)]
        password_stdin: bool,
    },
    Session,
    /// Update the authenticated user's display preferences.
    Preferences {
        /// IANA time zone used to render timestamps, for example `Europe/Warsaw` or `UTC`.
        #[arg(
            long,
            value_name = "IANA_ZONE",
            required_unless_present = "clear_time_zone"
        )]
        time_zone: Option<String>,
        /// Follow each client's own time zone instead of a stored preference.
        #[arg(long, conflicts_with = "time_zone")]
        clear_time_zone: bool,
    },
    /// Change the authenticated user's display name.
    DisplayName {
        /// 2–64 letters, digits, and spaces; no leading or trailing space.
        name: String,
    },
    Logout,
    Renew,
}

pub(crate) async fn auth_command(
    client: &Client,
    server: &Url,
    command: AuthCommand,
    session_file: Option<&PathBuf>,
) -> Result<String, CliError> {
    match command {
        AuthCommand::Discover { login_identifier } => {
            request(
                client,
                server,
                Method::POST,
                "/auth/discover",
                Some(json!({ "login_identifier": login_identifier })),
            )
            .await
        }
        AuthCommand::Login {
            login_identifier,
            email,
            password_stdin,
        } => {
            if !password_stdin {
                return Err(CliError::Input(
                    "login requires --password-stdin".to_owned(),
                ));
            }
            let response = client
                .post(endpoint(server, "/auth/login")?)
                .timeout(REQUEST_TIMEOUT)
                .json(&json!({
                    "login_identifier": login_identifier,
                    "email": email,
                    "password": read_secret_stdin("password")?,
                }))
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            save_session_response(response, session_file).await
        }
        AuthCommand::PasswordReset { email } => {
            request(
                client,
                server,
                Method::POST,
                "/auth/password-reset",
                Some(json!({ "email": email })),
            )
            .await
        }
        AuthCommand::PasswordResetConfirm {
            reset_token_stdin,
            password_stdin,
        } => {
            if !reset_token_stdin || !password_stdin {
                return Err(CliError::Input(
                    "password reset confirmation requires --reset-token-stdin and --password-stdin"
                        .to_owned(),
                ));
            }
            let (token, password) = read_two_secrets_stdin("reset token", "password")?;
            request(
                client,
                server,
                Method::POST,
                "/auth/password-reset/confirm",
                Some(json!({
                    "token": token, "password": password,
                })),
            )
            .await
        }
        AuthCommand::Session => request(client, server, Method::GET, "/auth/session", None).await,
        AuthCommand::Preferences { time_zone, .. } => {
            request(
                client,
                server,
                Method::PATCH,
                "/auth/preferences",
                Some(json!({ "time_zone": time_zone })),
            )
            .await
        }
        AuthCommand::DisplayName { name } => {
            request(
                client,
                server,
                Method::PUT,
                "/auth/display-name",
                Some(json!({ "display_name": name })),
            )
            .await
        }
        AuthCommand::Logout => {
            let response = client
                .post(endpoint(server, "/auth/logout")?)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            let output = raw_response(response).await?;
            if let Some(path) = session_file {
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(CliError::Input(format!(
                            "cannot remove session file {}: {error}",
                            path.display()
                        )));
                    }
                }
            }
            Ok(output)
        }
        AuthCommand::Renew => {
            let response = client
                .post(endpoint(server, "/auth/renew")?)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await
                .map_err(|error| CliError::Transport(error.to_string()))?;
            save_session_response(response, session_file).await
        }
    }
}
