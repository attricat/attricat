use std::{
    fs,
    io::{self},
    path::PathBuf,
};

use clap::Subcommand;
use reqwest::Method;
use serde_json::json;

use crate::error::CliError;
use crate::input::{read_secret_stdin, read_stdin_lines};
use crate::session::save_session_response;
use crate::transport::{Api, raw_response};

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
    api: &Api,
    command: AuthCommand,
    session_file: Option<&PathBuf>,
) -> Result<String, CliError> {
    match command {
        AuthCommand::Discover { login_identifier } => {
            api.post_json(
                "/auth/discover",
                json!({ "login_identifier": login_identifier }),
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
            let body = json!({
                "login_identifier": login_identifier,
                "email": email,
                "password": read_secret_stdin("password")?,
            });
            let response = api
                .send_raw(Method::POST, "/auth/login", Some(body))
                .await?;
            save_session_response(response, session_file).await
        }
        AuthCommand::PasswordReset { email } => {
            api.post_json("/auth/password-reset", json!({ "email": email }))
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
            let [token, password] = read_stdin_lines(["reset token", "password"])?;
            api.post_json(
                "/auth/password-reset/confirm",
                json!({
                    "token": token, "password": password,
                }),
            )
            .await
        }
        AuthCommand::Session => api.get("/auth/session").await,
        AuthCommand::Preferences { time_zone, .. } => {
            api.patch_json("/auth/preferences", json!({ "time_zone": time_zone }))
                .await
        }
        AuthCommand::DisplayName { name } => {
            api.put_json("/auth/display-name", json!({ "display_name": name }))
                .await
        }
        AuthCommand::Logout => {
            let response = api.send_raw(Method::POST, "/auth/logout", None).await?;
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
            let response = api.send_raw(Method::POST, "/auth/renew", None).await?;
            save_session_response(response, session_file).await
        }
    }
}
