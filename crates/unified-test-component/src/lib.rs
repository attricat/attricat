//! Fixture for the unified `attricat:host@1.6.0` ABI. One component exports
//! both the event/command `handler` and the checkpointed `operations`, and
//! probes the boundary between them: run-bound interfaces fail outside a run,
//! and direct `api` catalog access fails inside one.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "attricat-extension",
});

use attricat::host::{api, attricat_data, clock, selection};
use exports::attricat::host::{
    handler::{self, CommandRequest, CommandResponse},
    operations::{self, BatchResult, OperationRequest},
};
use serde_json::{Value, json};

struct Component;

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

/// Records this call under its own storage key, so a test can count calls;
/// a test that holds the storage table locked makes the call as slow as it
/// likes without spending the call's fuel.
fn record_field_notes_call() -> Result<(), String> {
    let now = clock::now();
    api::call(
        "storage.set.v1",
        &json!({"key": format!("call-{}-{}", now.seconds, now.nanoseconds), "value": true})
            .to_string(),
    )?;
    Ok(())
}

impl handler::Guest for Component {
    fn handle_event(_event: api::Event) -> Result<(), String> {
        Ok(())
    }

    fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
        let payload = parse(&request.payload)?;
        // Field notes for the host: some belong to the record and some must
        // be dropped (an unknown attribute and file, an undeclared action).
        let record_ids: Vec<String> = payload["records"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|record| record["record_id"].as_str().map(str::to_owned))
            .collect();
        let notes = match request.handler.as_str() {
            "field-notes-counted" => {
                record_field_notes_call()?;
                Some(json!({
                    "records": record_ids.iter().map(|record_id| (record_id.clone(), json!({
                        "attributes": {"title": {"note": "Counted"}}
                    }))).collect::<serde_json::Map<_, _>>()
                }))
            }
            "field-notes" => Some(json!({
                "records": record_ids.iter().map(|record_id| (record_id.clone(), json!({
                    "attributes": {
                        "title": {
                            "badge": {"label": "Checked", "tone": "success", "icon": "check", "tooltip": "From the fixture", "action": "explain"},
                            "note": format!("Checked record {record_id} in {}", payload["locale"].as_str().unwrap_or_default())
                        },
                        "sku": {"badge": {"label": "SKU", "action": "undeclared"}},
                        "default:depth": {"note": "Depth is attached"},
                        "not_an_attribute": {"note": "dropped"}
                    },
                    "files": {"00000000-0000-4000-8000-000000000000": {"note": "dropped"}},
                    "actions": {"explain": {"disabled": "Nothing to explain"}, "undeclared": {"hidden": true}}
                }))).collect::<serde_json::Map<_, _>>(),
                "cacheable": true
            })),
            "field-notes-invalid" => Some(json!({
                "records": {"x": {"attributes": {"title": {"badge": {"label": "x".repeat(30)}}}}}
            })),
            _ => None,
        };
        if let Some(notes) = notes {
            return Ok(CommandResponse {
                payload: notes.to_string(),
            });
        }
        let response = json!({
            "handler": request.handler,
            "echo": payload,
            "selection_error": selection::describe().err(),
            "attricat_data_error": attricat_data::read("{}").err(),
        });
        Ok(CommandResponse {
            payload: response.to_string(),
        })
    }
}

impl operations::Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("prepared".into())
    }

    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("started".into())
    }

    fn process_batch(request: OperationRequest) -> Result<BatchResult, String> {
        let mut checkpoint = parse(&request.checkpoint)?;
        let cursor = checkpoint["cursor"].as_str().unwrap_or_default().to_owned();
        let described = parse(&selection::describe()?)?;
        let page = parse(&selection::page(&cursor, 1)?)?;
        let record_id = page["records"][0]["record_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let batch = json!({"operation": "batch", "batch": {
            "batch_key": request.batch_key,
            "dry_run": false,
            "intents": [{
                "kind": "annotate",
                "intent_key": format!("unified-{record_id}"),
                "record_id": record_id,
                "add_tags": ["unified"]
            }]
        }});
        attricat_data::batch(&batch.to_string())?;
        let direct_read = api::read(&api::ReadRequest::Record(api::RecordReference {
            record_id: record_id.clone(),
        }));
        let direct_command = api::call("attricat.read.v1", "{}");
        checkpoint["direct_read_error"] = json!(direct_read.err());
        checkpoint["direct_command_error"] = json!(direct_command.err());
        let mut seen = checkpoint["seen"].as_array().cloned().unwrap_or_default();
        seen.push(json!(record_id));
        checkpoint["seen"] = Value::Array(seen.clone());
        let next = page["next_cursor"].as_str().map(str::to_owned);
        let done = next.is_none();
        if let Some(next) = next {
            checkpoint["cursor"] = json!(next);
        }
        Ok(BatchResult {
            progress: json!({"completed": seen.len(), "total": described["count"]}).to_string(),
            checkpoint: checkpoint.to_string(),
            done,
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
