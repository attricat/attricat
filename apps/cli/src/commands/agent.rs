use std::io::Write;
use std::path::PathBuf;

use clap::Subcommand;
use futures_util::StreamExt;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::error::CliError;
use crate::events::EventParser;
use crate::input::json_object_input;
use crate::transport::{Api, raw_response, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum AgentCommand {
    /// Your agent conversations.
    Conversation {
        #[command(subcommand)]
        command: ConversationCommand,
    },
    /// Read a conversation's messages or send one, which starts a run.
    Message {
        #[command(subcommand)]
        command: MessageCommand,
    },
    /// Attach files to a conversation for later messages.
    Upload {
        conversation_id: Uuid,
        #[arg(long = "file", required = true)]
        files: Vec<PathBuf>,
    },
    /// A conversation's runs and their live events.
    Run {
        #[command(subcommand)]
        command: AgentRunCommand,
    },
    /// Tool calls waiting for your approval.
    Approval {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
    /// Suggest values for a record's fields from text and attachments.
    SmartFill {
        #[arg(long)]
        record_id: Uuid,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// The values are for the record's default context.
        #[arg(long)]
        default_context: bool,
        /// Text to extract values from.
        #[arg(long)]
        content: String,
        #[arg(long)]
        conversation_id: Option<Uuid>,
        /// Unsaved form values: a JSON object of strings or a file containing it.
        #[arg(long)]
        draft_values: Option<String>,
        /// Uploaded attachment ID; repeat for several.
        #[arg(long = "attachment-id")]
        attachment_ids: Vec<Uuid>,
    },
}

#[derive(Subcommand)]
pub(crate) enum ConversationCommand {
    /// Conversations, newest first; by default only those not about a record.
    List {
        #[command(flatten)]
        filter: ConversationFilter,
    },
    Search {
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        filter: ConversationFilter,
    },
    Create {
        #[arg(long)]
        title: Option<String>,
        /// Start the conversation about a record.
        #[arg(long)]
        record_id: Option<Uuid>,
        #[arg(long, requires = "record_id")]
        context_id: Option<Uuid>,
    },
    Get {
        conversation_id: Uuid,
    },
    Rename {
        conversation_id: Uuid,
        #[arg(long)]
        title: String,
    },
    /// Archive a conversation.
    Delete {
        conversation_id: Uuid,
    },
}

#[derive(clap::Args)]
pub(crate) struct ConversationFilter {
    /// Only conversations about this record.
    #[arg(long, conflicts_with = "all")]
    record_id: Option<Uuid>,
    #[arg(long, requires = "record_id")]
    context_id: Option<Uuid>,
    /// Include conversations about records.
    #[arg(long)]
    all: bool,
    /// Continue after a page: its `next_cursor`.
    #[arg(long)]
    cursor: Option<String>,
}

const CONVERSATION_SCOPE_ALL: &str = "all";

fn search_conversations_path(query: Option<String>, filter: ConversationFilter) -> String {
    with_query(
        &format!("{CONVERSATIONS_PATH}/search"),
        [
            ("q", query),
            ("record_id", filter.record_id.map(|id| id.to_string())),
            ("context_id", filter.context_id.map(|id| id.to_string())),
            (
                "scope",
                filter.all.then(|| CONVERSATION_SCOPE_ALL.to_owned()),
            ),
            ("cursor", filter.cursor),
        ],
    )
}

#[derive(Subcommand)]
pub(crate) enum MessageCommand {
    List {
        conversation_id: Uuid,
    },
    Send {
        conversation_id: Uuid,
        #[arg(long)]
        content: String,
        /// Uploaded attachment ID; repeat for several.
        #[arg(long = "attachment-id")]
        attachment_ids: Vec<Uuid>,
        /// Print the run's events as JSON lines until it finishes.
        #[arg(long)]
        follow: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AgentRunCommand {
    List {
        conversation_id: Uuid,
    },
    /// Print a run's events as JSON lines until it finishes.
    Events {
        run_id: Uuid,
        /// Resume after this event ID instead of from the start.
        #[arg(long)]
        last_event_id: Option<Uuid>,
    },
}

#[derive(Subcommand)]
pub(crate) enum ApprovalCommand {
    List {
        #[arg(long)]
        conversation_id: Option<Uuid>,
    },
    Approve {
        tool_call_id: Uuid,
    },
    Reject {
        tool_call_id: Uuid,
    },
}

const CONVERSATIONS_PATH: &str = "/agent/conversations";
/// The event that ends a run's stream.
const TERMINAL_EVENT: &str = "terminal";
/// The event the server sends when it stops a stream abnormally.
const ERROR_EVENT: &str = "error";
/// Pause before resuming a stream the server closed before the run ended.
const STREAM_RECONNECT_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) async fn agent_command(api: &Api, command: AgentCommand) -> Result<String, CliError> {
    match command {
        AgentCommand::Conversation { command } => conversation_command(api, command).await,
        AgentCommand::Message { command } => message_command(api, command).await,
        AgentCommand::Upload {
            conversation_id,
            files,
        } => {
            api.multipart(&conversation_path(conversation_id, "/uploads"), files, None)
                .await
        }
        AgentCommand::Run { command } => match command {
            AgentRunCommand::List { conversation_id } => {
                api.get(&conversation_path(conversation_id, "/runs")).await
            }
            AgentRunCommand::Events {
                run_id,
                last_event_id,
            } => follow_run(api, run_id, last_event_id.map(|id| id.to_string())).await,
        },
        AgentCommand::Approval { command } => match command {
            ApprovalCommand::List { conversation_id } => {
                api.get(&with_query(
                    "/agent/approvals",
                    [("conversation_id", conversation_id.map(|id| id.to_string()))],
                ))
                .await
            }
            ApprovalCommand::Approve { tool_call_id } => {
                api.post(&format!(
                    "/agent/tool-calls/{}/approve",
                    segment(tool_call_id)
                ))
                .await
            }
            ApprovalCommand::Reject { tool_call_id } => {
                api.post(&format!(
                    "/agent/tool-calls/{}/reject",
                    segment(tool_call_id)
                ))
                .await
            }
        },
        AgentCommand::SmartFill {
            record_id,
            context_id,
            default_context,
            content,
            conversation_id,
            draft_values,
            attachment_ids,
        } => {
            let mut body = Map::new();
            body.insert("record_id".into(), json!(record_id));
            body.insert("context_id".into(), json!(context_id));
            body.insert("is_default_context".into(), json!(default_context));
            body.insert("content".into(), json!(content));
            body.insert("conversation_id".into(), json!(conversation_id));
            if let Some(values) = draft_values {
                body.insert(
                    "draft_values".into(),
                    json_object_input(&values, "--draft-values")?,
                );
            }
            body.insert("attachment_ids".into(), json!(attachment_ids));
            api.post_json("/agent/smart-fill", Value::Object(body))
                .await
        }
    }
}

async fn conversation_command(api: &Api, command: ConversationCommand) -> Result<String, CliError> {
    match command {
        ConversationCommand::List { filter } => {
            api.get(&search_conversations_path(None, filter)).await
        }
        ConversationCommand::Search { query, filter } => {
            api.get(&search_conversations_path(query, filter)).await
        }
        ConversationCommand::Create {
            title,
            record_id,
            context_id,
        } => {
            let mut body = Map::new();
            if let Some(title) = title {
                body.insert("title".into(), json!(title));
            }
            body.insert("record_id".into(), json!(record_id));
            body.insert("context_id".into(), json!(context_id));
            api.post_json(CONVERSATIONS_PATH, Value::Object(body)).await
        }
        ConversationCommand::Get { conversation_id } => {
            api.get(&conversation_path(conversation_id, "")).await
        }
        ConversationCommand::Rename {
            conversation_id,
            title,
        } => {
            api.patch_json(
                &conversation_path(conversation_id, ""),
                json!({ "title": title }),
            )
            .await
        }
        ConversationCommand::Delete { conversation_id } => {
            api.delete(&conversation_path(conversation_id, "")).await
        }
    }
}

async fn message_command(api: &Api, command: MessageCommand) -> Result<String, CliError> {
    match command {
        MessageCommand::List { conversation_id } => {
            api.get(&conversation_path(conversation_id, "/messages"))
                .await
        }
        MessageCommand::Send {
            conversation_id,
            content,
            attachment_ids,
            follow,
        } => {
            let run = api
                .post_json(
                    &conversation_path(conversation_id, "/messages"),
                    json!({ "content": content, "attachment_ids": attachment_ids }),
                )
                .await?;
            if !follow {
                return Ok(run);
            }
            let run_id = serde_json::from_str::<Value>(&run)
                .ok()
                .and_then(|run| run["id"].as_str().and_then(|id| id.parse::<Uuid>().ok()))
                .ok_or(CliError::InvalidResponse)?;
            print_line(&run)?;
            follow_run(api, run_id, None).await
        }
    }
}

/// Prints a run's events as JSON lines until its terminal event, resuming
/// with `Last-Event-ID` when the server ends the stream early. Returns no
/// further output.
pub(crate) async fn follow_run(
    api: &Api,
    run_id: Uuid,
    mut last_event_id: Option<String>,
) -> Result<String, CliError> {
    let path = format!("/agent/runs/{}/events", segment(run_id));
    let mut resumed = false;
    loop {
        if resumed {
            tokio::time::sleep(STREAM_RECONNECT_DELAY).await;
        }
        resumed = true;
        let response = api.open_stream(&path, last_event_id.as_deref()).await?;
        if !response.status().is_success() {
            return raw_response(response).await;
        }
        let mut parser = EventParser::default();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
            for event in parser.push(&chunk) {
                if event.event.as_deref() == Some(ERROR_EVENT) {
                    return Err(CliError::Transport(event.data));
                }
                print_line(&event.data)?;
                if event.event.as_deref() == Some(TERMINAL_EVENT) {
                    return Ok(String::new());
                }
                if event.id.is_some() {
                    last_event_id = event.id;
                }
            }
        }
    }
}

fn print_line(line: &str) -> Result<(), CliError> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .map_err(|error| CliError::Transport(error.to_string()))
}

fn conversation_path(conversation_id: Uuid, rest: &str) -> String {
    format!("{CONVERSATIONS_PATH}/{}{rest}", segment(conversation_id))
}
