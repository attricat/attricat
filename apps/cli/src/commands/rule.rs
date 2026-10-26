use clap::Subcommand;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::cli::SourceInput;
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{request, segment};

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
    Revision {
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

pub(crate) async fn rule_command(
    client: &Client,
    server: &Url,
    command: RuleCommand,
) -> Result<String, CliError> {
    match command {
        RuleCommand::List { blueprint_id } => {
            let path = match blueprint_id {
                Some(id) => format!("/rules?blueprint_id={}", segment(id)),
                None => "/rules".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        RuleCommand::Get { rule_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/rules/{}", segment(rule_id)),
                None,
            )
            .await
        }
        RuleCommand::Validate {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                "/rules/validate",
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Create {
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                "/rules",
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Revision {
            rule_id,
            blueprint_id,
            blueprint_version,
            context_id,
            source,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{}/versions", segment(rule_id)),
                Some(rule_body(
                    blueprint_id,
                    blueprint_version,
                    context_id,
                    source,
                )?),
            )
            .await
        }
        RuleCommand::Publish { rule_id, version } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/versions/{version}/publish"),
                None,
            )
            .await
        }
        RuleCommand::Enable {
            rule_id,
            version,
            accept_existing_violations,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{rule_id}/versions/{version}/enable"),
                Some(json!({ "accept_existing_violations": accept_existing_violations })),
            )
            .await
        }
        RuleCommand::Disable { rule_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rules/{}/disable", segment(rule_id)),
                None,
            )
            .await
        }
        RuleCommand::RunNow {
            rule_id,
            idempotency_key,
            record_id,
            dry_run,
        } => request(
            client,
            server,
            Method::POST,
            &format!("/rules/{}/run-now", segment(rule_id)),
            Some(
                json!({"idempotency_key":idempotency_key,"record_id":record_id,"dry_run":dry_run}),
            ),
        )
        .await,
        RuleCommand::RunList => request(client, server, Method::GET, "/rule-runs", None).await,
        RuleCommand::RunReplay { run_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rule-runs/{}/replay", segment(run_id)),
                None,
            )
            .await
        }
        RuleCommand::Findings { record_id } => {
            let path = match record_id {
                Some(id) => format!("/rule-findings?record_id={}", segment(id)),
                None => "/rule-findings".to_owned(),
            };
            request(client, server, Method::GET, &path, None).await
        }
        RuleCommand::Acknowledge { finding_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/rule-findings/{}/acknowledge", segment(finding_id)),
                None,
            )
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
    Ok(
        json!({"blueprint_id":blueprint_id,"blueprint_version":blueprint_version,"context_id":context_id,"definition":read_source(source)?}),
    )
}
