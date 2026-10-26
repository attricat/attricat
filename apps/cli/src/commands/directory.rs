use clap::Subcommand;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::{Api, segment};

#[derive(Subcommand)]
pub(crate) enum DirectoryCommand {
    /// List workspace users and teams.
    List,
    /// Show one member's profile, with their teams.
    User { user_id: Uuid },
}

pub(crate) async fn directory_command(
    api: &Api,
    command: DirectoryCommand,
) -> Result<String, CliError> {
    match command {
        DirectoryCommand::List => api.get("/directory").await,
        DirectoryCommand::User { user_id } => {
            api.get(&format!("/directory/users/{}", segment(user_id)))
                .await
        }
    }
}
