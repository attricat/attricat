use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::commands::extension::operation::{destination_reference, operation_source};
use crate::error::CliError;
use crate::input::json_input;
use crate::transport::{Api, segment};

pub(crate) const MIN_SCHEDULE_INTERVAL_SECONDS: i32 = 60;

/// Thirty days.
pub(crate) const MAX_SCHEDULE_INTERVAL_SECONDS: i32 = 30 * 24 * 60 * 60;

#[derive(Subcommand)]
pub(crate) enum ExtensionScheduleCommand {
    List,
    Create {
        extension_id: String,
        #[arg(long)]
        operation_id: String,
        /// JSON value or path to a JSON file, checked against the
        /// operation's request schema.
        #[arg(long)]
        input: String,
        #[arg(long)]
        input_file_id: Option<Uuid>,
        /// Where the operation delivers its output: a JSON object or a file
        /// containing it, interpreted by the extension.
        #[arg(long)]
        destination: Option<String>,
        #[arg(long, value_parser = schedule_interval_seconds())]
        interval_seconds: i32,
    },
    Update {
        schedule_id: Uuid,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long, value_parser = schedule_interval_seconds())]
        interval_seconds: i32,
    },
}

fn schedule_interval_seconds() -> clap::builder::RangedI64ValueParser<i32> {
    clap::value_parser!(i32)
        .range(i64::from(MIN_SCHEDULE_INTERVAL_SECONDS)..=i64::from(MAX_SCHEDULE_INTERVAL_SECONDS))
}

pub(crate) async fn extension_schedule_command(
    api: &Api,
    command: ExtensionScheduleCommand,
) -> Result<String, CliError> {
    match command {
        ExtensionScheduleCommand::List => api.get("/extension-operation-schedules").await,
        ExtensionScheduleCommand::Create {
            extension_id,
            operation_id,
            input,
            input_file_id,
            destination,
            interval_seconds,
        } => {
            let body = json!({
                "operation_id": operation_id,
                "input": json_input(&input, "--input")?,
                "source_reference": operation_source(input_file_id),
                "destination_reference": destination_reference(destination.as_deref())?,
                "interval_seconds": interval_seconds,
            });
            api.post_json(
                &format!("/extensions/{}/operation-schedules", segment(extension_id)),
                body,
            )
            .await
        }
        ExtensionScheduleCommand::Update {
            schedule_id,
            enabled,
            interval_seconds,
        } => {
            api.patch_json(
                &format!("/extension-operation-schedules/{}", segment(schedule_id)),
                json!({ "enabled": enabled, "interval_seconds": interval_seconds }),
            )
            .await
        }
    }
}
