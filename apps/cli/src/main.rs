use std::{io, process::ExitCode};

use clap::Parser;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;

use crate::cli::{Cli, Command, MetricsCommand};
use crate::commands::audit::audit_command;
use crate::commands::auth::auth_command;
use crate::commands::blueprint::BlueprintCommand;
use crate::commands::context::ContextCommand;
use crate::commands::data_health::data_health_command;
use crate::commands::event::EventCommand;
use crate::commands::extension::{
    connector_job_command, extension_command, extension_operation_command,
    extension_registry_command, extension_run_command, extension_schedule_command,
};
use crate::commands::file::file_command;
use crate::commands::lexicon::lexicon_request;
use crate::commands::notification::notification_request;
use crate::commands::presentation_asset::presentation_asset_command;
use crate::commands::record::{
    RecordCommand, migrate_record, migrate_records, record_publication_command,
};
use crate::commands::rule::rule_command;
use crate::commands::saved_view::{SAVED_VIEW_KIND, SavedViewCommand, changed_saved_view};
use crate::commands::solution_pack::solution_pack_command;
use crate::commands::team::team_request;
use crate::commands::token::token_command;
use crate::commands::value::ValueCommand;
use crate::commands::workflow::workflow_command;
use crate::commands::workspace::workspace_command;
use crate::error::CliError;
use crate::input::{
    context_body_from_file, form_update_body, json_array_argument, json_array_input, json_input,
    json_object_input, json_string_array_input, read_secret_stdin, read_source,
    relationships_body_from_file, values_body_from_file,
};
use crate::session::{CSRF_COOKIE, CSRF_HEADER, SESSION_COOKIE, SessionFile};
use crate::transport::{
    CONNECT_TIMEOUT, is_secure_credential_transport, raw_download, request, request_value, segment,
};

mod cli;
mod commands;
mod error;
mod input;
mod session;
#[cfg(test)]
mod tests;
mod transport;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = load_dotenv() {
        eprintln!("{}", error.json());
        return ExitCode::from(error.exit_code());
    }
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => {
            let error = CliError::Input(error.to_string());
            eprintln!("{}", error.json());
            return ExitCode::from(error.exit_code());
        }
        Err(error) => {
            print!("{error}");
            return ExitCode::SUCCESS;
        }
    };
    match run(cli).await {
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

fn load_dotenv() -> Result<(), CliError> {
    if std::env::args_os().any(|argument| argument == "--no-env") {
        return Ok(());
    }
    match dotenvy::from_filename(".env") {
        Ok(_) => Ok(()),
        Err(dotenvy::Error::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::Input(format!("cannot load .env: {error}"))),
    }
}

async fn run(cli: Cli) -> Result<String, CliError> {
    let server = cli.server;
    let session_file = cli.session_file;
    // A personal token is the explicit automation credential. Do not even
    // read a saved browser session when one is supplied: it takes precedence
    // and avoids accidentally combining two identities on a request.
    let token = if cli.token_stdin {
        Some(read_secret_stdin("personal API token")?)
    } else {
        cli.token
    };
    let session = if token.is_some() {
        None
    } else {
        SessionFile::load(session_file.as_ref())?
    };
    if (token.is_some() || session.is_some()) && !is_secure_credential_transport(&server) {
        return Err(CliError::Input(
            "refusing to send credentials over insecure HTTP to a non-loopback host".to_owned(),
        ));
    }
    let mut client = Client::builder().connect_timeout(CONNECT_TIMEOUT);
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(token) = token {
        let value =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                CliError::Input("ATTRICAT_TOKEN contains invalid header characters".to_owned())
            })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    if let Some(session) = &session {
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={}; {CSRF_COOKIE}={}",
                session.session, session.csrf
            ))
            .map_err(|_| {
                CliError::Input("session file contains invalid cookie characters".to_owned())
            })?,
        );
        headers.insert(
            reqwest::header::HeaderName::from_static(CSRF_HEADER),
            reqwest::header::HeaderValue::from_str(&session.csrf).map_err(|_| {
                CliError::Input("session file contains invalid CSRF characters".to_owned())
            })?,
        );
    }
    if !headers.is_empty() {
        client = client.default_headers(headers);
    }
    let client = client
        .build()
        .map_err(|error| CliError::Transport(error.to_string()))?;

    match cli.command {
        Command::Health => request(&client, &server, Method::GET, "/health", None).await,
        Command::Directory { user: None } => {
            request(&client, &server, Method::GET, "/directory", None).await
        }
        Command::Directory { user: Some(id) } => {
            request(&client, &server, Method::GET, &format!("/directory/users/{}", segment(id)), None).await
        }
        Command::Notification { command } => {
            let (method, path, payload) = notification_request(command);
            request(&client, &server, method, &path, payload).await
        }
        Command::Team { command } => {
            let (method, path, payload) = team_request(command);
            request(&client, &server, method, &path, payload).await
        }
        Command::Lexicon { command } => {
            let (method, path, payload) = lexicon_request(command)?;
            request(&client, &server, method, &path, payload).await
        }
        Command::SavedView { command } => {
            let (method, path, payload, url_key) = match command {
                SavedViewCommand::List { query } => {
                    let path = match query {
                        Some(query) => {
                            let mut params = url::form_urlencoded::Serializer::new(String::new());
                            params.append_pair("q", &query);
                            format!("/saved-views?{}", params.finish())
                        }
                        None => "/saved-views".to_owned(),
                    };
                    (Method::GET, path, None, None)
                }
                SavedViewCommand::Get { id } => {
                    (Method::GET, format!("/saved-views/{}", segment(id)), None, None)
                }
                SavedViewCommand::Create {
                    name,
                    state,
                    visibility,
                    description,
                } => (
                    Method::POST,
                    "/saved-views".to_owned(),
                    Some(json!({
                        "kind": SAVED_VIEW_KIND,
                        "name": name,
                        "description": description,
                        "visibility": visibility.as_str(),
                        "state": json_object_input(&state, "--state")?,
                    })),
                    Some("savedView"),
                ),
                SavedViewCommand::Update {
                    id,
                    name,
                    state,
                    visibility,
                    description,
                } => {
                    let path = format!("/saved-views/{}", segment(id));
                    let current = request_value(&client, &server, Method::GET, &path, None).await?;
                    let state = state
                        .map(|state| json_object_input(&state, "--state"))
                        .transpose()?;
                    (
                        Method::PUT,
                        path,
                        Some(changed_saved_view(&current, name, description, visibility, state)?),
                        Some("savedView"),
                    )
                }
                SavedViewCommand::Delete { id } => {
                    (Method::DELETE, format!("/saved-views/{}", segment(id)), None, None)
                }
                SavedViewCommand::Link { state } => (
                    Method::POST,
                    "/view-state-links".to_owned(),
                    Some(json!({"kind": SAVED_VIEW_KIND, "state": json_object_input(&state, "--state")?})),
                    Some("viewState"),
                ),
            };
            let body = request(&client, &server, method, &path, payload).await?;
            if let Some(key) = url_key {
                let mut output: Value =
                    serde_json::from_str(&body).map_err(|_| CliError::InvalidResponse)?;
                if let Some(id) = output.get("id").and_then(Value::as_str).map(str::to_owned)
                    && let Ok(web) = std::env::var("ATTRICAT_WEB_URL") {
                        let mut url = Url::parse(&web).map_err(|error| {
                            CliError::Input(format!("invalid ATTRICAT_WEB_URL: {error}"))
                        })?;
                        url.set_path("/");
                        url.set_query(Some(&format!("{key}={id}")));
                        output["url"] = json!(url.to_string());
                    }
                Ok(output.to_string())
            } else {
                Ok(body)
            }
        },
        Command::Auth { command } => auth_command(&client, &server, command, session_file.as_ref()).await,
        Command::Event { command } => match command {
            EventCommand::DeadLetters => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    "/event-deliveries/dead-letters",
                    None,
                )
                .await
            }
            EventCommand::Replay {
                consumer_id,
                event_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!(
                        "/event-deliveries/{}/{}/replay",
                        segment(consumer_id),
                        segment(event_id)
                    ),
                    None,
                )
                .await
            }
        },
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
            BlueprintCommand::Validate {
                blueprint_id,
                source,
            } => {
                let definition = read_source(source)?;
                let path = match blueprint_id {
                    Some(blueprint_id) => {
                        format!("/blueprints/{}/versions/validate", segment(blueprint_id))
                    }
                    None => "/blueprints/validate".to_owned(),
                };
                request(
                    &client,
                    &server,
                    Method::POST,
                    &path,
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
            BlueprintCommand::Catalogue => request(&client, &server, Method::GET, "/blueprints/catalogue", None).await,
            BlueprintCommand::RevisionList { blueprint_id } => request(&client, &server, Method::GET, &format!("/blueprints/{}/versions", segment(blueprint_id)), None).await,
            BlueprintCommand::PublishRecords { blueprint_id, version, context_id } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/record-publications", segment(blueprint_id)), Some(json!({"context_id": context_id}))).await,
            BlueprintCommand::PublishRecordsAll { blueprint_id, version } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/record-publications/publish-all", segment(blueprint_id)), None).await,
            BlueprintCommand::SafeMigrationBatch { blueprint_id, version } => request(&client, &server, Method::POST, &format!("/blueprints/{}/versions/{version}/safe-migration-batches", segment(blueprint_id)), None).await,
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
                    (None, Some(code), Some(data), parent_id) => json!({
                        "code": code,
                        "data": json_input(&data, "--data")?,
                        "parent_id": parent_id,
                    }),
                    _ => {
                        return Err(CliError::Input(
                            "provide --file or both --code and --data".to_owned(),
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
                    "data": json_input(&data, "--data")?,
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
        Command::Record { command } => match command {
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
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_string_array_input(&tags, "--system-tags")?;
                }
                if let Some(metadata) = system_metadata {
                    body["system_metadata"] = json_object_input(&metadata, "--system-metadata")?;
                }
                request(&client, &server, Method::POST, "/v1/records", Some(body)).await
            }
            RecordCommand::Get { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/records/{}", segment(record_id)),
                    None,
                )
                .await
            }
            RecordCommand::Delete { record_id } => {
                request(
                    &client,
                    &server,
                    Method::DELETE,
                    &format!("/records/{}", segment(record_id)),
                    None,
                )
                .await
            }
            RecordCommand::Batch { operations } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/records/batch",
                    Some(json!({ "operations": json_array_input(&operations, "--operations")? })),
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
                    &format!("/records?{}", query.finish()),
                    None,
                )
                .await
            }
            RecordCommand::Preview {
                record_id,
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
                    "/records/{}/preview{}",
                    segment(record_id),
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    }
                );
                request(&client, &server, Method::GET, &path, None).await
            }
            RecordCommand::ResolvedPreview {
                record_id,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!(
                        "/records/{}/resolved-preview?context_id={}",
                        segment(record_id),
                        segment(context_id)
                    ),
                    None,
                )
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
                    "relationship_tree_facets": json_array_argument(relationship_tree_facets.as_deref(), "--relationship-tree-facets")?,
                    "page": { "size": size, "cursor": cursor },
                });
                if let Some(field) = sort_field {
                    body["sort"] = json!({ "field": field, "direction": sort_direction.unwrap_or_else(|| "asc".to_owned()) });
                }
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_string_array_input(&tags, "--system-tags")?;
                }
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/v1/records/search",
                    Some(body),
                )
                .await
            }
            RecordCommand::Form { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/v1/records/{}", segment(record_id)),
                    None,
                )
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
                if let Some(tags) = system_tags {
                    body["system_tags"] = json_string_array_input(&tags, "--system-tags")?;
                }
                if let Some(metadata) = system_metadata {
                    body["system_metadata"] = json_object_input(&metadata, "--system-metadata")?;
                }
                request(
                    &client,
                    &server,
                    Method::PUT,
                    &format!("/v1/records/{}", segment(record_id)),
                    Some(body),
                )
                .await
            }
            RecordCommand::Hierarchy { record_id, context_id, field } => request(&client, &server, Method::GET, &format!("/records/{}/hierarchy?context_id={}&field={}", segment(record_id), segment(context_id), segment(&field)), None).await,
            RecordCommand::IncomingRelationships { record_id, relationships, size, cursor } => request(&client, &server, Method::POST, &format!("/v1/records/{}/incoming-relationships", segment(record_id)), Some(json!({"relationships": json_array_input(&relationships, "--relationships")?, "page": {"size": size, "cursor": cursor}}))).await,
            RecordCommand::FacetChildren { blueprint, version, query, source_relationship_field, hierarchy_field, context_id, parent_id, cursor, selected_target_ids, include_ids } => request(&client, &server, Method::POST, "/v1/records/facets/relationship-tree/children", Some(json!({"blueprint":{"code": blueprint, "version":version}, "query":query, "source_relationship_field":source_relationship_field, "hierarchy_field":hierarchy_field, "context_id":context_id, "parent_id":parent_id, "cursor":cursor, "selected_target_ids":json_array_argument(selected_target_ids.as_deref(), "--selected-target-ids")?, "include_ids":json_array_argument(include_ids.as_deref(), "--include-ids")?}))).await,
            RecordCommand::Publication { command } => record_publication_command(&client, &server, command).await,
            RecordCommand::Changes { record_id } => request(&client, &server, Method::GET, &format!("/records/{}/changes", segment(record_id)), None).await,
            RecordCommand::ValueHistory { record_id } => request(&client, &server, Method::GET, &format!("/records/{}/values/history", segment(record_id)), None).await,
            RecordCommand::RestoreValue { record_id, history_id } => request(&client, &server, Method::POST, &format!("/records/{}/values/history/{}/restore", segment(record_id), segment(history_id)), None).await,
            RecordCommand::Migrate { record_id, values, relationships, discard_attributes } => {
                let result = migrate_record(&client, &server, record_id, false, values.as_deref(), relationships.as_deref(), discard_attributes.as_deref()).await?;
                serde_json::to_string(&result).map_err(|_| CliError::InvalidResponse)
            }
            RecordCommand::MigrateBulk {
                blueprint,
                from_version,
                size,
                dry_run,
            } => migrate_records(&client, &server, &blueprint, from_version, size, dry_run).await,
        },
        Command::Workspace { command } => workspace_command(&client, &server, command).await,
        Command::Token { command } => token_command(&client, &server, command).await,
        Command::Audit { command } => audit_command(&client, &server, command).await,
        Command::DataHealth { command } => data_health_command(&client, &server, command).await,
        Command::Workflow { command } => workflow_command(&client, &server, command).await,
        Command::Rule { command } => rule_command(&client, &server, command).await,
        Command::ExtensionRegistry { command } => extension_registry_command(&client, &server, command).await,
        Command::Extension { command } => extension_command(&client, &server, command).await,
        Command::ExtensionOperation { command } => extension_operation_command(&client, &server, command).await,
        Command::ExtensionRun { command } => extension_run_command(&client, &server, command).await,
        Command::ExtensionSchedule { command } => extension_schedule_command(&client, &server, command).await,
        Command::ConnectorJob { command } => connector_job_command(&client, &server, command).await,
        Command::SolutionPack { command } => {
            solution_pack_command(&client, &server, command).await
        }
        Command::PresentationAsset { command } => {
            presentation_asset_command(&client, &server, command).await
        }
        Command::File { command } => file_command(&client, &server, command).await,
        Command::Metrics { command: MetricsCommand::Get { output } } => raw_download(&client, &server, "/metrics", &output, None).await,
        Command::Value { command } => match command {
            ValueCommand::Append {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/values", segment(record_id)),
                    Some(values_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Current { record_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/records/{}/values/current", segment(record_id)),
                    None,
                )
                .await
            }
            ValueCommand::Replace {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/relationships/replace", segment(record_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
            ValueCommand::Remove {
                record_id,
                file,
                context_id,
            } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/records/{}/relationships/remove", segment(record_id)),
                    Some(relationships_body_from_file(&file, context_id)?),
                )
                .await
            }
        },
    }
}
