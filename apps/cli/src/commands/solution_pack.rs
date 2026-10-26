use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Subcommand, ValueEnum};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::cli::DEFAULT_LIST_LIMIT;
use crate::error::CliError;
use crate::transport::{
    TRANSFER_TIMEOUT, ZSTD_MEDIA_TYPE, endpoint, raw_response, raw_upload, request, segment,
};

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
    /// Show one immutable workspace-scoped plan.
    Show { plan_id: Uuid },
}

#[derive(Subcommand)]
pub(crate) enum SolutionPackApplicationsCommand {
    /// List bounded application history for the active workspace.
    List {
        #[arg(long, default_value_t = u32::from(DEFAULT_LIST_LIMIT))]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show one workspace-scoped application and its ordered steps.
    Show { application_id: Uuid },
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
        #[arg(long, default_value_t = u32::from(DEFAULT_LIST_LIMIT))]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show one immutable check run and its ordered results.
    Show { application_id: Uuid, run_id: Uuid },
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
        command: Option<SolutionPackPlanCommand>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        blueprint_publication: Option<BlueprintPublicationArgument>,
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
    client: &Client,
    server: &Url,
    command: SolutionPackCommand,
) -> Result<String, CliError> {
    match command {
        SolutionPackCommand::Inspect { file } => {
            raw_upload(
                client,
                server,
                "/solution-packs/inspect",
                &file,
                ZSTD_MEDIA_TYPE,
            )
            .await
        }
        SolutionPackCommand::Plan {
            command: Some(SolutionPackPlanCommand::Show { plan_id }),
            file: None,
            prefix: None,
            blueprint_publication: None,
            blueprint_maps,
            asset_maps,
            context_maps,
            include_sample_data: false,
        } if blueprint_maps.is_empty() && asset_maps.is_empty() && context_maps.is_empty() => {
            request(
                client,
                server,
                Method::GET,
                &format!("/solution-packs/plans/{}", segment(plan_id)),
                None,
            )
            .await
        }
        SolutionPackCommand::Plan {
            command: None,
            file: Some(file),
            prefix: Some(prefix),
            blueprint_publication: Some(publication),
            blueprint_maps,
            asset_maps,
            context_maps,
            include_sample_data,
        } => {
            let mut path = format!(
                "/solution-packs/plans?prefix={}&blueprint_publication={}",
                segment(prefix),
                publication.as_str()
            );
            if include_sample_data {
                path.push_str("&include_sample_data=true");
            }
            if blueprint_maps.is_empty() && asset_maps.is_empty() && context_maps.is_empty() {
                raw_upload(client, server, &path, &file, ZSTD_MEDIA_TYPE).await
            } else {
                solution_pack_plan_upload(
                    client,
                    server,
                    &path,
                    &file,
                    &blueprint_maps,
                    &asset_maps,
                    &context_maps,
                )
                .await
            }
        }
        SolutionPackCommand::Plan { .. } => Err(CliError::Input(
            "solution-pack plan requires --file, --prefix, and --blueprint-publication; plan show accepts only a plan ID".to_owned(),
        )),
        SolutionPackCommand::Apply { plan_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/solution-packs/plans/{}/apply", segment(plan_id)),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::List { limit, offset },
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/solution-packs/applications?limit={limit}&offset={offset}"),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::Show { application_id },
        } => {
            request(
                client,
                server,
                Method::GET,
                &format!(
                    "/solution-packs/applications/{}",
                    segment(application_id)
                ),
                None,
            )
            .await
        }
        SolutionPackCommand::Applications {
            command: SolutionPackApplicationsCommand::Abandon { application_id },
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/solution-packs/applications/{}/abandon",
                    segment(application_id)
                ),
                None,
            )
            .await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::Rerun { application_id } } => {
            request(client, server, Method::POST, &format!("/solution-packs/applications/{}/checks", segment(application_id)), None).await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::List { application_id, limit, offset } } => {
            request(client, server, Method::GET, &format!("/solution-packs/applications/{}/checks?limit={limit}&offset={offset}", segment(application_id)), None).await
        }
        SolutionPackCommand::Checks { command: SolutionPackChecksCommand::Show { application_id, run_id } } => {
            request(client, server, Method::GET, &format!("/solution-packs/applications/{}/checks/{}", segment(application_id), segment(run_id)), None).await
        }
    }
}

pub(crate) async fn solution_pack_plan_upload(
    client: &Client,
    server: &Url,
    path: &str,
    archive: &Path,
    requested_maps: &[String],
    requested_asset_maps: &[String],
    requested_context_maps: &[String],
) -> Result<String, CliError> {
    let blueprint_maps = parse_pack_mappings(
        requested_maps,
        "--map",
        "blueprints/",
        "EXISTING_CODE",
        blueprint_mapping_value,
    )?;
    let asset_maps = parse_pack_mappings(
        requested_asset_maps,
        "--map-asset",
        "assets/",
        "ASSET_UUID",
        |id| Uuid::parse_str(id).ok().map(|id| json!({ "id": id })),
    )?;
    let context_maps = parse_pack_mappings(
        requested_context_maps,
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
    raw_response(
        client
            .post(endpoint(server, path)?)
            .multipart(form)
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
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
