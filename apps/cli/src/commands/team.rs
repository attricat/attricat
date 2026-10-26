use clap::Subcommand;
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::transport::segment;

#[derive(Subcommand)]
pub(crate) enum TeamCommand {
    /// List teams and their member user IDs.
    List,
    /// Create a team.
    Create {
        #[arg(long)]
        code: String,
        #[arg(long)]
        name: String,
        /// Member user ID; repeat for several.
        #[arg(long = "member")]
        members: Vec<Uuid>,
    },
    /// Rename a team and/or replace its members.
    Update {
        id: Uuid,
        #[arg(long)]
        name: Option<String>,
        /// Member user ID; repeat for several. Replaces every member.
        #[arg(long = "member")]
        members: Vec<Uuid>,
        /// Remove every member.
        #[arg(long, conflicts_with = "members")]
        clear_members: bool,
    },
    /// Delete a team. Existing assignments keep it and show it as deleted.
    Delete { id: Uuid },
}

pub(crate) fn team_request(command: TeamCommand) -> (Method, String, Option<Value>) {
    match command {
        TeamCommand::List => (Method::GET, "/workspace/teams".to_owned(), None),
        TeamCommand::Create {
            code,
            name,
            members,
        } => (
            Method::POST,
            "/workspace/teams".to_owned(),
            Some(json!({"code": code, "name": name, "member_user_ids": members})),
        ),
        TeamCommand::Update {
            id,
            name,
            members,
            clear_members,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(name) = name {
                body.insert("name".into(), json!(name));
            }
            if clear_members || !members.is_empty() {
                body.insert("member_user_ids".into(), json!(members));
            }
            (
                Method::PATCH,
                format!("/workspace/teams/{}", segment(id)),
                Some(Value::Object(body)),
            )
        }
        TeamCommand::Delete { id } => (
            Method::DELETE,
            format!("/workspace/teams/{}", segment(id)),
            None,
        ),
    }
}
