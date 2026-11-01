//! Interactive operation fixture (`attricat:host@1.0.0`). Each
//! batch reads one selection member, appends a deterministic line to a run
//! output, and annotates the member in the component's own namespace. It also
//! probes the selection boundary so host tests can assert the denials. The
//! `import` operation is a blueprint-scoped run: it creates one record per line
//! of its input file and probes what such a run may not do.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "operation-extension",
});

use attricat::host::{artifacts, attricat as catalog, attricat_data, selection};
use exports::attricat::host::operations::{BatchResult, Guest, OperationRequest};
use serde_json::{Value, json};

const OUTPUT: &str = "summary.txt";
/// The operation whose run is scoped to a blueprint revision.
const IMPORT_OPERATION: &str = "import";
/// The most input bytes the fixture reads.
const MAX_IMPORT_BYTES: u32 = 4096;

struct Component;

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

/// Creates a record per input line, then records in the checkpoint what the
/// host refused, so tests can assert the boundary of a blueprint run.
fn import(request: &OperationRequest) -> Result<BatchResult, String> {
    let described = parse(&selection::describe()?)?;
    let blueprint_id = described["blueprint_id"].as_str().unwrap_or_default();
    let blueprint_version = described["blueprint_version"].as_u64().unwrap_or_default();
    let context_id = described["context_id"]
        .as_str()
        .or_else(|| described["default_context_id"].as_str())
        .unwrap_or_default()
        .to_owned();
    let schema = catalog::schema(blueprint_id, blueprint_version, &context_id);
    let source = artifacts::open_input("source")?;
    let metadata = artifacts::describe_input(&source)?;
    let bytes = artifacts::read(&source, MAX_IMPORT_BYTES)?;
    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    let value = |title: &str| json!([{"kind": "scalar", "attribute_code": "title", "context_id": context_id, "value": title}]);
    let intents: Vec<Value> = text
        .lines()
        .map(|title| {
            json!({
                "kind": "create",
                "intent_key": format!("line-{title}"),
                "blueprint_id": blueprint_id,
                "blueprint_version": blueprint_version,
                "values": value(title)
            })
        })
        .collect();
    let batch = |intents: Value| {
        json!({"operation": "batch", "batch": {
            "batch_key": request.batch_key, "dry_run": false, "intents": intents
        }})
        .to_string()
    };
    let outcomes = parse(&attricat_data::batch(&batch(Value::Array(intents)))?)?;
    let refused = |intent: Value| attricat_data::batch(&batch(json!([intent]))).is_err();
    let checkpoint = json!({
        "scope": described["scope"],
        "count": described["count"],
        "media_type": metadata.media_type,
        "schema_read": schema.is_ok(),
        "statuses": outcomes.as_array().map(|items| items.iter().map(|item| item["status"].clone()).collect::<Vec<_>>()),
        "errors": outcomes.as_array().map(|items| items.iter().map(|item| item["error"].clone()).collect::<Vec<_>>()),
        "generic_read_rejected": attricat_data::read(
            &json!({"operation": "page", "blueprint_id": blueprint_id, "blueprint_version": blueprint_version, "context_id": context_id, "cursor": null, "limit": 1}).to_string(),
        ).is_err(),
        "other_schema_rejected": catalog::schema("00000000-0000-4000-8000-00000000ffff", blueprint_version, &context_id).is_err(),
        "update_rejected": refused(json!({"kind": "update", "intent_key": "update", "record_id": "00000000-0000-4000-8000-00000000ffff"})),
        "other_blueprint_rejected": refused(json!({"kind": "create", "intent_key": "other", "blueprint_id": "00000000-0000-4000-8000-00000000ffff", "blueprint_version": blueprint_version, "values": []})),
        "system_tags_rejected": refused(json!({"kind": "create", "intent_key": "tags", "blueprint_id": blueprint_id, "blueprint_version": blueprint_version, "values": value("tagged"), "system_tags": ["x"]})),
    });
    Ok(BatchResult {
        checkpoint: checkpoint.to_string(),
        progress: json!({"completed": 1, "total": 1}).to_string(),
        done: true,
    })
}

impl Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("prepared".into())
    }

    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("started".into())
    }

    fn process_batch(request: OperationRequest) -> Result<BatchResult, String> {
        if request.operation_id == IMPORT_OPERATION {
            return import(&request);
        }
        let mut checkpoint = parse(&request.checkpoint)?;
        let cursor = checkpoint["cursor"].as_str().unwrap_or_default().to_owned();
        let described = parse(&selection::describe()?)?;
        let page = parse(&selection::page(&cursor, 1)?)?;
        let member = &page["records"][0];
        let record_id = member["record_id"].as_str().unwrap_or_default().to_owned();
        let status = member["status"].as_str().unwrap_or_default().to_owned();
        if status == "available" {
            let line = format!("{record_id}\n");
            artifacts::append_output(OUTPUT, "text/plain", &request.batch_key, line.as_bytes())?;
            let batch = json!({"operation": "batch", "batch": {
                "batch_key": request.batch_key,
                "dry_run": false,
                "intents": [{
                    "kind": "annotate",
                    "intent_key": format!("processed-{record_id}"),
                    "record_id": record_id,
                    "add_tags": ["processed"],
                    "set_metadata": {"run_id": request.run_id, "cleared": null}
                }]
            }});
            let outcomes = parse(&attricat_data::batch(&batch.to_string())?)?;
            checkpoint["annotation_status"] = outcomes[0]["status"].clone();
        }
        let generic_read = attricat_data::read(
            &json!({"operation": "lookup", "blueprint_id": described["blueprint_id"], "blueprint_version": described["blueprint_version"], "attribute_id": record_id, "value": "x"}).to_string(),
        );
        let outside = attricat_data::batch(
            &json!({"operation": "batch", "batch": {
                "batch_key": request.batch_key,
                "dry_run": false,
                "intents": [{"kind": "annotate", "intent_key": "outside", "record_id": "00000000-0000-4000-8000-00000000ffff", "add_tags": ["x"]}]
            }})
            .to_string(),
        );
        let mut seen = checkpoint["seen"].as_array().cloned().unwrap_or_default();
        seen.push(json!({"record_id": record_id, "status": status}));
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
