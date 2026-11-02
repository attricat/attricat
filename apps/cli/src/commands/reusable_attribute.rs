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
    /// List the reusable attributes the Explorer offers as filters on a
    /// blueprint's current-version records.
    FilterFields {
        /// The blueprint's code.
        #[arg(long)]
        blueprint: String,
    },
    /// Create a definition from a YAML definition file.
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
        /// Attribute as `namespace:code`; repeat for several. Applying the
        /// group uses each attribute's latest published revision.
        #[arg(long = "attribute", required = true)]
        attributes: Vec<String>,
    },
    /// Change a group. Options left out keep their value; its code cannot change.
    Update {
        group_id: Uuid,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        position: Option<i64>,
        /// Replaces the attributes, as `namespace:code`; repeat for several.
        #[arg(long = "attribute")]
        attributes: Vec<String>,
    },
    /// Delete a group. Records keep the attributes it already added.
    Delete {
        group_id: Uuid,
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
        ReusableAttributeCommand::FilterFields { blueprint } => {
            api.get(&with_query(
                "/v1/records/reusable-attributes",
                [("blueprint", Some(blueprint))],
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
                attributes,
            } => {
                let body = json!({
                    "code": code,
                    "name": name,
                    "position": position,
                    "reusable_attribute_codes": attributes,
                });
                api.post_json(GROUPS_PATH, body).await
            }
            ReusableAttributeGroupCommand::Update {
                group_id,
                name,
                position,
                attributes,
            } => {
                let mut body = serde_json::Map::new();
                if let Some(name) = name {
                    body.insert("name".to_owned(), json!(name));
                }
                if let Some(position) = position {
                    body.insert("position".to_owned(), json!(position));
                }
                if !attributes.is_empty() {
                    body.insert("reusable_attribute_codes".to_owned(), json!(attributes));
                }
                api.patch_json(&format!("{GROUPS_PATH}/{}", segment(group_id)), body.into())
                    .await
            }
            ReusableAttributeGroupCommand::Delete { group_id } => {
                api.delete(&format!("{GROUPS_PATH}/{}", segment(group_id)))
                    .await
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
