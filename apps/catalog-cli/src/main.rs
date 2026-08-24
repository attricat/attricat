use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use clap::{Args, Parser, Subcommand};
use futures_util::StreamExt;
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Parser)]
#[command(name = "catalog", about = "JSON-first client for the Catalog API")]
struct Cli {
    #[arg(long, env = "CATALOG_SERVER")]
    server: Option<Url>,
    /// Personal API token. It is sent only as an HTTP Bearer credential.
    #[arg(long, env = "CATALOG_TOKEN", hide_env_values = true)]
    token: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Health,
    Blueprint {
        #[command(subcommand)]
        command: BlueprintCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    Entity {
        #[command(subcommand)]
        command: EntityCommand,
    },
    Value {
        #[command(subcommand)]
        command: ValueCommand,
    },
}

#[derive(Subcommand)]
enum BlueprintCommand {
    List {
        #[arg(long)]
        include_drafts: bool,
    },
    Create(SourceInput),
    Revision {
        blueprint_id: Uuid,
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
    Resolve {
        code: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        include_drafts: bool,
    },
}

#[derive(Args)]
struct SourceInput {
    #[arg(long, conflicts_with = "stdin")]
    file: Option<PathBuf>,
    #[arg(long)]
    stdin: bool,
}

#[derive(Subcommand)]
enum ContextCommand {
    List,
    Create {
        #[arg(long, conflicts_with_all = ["code", "data", "parent_id"])]
        file: Option<PathBuf>,
        #[arg(long, requires = "data")]
        code: Option<String>,
        #[arg(long, requires = "code")]
        data: Option<String>,
        #[arg(long, requires_all = ["code", "data"])]
        parent_id: Option<Uuid>,
    },
    Get {
        code: String,
    },
    Update {
        context_id: Uuid,
        #[arg(long)]
        parent_id: Uuid,
        #[arg(long)]
        data: String,
    },
    Delete {
        context_id: Uuid,
    },
}

#[derive(Subcommand)]
enum EntityCommand {
    Create {
        #[arg(long)]
        blueprint: String,
        #[arg(long)]
        version: Option<i64>,
        #[arg(long)]
        values: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Get {
        entity_id: Uuid,
    },
    Delete {
        entity_id: Uuid,
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
        entity_id: Uuid,
        #[arg(long)]
        relationship_depth: Option<u8>,
        #[arg(long)]
        relationship_limit: Option<u32>,
    },
    ResolvedPreview {
        entity_id: Uuid,
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
    },
    Form {
        entity_id: Uuid,
    },
    Update {
        entity_id: Uuid,
        #[arg(long)]
        values: Option<PathBuf>,
        #[arg(long)]
        relationships: Option<PathBuf>,
        #[arg(long)]
        remove_values: Option<PathBuf>,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Migrate {
        entity_id: Uuid,
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
enum ValueCommand {
    Append {
        entity_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Current {
        entity_id: Uuid,
    },
    Replace {
        entity_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
    Remove {
        entity_id: Uuid,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        context_id: Option<Uuid>,
    },
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Transport(String),
    #[error("{message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    #[error("server returned invalid JSON")]
    InvalidResponse,
    #[error("server response exceeded {MAX_RESPONSE_BYTES} bytes")]
    ResponseTooLarge,
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Input(_) => 2,
            Self::Transport(_) => 3,
            Self::Api { .. } => 4,
            Self::InvalidResponse | Self::ResponseTooLarge => 5,
        }
    }

    fn json(&self) -> Value {
        match self {
            Self::Input(message) => {
                json!({ "error": { "code": "invalid_input", "message": message, "status": null } })
            }
            Self::Transport(message) => {
                json!({ "error": { "code": "request_failed", "message": message, "status": null } })
            }
            Self::Api {
                status,
                code,
                message,
            } => json!({ "error": { "code": code, "message": message, "status": status } }),
            Self::InvalidResponse => {
                json!({ "error": { "code": "invalid_response", "message": self.to_string(), "status": null } })
            }
            Self::ResponseTooLarge => {
                json!({ "error": { "code": "response_too_large", "message": self.to_string(), "status": null } })
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFile {
    code: String,
    data: toml::Value,
    parent_id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueFile {
    values: Vec<ValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueInput {
    kind: String,
    attribute_id: Option<Uuid>,
    attribute_code: Option<String>,
    context_id: Option<Uuid>,
    value: Option<toml::Value>,
    target_entity_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipFile {
    relationships: Vec<RelationshipInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveValuesFile {
    remove_values: Vec<RemoveValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveValueInput {
    attribute_code: String,
    context_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationshipInput {
    attribute_id: Option<Uuid>,
    attribute_code: Option<String>,
    context_id: Option<Uuid>,
    target_entity_ids: Vec<Uuid>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(body) => {
            println!("{body}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.json());
            ExitCode::from(error.exit_code())
        }
    }
}

async fn run(cli: Cli) -> Result<String, CliError> {
    let server = cli
        .server
        .unwrap_or_else(|| Url::parse("http://127.0.0.1:3000").expect("valid default URL"));
    let mut client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT);
    if let Some(token) = cli.token {
        let mut headers = reqwest::header::HeaderMap::new();
        let value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                CliError::Input("CATALOG_TOKEN contains invalid header characters".to_owned())
            })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
        client = client.default_headers(headers);
    }
    let client = client
        .build()
        .map_err(|error| CliError::Transport(error.to_string()))?;

    match cli.command {
        Command::Health => request(&client, &server, Method::GET, "/health", None).await,
        Command::Blueprint { command } => match command {
            BlueprintCommand::List { include_drafts } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    if include_drafts {
                        "/blueprints?include_drafts=true"
                    } else {
                        "/blueprints"
                    },
                    None,
                )
                .await
            }
            BlueprintCommand::Create(source) => {
                let definition = read_source(source)?;
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/blueprints",
                    Some(json!({ "definition": definition })),
                )
                .await
            }
            BlueprintCommand::Revision {
                blueprint_id,
                source,
            } => {
                let definition = read_source(source)?;
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/blueprints/{}/versions", segment(blueprint_id)),
                    Some(json!({ "definition": definition })),
                )
                .await
            }
            BlueprintCommand::Publish {
                blueprint_id,
                version,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!(
                        "/blueprints/{}/versions/{version}/publish",
                        segment(blueprint_id)
                    ),
                    None,
                )
                .await
            }
            BlueprintCommand::Get { blueprint_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/blueprints/{}", segment(blueprint_id)),
                    None,
                )
                .await
            }
            BlueprintCommand::GetVersion {
                blueprint_id,
                version,
            } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/blueprints/{}/versions/{version}", segment(blueprint_id)),
                    None,
                )
                .await
            }
            BlueprintCommand::Resolve {
                code,
                version,
                include_drafts,
            } => {
                let path = match version {
                    Some(version) => {
                        format!("/blueprints/by-code/{}/versions/{version}", segment(&code))
                    }
                    None => format!(
                        "/blueprints/by-code/{}{}",
                        segment(&code),
                        if include_drafts {
                            "?include_drafts=true"
                        } else {
                            ""
                        }
                    ),
                };
                request(&client, &server, Method::GET, &path, None).await
            }
        },
        Command::Context { command } => match command {
            ContextCommand::List => request(&client, &server, Method::GET, "/contexts", None).await,
            ContextCommand::Create {
                file,
                code,
                data,
                parent_id,
            } => {
                let body = match (file, code, data, parent_id) {
                    (Some(file), None, None, None) => context_body_from_file(&file)?,
                    (None, Some(code), Some(data), Some(parent_id)) => json!({
                        "code": code,
                        "data": serde_json::from_str::<Value>(&data)
                            .map_err(|error| CliError::Input(format!("invalid --data JSON: {error}")))?,
                        "parent_id": parent_id,
                    }),
                    _ => {
                        return Err(CliError::Input(
                            "provide --file or --code, --data, and --parent-id".to_owned(),
                        ));
                    }
                };
                request(&client, &server, Method::POST, "/contexts", Some(body)).await
            }
            ContextCommand::Get { code } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/contexts/{}", segment(&code)),
                    None,
                )
                .await
            }
            ContextCommand::Update {
                context_id,
                parent_id,
                data,
            } => request(
                &client,
                &server,
                Method::PUT,
                &format!("/contexts/id/{}", segment(context_id)),
                Some(json!({
                    "parent_id": parent_id,
                    "data": serde_json::from_str::<Value>(&data)
                        .map_err(|error| CliError::Input(format!("invalid --data JSON: {error}")))?,
                })),
            )
            .await,
            ContextCommand::Delete { context_id } => {
                request(
                    &client,
                    &server,
                    Method::DELETE,
                    &format!("/contexts/id/{}", segment(context_id)),
                    None,
                )
                .await
            }
        },
        Command::Entity { command } => match command {
            EntityCommand::Create {
                blueprint,
                version,
                values,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/entities",
                    Some(json!({
                        "blueprint": { "code": blueprint, "version": version },
                        "values": values_body_from_file(&values, context_id)?["values"].clone(),
                    })),
                )
                .await
            }
            EntityCommand::Get { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/{}", segment(entity_id)),
                    None,
                )
                .await
            }
            EntityCommand::Delete { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::DELETE,
                    &format!("/entities/{}", segment(entity_id)),
                    None,
                )
                .await
            }
            EntityCommand::List {
                blueprint,
                related_from,
                relationship,
                limit,
                cursor,
            } => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                query.append_pair("blueprint", &blueprint);
                query.append_pair("related_from", &related_from.to_string());
                query.append_pair("relationship", &relationship);
                if let Some(limit) = limit {
                    query.append_pair("limit", &limit.to_string());
                }
                if let Some(cursor) = cursor {
                    query.append_pair("cursor", &cursor.to_string());
                }
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities?{}", query.finish()),
                    None,
                )
                .await
            }
            EntityCommand::Preview {
                entity_id,
                relationship_depth,
                relationship_limit,
            } => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                if let Some(depth) = relationship_depth {
                    query.append_pair("relationship_depth", &depth.to_string());
                }
                if let Some(limit) = relationship_limit {
                    query.append_pair("relationship_limit", &limit.to_string());
                }
                let query = query.finish();
                let path = format!(
                    "/entities/{}/preview{}",
                    segment(entity_id),
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    }
                );
                request(&client, &server, Method::GET, &path, None).await
            }
            EntityCommand::ResolvedPreview {
                entity_id,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!(
                        "/entities/{}/resolved-preview?context_id={}",
                        segment(entity_id),
                        segment(context_id)
                    ),
                    None,
                )
                .await
            }
            EntityCommand::Search {
                blueprint,
                version,
                query,
                size,
                cursor,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/entities/search",
                    Some(json!({
                        "blueprint": { "code": blueprint, "version": version },
                        "query": query,
                        "filters": [],
                        "page": { "size": size, "cursor": cursor },
                    })),
                )
                .await
            }
            EntityCommand::Form { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/v1/entities/{}", segment(entity_id)),
                    None,
                )
                .await
            }
            EntityCommand::Update {
                entity_id,
                values,
                relationships,
                remove_values,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::PUT,
                    &format!("/v1/entities/{}", segment(entity_id)),
                    Some(form_update_body(
                        values.as_ref(),
                        relationships.as_ref(),
                        remove_values.as_ref(),
                        context_id,
                    )?),
                )
                .await
            }
            EntityCommand::Migrate { entity_id } => {
                let result = migrate_entity(&client, &server, entity_id, false).await?;
                serde_json::to_string(&result).map_err(|_| CliError::InvalidResponse)
            }
            EntityCommand::MigrateBulk {
                blueprint,
                from_version,
                size,
                dry_run,
            } => migrate_entities(&client, &server, &blueprint, from_version, size, dry_run).await,
        },
        Command::Value { command } => match command {
            ValueCommand::Append {
                entity_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/entities/{}/values", segment(entity_id)),
                    Some(values_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Current { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/{}/values/current", segment(entity_id)),
                    None,
                )
                .await
            }
            ValueCommand::Replace {
                entity_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/entities/{}/relationships/replace", segment(entity_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Remove {
                entity_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/entities/{}/relationships/remove", segment(entity_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
        },
    }
}

async fn migrate_entity(
    client: &Client,
    server: &Url,
    entity_id: Uuid,
    dry_run: bool,
) -> Result<Value, CliError> {
    let preview = request_value(
        client,
        server,
        Method::POST,
        &format!(
            "/v1/entities/{}/blueprint-migration/preview",
            segment(entity_id)
        ),
        None,
    )
    .await?;
    let status = preview["status"]
        .as_str()
        .ok_or(CliError::InvalidResponse)?;
    if status != "ready" || dry_run {
        return Ok(json!({
            "entity_id": entity_id,
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
        &format!("/v1/entities/{}/blueprint-migration", segment(entity_id)),
        Some(json!({
            "migration_id": migration_id,
            "expected_target_version": target_version,
            "values": [],
            "relationships": [],
        })),
    )
    .await
}

async fn migrate_entities(
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
    let mut cursor = None;
    let mut migrated = 0;
    let mut ready = 0;
    let mut needs_input = Vec::new();
    let mut blocked = Vec::new();
    let mut failed = Vec::new();
    loop {
        let page = request_value(
            client,
            server,
            Method::POST,
            "/v1/entities/search",
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
            let entity_id = item["id"]
                .as_str()
                .ok_or(CliError::InvalidResponse)?
                .parse::<Uuid>()
                .map_err(|_| CliError::InvalidResponse)?;
            match migrate_entity(client, server, entity_id, dry_run).await {
                Ok(result) => match result["status"].as_str() {
                    Some("ready") => ready += 1,
                    Some("needs_input") => needs_input.push(result),
                    Some("blocked") => blocked.push(result),
                    _ => migrated += 1,
                },
                Err(error) => failed.push(json!({
                    "entity_id": entity_id,
                    "error": error.json()["error"],
                })),
            }
        }
        cursor = page["next_cursor"].as_str().map(ToOwned::to_owned);
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

fn read_source(input: SourceInput) -> Result<String, CliError> {
    match (input.file, input.stdin) {
        (Some(file), false) => {
            fs::read_to_string(file).map_err(|error| CliError::Input(error.to_string()))
        }
        (None, true) => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| CliError::Input(error.to_string()))?;
            Ok(source)
        }
        _ => Err(CliError::Input(
            "provide exactly one of --file or --stdin".to_owned(),
        )),
    }
}

fn context_body_from_file(path: &PathBuf) -> Result<Value, CliError> {
    let input: ContextFile = parse_toml_file(path)?;
    Ok(
        json!({ "code": input.code, "data": toml_to_json(input.data)?, "parent_id": input.parent_id }),
    )
}

fn values_body_from_file(path: &PathBuf, context_id: Option<Uuid>) -> Result<Value, CliError> {
    let input: ValueFile = parse_toml_file(path)?;
    values_body(input, context_id)
}

fn values_body(input: ValueFile, default_context_id: Option<Uuid>) -> Result<Value, CliError> {
    let values = input
        .values
        .into_iter()
        .map(|value| {
            let (attribute_key, attribute_value) = match (value.attribute_id, value.attribute_code)
            {
                (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                _ => {
                    return Err(CliError::Input(
                        "provide exactly one of attribute_id or attribute_code".to_owned(),
                    ));
                }
            };

            match value.kind.as_str() {
                "scalar" => {
                    if value.target_entity_id.is_some() {
                        return Err(CliError::Input(
                            "scalar values must not include target_entity_id".to_owned(),
                        ));
                    }
                    let payload = value
                        .value
                        .ok_or_else(|| CliError::Input("scalar values require value".to_owned()))?;
                    let mut output = json!({
                        "kind": "scalar",
                        "context_id": value.context_id.or(default_context_id),
                        "value": toml_to_json(payload)?,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                "relationship" => {
                    if value.value.is_some() {
                        return Err(CliError::Input(
                            "relationship values must not include value".to_owned(),
                        ));
                    }
                    let target_entity_id = value.target_entity_id.ok_or_else(|| {
                        CliError::Input("relationship values require target_entity_id".to_owned())
                    })?;
                    let mut output = json!({
                        "kind": "relationship",
                        "context_id": value.context_id.or(default_context_id),
                        "target_entity_id": target_entity_id,
                    });
                    output[attribute_key] = attribute_value;
                    Ok(output)
                }
                _ => Err(CliError::Input(
                    "value kind must be scalar or relationship".to_owned(),
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "values": values }))
}

fn relationships_body_from_file(
    path: &PathBuf,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RelationshipFile = parse_toml_file(path)?;
    relationships_body(input, context_id)
}

fn relationships_body(
    input: RelationshipFile,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let relationships = input
        .relationships
        .into_iter()
        .map(|relationship| {
            let (attribute_key, attribute_value) =
                match (relationship.attribute_id, relationship.attribute_code) {
                    (Some(attribute_id), None) => ("attribute_id", json!(attribute_id)),
                    (None, Some(attribute_code)) => ("attribute_code", json!(attribute_code)),
                    _ => {
                        return Err(CliError::Input(
                            "provide exactly one of attribute_id or attribute_code".to_owned(),
                        ));
                    }
                };
            let mut output = json!({
                "context_id": relationship.context_id.or(default_context_id),
                "target_entity_ids": relationship.target_entity_ids,
            });
            output[attribute_key] = attribute_value;
            Ok(output)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "relationships": relationships }))
}

fn form_update_body(
    values_path: Option<&PathBuf>,
    relationships_path: Option<&PathBuf>,
    remove_values_path: Option<&PathBuf>,
    context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let values = match values_path {
        Some(path) => values_body_from_file(path, context_id)?["values"].clone(),
        None => Value::Array(Vec::new()),
    };
    if values
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["kind"] == "relationship"))
    {
        return Err(CliError::Input(
            "--values accepts scalar values only; use --relationships for relationship sets"
                .to_owned(),
        ));
    }
    let relationships = match relationships_path {
        Some(path) => relationships_body_from_file(path, context_id)?["relationships"].clone(),
        None => Value::Array(Vec::new()),
    };
    let remove_values = match remove_values_path {
        Some(path) => remove_values_body_from_file(path, context_id)?,
        None => Value::Array(Vec::new()),
    };
    Ok(json!({ "values": values, "relationships": relationships, "remove_values": remove_values }))
}

fn remove_values_body_from_file(
    path: &PathBuf,
    default_context_id: Option<Uuid>,
) -> Result<Value, CliError> {
    let input: RemoveValuesFile = parse_toml_file(path)?;
    Ok(Value::Array(
        input
            .remove_values
            .into_iter()
            .map(|value| {
                json!({
                    "attribute_code": value.attribute_code,
                    "context_id": value.context_id.or(default_context_id),
                })
            })
            .collect(),
    ))
}

fn parse_toml_file<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    toml::from_str(&source).map_err(|error| CliError::Input(format!("invalid TOML: {error}")))
}

fn toml_to_json(value: toml::Value) -> Result<Value, CliError> {
    Ok(match value {
        toml::Value::String(value) => Value::String(value),
        toml::Value::Integer(value) => json!(value),
        toml::Value::Float(value) => json!(value),
        toml::Value::Boolean(value) => Value::Bool(value),
        // TOML's serde representation makes temporal literals objects. The API
        // accepts its canonical ISO 8601 text representation instead.
        toml::Value::Datetime(value) => Value::String(value.to_string()),
        toml::Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(toml_to_json)
                .collect::<Result<_, _>>()?,
        ),
        toml::Value::Table(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, toml_to_json(value)?)))
                .collect::<Result<_, CliError>>()?,
        ),
    })
}

async fn request(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<String, CliError> {
    let url = endpoint(server, path)?;
    let mut request = client.request(method, url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(CliError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(CliError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(body).map_err(|_| CliError::InvalidResponse)?;

    if status.is_success() {
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok("null".to_owned());
        }
        serde_json::from_str::<Value>(&body).map_err(|_| CliError::InvalidResponse)?;
        return Ok(body);
    }

    let error = serde_json::from_str::<Value>(&body).ok();
    let code = error
        .as_ref()
        .and_then(|body| body["error"]["code"].as_str())
        .unwrap_or("api_error")
        .to_owned();
    let message = error
        .as_ref()
        .and_then(|body| body["error"]["message"].as_str())
        .unwrap_or(&body)
        .to_owned();
    Err(CliError::Api {
        status: status.as_u16(),
        code,
        message,
    })
}

async fn request_value(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, CliError> {
    let response = request(client, server, method, path, body).await?;
    serde_json::from_str(&response).map_err(|_| CliError::InvalidResponse)
}

fn endpoint(server: &Url, path: &str) -> Result<Url, CliError> {
    let mut base = server.clone();
    let mut base_path = base.path().to_owned();
    if !base_path.ends_with('/') {
        base_path.push('/');
        base.set_path(&base_path);
    }
    base.join(path.trim_start_matches('/'))
        .map_err(|error| CliError::Input(format!("invalid API path: {error}")))
}

fn segment(value: impl std::fmt::Display) -> String {
    url::form_urlencoded::byte_serialize(value.to_string().as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_value_file_to_api_shape() {
        let source = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
value = "Blue shirt"
"#;
        let input: ValueFile = toml::from_str(source).unwrap();
        assert_eq!(
            values_body(input, None).unwrap(),
            json!({
                "values": [{
                    "kind": "scalar",
                    "attribute_id": "00000000-0000-0000-0000-000000000001",
                    "context_id": null,
                    "value": "Blue shirt"
                }]
            })
        );
    }

    #[test]
    fn converts_attribute_code_to_api_shape() {
        let source = r#"
[[values]]
kind = "relationship"
attribute_code = "related_products"
target_entity_id = "00000000-0000-0000-0000-000000000002"
"#;
        let input: ValueFile = toml::from_str(source).unwrap();
        assert_eq!(
            values_body(input, None).unwrap(),
            json!({
                "values": [{
                    "kind": "relationship",
                    "attribute_code": "related_products",
                    "context_id": null,
                    "target_entity_id": "00000000-0000-0000-0000-000000000002"
                }]
            })
        );
    }

    #[test]
    fn converts_toml_temporal_literals_to_iso_strings() {
        let input: ValueFile = toml::from_str(
            r#"
[[values]]
kind = "scalar"
attribute_code = "available_on"
value = 2026-08-12

[[values]]
kind = "scalar"
attribute_code = "released_at"
value = 2026-08-12T14:30:00Z
"#,
        )
        .unwrap();
        assert_eq!(
            values_body(input, None).unwrap()["values"],
            json!([
                { "kind": "scalar", "attribute_code": "available_on", "context_id": null, "value": "2026-08-12" },
                { "kind": "scalar", "attribute_code": "released_at", "context_id": null, "value": "2026-08-12T14:30:00Z" }
            ])
        );
    }

    #[test]
    fn converts_relationship_replacement_file_to_api_shape() {
        let source = r#"
[[relationships]]
attribute_code = "categories"
target_entity_ids = [
  "00000000-0000-0000-0000-000000000002",
  "00000000-0000-0000-0000-000000000003",
]
"#;
        let input: RelationshipFile = toml::from_str(source).unwrap();
        assert_eq!(
            relationships_body(input, None).unwrap(),
            json!({
                "relationships": [{
                    "attribute_code": "categories",
                    "context_id": null,
                    "target_entity_ids": [
                        "00000000-0000-0000-0000-000000000002",
                        "00000000-0000-0000-0000-000000000003"
                    ]
                }]
            })
        );
    }

    #[test]
    fn combines_entity_update_files_into_the_form_contract() {
        let values = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            values.path(),
            "[[values]]\nkind = \"scalar\"\nattribute_code = \"title\"\nvalue = \"Updated shirt\"\n",
        )
        .unwrap();
        let removals = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            removals.path(),
            "[[remove_values]]\nattribute_code = \"subtitle\"\n",
        )
        .unwrap();
        let context_id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();

        assert_eq!(
            form_update_body(
                Some(&values.path().to_path_buf()),
                None,
                Some(&removals.path().to_path_buf()),
                Some(context_id),
            )
            .unwrap(),
            json!({
                "values": [{
                    "kind": "scalar",
                    "attribute_code": "title",
                    "context_id": context_id,
                    "value": "Updated shirt"
                }],
                "relationships": [],
                "remove_values": [{ "attribute_code": "subtitle", "context_id": context_id }]
            })
        );
    }

    #[test]
    fn rejects_incompatible_value_fields_and_unknown_toml_keys() {
        let relationship_with_value = r#"
[[values]]
kind = "relationship"
attribute_id = "00000000-0000-0000-0000-000000000001"
target_entity_id = "00000000-0000-0000-0000-000000000002"
value = "ignored before this validation"
"#;
        let input: ValueFile = toml::from_str(relationship_with_value).unwrap();
        assert!(matches!(values_body(input, None), Err(CliError::Input(_))));

        let conflicting_selectors = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
attribute_code = "title"
value = "Blue shirt"
"#;
        let input: ValueFile = toml::from_str(conflicting_selectors).unwrap();
        assert!(matches!(values_body(input, None), Err(CliError::Input(_))));
    }

    #[test]
    fn preserves_base_path_and_encodes_dynamic_segments() {
        let server = Url::parse("https://example.test/catalog-api/").unwrap();
        let url = endpoint(&server, &format!("/contexts/{}", segment("en/GB?#"))).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.test/catalog-api/contexts/en%2FGB%3F%23"
        );
    }

    #[test]
    fn maps_api_errors_to_exit_code_four() {
        let error = CliError::Api {
            status: reqwest::StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
            code: "invalid_input".to_owned(),
            message: "bad value".to_owned(),
        };
        assert_eq!(error.exit_code(), 4);
        assert_eq!(error.json()["error"]["status"], 422);
    }

    #[tokio::test]
    async fn forwards_http_json_without_reformatting() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::Router::new().route(
                    "/health",
                    axum::routing::get(|| async { axum::Json(json!({ "status": "ok" })) }),
                ),
            )
            .await
            .unwrap()
        });

        let body = run(Cli {
            server: Some(Url::parse(&format!("http://{address}")).unwrap()),
            token: None,
            command: Command::Health,
        })
        .await
        .unwrap();
        assert_eq!(body, r#"{"status":"ok"}"#);

        server.abort();
    }
}
