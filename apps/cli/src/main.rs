use std::{io, process::ExitCode};

use clap::{CommandFactory, FromArgMatches};
use reqwest::Client;

use crate::cli::{Cli, Command, HealthCommand, MetricsCommand, reject_shared_stdin};
use crate::commands::agent::agent_command;
use crate::commands::audit::audit_command;
use crate::commands::auth::auth_command;
use crate::commands::blueprint::blueprint_command;
use crate::commands::context::context_command;
use crate::commands::data_health::data_health_command;
use crate::commands::directory::directory_command;
use crate::commands::event::event_command;
use crate::commands::extension::extension_command;
use crate::commands::file::file_command;
use crate::commands::lexicon::lexicon_command;
use crate::commands::notification::notification_command;
use crate::commands::presentation_asset::presentation_asset_command;
use crate::commands::record::record_command;
use crate::commands::reusable_attribute::reusable_attribute_command;
use crate::commands::rule::rule_command;
use crate::commands::saved_view::saved_view_command;
use crate::commands::solution_pack::solution_pack_command;
use crate::commands::team::team_command;
use crate::commands::token::token_command;
use crate::commands::value::value_command;
use crate::commands::workflow::workflow_command;
use crate::commands::workspace::workspace_command;
use crate::error::CliError;
use crate::input::read_secret_stdin;
use crate::session::{CSRF_COOKIE, CSRF_HEADER, SESSION_COOKIE, SessionFile};
use crate::transport::{Api, CONNECT_TIMEOUT, is_secure_credential_transport};

mod cli;
mod commands;
mod error;
mod events;
mod input;
mod session;
#[cfg(test)]
mod tests;
mod transport;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = load_dotenv() {
        eprintln!("{}", error.json());
        return ExitCode::from(error.exit_code());
    }
    let parsed = Cli::command()
        .try_get_matches()
        .and_then(|matches| Cli::from_arg_matches(&matches).map(|cli| (cli, matches)));
    let cli = match parsed {
        Ok((cli, matches)) => {
            if let Err(error) = reject_shared_stdin(&matches) {
                eprintln!("{}", error.json());
                return ExitCode::from(error.exit_code());
            }
            cli
        }
        Err(error) if error.use_stderr() => {
            let error = CliError::Input(error.to_string());
            eprintln!("{}", error.json());
            return ExitCode::from(error.exit_code());
        }
        Err(error) => {
            print!("{error}");
            return ExitCode::SUCCESS;
        }
    };
    match run(cli).await {
        Ok(body) => {
            // Streaming commands print as they go and return nothing more.
            if !body.is_empty() {
                println!("{body}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.json());
            ExitCode::from(error.exit_code())
        }
    }
}

fn health_path(command: Option<HealthCommand>) -> &'static str {
    match command {
        None => "/health",
        Some(HealthCommand::Live) => "/health/live",
        Some(HealthCommand::Ready) => "/health/ready",
        Some(HealthCommand::System) => "/system/health",
    }
}

fn load_dotenv() -> Result<(), CliError> {
    if std::env::args_os().any(|argument| argument == "--no-env") {
        return Ok(());
    }
    match dotenvy::from_filename(".env") {
        Ok(_) => Ok(()),
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::Input(format!("cannot load .env: {error}"))),
    }
}

async fn run(cli: Cli) -> Result<String, CliError> {
    let server = cli.server;
    let session_file = cli.session_file;
    // A personal token is the explicit automation credential. Do not even
    // read a saved browser session when one is supplied: it takes precedence
    // and avoids accidentally combining two identities on a request.
    let token = if cli.token_stdin {
        Some(read_secret_stdin("personal API token")?)
    } else {
        cli.token
    };
    let session = if token.is_some() {
        None
    } else {
        SessionFile::load(session_file.as_ref())?
    };
    if (token.is_some() || session.is_some()) && !is_secure_credential_transport(&server) {
        return Err(CliError::Input(
            "refusing to send credentials over insecure HTTP to a non-loopback host".to_owned(),
        ));
    }
    let mut client = Client::builder().connect_timeout(CONNECT_TIMEOUT);
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(token) = token {
        let value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                CliError::Input("ATTRICAT_TOKEN contains invalid header characters".to_owned())
            })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    if let Some(session) = &session {
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={}; {CSRF_COOKIE}={}",
                session.session, session.csrf
            ))
            .map_err(|_| {
                CliError::Input("session file contains invalid cookie characters".to_owned())
            })?,
        );
        headers.insert(
            reqwest::header::HeaderName::from_static(CSRF_HEADER),
            reqwest::header::HeaderValue::from_str(&session.csrf).map_err(|_| {
                CliError::Input("session file contains invalid CSRF characters".to_owned())
            })?,
        );
    }
    if !headers.is_empty() {
        client = client.default_headers(headers);
    }
    let client = client
        .build()
        .map_err(|error| CliError::Transport(error.to_string()))?;
    let api = Api::new(client, server);

    match cli.command {
        Command::Health { command } => api.get(health_path(command)).await,
        Command::Directory { command } => directory_command(&api, command).await,
        Command::Notification { command } => notification_command(&api, command).await,
        Command::Team { command } => team_command(&api, command).await,
        Command::Lexicon { command } => lexicon_command(&api, command).await,
        Command::SavedView { command } => saved_view_command(&api, command).await,
        Command::Auth { command } => auth_command(&api, command, session_file.as_ref()).await,
        Command::Event { command } => event_command(&api, command).await,
        Command::Blueprint { command } => blueprint_command(&api, command).await,
        Command::Context { command } => context_command(&api, command).await,
        Command::ReusableAttribute { command } => reusable_attribute_command(&api, command).await,
        Command::Record { command } => record_command(&api, command).await,
        Command::Value { command } => value_command(&api, command).await,
        Command::Workspace { command } => workspace_command(&api, command).await,
        Command::Token { command } => token_command(&api, command).await,
        Command::Agent { command } => agent_command(&api, command).await,
        Command::Audit { command } => audit_command(&api, command).await,
        Command::DataHealth { command } => data_health_command(&api, command).await,
        Command::Workflow { command } => workflow_command(&api, command).await,
        Command::Rule { command } => rule_command(&api, command).await,
        Command::Extension { command } => extension_command(&api, command).await,
        Command::SolutionPack { command } => solution_pack_command(&api, command).await,
        Command::PresentationAsset { command } => presentation_asset_command(&api, command).await,
        Command::File { command } => file_command(&api, command).await,
        Command::Metrics {
            command: MetricsCommand::Get { output },
        } => api.download("/metrics", &output, None).await,
    }
}
