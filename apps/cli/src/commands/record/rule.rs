use clap::Subcommand;
use uuid::Uuid;

use crate::cli::PageArgs;
use crate::error::CliError;
use crate::transport::{Api, segment, with_query};

/// A record's data quality rules, findings and runs, for anyone who may read
/// the record; the `rule` commands need `rules.read`.
#[derive(Subcommand)]
pub(crate) enum RecordRuleCommand {
    /// List the rules of the record's blueprint.
    List { record_id: Uuid },
    /// List the record's open and acknowledged findings.
    Findings {
        record_id: Uuid,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Count the record's open and acknowledged findings.
    FindingCount { record_id: Uuid },
    /// List the runs that checked the record, newest first.
    Runs {
        record_id: Uuid,
        #[command(flatten)]
        page: PageArgs,
    },
}

pub(crate) async fn record_rule_command(
    api: &Api,
    command: RecordRuleCommand,
) -> Result<String, CliError> {
    match command {
        RecordRuleCommand::List { record_id } => {
            api.get(&format!("{}/rules", record_path(record_id))).await
        }
        RecordRuleCommand::Findings { record_id, page } => {
            api.get(&with_query(
                &format!("{}/rule-findings", record_path(record_id)),
                page.query(),
            ))
            .await
        }
        RecordRuleCommand::FindingCount { record_id } => {
            api.get(&format!("{}/rule-findings/count", record_path(record_id)))
                .await
        }
        RecordRuleCommand::Runs { record_id, page } => {
            api.get(&with_query(
                &format!("{}/rule-runs", record_path(record_id)),
                page.query(),
            ))
            .await
        }
    }
}

fn record_path(record_id: Uuid) -> String {
    format!("/v1/records/{}", segment(record_id))
}
