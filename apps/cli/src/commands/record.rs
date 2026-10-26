use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cli::DEFAULT_LIST_LIMIT;
use crate::error::CliError;
use crate::input::{
    form_update_body, json_array_argument, json_array_input, json_object_input,
    json_string_array_input, values_body_from_file,
};
use crate::transport::{Api, segment, with_query};

/// Records `migrate-bulk` reads per search page.
const MIGRATE_BULK_PAGE_SIZE: u32 = 100;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

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
        #[arg(long, default_value_t = DEFAULT_LIST_LIMIT)]
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
        #[arg(long, value_enum, requires = "sort_field", default_value_t = SortDirection::Asc)]
        sort_direction: SortDirection,
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
        #[arg(long, default_value_t = MIGRATE_BULK_PAGE_SIZE)]
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

pub(crate) async fn record_command(api: &Api, command: RecordCommand) -> Result<String, CliError> {
    match command {
        RecordCommand::Create {
            blueprint,
            version,
            values,
            context_id,
            system_tags,
            system_metadata,
        } => {
            let mut body = json!({
                "blueprint": { "code": blueprint, "version": version },
                "values": values_body_from_file(&values, context_id)?["values"].clone(),
            });
            add_system_fields(&mut body, system_tags, system_metadata)?;
            api.post_json("/v1/records", body).await
        }
        RecordCommand::Get { record_id } => {
            api.get(&format!("/records/{}", segment(record_id))).await
        }
        RecordCommand::Delete { record_id } => {
            api.delete(&format!("/records/{}", segment(record_id)))
                .await
        }
        RecordCommand::Batch { operations } => {
            api.post_json(
                "/v1/records/batch",
                json!({ "operations": json_array_input(&operations, "--operations")? }),
            )
            .await
        }
        RecordCommand::List {
            blueprint,
            related_from,
            relationship,
            limit,
            cursor,
        } => {
            api.get(&with_query(
                "/records",
                [
                    ("blueprint", Some(blueprint)),
                    ("related_from", Some(related_from.to_string())),
                    ("relationship", Some(relationship)),
                    ("limit", limit.map(|limit| limit.to_string())),
                    ("cursor", cursor.map(|cursor| cursor.to_string())),
                ],
            ))
            .await
        }
        RecordCommand::Preview {
            record_id,
            relationship_depth,
            relationship_limit,
        } => {
            api.get(&with_query(
                &format!("/records/{}/preview", segment(record_id)),
                [
                    (
                        "relationship_depth",
                        relationship_depth.map(|depth| depth.to_string()),
                    ),
                    (
                        "relationship_limit",
                        relationship_limit.map(|limit| limit.to_string()),
                    ),
                ],
            ))
            .await
        }
        RecordCommand::ResolvedPreview {
            record_id,
            context_id,
        } => {
            api.get(&with_query(
                &format!("/records/{}/resolved-preview", segment(record_id)),
                [("context_id", Some(context_id.to_string()))],
            ))
            .await
        }
        RecordCommand::Search {
            blueprint,
            version,
            query,
            size,
            cursor,
            system_tags,
            filters,
            outdated,
            include_total,
            relationship_tree_facets,
            sort_field,
            sort_direction,
        } => {
            let mut body = json!({
                "blueprint": { "code": blueprint, "version": version },
                "query": query,
                "filters": json_array_argument(filters.as_deref(), "--filters")?,
                "outdated": outdated,
                "include_total": include_total,
                "relationship_tree_facets": json_array_argument(
                    relationship_tree_facets.as_deref(),
                    "--relationship-tree-facets",
                )?,
                "page": { "size": size, "cursor": cursor },
            });
            if let Some(field) = sort_field {
                body["sort"] = json!({
                    "field": field,
                    "direction": sort_direction.as_str(),
                });
            }
            if let Some(tags) = system_tags {
                body["system_tags"] = json_string_array_input(&tags, "--system-tags")?;
            }
            api.post_json("/v1/records/search", body).await
        }
        RecordCommand::Form { record_id } => {
            api.get(&format!("/v1/records/{}", segment(record_id)))
                .await
        }
        RecordCommand::Update {
            record_id,
            values,
            relationships,
            remove_values,
            context_id,
            system_tags,
            system_metadata,
        } => {
            let mut body = form_update_body(
                values.as_ref(),
                relationships.as_ref(),
                remove_values.as_ref(),
                context_id,
            )?;
            add_system_fields(&mut body, system_tags, system_metadata)?;
            api.put_json(&format!("/v1/records/{}", segment(record_id)), body)
                .await
        }
        RecordCommand::Hierarchy {
            record_id,
            context_id,
            field,
        } => {
            api.get(&with_query(
                &format!("/records/{}/hierarchy", segment(record_id)),
                [
                    ("context_id", Some(context_id.to_string())),
                    ("field", Some(field)),
                ],
            ))
            .await
        }
        RecordCommand::IncomingRelationships {
            record_id,
            relationships,
            size,
            cursor,
        } => {
            api.post_json(
                &format!("/v1/records/{}/incoming-relationships", segment(record_id)),
                json!({
                    "relationships": json_array_input(&relationships, "--relationships")?,
                    "page": { "size": size, "cursor": cursor },
                }),
            )
            .await
        }
        RecordCommand::FacetChildren {
            blueprint,
            version,
            query,
            source_relationship_field,
            hierarchy_field,
            context_id,
            parent_id,
            cursor,
            selected_target_ids,
            include_ids,
        } => {
            api.post_json(
                "/v1/records/facets/relationship-tree/children",
                json!({
                    "blueprint": { "code": blueprint, "version": version },
                    "query": query,
                    "source_relationship_field": source_relationship_field,
                    "hierarchy_field": hierarchy_field,
                    "context_id": context_id,
                    "parent_id": parent_id,
                    "cursor": cursor,
                    "selected_target_ids": json_array_argument(
                        selected_target_ids.as_deref(),
                        "--selected-target-ids",
                    )?,
                    "include_ids": json_array_argument(include_ids.as_deref(), "--include-ids")?,
                }),
            )
            .await
        }
        RecordCommand::Publication { command } => record_publication_command(api, command).await,
        RecordCommand::Changes { record_id } => {
            api.get(&format!("/records/{}/changes", segment(record_id)))
                .await
        }
        RecordCommand::ValueHistory { record_id } => {
            api.get(&format!("/records/{}/values/history", segment(record_id)))
                .await
        }
        RecordCommand::RestoreValue {
            record_id,
            history_id,
        } => {
            api.post(&format!(
                "/records/{}/values/history/{}/restore",
                segment(record_id),
                segment(history_id)
            ))
            .await
        }
        RecordCommand::Migrate {
            record_id,
            values,
            relationships,
            discard_attributes,
        } => {
            let result = migrate_record(
                api,
                record_id,
                false,
                values.as_deref(),
                relationships.as_deref(),
                discard_attributes.as_deref(),
            )
            .await?;
            Ok(result.to_string())
        }
        RecordCommand::MigrateBulk {
            blueprint,
            from_version,
            size,
            dry_run,
        } => migrate_records(api, &blueprint, from_version, size, dry_run).await,
    }
}

/// Sets the agent/operator-owned tags and metadata a write was given.
fn add_system_fields(
    body: &mut Value,
    system_tags: Option<String>,
    system_metadata: Option<String>,
) -> Result<(), CliError> {
    if let Some(tags) = system_tags {
        body["system_tags"] = json_string_array_input(&tags, "--system-tags")?;
    }
    if let Some(metadata) = system_metadata {
        body["system_metadata"] = json_object_input(&metadata, "--system-metadata")?;
    }
    Ok(())
}

pub(crate) async fn record_publication_command(
    api: &Api,
    command: RecordPublicationCommand,
) -> Result<String, CliError> {
    match command {
        RecordPublicationCommand::List { record_id } => {
            api.get(&format!("/v1/records/{}/publications", segment(record_id)))
                .await
        }
        RecordPublicationCommand::Publish {
            record_id,
            context_id,
        } => {
            api.post_json(
                &format!("/v1/records/{}/publications", segment(record_id)),
                json!({"context_id":context_id}),
            )
            .await
        }
        RecordPublicationCommand::Unpublish {
            record_id,
            context_id,
        } => {
            api.post_json(
                &format!("/v1/records/{}/publications/unpublish", segment(record_id)),
                json!({"context_id":context_id}),
            )
            .await
        }
        RecordPublicationCommand::PublishAll { record_id } => {
            api.post(&format!(
                "/v1/records/{}/publications/publish-all",
                segment(record_id)
            ))
            .await
        }
    }
}

pub(crate) async fn migrate_record(
    api: &Api,
    record_id: Uuid,
    dry_run: bool,
    values: Option<&str>,
    relationships: Option<&str>,
    discard_attributes: Option<&str>,
) -> Result<Value, CliError> {
    let preview = api
        .send_value(
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
    api.send_value(
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
    api: &Api,
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
        let page = api
            .send_value(
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
            match migrate_record(api, record_id, dry_run, None, None, None).await {
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
