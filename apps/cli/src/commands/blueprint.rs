use clap::Subcommand;
use uuid::Uuid;

use crate::cli::SourceInput;
use serde_json::json;

use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{Api, flag, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum BlueprintCommand {
    List {
        #[arg(long)]
        include_drafts: bool,
    },
    Create(SourceInput),
    CreateVersion {
        blueprint_id: Uuid,
        #[command(flatten)]
        source: SourceInput,
    },
    /// Run every save check on a definition without storing it. With
    /// `--blueprint-id`, check it as that blueprint's next revision.
    Validate {
        #[arg(long)]
        blueprint_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        blueprint_id: Uuid,
        version: i64,
    },
    Get {
        blueprint_id: Uuid,
    },
    GetVersion {
        blueprint_id: Uuid,
        version: i64,
    },
    Catalogue,
    ListVersions {
        blueprint_id: Uuid,
    },
    PublishRecords {
        blueprint_id: Uuid,
        version: i64,
        #[arg(long)]
        context_id: Uuid,
    },
    PublishRecordsAll {
        blueprint_id: Uuid,
        version: i64,
    },
    /// Show how a version's safe migration would affect existing records.
    MigrationImpact {
        blueprint_id: Uuid,
        version: i64,
    },
    /// List the blueprint's migration batches and their progress.
    MigrationBatches {
        blueprint_id: Uuid,
    },
    SafeMigrationBatch {
        blueprint_id: Uuid,
        version: i64,
    },
    Resolve {
        code: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        include_drafts: bool,
    },
}

pub(crate) async fn blueprint_command(
    api: &Api,
    command: BlueprintCommand,
) -> Result<String, CliError> {
    match command {
        BlueprintCommand::List { include_drafts } => {
            api.get(&with_query(
                "/blueprints",
                [("include_drafts", flag(include_drafts))],
            ))
            .await
        }
        BlueprintCommand::Create(source) => {
            api.post_json("/blueprints", json!({ "definition": read_source(source)? }))
                .await
        }
        BlueprintCommand::CreateVersion {
            blueprint_id,
            source,
        } => {
            api.post_json(
                &format!("/blueprints/{}/versions", segment(blueprint_id)),
                json!({ "definition": read_source(source)? }),
            )
            .await
        }
        BlueprintCommand::Validate {
            blueprint_id,
            source,
        } => {
            let path = match blueprint_id {
                Some(blueprint_id) => {
                    format!("/blueprints/{}/versions/validate", segment(blueprint_id))
                }
                None => "/blueprints/validate".to_owned(),
            };
            api.post_json(&path, json!({ "definition": read_source(source)? }))
                .await
        }
        BlueprintCommand::Publish {
            blueprint_id,
            version,
        } => {
            api.post(&format!(
                "/blueprints/{}/versions/{version}/publish",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::Get { blueprint_id } => {
            api.get(&format!("/blueprints/{}", segment(blueprint_id)))
                .await
        }
        BlueprintCommand::GetVersion {
            blueprint_id,
            version,
        } => {
            api.get(&format!(
                "/blueprints/{}/versions/{version}",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::Catalogue => api.get("/blueprints/catalogue").await,
        BlueprintCommand::ListVersions { blueprint_id } => {
            api.get(&format!("/blueprints/{}/versions", segment(blueprint_id)))
                .await
        }
        BlueprintCommand::PublishRecords {
            blueprint_id,
            version,
            context_id,
        } => {
            api.post_json(
                &format!(
                    "/blueprints/{}/versions/{version}/record-publications",
                    segment(blueprint_id)
                ),
                json!({ "context_id": context_id }),
            )
            .await
        }
        BlueprintCommand::PublishRecordsAll {
            blueprint_id,
            version,
        } => {
            api.post(&format!(
                "/blueprints/{}/versions/{version}/record-publications/publish-all",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::MigrationImpact {
            blueprint_id,
            version,
        } => {
            api.get(&format!(
                "/blueprints/{}/versions/{version}/safe-migration-impact",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::MigrationBatches { blueprint_id } => {
            api.get(&format!(
                "/blueprints/{}/migration-batches",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::SafeMigrationBatch {
            blueprint_id,
            version,
        } => {
            api.post(&format!(
                "/blueprints/{}/versions/{version}/safe-migration-batches",
                segment(blueprint_id)
            ))
            .await
        }
        BlueprintCommand::Resolve {
            code,
            version: Some(version),
            ..
        } => {
            api.get(&format!(
                "/blueprints/by-code/{}/versions/{version}",
                segment(&code)
            ))
            .await
        }
        BlueprintCommand::Resolve {
            code,
            version: None,
            include_drafts,
        } => {
            api.get(&with_query(
                &format!("/blueprints/by-code/{}", segment(&code)),
                [("include_drafts", flag(include_drafts))],
            ))
            .await
        }
    }
}
