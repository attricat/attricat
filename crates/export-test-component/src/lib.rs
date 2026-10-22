//! Fixture for the `attricat:host@1.1.0` export platform. It targets the
//! `outcome-extension` world and exercises typed handler outcomes, the
//! mapping store with revision guards, connector page v2, the withdrawn
//! feed, annotation writes in scoped runs, deferred batches and the clock.
//! It "pushes" nothing: a successful push is a mapping committed as synced.
//!
//! Record titles steer the event handler: `reject…` is a record-level
//! rejection, `retry…` fails temporarily twice, `fault…` is a component
//! fault. The operation input's `fail` field steers its first batch the
//! same way.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "outcome-extension",
});

use attricat::host::{
    api, artifacts, attricat as catalog, attricat_data, clock, outcomes, transfer,
};
use exports::attricat::host::{
    command_handler::{self, CommandRequest, CommandResponse},
    deferrable_operations::{self, BatchOutcome, OperationRequest},
    event_handler::{self, Delivery},
};
use serde_json::{Value, json};

struct Component;

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

fn call(operation: &str, request: Value) -> Result<Value, String> {
    parse(&api::call(operation, &request.to_string())?)
}

fn fault(message: impl Into<String>) -> outcomes::Failure {
    outcomes::Failure::Fault(message.into())
}

/// A record's title and content revision, read through the typed API.
fn read_title(record_id: &str) -> Result<(String, i64), String> {
    let response = api::read(&api::ReadRequest::Record(api::RecordReference {
        record_id: record_id.into(),
    }))?;
    let record = parse(&response.record)?;
    let blueprint = parse(&response.blueprint)?;
    let values = parse(&response.direct_values)?;
    let title_id = blueprint["attributes"]
        .as_array()
        .and_then(|attributes| {
            attributes
                .iter()
                .find(|attribute| attribute["code"] == "title")
        })
        .and_then(|attribute| attribute["id"].as_str())
        .unwrap_or_default()
        .to_owned();
    let title = values
        .as_array()
        .and_then(|values| {
            values
                .iter()
                .find(|value| value["attribute_id"].as_str() == Some(title_id.as_str()))
        })
        .and_then(|value| value["value"].as_str())
        .unwrap_or_default()
        .to_owned();
    Ok((title, record["content_revision"].as_i64().unwrap_or(0)))
}

/// A tiny stable digest, so tests can compare stored content hashes.
fn content_hash(value: &Value) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Pushes one record version unless the destination already has it.
fn push(
    connection: &Value,
    record_id: &str,
    revision: i64,
    content: &Value,
) -> Result<&'static str, String> {
    let begin = call(
        "mappings.begin.v1",
        json!({"connection": connection, "record_id": record_id, "record_revision": revision}),
    )?;
    match begin["outcome"].as_str() {
        Some("proceed") => {}
        Some("stale") => return Ok("stale"),
        _ => return Ok("busy"),
    }
    let hash = content_hash(content);
    // A resync skips records whose content did not change.
    if begin["mapping"]["content_hash"].as_str() == Some(hash.as_str()) {
        call(
            "mappings.commit.v1",
            json!({"connection": connection, "record_id": record_id,
                   "lease_token": begin["lease_token"], "status": "synced",
                   "record_revision": revision}),
        )?;
        return Ok("unchanged");
    }
    call(
        "mappings.commit.v1",
        json!({
            "connection": connection,
            "record_id": record_id,
            "lease_token": begin["lease_token"],
            "status": "synced",
            "record_revision": revision,
            "external_id": format!("ext-{record_id}"),
            "external_ids": {"listing": format!("listing-{record_id}")},
            "content_hash": hash,
        }),
    )?;
    Ok("pushed")
}

/// Runs the transfer steps of a test operation and reports each result.
fn run_transfer(steps: &Value, batch_key: &str) -> Result<Value, String> {
    let mut results = json!({});
    let report = |result: Result<String, String>| match result {
        Ok(value) => parse(&value).unwrap_or(json!(value)),
        Err(error) => json!({"error": error}),
    };
    if let Some(upload) = steps.get("output") {
        let bytes = upload["text"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec();
        artifacts::append_output(
            "upload",
            upload["media_type"].as_str().unwrap_or("text/plain"),
            batch_key,
            &bytes,
        )?;
        let artifact_id = artifacts::finalize_output("upload")?;
        results["artifact_id"] = json!(artifact_id);
        for name in ["deliver", "multipart", "deliver_v1"] {
            if let Some(template) = steps.get(name) {
                let mut request = template.clone();
                request["artifact_id"] = json!(artifact_id);
                results[name] = report(if name == "deliver_v1" {
                    transfer::deliver_output(&request.to_string())
                } else {
                    transfer::deliver_output_v2(&request.to_string())
                });
            }
        }
    }
    for (name, call) in [
        (
            "fetch",
            transfer::fetch_input_v2 as fn(&str) -> Result<String, String>,
        ),
        ("record_file", transfer::open_record_file),
    ] {
        let Some(request) = steps.get(name) else {
            continue;
        };
        let opened = report(call(&request.to_string()));
        if let Some(id) = opened["artifact_id"].as_str() {
            let handle = artifacts::open_input(id)?;
            let metadata = artifacts::describe_input(&handle)?;
            let bytes = artifacts::read(&handle, 65536)?;
            results[name] = json!({
                "result": opened,
                "length": metadata.content_length,
                "media_type": metadata.media_type,
                "text": String::from_utf8_lossy(&bytes),
            });
        } else {
            results[name] = opened;
        }
    }
    Ok(results)
}

impl event_handler::Guest for Component {
    fn handle_event(event: api::Event, delivery: Delivery) -> Result<(), outcomes::Failure> {
        let payload = parse(&event.payload).map_err(fault)?;
        let connection = payload.get("connection").cloned().unwrap_or(Value::Null);
        let record_id = event.aggregate_id.clone();
        let (title, revision) = read_title(&record_id).map_err(fault)?;
        if title.starts_with("reject") {
            return Err(outcomes::Failure::Rejected(outcomes::Rejection {
                code: "invalid_title".into(),
                message: format!("the destination rejects '{title}'"),
                record_id: Some(record_id),
                connection: None,
            }));
        }
        if title.starts_with("retry") && delivery.attempt < 3 {
            return Err(outcomes::Failure::Retry(outcomes::Retry {
                message: "destination is busy".into(),
                after_ms: Some(1),
            }));
        }
        if title.starts_with("fault") {
            return Err(fault("the component broke"));
        }
        // An event older than the record never wins over its current state.
        let revision = revision.max(delivery.record_revision.unwrap_or(0));
        push(&connection, &record_id, revision, &json!({"title": title})).map_err(fault)?;
        api::call(
            "storage.set.v1",
            &json!({"key": format!("delivered:{}", event.id), "value": {
                "attempt": delivery.attempt,
                "sequence": delivery.sequence,
                "record_revision": delivery.record_revision,
            }})
            .to_string(),
        )
        .map_err(fault)?;
        Ok(())
    }
}

impl command_handler::Guest for Component {
    fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
        let payload = parse(&request.payload)?;
        let response = match request.handler.as_str() {
            "preview" => {
                let record_id = payload["record_id"].as_str().unwrap_or_default();
                let (title, revision) = read_title(record_id)?;
                let issues: Vec<&str> = if title.is_empty() {
                    vec!["title is required by the destination"]
                } else {
                    vec![]
                };
                json!({"payload": {"title": title, "revision": revision}, "issues": issues})
            }
            "clock" => {
                let now = clock::now();
                json!({"seconds": now.seconds, "nanoseconds": now.nanoseconds})
            }
            // Passes one host call through, so tests can drive host APIs
            // from inside a component.
            "call" => {
                let operation = payload["operation"].as_str().unwrap_or_default();
                match api::call(operation, &payload["request"].to_string()) {
                    Ok(result) => json!({"ok": parse(&result)?}),
                    Err(error) => json!({"error": error}),
                }
            }
            _ => return Err("unknown command".into()),
        };
        Ok(CommandResponse {
            payload: response.to_string(),
        })
    }
}

impl deferrable_operations::Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("prepared".into())
    }

    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("started".into())
    }

    fn process_batch(request: OperationRequest) -> Result<BatchOutcome, outcomes::Failure> {
        let mut checkpoint = parse(&request.checkpoint).map_err(fault)?;
        let input = parse(&request.input).map_err(fault)?;
        let scope = call("connector.scope.v1", json!({})).map_err(fault)?;
        let connection = scope["connection"].clone();
        // The first batch defers itself once, as a destination poll would.
        if checkpoint["deferred"].is_null() {
            checkpoint["deferred"] = json!(true);
            checkpoint["started_at"] = json!(clock::now().seconds);
            return Ok(BatchOutcome {
                checkpoint: checkpoint.to_string(),
                progress: json!({"completed": 0}).to_string(),
                done: false,
                resume_after_ms: Some(1),
            });
        }
        if let Some(steps) = input.get("transfer") {
            checkpoint["transfer"] = run_transfer(steps, &request.batch_key).map_err(fault)?;
            return Ok(BatchOutcome {
                checkpoint: checkpoint.to_string(),
                progress: json!({"completed": 1}).to_string(),
                done: true,
                resume_after_ms: None,
            });
        }
        match input["fail"].as_str() {
            Some("retry") if checkpoint["retried"].is_null() => {
                // Checkpoints are not committed on failure; remember the
                // retry in storage instead.
                let key = format!("retried:{}", request.run_id);
                let seen = call("storage.get.v1", json!({"key": key})).map_err(fault)?;
                if seen.is_null() {
                    call("storage.set.v1", json!({"key": key, "value": true})).map_err(fault)?;
                    return Err(outcomes::Failure::Retry(outcomes::Retry {
                        message: "destination throttled".into(),
                        after_ms: Some(1),
                    }));
                }
                checkpoint["retried"] = json!(true);
            }
            Some("reject") => {
                return Err(outcomes::Failure::Rejected(outcomes::Rejection {
                    code: "feed_rejected".into(),
                    message: "the destination rejected the feed".into(),
                    record_id: None,
                    connection: None,
                }));
            }
            Some("fault") => return Err(fault("operation broke")),
            _ => {}
        }
        if checkpoint["phase"].as_str() != Some("withdrawn") {
            let cursor = checkpoint["cursor"].as_str().unwrap_or_default().to_owned();
            let page = parse(
                &catalog::page_v2(
                    &json!({"cursor": cursor, "limit": 16, "expand": input["expand"].as_array().cloned().unwrap_or_default()})
                        .to_string(),
                )
                .map_err(fault)?,
            )
            .map_err(fault)?;
            let mut annotations = Vec::new();
            for row in page["rows"].as_array().cloned().unwrap_or_default() {
                let record_id = row["record_id"].as_str().unwrap_or_default().to_owned();
                let revision = row["content_revision"].as_i64().unwrap_or(0);
                let content = json!({"values": row["values"], "related": row["related"], "files": row["files"]});
                let outcome = push(&connection, &record_id, revision, &content).map_err(fault)?;
                let mut rows = checkpoint["rows"].as_array().cloned().unwrap_or_default();
                rows.push(json!({"record_id": record_id, "outcome": outcome, "row": row}));
                checkpoint["rows"] = Value::Array(rows);
                annotations.push(json!({
                    "kind": "annotate",
                    "intent_key": format!("{}:{record_id}", request.batch_key),
                    "record_id": record_id,
                    "add_tags": ["exported"],
                    "set_metadata": {"export": {"revision": revision}},
                }));
            }
            if !annotations.is_empty() {
                attricat_data::batch(
                    &json!({"operation": "batch", "batch": {
                        "batch_key": request.batch_key,
                        "dry_run": false,
                        "intents": annotations,
                    }})
                    .to_string(),
                )
                .map_err(fault)?;
            }
            match page["next_cursor"].as_str() {
                Some(next) => checkpoint["cursor"] = json!(next),
                None => checkpoint["phase"] = json!("withdrawn"),
            }
            return Ok(BatchOutcome {
                progress: json!({"completed": checkpoint["rows"].as_array().map_or(0, Vec::len)})
                    .to_string(),
                checkpoint: checkpoint.to_string(),
                done: false,
                resume_after_ms: None,
            });
        }
        let cursor = checkpoint["withdrawn_cursor"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let withdrawn = parse(&catalog::withdrawn(&cursor, 100).map_err(fault)?).map_err(fault)?;
        for item in withdrawn["records"].as_array().cloned().unwrap_or_default() {
            let record_id = item["record_id"].as_str().unwrap_or_default();
            call(
                "mappings.commit.v1",
                json!({"connection": connection, "record_id": record_id, "status": "withdrawn",
                       "record_revision": item["record_revision"]}),
            )
            .map_err(fault)?;
            let mut closed = checkpoint["withdrawn"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            closed.push(item.clone());
            checkpoint["withdrawn"] = Value::Array(closed);
        }
        checkpoint["withdrawn_cursor"] = withdrawn["next_cursor"].clone();
        let done = withdrawn["more"] != json!(true);
        Ok(BatchOutcome {
            progress: json!({"completed": checkpoint["rows"].as_array().map_or(0, Vec::len)})
                .to_string(),
            checkpoint: checkpoint.to_string(),
            done,
            resume_after_ms: None,
        })
    }

    fn checkpoint(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }

    fn finish(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }

    fn cancel(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }
}

export!(Component);
