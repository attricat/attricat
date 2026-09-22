wit_bindgen::generate!({
    path: "../extension-runtime/wit-artifacts",
    world: "catalog-extension-operation",
});

use exports::catalog::host::operations::{BatchResult, Guest, OperationRequest};
use sha2::{Digest, Sha256};

struct Component;

impl Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("prepared".into())
    }

    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("started".into())
    }

    fn process_batch(_request: OperationRequest) -> Result<BatchResult, String> {
        let input = catalog::host::artifacts::open_input("source")?;
        let output = catalog::host::artifacts::create_output("application/octet-stream")?;
        let mut hasher = Sha256::new();
        loop {
            let chunk = catalog::host::artifacts::read(&input, 64 * 1024)?;
            if chunk.is_empty() {
                break;
            }
            hasher.update(&chunk);
            catalog::host::artifacts::write(&output, &chunk)?;
        }
        let checksum = format!("{:x}", hasher.finalize());
        let completed = catalog::host::artifacts::complete(output, &checksum)?;
        Ok(BatchResult {
            checkpoint: format!("{{\"artifact_id\":\"{}\"}}", completed.artifact_id),
            progress: format!("{{\"bytes\":{}}}", completed.content_length),
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
