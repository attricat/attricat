//! Packaged transfer fixture. The local policy probe never uses network;
//! the optional public-internet probe exercises range input and delivery.
wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "operation-extension",
});
use exports::attricat::host::operations::{BatchResult, Guest, OperationRequest};
use serde_json::{Value, json};

struct Component;
impl Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("{}".into())
    }
    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("{}".into())
    }
    fn process_batch(request: OperationRequest) -> Result<BatchResult, String> {
        let progress = match request.operation_id.as_str() {
            "probe" => {
                let input = r#"{"host_permission_id":"api","url":"https://api.example.com/v1/data?token=leak","transfer_key":"unsafe","offset":0,"max_bytes":1024}"#;
                if attricat::host::transfer::fetch_input(input).is_ok() {
                    return Err("host accepted an unsafe bulk transfer URL".into());
                }
                json!({"unsafe_url_denied":true})
            }
            "redirect" => {
                let input = r#"{"host_permission_id":"redirect","url":"https://httpbin.org/redirect/1","transfer_key":"redirect","offset":0,"max_bytes":1024}"#;
                let error = attricat::host::transfer::fetch_input(input)
                    .err()
                    .ok_or("host followed a redirect")?;
                if !error.contains("redirect") {
                    return Err("host did not identify the denied redirect".into());
                }
                json!({"redirect_denied":true})
            }
            "fetch" => {
                let input = r#"{"host_permission_id":"source","url":"https://httpbin.org/bytes/32","transfer_key":"input-0","offset":0,"max_bytes":1024}"#;
                let response = attricat::host::transfer::fetch_input(input)?;
                let value: Value =
                    serde_json::from_str(&response).map_err(|_| "invalid host response")?;
                let id = value["artifact_id"].as_str().ok_or("missing artifact ID")?;
                let handle = attricat::host::artifacts::open_input(id)?;
                let bytes = attricat::host::artifacts::read(&handle, 1024)?;
                if bytes.len() != 32 {
                    return Err("unexpected input length".into());
                }
                json!({"source_bytes":bytes.len()})
            }
            "deliver" => {
                attricat::host::artifacts::append_output(
                    "test.txt",
                    "text/plain",
                    &request.batch_key,
                    b"durable delivery",
                )?;
                let id = attricat::host::artifacts::finalize_output("test.txt")?;
                let body = json!({"host_permission_id":"target","url":"https://httpbin.org/put","method":"PUT","artifact_id":id,"delivery_key":"delivery-0"}).to_string();
                let outcome = attricat::host::transfer::deliver_output(&body)?;
                let value: Value =
                    serde_json::from_str(&outcome).map_err(|_| "invalid delivery response")?;
                if !matches!(
                    value["outcome"].as_str(),
                    Some("succeeded" | "uncertain" | "failed")
                ) {
                    return Err("missing explicit delivery outcome".into());
                }
                json!({"delivery_outcome":value["outcome"]})
            }
            _ => return Err("unknown test operation".into()),
        };
        Ok(BatchResult {
            checkpoint: "{}".into(),
            progress: progress.to_string(),
            done: true,
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
