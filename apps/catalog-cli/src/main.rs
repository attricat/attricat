use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, Parser, Subcommand};
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

#[derive(Parser)]
#[command(name = "catalog", about = "JSON-first client for the Catalog API")]
struct Cli {
    #[arg(long, env = "CATALOG_SERVER")]
    server: Option<Url>,
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
    Create(SourceInput),
    Revision {
        blueprint_id: String,
        #[command(flatten)]
        source: SourceInput,
    },
    Get {
        blueprint_id: String,
    },
    GetVersion {
        blueprint_id: String,
        version: i64,
    },
    Resolve {
        code: String,
        #[arg(long)]
        version: Option<i64>,
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
    Create {
        #[arg(long, conflicts_with_all = ["code", "data"])]
        file: Option<PathBuf>,
        #[arg(long, requires = "data")]
        code: Option<String>,
        #[arg(long, requires = "code")]
        data: Option<String>,
    },
    Get {
        code: String,
    },
}

#[derive(Subcommand)]
enum EntityCommand {
    Create {
        #[arg(long)]
        file: PathBuf,
    },
    Get {
        entity_id: String,
    },
    GetByCode {
        blueprint_id: String,
        code: String,
    },
    Preview {
        entity_id: String,
    },
}

#[derive(Subcommand)]
enum ValueCommand {
    Append {
        entity_id: String,
        #[arg(long)]
        file: PathBuf,
    },
    Current {
        entity_id: String,
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
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Input(_) => 2,
            Self::Transport(_) => 3,
            Self::Api { .. } => 4,
            Self::InvalidResponse => 5,
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
        }
    }
}

#[derive(Deserialize)]
struct ContextFile {
    code: String,
    data: toml::Value,
}

#[derive(Deserialize)]
struct EntityFile {
    code: String,
    blueprint_id: String,
    blueprint_version: i64,
    projections: Option<toml::Value>,
}

#[derive(Deserialize)]
struct ValueFile {
    values: Vec<ValueInput>,
}

#[derive(Deserialize)]
struct ValueInput {
    kind: String,
    attribute_id: String,
    context_id: Option<String>,
    value: Option<toml::Value>,
    target_entity_id: Option<String>,
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
    let client = Client::new();

    match cli.command {
        Command::Health => request(&client, &server, Method::GET, "/health", None).await,
        Command::Blueprint { command } => match command {
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
                    &format!("/blueprints/{blueprint_id}/versions"),
                    Some(json!({ "definition": definition })),
                )
                .await
            }
            BlueprintCommand::Get { blueprint_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/blueprints/{blueprint_id}"),
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
                    &format!("/blueprints/{blueprint_id}/versions/{version}"),
                    None,
                )
                .await
            }
            BlueprintCommand::Resolve { code, version } => {
                let path = match version {
                    Some(version) => format!("/blueprints/by-code/{code}/versions/{version}"),
                    None => format!("/blueprints/by-code/{code}"),
                };
                request(&client, &server, Method::GET, &path, None).await
            }
        },
        Command::Context { command } => match command {
            ContextCommand::Create { file, code, data } => {
                let body = match (file, code, data) {
                    (Some(file), None, None) => context_body_from_file(&file)?,
                    (None, Some(code), Some(data)) => json!({
                        "code": code,
                        "data": serde_json::from_str::<Value>(&data)
                            .map_err(|error| CliError::Input(format!("invalid --data JSON: {error}")))?,
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
                    &format!("/contexts/{code}"),
                    None,
                )
                .await
            }
        },
        Command::Entity { command } => match command {
            EntityCommand::Create { file } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    "/entities",
                    Some(entity_body_from_file(&file)?),
                )
                .await
            }
            EntityCommand::Get { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/{entity_id}"),
                    None,
                )
                .await
            }
            EntityCommand::GetByCode { blueprint_id, code } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/by-code/{blueprint_id}/{code}"),
                    None,
                )
                .await
            }
            EntityCommand::Preview { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/{entity_id}/projections/preview"),
                    None,
                )
                .await
            }
        },
        Command::Value { command } => match command {
            ValueCommand::Append { entity_id, file } => {
                request(
                    &client,
                    &server,
                    Method::POST,
                    &format!("/entities/{entity_id}/values"),
                    Some(values_body_from_file(&file)?),
                )
                .await
            }
            ValueCommand::Current { entity_id } => {
                request(
                    &client,
                    &server,
                    Method::GET,
                    &format!("/entities/{entity_id}/values/current"),
                    None,
                )
                .await
            }
        },
    }
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
    Ok(json!({ "code": input.code, "data": toml_to_json(input.data)? }))
}

fn entity_body_from_file(path: &PathBuf) -> Result<Value, CliError> {
    let input: EntityFile = parse_toml_file(path)?;
    let mut body = json!({
        "code": input.code,
        "blueprint_id": input.blueprint_id,
        "blueprint_version": input.blueprint_version,
    });
    if let Some(projections) = input.projections {
        body["projections"] = toml_to_json(projections)?;
    }
    Ok(body)
}

fn values_body_from_file(path: &PathBuf) -> Result<Value, CliError> {
    let input: ValueFile = parse_toml_file(path)?;
    let values = input
        .values
        .into_iter()
        .map(|value| match value.kind.as_str() {
            "scalar" => {
                let payload = value
                    .value
                    .ok_or_else(|| CliError::Input("scalar values require value".to_owned()))?;
                Ok(json!({
                    "kind": "scalar",
                    "attribute_id": value.attribute_id,
                    "context_id": value.context_id,
                "value": toml_to_json(payload)?,
                }))
            }
            "relationship" => {
                let target_entity_id = value.target_entity_id.ok_or_else(|| {
                    CliError::Input("relationship values require target_entity_id".to_owned())
                })?;
                Ok(json!({
                    "kind": "relationship",
                    "attribute_id": value.attribute_id,
                    "context_id": value.context_id,
                    "target_entity_id": target_entity_id,
                }))
            }
            _ => Err(CliError::Input(
                "value kind must be scalar or relationship".to_owned(),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "values": values }))
}

fn parse_toml_file<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, CliError> {
    let source = fs::read_to_string(path).map_err(|error| CliError::Input(error.to_string()))?;
    toml::from_str(&source).map_err(|error| CliError::Input(format!("invalid TOML: {error}")))
}

fn toml_to_json(value: toml::Value) -> Result<Value, CliError> {
    serde_json::to_value(value)
        .map_err(|error| CliError::Input(format!("could not convert TOML data: {error}")))
}

async fn request(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<String, CliError> {
    let url = server
        .join(path)
        .map_err(|error| CliError::Input(format!("invalid API path: {error}")))?;
    let mut request = client.request(method, url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;

    if status.is_success() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_value_file_to_api_shape() {
        let source = r#"
[[values]]
kind = "scalar"
attribute_id = "attribute"
value = "Blue shirt"
"#;
        let input: ValueFile = toml::from_str(source).unwrap();
        let path = PathBuf::from("unused");
        let _ = path;
        let values = input
            .values
            .into_iter()
            .map(|value| {
                json!({
                    "kind": value.kind,
                    "attribute_id": value.attribute_id,
                    "value": toml_to_json(value.value.unwrap()).unwrap(),
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![json!({ "kind": "scalar", "attribute_id": "attribute", "value": "Blue shirt" })]
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
            command: Command::Health,
        })
        .await
        .unwrap();
        assert_eq!(body, r#"{"status":"ok"}"#);

        server.abort();
    }
}
