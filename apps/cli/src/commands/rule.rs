use clap::{Subcommand, ValueEnum};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::{PageArgs, SourceInput};
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{Api, flag, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum RuleCommand {
    List {
        #[arg(long)]
        blueprint_id: Option<Uuid>,
    },
    Get {
        rule_id: Uuid,
    },
    /// List a rule's saved revisions, newest first.
    Revisions {
        rule_id: Uuid,
    },
    /// Show one saved revision of a rule.
    Revision {
        rule_id: Uuid,
        version: i64,
    },
    Validate {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Create {
        #[arg(long)]
        blueprint_id: Uuid,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    /// Save a changed definition as the rule's new current revision.
    Update {
        rule_id: Uuid,
        /// Save an enforcing definition of an enabled rule although its
        /// completed dry run found existing violations.
        #[arg(long)]
        accept_existing_violations: bool,
        #[command(flatten)]
        source: SourceInput,
    },
    Enable {
        rule_id: Uuid,
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
        /// Dry-run this unsaved definition instead of the current one.
        #[arg(long, requires = "dry_run")]
        definition_file: Option<std::path::PathBuf>,
    },
    RunList {
        #[arg(long)]
        record_id: Option<Uuid>,
        #[command(flatten)]
        page: PageArgs,
    },
    RunReplay {
        run_id: Uuid,
    },
    Findings {
        #[arg(long)]
        record_id: Option<Uuid>,
        #[arg(long)]
        rule_id: Option<Uuid>,
        #[arg(long)]
        blueprint_id: Option<Uuid>,
        #[arg(long)]
        context_id: Option<Uuid>,
        #[arg(long, value_enum)]
        severity: Option<FindingSeverity>,
        #[arg(long, value_enum)]
        state: Option<FindingState>,
        /// Leave out resolved findings.
        #[arg(long)]
        unresolved: bool,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Count unresolved findings, optionally of one record.
    FindingCount {
        #[arg(long)]
        record_id: Option<Uuid>,
    },
    /// Re-check one record against every enabled rule of its blueprint.
    Recheck {
        #[arg(long)]
        record_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
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
        RuleCommand::Revisions { rule_id } => {
            api.get(&format!("/rules/{}/revisions", segment(rule_id)))
                .await
        }
        RuleCommand::Revision { rule_id, version } => {
            api.get(&format!("/rules/{}/revisions/{version}", segment(rule_id)))
                .await
        }
        RuleCommand::Validate {
            blueprint_id,
            context_id,
            source,
        } => {
            api.post_json(
                "/rules/validate",
                rule_body(blueprint_id, context_id, source)?,
            )
            .await
        }
        RuleCommand::Create {
            blueprint_id,
            context_id,
            source,
        } => {
            api.post_json("/rules", rule_body(blueprint_id, context_id, source)?)
                .await
        }
        RuleCommand::Update {
            rule_id,
            accept_existing_violations,
            source,
        } => {
            api.put_json(
                &format!("/rules/{}", segment(rule_id)),
                json!({
                    "definition": read_source(source)?,
                    "accept_existing_violations": accept_existing_violations,
                }),
            )
            .await
        }
        RuleCommand::Enable {
            rule_id,
            accept_existing_violations,
        } => {
            api.post_json(
                &format!("/rules/{}/enable", segment(rule_id)),
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
            definition_file,
        } => {
            let definition = definition_file
                .map(|file| {
                    read_source(SourceInput {
                        file: Some(file),
                        stdin: false,
                    })
                })
                .transpose()?;
            api.post_json(
                &format!("/rules/{}/run-now", segment(rule_id)),
                json!({"idempotency_key":idempotency_key,"record_id":record_id,"dry_run":dry_run,"definition":definition}),
            )
            .await
        }
        RuleCommand::RunList { record_id, page } => {
            let [limit, offset] = page.query();
            api.get(&with_query(
                "/rule-runs",
                [
                    ("record_id", record_id.map(|id| id.to_string())),
                    limit,
                    offset,
                ],
            ))
            .await
        }
        RuleCommand::RunReplay { run_id } => {
            api.post(&format!("/rule-runs/{}/replay", segment(run_id)))
                .await
        }
        RuleCommand::Findings {
            record_id,
            rule_id,
            blueprint_id,
            context_id,
            severity,
            state,
            unresolved,
            page,
        } => {
            let id = |id: Option<Uuid>| id.map(|id| id.to_string());
            let [limit, offset] = page.query();
            api.get(&with_query(
                FINDINGS_PATH,
                [
                    ("record_id", id(record_id)),
                    ("rule_id", id(rule_id)),
                    ("blueprint_id", id(blueprint_id)),
                    ("context_id", id(context_id)),
                    (
                        "severity",
                        severity.map(|severity| severity.as_str().to_owned()),
                    ),
                    ("state", state.map(|state| state.as_str().to_owned())),
                    ("unresolved", flag(unresolved)),
                    limit,
                    offset,
                ],
            ))
            .await
        }
        RuleCommand::FindingCount { record_id } => {
            api.get(&with_query(
                &format!("{FINDINGS_PATH}/count"),
                [("record_id", record_id.map(|id| id.to_string()))],
            ))
            .await
        }
        RuleCommand::Recheck {
            record_id,
            idempotency_key,
        } => {
            api.post_json(
                &format!("{FINDINGS_PATH}/recheck"),
                json!({ "record_id": record_id, "idempotency_key": idempotency_key }),
            )
            .await
        }
        RuleCommand::Acknowledge { finding_id } => {
            api.post(&format!(
                "{FINDINGS_PATH}/{}/acknowledge",
                segment(finding_id)
            ))
            .await
        }
    }
}

const FINDINGS_PATH: &str = "/rule-findings";

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum FindingSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl FindingSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum FindingState {
    Open,
    Acknowledged,
    Resolved,
}

impl FindingState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
        }
    }
}

pub(crate) fn rule_body(
    blueprint_id: Uuid,
    context_id: Option<Uuid>,
    source: SourceInput,
) -> Result<Value, CliError> {
    Ok(json!({
        "blueprint_id": blueprint_id,
        "context_id": context_id,
        "definition": read_source(source)?,
    }))
}
