use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::cli::BeforeArgs;
use crate::error::CliError;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum CommentCommand {
    /// List a record's comments, newest first.
    List {
        record_id: Uuid,
        #[command(flatten)]
        before: BeforeArgs,
    },
    /// Comment on a record.
    Add {
        record_id: Uuid,
        #[arg(long)]
        body: String,
    },
    /// Count a record's comments.
    Count { record_id: Uuid },
    /// Edit your own comment.
    Edit {
        record_id: Uuid,
        comment_id: Uuid,
        #[arg(long)]
        body: String,
        /// The comment's current revision, from `list`.
        #[arg(long)]
        revision: i64,
    },
}

pub(crate) async fn comment_command(
    api: &Api,
    command: CommentCommand,
) -> Result<String, CliError> {
    match command {
        CommentCommand::List { record_id, before } => {
            api.get(&with_query(&comments_path(record_id), before.query()))
                .await
        }
        CommentCommand::Add { record_id, body } => {
            api.post_json(&comments_path(record_id), json!({ "body": body }))
                .await
        }
        CommentCommand::Count { record_id } => {
            api.get(&format!("{}/count", comments_path(record_id)))
                .await
        }
        CommentCommand::Edit {
            record_id,
            comment_id,
            body,
            revision,
        } => {
            api.patch_json(
                &format!("{}/{}", comments_path(record_id), segment(comment_id)),
                json!({ "body": body, "revision": revision }),
            )
            .await
        }
    }
}

fn comments_path(record_id: Uuid) -> String {
    format!("/v1/records/{}/comments", segment(record_id))
}
