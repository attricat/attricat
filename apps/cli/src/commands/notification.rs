use clap::Subcommand;
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::BeforeArgs;
use crate::error::CliError;
use crate::transport::{Api, flag, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum NotificationCommand {
    /// List notifications, newest first, with the unread count.
    List {
        /// Only list unread notifications.
        #[arg(long)]
        unread: bool,
        #[command(flatten)]
        before: BeforeArgs,
    },
    /// Print the number of unread notifications.
    Count,
    /// Show one notification.
    Get { id: Uuid },
    /// Mark a notification as read.
    Read { id: Uuid },
    /// Mark a notification as unread.
    Unread { id: Uuid },
    /// Mark every unread notification as read.
    ReadAll,
    /// Permanently delete a notification.
    Delete { id: Uuid },
}

pub(crate) fn notification_request(
    command: NotificationCommand,
) -> (Method, String, Option<Value>) {
    match command {
        NotificationCommand::List { unread, before } => {
            let [before_time, before_id] = before.query();
            let path = with_query(
                "/notifications",
                [("unread_only", flag(unread)), before_time, before_id],
            );
            (Method::GET, path, None)
        }
        NotificationCommand::Count => (Method::GET, "/notifications/unread-count".to_owned(), None),
        NotificationCommand::Get { id } => {
            (Method::GET, format!("/notifications/{}", segment(id)), None)
        }
        NotificationCommand::Read { id } => (
            Method::PATCH,
            format!("/notifications/{}", segment(id)),
            Some(json!({"read": true})),
        ),
        NotificationCommand::Unread { id } => (
            Method::PATCH,
            format!("/notifications/{}", segment(id)),
            Some(json!({"read": false})),
        ),
        NotificationCommand::ReadAll => (
            Method::POST,
            "/notifications/read-all".to_owned(),
            Some(json!({})),
        ),
        NotificationCommand::Delete { id } => (
            Method::DELETE,
            format!("/notifications/{}", segment(id)),
            None,
        ),
    }
}

pub(crate) async fn notification_command(
    api: &Api,
    command: NotificationCommand,
) -> Result<String, CliError> {
    let (method, path, body) = notification_request(command);
    api.send(method, &path, body).await
}
