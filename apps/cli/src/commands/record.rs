use std::path::PathBuf;

use clap::Subcommand;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::error::CliError;
use crate::input::json_array_argument;
use crate::transport::{request, request_value, segment};

#[derive(Subcommand)]
pub(crate) enum RecordCommand {
    Create {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        values: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// JSON array of agent/operator-owned tags.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON object of agent/operator-owned metadata.
        #[arg(long)]
        system_metadata: Option<String>,
    },
    Get {
        record_id: Uuid,
    },
    Delete {
        record_id: Uuid,
    },
    /// Apply create, update and delete operations to several records
    /// atomically: all succeed or none do.
    Batch {
        /// JSON array of batch operations, or a file containing it.
        #[arg(long)]
        operations: String,
    },
    List {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        related_from: Uuid,
        #[arg(long)]
        relationship: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<Uuid>,
    },
    Preview {
        record_id: Uuid,
        #[arg(long)]
        relationship_depth: Option<u8>,
        #[arg(long)]
        relationship_limit: Option<u32>,
    },
    ResolvedPreview {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    Search {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 25)]
        size: u32,
        #[arg(long)]
        cursor: Option<String>,
        /// JSON array; return records containing every requested system tag.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON array of structured search filters, or a file containing it.
        #[arg(long)]
        filters: Option<String>,
        #[arg(long)]
        outdated: bool,
        #[arg(long)]
        include_total: bool,
        /// JSON array of relationship-tree facet requests, or a file containing it.
        #[arg(long)]
        relationship_tree_facets: Option<String>,
        #[arg(long)]
        sort_field: Option<String>,
        #[arg(long, requires = "sort_field", value_parser = ["asc", "desc"])]
        sort_direction: Option<String>,
    },
    Form {
        record_id: Uuid,
    },
    Update {
        record_id: Uuid,
        #[arg(long)]
        values: Option<PathBuf>,
        #[arg(long)]
        relationships: Option<PathBuf>,
        #[arg(long)]
        remove_values: Option<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
        /// JSON array of agent/operator-owned tags. Omit to retain existing tags.
        #[arg(long)]
        system_tags: Option<String>,
        /// JSON object of agent/operator-owned metadata. Omit to retain existing metadata.
        #[arg(long)]
        system_metadata: Option<String>,
    },
    Migrate {
        record_id: Uuid,
        /// JSON array or file of scalar migration values.
        #[arg(long)]
        values: Option<String>,
        /// JSON array or file of relationship migration sets.
        #[arg(long)]
        relationships: Option<String>,
        /// JSON array or file of attribute codes to discard.
        #[arg(long)]
        discard_attributes: Option<String>,
    },
    Hierarchy {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
        #[arg(long)]
        field: String,
    },
    IncomingRelationships {
        record_id: Uuid,
        #[arg(long)]
        relationships: String,
        #[arg(long)]
        size: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
    FacetChildren {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        source_relationship_field: Option<String>,
        #[arg(long)]
        hierarchy_field: Option<String>,
        #[arg(long)]
        context_id: Uuid,
        #[arg(long)]
        parent_id: Option<Uuid>,
        #[arg(long)]
        cursor: Option<Uuid>,
        #[arg(long)]
        selected_target_ids: Option<String>,
        /// JSON array of this level's node IDs to return even outside the page.
        #[arg(long)]
        include_ids: Option<String>,
    },
    Publication {
        #[command(subcommand)]
        command: RecordPublicationCommand,
    },
    Changes {
        record_id: Uuid,
    },
    ValueHistory {
        record_id: Uuid,
    },
    RestoreValue {
        record_id: Uuid,
        history_id: Uuid,
    },
    MigrateBulk {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        from_version: i64,
        #[arg(long, default_value_t = 100)]
        size: u32,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RecordPublicationCommand {
    List {
        record_id: Uuid,
    },
    Publish {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    Unpublish {
        record_id: Uuid,
        #[arg(long)]
        context_id: Uuid,
    },
    PublishAll {
        record_id: Uuid,
    },
}

pub(crate) async fn record_publication_command(
    client: &Client,
    server: &Url,
    command: RecordPublicationCommand,
) -> Result<String, CliError> {
    match command {
        RecordPublicationCommand::List { record_id } => {
            request(
                client,
                server,
                Method::GET,
                &format!("/v1/records/{}/publications", segment(record_id)),
                None,
            )
            .await
        }
        RecordPublicationCommand::Publish {
            record_id,
            context_id,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/v1/records/{}/publications", segment(record_id)),
                Some(json!({"context_id":context_id})),
            )
            .await
        }
        RecordPublicationCommand::Unpublish {
            record_id,
            context_id,
        } => {
            request(
                client,
                server,
                Method::POST,
                &format!("/v1/records/{}/publications/unpublish", segment(record_id)),
                Some(json!({"context_id":context_id})),
            )
            .await
        }
        RecordPublicationCommand::PublishAll { record_id } => {
            request(
                client,
                server,
                Method::POST,
                &format!(
                    "/v1/records/{}/publications/publish-all",
                    segment(record_id)
                ),
                None,
            )
            .await
        }
    }
}

pub(crate) async fn migrate_record(
    client: &Client,
    server: &Url,
    record_id: Uuid,
    dry_run: bool,
    values: Option<&str>,
    relationships: Option<&str>,
    discard_attributes: Option<&str>,
) -> Result<Value, CliError> {
    let preview = request_value(
        client,
        server,
        Method::POST,
        &format!(
            "/v1/records/{}/blueprint-migration/preview",
            segment(record_id)
        ),
        None,
    )
    .await?;
    let status = preview["status"]
        .as_str()
        .ok_or(CliError::InvalidResponse)?;
    // Bulk migration supplies no remediation input, so a needs-input preview is
    // a classification result, not a migration request. Manual migration may
    // submit the caller's explicit remediation arrays.
    if status == "blocked"
        || dry_run
        || (status == "needs_input"
            && values.is_none()
            && relationships.is_none()
            && discard_attributes.is_none())
    {
        return Ok(json!({
            "record_id": record_id,
            "status": status,
            "issues": preview["issues"],
        }));
    }
    let migration_id = preview["migration_id"]
        .as_str()
        .ok_or(CliError::InvalidResponse)?;
    let target_version = preview["target"]["blueprint"]["version"]
        .as_i64()
        .ok_or(CliError::InvalidResponse)?;
    request_value(
        client,
        server,
        Method::POST,
        &format!("/v1/records/{}/blueprint-migration", segment(record_id)),
        Some(json!({
            "migration_id": migration_id,
            "expected_target_version": target_version,
            "values": json_array_argument(values, "--values")?,
            "relationships": json_array_argument(relationships, "--relationships")?,
            "discard_attributes": json_array_argument(discard_attributes, "--discard-attributes")?,
        })),
    )
    .await
}

pub(crate) async fn migrate_records(
    client: &Client,
    server: &Url,
    blueprint: &str,
    from_version: i64,
    size: u32,
    dry_run: bool,
) -> Result<String, CliError> {
    if from_version <= 0 {
        return Err(CliError::Input(
            "--from-version must be positive".to_owned(),
        ));
    }
    if size == 0 {
        return Err(CliError::Input("--size must be positive".to_owned()));
    }
    let mut cursor: Option<String> = None;
    let mut seen_cursors = std::collections::HashSet::new();
    let mut migrated = 0;
    let mut ready = 0;
    let mut needs_input = Vec::new();
    let mut blocked = Vec::new();
    let mut failed = Vec::new();
    loop {
        if let Some(current_cursor) = cursor.as_ref()
            && !seen_cursors.insert(current_cursor.clone())
        {
            return Err(CliError::InvalidResponse);
        }
        let page = request_value(
            client,
            server,
            Method::POST,
            "/v1/records/search",
            Some(json!({
                "blueprint": { "code": blueprint, "version": from_version },
                "query": "",
                "filters": [],
                "page": { "size": size, "cursor": cursor },
            })),
        )
        .await?;
        let items = page["items"].as_array().ok_or(CliError::InvalidResponse)?;
        for item in items {
            let record_id = item["id"]
                .as_str()
                .ok_or(CliError::InvalidResponse)?
                .parse::<Uuid>()
                .map_err(|_| CliError::InvalidResponse)?;
            match migrate_record(client, server, record_id, dry_run, None, None, None).await {
                Ok(result) => match result["status"].as_str() {
                    Some("ready") => ready += 1,
                    Some("needs_input") => needs_input.push(result),
                    Some("blocked") => blocked.push(result),
                    _ => migrated += 1,
                },
                Err(error) => failed.push(json!({
                    "record_id": record_id,
                    "error": error.json()["error"],
                })),
            }
        }
        cursor = match page.get("next_cursor") {
            Some(Value::Null) => None,
            Some(Value::String(cursor)) if !cursor.is_empty() => Some(cursor.clone()),
            _ => return Err(CliError::InvalidResponse),
        };
        if cursor.is_none() {
            break;
        }
    }
    serde_json::to_string(&json!({
        "blueprint": blueprint,
        "from_version": from_version,
        "dry_run": dry_run,
        "migrated": migrated,
        "ready": ready,
        "needs_input": needs_input,
        "blocked": blocked,
        "failed": failed,
    }))
    .map_err(|_| CliError::InvalidResponse)
}
