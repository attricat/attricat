//! Interactive operation fixture (`catalog:host@1.0.0`). Each
//! batch reads one selection member, appends a deterministic line to a run
//! output, and annotates the member in the component's own namespace. It also
//! probes the selection boundary so host tests can assert the denials.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "operation-extension",
});

use catalog::host::{artifacts, catalog_data, selection};
use exports::catalog::host::operations::{BatchResult, Guest, OperationRequest};
use serde_json::{Value, json};

const OUTPUT: &str = "summary.txt";

struct Component;

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

impl Guest for Component {
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
        let member = &page["entities"][0];
        let entity_id = member["entity_id"].as_str().unwrap_or_default().to_owned();
        let status = member["status"].as_str().unwrap_or_default().to_owned();
        if status == "available" {
            let line = format!("{entity_id}\n");
            artifacts::append_output(OUTPUT, "text/plain", &request.batch_key, line.as_bytes())?;
            let batch = json!({"operation": "batch", "batch": {
                "batch_key": request.batch_key,
                "dry_run": false,
                "intents": [{
                    "kind": "annotate",
                    "intent_key": format!("processed-{entity_id}"),
                    "entity_id": entity_id,
                    "add_tags": ["processed"],
                    "set_metadata": {"run_id": request.run_id, "cleared": null}
                }]
            }});
            let outcomes = parse(&catalog_data::batch(&batch.to_string())?)?;
            checkpoint["annotation_status"] = outcomes[0]["status"].clone();
        }
        let generic_read = catalog_data::read(
            &json!({"operation": "lookup", "blueprint_id": described["blueprint_id"], "blueprint_version": described["blueprint_version"], "attribute_id": entity_id, "value": "x"}).to_string(),
        );
        let outside = catalog_data::batch(
            &json!({"operation": "batch", "batch": {
                "batch_key": request.batch_key,
                "dry_run": false,
                "intents": [{"kind": "annotate", "intent_key": "outside", "entity_id": "00000000-0000-4000-8000-00000000ffff", "add_tags": ["x"]}]
            }})
            .to_string(),
        );
        let mut seen = checkpoint["seen"].as_array().cloned().unwrap_or_default();
        seen.push(json!({"entity_id": entity_id, "status": status}));
        checkpoint["seen"] = Value::Array(seen.clone());
        checkpoint["generic_read_rejected"] = json!(generic_read.is_err());
        checkpoint["outside_rejected"] = json!(outside.is_err());
        let next = page["next_cursor"].as_str().map(str::to_owned);
        let done = next.is_none();
        if let Some(next) = next {
            checkpoint["cursor"] = json!(next);
        } else {
            checkpoint["output"] = json!(artifacts::finalize_output(OUTPUT)?);
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
