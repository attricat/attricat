use clap::Subcommand;
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::transport::segment;

#[derive(Subcommand)]
pub(crate) enum NotificationCommand {
    /// List notifications, newest first, with the unread count.
    List {
        /// Only list unread notifications.
        #[arg(long)]
        unread: bool,
        /// Continue after a page: the created_at of its last item.
        #[arg(long, requires = "before_id")]
        before_time: Option<String>,
        /// Continue after a page: the id of its last item.
        #[arg(long, requires = "before_time")]
        before_id: Option<Uuid>,
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
        NotificationCommand::List {
            unread,
            before_time,
            before_id,
        } => {
            let mut params = url::form_urlencoded::Serializer::new(String::new());
            if unread {
                params.append_pair("unread_only", "true");
            }
            if let (Some(time), Some(id)) = (before_time, before_id) {
                params.append_pair("before_time", &time);
                params.append_pair("before_id", &id.to_string());
            }
            let query = params.finish();
            let path = if query.is_empty() {
                "/notifications".to_owned()
            } else {
                format!("/notifications?{query}")
            };
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
