use clap::{Subcommand, ValueEnum};
use uuid::Uuid;

use crate::cli::PageArgs;
use crate::error::CliError;
use crate::transport::{Api, with_query};

#[derive(Subcommand)]
pub(crate) enum AuditCommand {
    List {
        #[command(flatten)]
        page: PageArgs,
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
        #[arg(long, value_enum)]
        executor_type: Option<ExecutorType>,
        #[arg(long)]
        agent_run_id: Option<Uuid>,
        #[arg(long)]
        agent_tool_call_id: Option<Uuid>,
    },
}

/// Who performed an audited action.
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ExecutorType {
    Human,
    Agent,
}

impl ExecutorType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }
}

pub(crate) async fn audit_command(api: &Api, command: AuditCommand) -> Result<String, CliError> {
    match command {
        AuditCommand::List {
            page,
            occurred_after,
            occurred_before,
            actor_user_id,
            action_category,
            target_type,
            executor_type,
            agent_run_id,
            agent_tool_call_id,
        } => {
            let to_string = |value: Uuid| value.to_string();
            let [limit, offset] = page.query();
            api.get(&with_query(
                "/audit-events",
                [
                    limit,
                    offset,
                    ("occurred_after", occurred_after),
                    ("occurred_before", occurred_before),
                    ("actor_user_id", actor_user_id.map(to_string)),
                    ("action_category", action_category),
                    ("target_type", target_type),
                    (
                        "executor_type",
                        executor_type.map(|executor| executor.as_str().to_owned()),
                    ),
                    ("agent_run_id", agent_run_id.map(to_string)),
                    ("agent_tool_call_id", agent_tool_call_id.map(to_string)),
                ],
            ))
            .await
        }
    }
}
