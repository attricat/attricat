use std::{fs, io, path::PathBuf};

use clap::{Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::json;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::{read_secret_stdin, read_stdin_lines};
use crate::session::save_session_response;
use crate::transport::{Api, file_part, raw_response};

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
    /// Set or remove your profile picture.
    Avatar {
        #[command(subcommand)]
        command: AvatarCommand,
    },
    /// Check which records, blueprints or contexts you hold a permission on.
    Access {
        /// Permission code, such as `records.write`.
        #[arg(long)]
        permission: String,
        #[arg(long, value_enum)]
        kind: AccessKind,
        /// ID to check; repeat for several (at most 500).
        #[arg(long = "id", required = true)]
        ids: Vec<Uuid>,
    },
    /// Show the demo sign-in accounts, when the server offers them.
    SampleLogins,
    Logout,
    Renew,
}

#[derive(Subcommand)]
pub(crate) enum AvatarCommand {
    /// Upload a PNG or JPEG of at most 10 MiB.
    Set {
        #[arg(long)]
        file: PathBuf,
    },
    Remove,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum AccessKind {
    Record,
    Blueprint,
    Context,
}

impl AccessKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Blueprint => "blueprint",
            Self::Context => "context",
        }
    }
}

const AVATAR_PATH: &str = "/auth/avatar";

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
        AuthCommand::Avatar {
            command: AvatarCommand::Set { file },
        } => {
            let form = reqwest::multipart::Form::new().part("file", file_part(&file).await?);
            api.send_form(Method::PUT, AVATAR_PATH, form).await
        }
        AuthCommand::Avatar {
            command: AvatarCommand::Remove,
        } => api.delete(AVATAR_PATH).await,
        AuthCommand::Access {
            permission,
            kind,
            ids,
        } => {
            api.post_json(
                "/auth/access",
                json!({ "permission": permission, "kind": kind.as_str(), "ids": ids }),
            )
            .await
        }
        AuthCommand::SampleLogins => api.get("/auth/sample-logins").await,
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
