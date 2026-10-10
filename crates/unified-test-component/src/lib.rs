//! Fixture for the unified `catalog:host@1.6.0` ABI. One component exports
//! both the event/command `handler` and the checkpointed `operations`, and
//! probes the boundary between them: run-bound interfaces fail outside a run,
//! and direct `api` catalog access fails inside one.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "catalog-extension",
});

use catalog::host::{api, catalog_data, selection};
use exports::catalog::host::{
    handler::{self, CommandRequest, CommandResponse},
    operations::{self, BatchResult, OperationRequest},
};
use serde_json::{Value, json};

struct Component;

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

impl handler::Guest for Component {
    fn handle_event(_event: api::Event) -> Result<(), String> {
        Ok(())
    }

    fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
        let payload = parse(&request.payload)?;
        let response = json!({
            "handler": request.handler,
            "echo": payload,
            "selection_error": selection::describe().err(),
            "catalog_data_error": catalog_data::read("{}").err(),
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
        catalog_data::batch(&batch.to_string())?;
        let direct_read = api::read(&api::ReadRequest::Entity(api::EntityReference {
            entity_id: record_id.clone(),
        }));
        let direct_command = api::call("catalog.read.v1", "{}");
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
