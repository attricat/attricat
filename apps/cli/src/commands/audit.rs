use clap::Subcommand;
use reqwest::{Client, Method};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::transport::request;

#[derive(Subcommand)]
pub(crate) enum AuditCommand {
    List {
        #[arg(long)]
        limit: Option<i64>,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long)]
        occurred_after: Option<String>,
        #[arg(long)]
        occurred_before: Option<String>,
        #[arg(long)]
        actor_user_id: Option<Uuid>,
        #[arg(long)]
        action_category: Option<String>,
        #[arg(long)]
        target_type: Option<String>,
        #[arg(long, value_parser = ["human", "agent"])]
        executor_type: Option<String>,
        #[arg(long)]
        agent_run_id: Option<Uuid>,
        #[arg(long)]
        agent_tool_call_id: Option<Uuid>,
    },
}

pub(crate) async fn audit_command(
    client: &Client,
    server: &Url,
    command: AuditCommand,
) -> Result<String, CliError> {
    match command {
        AuditCommand::List {
            limit,
            offset,
            occurred_after,
            occurred_before,
            actor_user_id,
            action_category,
            target_type,
            executor_type,
            agent_run_id,
            agent_tool_call_id,
        } => {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            for (key, value) in [
                ("limit", limit.map(|v| v.to_string())),
                ("offset", offset.map(|v| v.to_string())),
                ("occurred_after", occurred_after),
                ("occurred_before", occurred_before),
                ("actor_user_id", actor_user_id.map(|v| v.to_string())),
                ("action_category", action_category),
                ("target_type", target_type),
                ("executor_type", executor_type),
                ("agent_run_id", agent_run_id.map(|v| v.to_string())),
                (
                    "agent_tool_call_id",
                    agent_tool_call_id.map(|v| v.to_string()),
                ),
            ] {
                if let Some(value) = value {
                    query.append_pair(key, &value);
                }
            }
            let query = query.finish();
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/audit-events{}",
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    }
                ),
                None,
            )
            .await
        }
    }
}
