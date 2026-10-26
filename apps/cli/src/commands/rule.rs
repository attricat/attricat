use clap::Subcommand;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::SourceInput;
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{Api, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum RuleCommand {
    List {
        #[arg(long)]
        blueprint_id: Option<Uuid>,
    },
    Get {
        rule_id: Uuid,
    },
    Validate {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Create {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    CreateVersion {
        rule_id: Uuid,
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        blueprint_version: i64,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        rule_id: Uuid,
        version: i64,
    },
    Enable {
        rule_id: Uuid,
        version: i64,
        /// Enable an enforcing rule although its completed dry run found
        /// existing violations; those records cannot be saved until fixed.
        #[arg(long)]
        accept_existing_violations: bool,
    },
    Disable {
        rule_id: Uuid,
    },
    RunNow {
        rule_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        record_id: Option<Uuid>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    RunList,
    RunReplay {
        run_id: Uuid,
    },
    Findings {
        #[arg(long)]
        record_id: Option<Uuid>,
    },
    Acknowledge {
        finding_id: Uuid,
    },
}

pub(crate) async fn rule_command(api: &Api, command: RuleCommand) -> Result<String, CliError> {
    match command {
        RuleCommand::List { blueprint_id } => {
            api.get(&with_query(
                "/rules",
                [("blueprint_id", blueprint_id.map(|id| id.to_string()))],
            ))
            .await
        }
        RuleCommand::Get { rule_id } => api.get(&format!("/rules/{}", segment(rule_id))).await,
        RuleCommand::Validate {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            api.post_json(
                "/rules/validate",
                rule_body(blueprint_id, blueprint_version, context_id, source)?,
            )
            .await
        }
        RuleCommand::Create {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            api.post_json(
                "/rules",
                rule_body(blueprint_id, blueprint_version, context_id, source)?,
            )
            .await
        }
        RuleCommand::CreateVersion {
            rule_id,
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            api.post_json(
                &format!("/rules/{}/versions", segment(rule_id)),
                rule_body(blueprint_id, blueprint_version, context_id, source)?,
            )
            .await
        }
        RuleCommand::Publish { rule_id, version } => {
            api.post(&format!("/rules/{rule_id}/versions/{version}/publish"))
                .await
        }
        RuleCommand::Enable {
            rule_id,
            version,
            accept_existing_violations,
        } => {
            api.post_json(
                &format!("/rules/{rule_id}/versions/{version}/enable"),
                json!({ "accept_existing_violations": accept_existing_violations }),
            )
            .await
        }
        RuleCommand::Disable { rule_id } => {
            api.post(&format!("/rules/{}/disable", segment(rule_id)))
                .await
        }
        RuleCommand::RunNow {
            rule_id,
            idempotency_key,
            record_id,
            dry_run,
        } => {
            api.post_json(
                &format!("/rules/{}/run-now", segment(rule_id)),
                json!({"idempotency_key":idempotency_key,"record_id":record_id,"dry_run":dry_run}),
            )
            .await
        }
        RuleCommand::RunList => api.get("/rule-runs").await,
        RuleCommand::RunReplay { run_id } => {
            api.post(&format!("/rule-runs/{}/replay", segment(run_id)))
                .await
        }
        RuleCommand::Findings { record_id } => {
            api.get(&with_query(
                "/rule-findings",
                [("record_id", record_id.map(|id| id.to_string()))],
            ))
            .await
        }
        RuleCommand::Acknowledge { finding_id } => {
            api.post(&format!(
                "/rule-findings/{}/acknowledge",
                segment(finding_id)
            ))
            .await
        }
    }
}

pub(crate) fn rule_body(
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    source: SourceInput,
) -> Result<Value, CliError> {
    Ok(json!({
        "blueprint_id": blueprint_id,
        "blueprint_version": blueprint_version,
        "context_id": context_id,
        "definition": read_source(source)?,
    }))
}
