use clap::Subcommand;
use serde_json::json;
use uuid::Uuid;

use crate::cli::SourceInput;
use crate::error::CliError;
use crate::input::read_source;
use crate::transport::{Api, flag, segment, with_query};

#[derive(Subcommand)]
pub(crate) enum ReusableAttributeCommand {
    List {
        #[arg(long)]
        include_drafts: bool,
    },
    /// Create a definition from a TOML definition file.
    Create(SourceInput),
    /// Run every save check on a definition without storing it. With
    /// `--definition-id`, check it as that definition's next version.
    Validate {
        #[arg(long)]
        definition_id: Option<Uuid>,
        #[command(flatten)]
        source: SourceInput,
    },
    CreateVersion {
        definition_id: Uuid,
        #[command(flatten)]
        source: SourceInput,
    },
    Publish {
        revision_id: Uuid,
    },
    /// Add a published revision to a record.
    Apply {
        record_id: Uuid,
        #[arg(long)]
        revision_id: Uuid,
    },
    /// Named sets of reusable attributes applied together.
    Group {
        #[command(subcommand)]
        command: ReusableAttributeGroupCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum ReusableAttributeGroupCommand {
    List,
    Create {
        #[arg(long)]
        code: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 0)]
        position: i64,
        /// Published revision ID; repeat for several.
        #[arg(long = "revision-id", required = true)]
        revision_ids: Vec<Uuid>,
    },
    /// Add every attribute of a group to a record.
    Apply {
        record_id: Uuid,
        group_id: Uuid,
    },
}

const DEFINITIONS_PATH: &str = "/reusable-attributes";

pub(crate) async fn reusable_attribute_command(
    api: &Api,
    command: ReusableAttributeCommand,
) -> Result<String, CliError> {
    match command {
        ReusableAttributeCommand::List { include_drafts } => {
            api.get(&with_query(
                DEFINITIONS_PATH,
                [("include_drafts", flag(include_drafts))],
            ))
            .await
        }
        ReusableAttributeCommand::Create(source) => {
            api.post_json(
                DEFINITIONS_PATH,
                json!({ "definition": read_source(source)? }),
            )
            .await
        }
        ReusableAttributeCommand::Validate {
            definition_id,
            source,
        } => {
            let path = match definition_id {
                Some(id) => format!("{DEFINITIONS_PATH}/{}/versions/validate", segment(id)),
                None => format!("{DEFINITIONS_PATH}/validate"),
            };
            api.post_json(&path, json!({ "definition": read_source(source)? }))
                .await
        }
        ReusableAttributeCommand::CreateVersion {
            definition_id,
            source,
        } => {
            api.post_json(
                &format!("{DEFINITIONS_PATH}/{}/versions", segment(definition_id)),
                json!({ "definition": read_source(source)? }),
            )
            .await
        }
        ReusableAttributeCommand::Publish { revision_id } => {
            api.post(&format!(
                "/reusable-attribute-revisions/{}/publish",
                segment(revision_id)
            ))
            .await
        }
        ReusableAttributeCommand::Apply {
            record_id,
            revision_id,
        } => {
            api.post_json(
                &format!("/v1/records/{}/reusable-attributes", segment(record_id)),
                json!({ "reusable_attribute_revision_id": revision_id }),
            )
            .await
        }
        ReusableAttributeCommand::Group { command } => match command {
            ReusableAttributeGroupCommand::List => api.get(GROUPS_PATH).await,
            ReusableAttributeGroupCommand::Create {
                code,
                name,
                position,
                revision_ids,
            } => {
                let body = json!({
                    "code": code,
                    "name": name,
                    "position": position,
                    "reusable_attribute_revision_ids": revision_ids,
                });
                api.post_json(GROUPS_PATH, body).await
            }
            ReusableAttributeGroupCommand::Apply {
                record_id,
                group_id,
            } => {
                api.post(&format!(
                    "/v1/records/{}/reusable-attribute-groups/{}",
                    segment(record_id),
                    segment(group_id)
                ))
                .await
            }
        },
    }
}

const GROUPS_PATH: &str = "/reusable-attribute-groups";
