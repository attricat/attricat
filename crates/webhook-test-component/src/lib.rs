//! Fixture for inbound webhooks (`attricat:host@1.2.0` `webhook-receiver`).
//!
//! `receive` checks `x-signature`, the hex SHA-256 of the `webhook-secret`
//! extension secret, a `.` and the body, and keeps the body with the
//! `x-delivery-id` header as its dedupe key. A `GET` with `challenge=<value>`
//! is a verification handshake answered with the value. `x-probe: <operation>`
//! tries a host operation and answers with its error, and `x-fail` makes
//! `receive` fail.
//!
//! The event handler reads each kept payload back and stores it under
//! `order-<id>`. A payload with `"retry": true` asks for one retry first, and
//! one with `"fault": true` faults.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "webhook-extension",
});

use attricat::host::{api, outcomes};
use exports::attricat::host::{
    command_handler::{self, CommandRequest, CommandResponse},
    deferrable_operations::{self, BatchOutcome, OperationRequest},
    event_handler::{self, Delivery},
    webhook_receiver::{
        self, KeptPayload, WebhookHeader, WebhookOutcome, WebhookReply, WebhookRequest,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Component;

fn call(operation: &str, request: Value) -> Result<Value, String> {
    serde_json::from_str(&api::call(operation, &request.to_string())?)
        .map_err(|error| error.to_string())
}

fn header<'a>(request: &'a WebhookRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|header| header.name == name)
        .map(|header| header.value.as_str())
}

fn reply(status: u16, body: impl Into<String>) -> WebhookOutcome {
    WebhookOutcome {
        reply: WebhookReply {
            status,
            headers: vec![WebhookHeader {
                name: "content-type".into(),
                value: "text/plain".into(),
            }],
            body: body.into(),
        },
        keep: None,
    }
}

impl webhook_receiver::Guest for Component {
    fn receive(request: WebhookRequest) -> Result<WebhookOutcome, String> {
        if header(&request, "x-fail").is_some() {
            return Err("asked to fail".into());
        }
        if let Some(operation) = header(&request, "x-probe") {
            let error = api::call(operation, "{}").err().unwrap_or_default();
            return Ok(reply(200, error));
        }
        if request.method == "GET" {
            return Ok(
                match request
                    .query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("challenge="))
                {
                    Some(challenge) => reply(200, challenge),
                    None => reply(400, "missing challenge"),
                },
            );
        }
        let secret = call("secrets.get.v1", json!({"name": "webhook-secret"}))?;
        let mut hash = Sha256::new();
        hash.update(secret["value"].as_str().unwrap_or_default().as_bytes());
        hash.update(b".");
        hash.update(&request.body);
        if header(&request, "x-signature") != Some(hex::encode(hash.finalize()).as_str()) {
            return Ok(reply(401, "bad signature"));
        }
        let dedupe_key = header(&request, "x-delivery-id").map(str::to_owned);
        let payload = String::from_utf8(request.body).map_err(|error| error.to_string())?;
        let mut outcome = reply(202, format!("kept for {}", request.endpoint));
        outcome.keep = Some(KeptPayload {
            payload,
            dedupe_key,
        });
        Ok(outcome)
    }
}

impl event_handler::Guest for Component {
    fn handle_event(event: api::Event, delivery: Delivery) -> Result<(), outcomes::Failure> {
        let fault = |message: String| outcomes::Failure::Fault(message);
        if event.event_type != "webhook.received.v1" {
            return Ok(());
        }
        let announced: Value =
            serde_json::from_str(&event.payload).map_err(|e| fault(e.to_string()))?;
        let receipt = call(
            "webhooks.receipt.get.v1",
            json!({"receipt_id": announced["receipt_id"]}),
        )
        .map_err(fault)?;
        let payload: Value = serde_json::from_str(receipt["payload"].as_str().unwrap_or("null"))
            .map_err(|e| fault(e.to_string()))?;
        if payload["fault"] == true {
            return Err(fault("asked to fault".into()));
        }
        if payload["retry"] == true && delivery.attempt < 2 {
            return Err(outcomes::Failure::Retry(outcomes::Retry {
                message: "asked to retry".into(),
                after_ms: Some(0),
            }));
        }
        call(
            "storage.set.v1",
            json!({
                "key": format!("order-{}", payload["id"]),
                "value": {
                    "payload": payload,
                    "webhook": receipt["webhook"],
                    "endpoint": receipt["endpoint"],
                    "attempt": delivery.attempt,
                },
            }),
        )
        .map_err(fault)?;
        Ok(())
    }
}

impl command_handler::Guest for Component {
    fn handle_command(_request: CommandRequest) -> Result<CommandResponse, String> {
        Err("no commands".into())
    }
}

impl deferrable_operations::Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Err("no operations".into())
    }
    fn start(_request: OperationRequest) -> Result<String, String> {
        Err("no operations".into())
    }
    fn process_batch(_request: OperationRequest) -> Result<BatchOutcome, outcomes::Failure> {
        Err(outcomes::Failure::Fault("no operations".into()))
    }
    fn checkpoint(_request: OperationRequest) -> Result<(), String> {
        Err("no operations".into())
    }
    fn finish(_request: OperationRequest) -> Result<(), String> {
        Err("no operations".into())
    }
    fn cancel(_request: OperationRequest) -> Result<(), String> {
        Err("no operations".into())
    }
}

export!(Component);
