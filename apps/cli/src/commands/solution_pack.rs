use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::PageArgs;
use crate::error::CliError;
use crate::transport::{Api, ZSTD_MEDIA_TYPE, flag, segment, with_query};

/// Longest solution-pack logical key or existing stable code a mapping accepts.
pub(crate) const MAX_PACK_MAPPING_KEY_BYTES: usize = 128;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum BlueprintPublicationArgument {
    Draft,
    Publish,
}

impl BlueprintPublicationArgument {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Publish => "publish",
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum SolutionPackPlanCommand {
    /// Plan applying a local archive; nothing changes until `apply`.
    Create {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        prefix: String,
        #[arg(long, value_enum)]
        blueprint_publication: BlueprintPublicationArgument,
        /// Explicitly reuse a published blueprint (`logical_key=existing_code`).
        #[arg(long = "map", value_name = "LOGICAL_KEY=EXISTING_CODE")]
        blueprint_maps: Vec<String>,
        /// Explicitly reuse an immutable presentation asset (`logical_key=asset_uuid`).
        #[arg(long = "map-asset", value_name = "LOGICAL_KEY=ASSET_UUID")]
        asset_maps: Vec<String>,
        /// Use an existing context for a pack context instead of creating one
        /// (`logical_key=existing_code`).
        #[arg(long = "map-context", value_name = "LOGICAL_KEY=EXISTING_CODE")]
        context_maps: Vec<String>,
        /// Explicitly select the pack's optional synthetic sample records.
        #[arg(long)]
        include_sample_data: bool,
    },
    /// Show one immutable workspace-scoped plan.
    Get { plan_id: Uuid },
}

#[derive(Subcommand)]
pub(crate) enum SolutionPackApplicationsCommand {
    /// List bounded application history for the active workspace.
    List {
        #[command(flatten)]
        page: PageArgs,
    },
    /// Show one workspace-scoped application and its ordered steps.
    Get { application_id: Uuid },
    /// Permanently abandon a resumable sample-selected application.
    Abandon { application_id: Uuid },
}

#[derive(Subcommand)]
pub(crate) enum SolutionPackChecksCommand {
    /// Evaluate the application's informational checks against current state.
    Rerun { application_id: Uuid },
    /// List bounded immutable check-run history.
    List {
        application_id: Uuid,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Show one immutable check run and its ordered results.
    Get { application_id: Uuid, run_id: Uuid },
}

#[derive(Subcommand)]
pub(crate) enum SolutionPackCommand {
    /// Validate and summarize a local archive on the authoritative server.
    Inspect {
        #[arg(long)]
        file: PathBuf,
    },
    /// Create a local-archive plan, or show an existing plan.
    Plan {
        #[command(subcommand)]
        command: SolutionPackPlanCommand,
    },
    /// Apply exactly the persisted immutable plan; no choices are recomputed.
    Apply { plan_id: Uuid },
    /// Read durable application history.
    Applications {
        #[command(subcommand)]
        command: SolutionPackApplicationsCommand,
    },
    /// Evaluate and read informational setup checks.
    Checks {
        #[command(subcommand)]
        command: SolutionPackChecksCommand,
    },
}

pub(crate) async fn solution_pack_command(
    api: &Api,
    command: SolutionPackCommand,
) -> Result<String, CliError> {
    match command {
        SolutionPackCommand::Inspect { file } => {
            api.upload("/solution-packs/inspect", &file, ZSTD_MEDIA_TYPE)
                .await
        }
        SolutionPackCommand::Plan {
            command: SolutionPackPlanCommand::Get { plan_id },
        } => {
            api.get(&format!("/solution-packs/plans/{}", segment(plan_id)))
                .await
        }
        SolutionPackCommand::Plan {
            command:
                SolutionPackPlanCommand::Create {
                    file,
                    prefix,
                    blueprint_publication,
                    blueprint_maps,
                    asset_maps,
                    context_maps,
                    include_sample_data,
                },
        } => {
            let path = with_query(
                "/solution-packs/plans",
                [
                    ("prefix", Some(prefix)),
                    (
                        "blueprint_publication",
                        Some(blueprint_publication.as_str().to_owned()),
                    ),
                    ("include_sample_data", flag(include_sample_data)),
                ],
            );
            if blueprint_maps.is_empty() && asset_maps.is_empty() && context_maps.is_empty() {
                api.upload(&path, &file, ZSTD_MEDIA_TYPE).await
            } else {
                let maps = PackMappings {
                    blueprints: &blueprint_maps,
                    assets: &asset_maps,
                    contexts: &context_maps,
                };
                solution_pack_plan_upload(api, &path, &file, maps).await
            }
        }
        SolutionPackCommand::Apply { plan_id } => {
            api.post(&format!("/solution-packs/plans/{}/apply", segment(plan_id)))
                .await
        }
        SolutionPackCommand::Applications { command } => match command {
            SolutionPackApplicationsCommand::List { page } => {
                api.get(&with_query(APPLICATIONS_PATH, page.query())).await
            }
            SolutionPackApplicationsCommand::Get { application_id } => {
                api.get(&format!("{APPLICATIONS_PATH}/{}", segment(application_id)))
                    .await
            }
            SolutionPackApplicationsCommand::Abandon { application_id } => {
                api.post(&format!(
                    "{APPLICATIONS_PATH}/{}/abandon",
                    segment(application_id)
                ))
                .await
            }
        },
        SolutionPackCommand::Checks { command } => match command {
            SolutionPackChecksCommand::Rerun { application_id } => {
                api.post(&format!(
                    "{APPLICATIONS_PATH}/{}/checks",
                    segment(application_id)
                ))
                .await
            }
            SolutionPackChecksCommand::List {
                application_id,
                page,
            } => {
                api.get(&with_query(
                    &format!("{APPLICATIONS_PATH}/{}/checks", segment(application_id)),
                    page.query(),
                ))
                .await
            }
            SolutionPackChecksCommand::Get {
                application_id,
                run_id,
            } => {
                api.get(&format!(
                    "{APPLICATIONS_PATH}/{}/checks/{}",
                    segment(application_id),
                    segment(run_id)
                ))
                .await
            }
        },
    }
}

const APPLICATIONS_PATH: &str = "/solution-packs/applications";

/// The `LOGICAL_KEY=VALUE` reuse mappings a plan was given, by kind.
pub(crate) struct PackMappings<'a> {
    pub(crate) blueprints: &'a [String],
    pub(crate) assets: &'a [String],
    pub(crate) contexts: &'a [String],
}

pub(crate) async fn solution_pack_plan_upload(
    api: &Api,
    path: &str,
    archive: &Path,
    maps: PackMappings<'_>,
) -> Result<String, CliError> {
    let blueprint_maps = parse_pack_mappings(
        maps.blueprints,
        "--map",
        "blueprints/",
        "EXISTING_CODE",
        blueprint_mapping_value,
    )?;
    let asset_maps =
        parse_pack_mappings(maps.assets, "--map-asset", "assets/", "ASSET_UUID", |id| {
            Uuid::parse_str(id).ok().map(|id| json!({ "id": id }))
        })?;
    let context_maps = parse_pack_mappings(
        maps.contexts,
        "--map-context",
        "contexts/",
        "EXISTING_CODE",
        |code| {
            let valid = !code.is_empty()
                && code.len() <= MAX_PACK_MAPPING_KEY_BYTES
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
            valid.then(|| json!({ "code": code }))
        },
    )?;

    let length = fs::metadata(archive)
        .map_err(|error| CliError::Input(format!("cannot read {}: {error}", archive.display())))?
        .len();
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::open(archive).await.map_err(
            |error| CliError::Input(format!("cannot read {}: {error}", archive.display())),
        )?);
    let archive_part =
        reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), length)
            .file_name("solution-pack.tar.zst")
            .mime_str(ZSTD_MEDIA_TYPE)
            .map_err(|error| CliError::Input(error.to_string()))?;
    let mut form = reqwest::multipart::Form::new().part("archive", archive_part);
    for (field, mappings) in [
        ("blueprint_map", blueprint_maps),
        ("asset_map", asset_maps),
        ("context_map", context_maps),
    ] {
        for mapping in mappings {
            form = form.text(field, mapping);
        }
    }
    api.send_form(Method::POST, path, form).await
}

// Match the solution-pack planner's stable-code contract, including codes it
// generates from hyphenated logical keys. Existing mappings are not prefixes.
pub(crate) fn blueprint_mapping_value(code: &str) -> Option<Value> {
    let valid = !code.is_empty()
        && code.len() <= MAX_PACK_MAPPING_KEY_BYTES
        && code.as_bytes()[0].is_ascii_lowercase()
        && code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
        && Uuid::parse_str(code).is_err();
    valid.then(|| json!({ "code": code }))
}

/// Parses `LOGICAL_KEY=VALUE` solution-pack mapping flags into the JSON parts
/// the plan endpoint accepts. Keys must start with `key_prefix` and appear
/// once; `parse_value` returns the part's value fields, or `None` when the
/// value is invalid.
pub(crate) fn parse_pack_mappings(
    requested: &[String],
    flag: &str,
    key_prefix: &str,
    value_name: &str,
    parse_value: impl Fn(&str) -> Option<Value>,
) -> Result<Vec<String>, CliError> {
    let mut seen = std::collections::BTreeSet::new();
    requested
        .iter()
        .map(|requested| {
            let (key, value) = requested.split_once('=').ok_or_else(|| {
                CliError::Input(format!("{flag} must be LOGICAL_KEY={value_name}"))
            })?;
            let invalid = || CliError::Input(format!("invalid {flag} value '{requested}'"));
            if !key.starts_with(key_prefix) || key.len() > MAX_PACK_MAPPING_KEY_BYTES {
                return Err(invalid());
            }
            let mut part = parse_value(value).ok_or_else(invalid)?;
            if !seen.insert(key) {
                return Err(CliError::Input(format!("duplicate {flag} key '{key}'")));
            }
            part["key"] = json!(key);
            Ok(part.to_string())
        })
        .collect()
}
