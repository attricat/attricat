//! Fixture for 1.3 client commands (`attricat:host@1.3.0` `commands`).
//!
//! A command answers with its handler and the request's locale. A payload
//! with `"reject": true` refuses with a message in that locale, and one with
//! `"fault": true` faults. The other exports are unused and fault.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "command-extension",
});

use attricat::host::outcomes;
use exports::attricat::host::{
    commands::{self, CommandFailure, CommandRejection, CommandRequest, CommandResponse},
    deferrable_operations::{self, BatchOutcome, OperationRequest},
    event_handler::{self, Delivery},
    webhook_receiver::{self, WebhookOutcome, WebhookRequest},
};
use serde_json::{Value, json};

struct Component;

const UNUSED: &str = "this fixture only answers commands";

impl commands::Guest for Component {
    fn handle_command(request: CommandRequest) -> Result<CommandResponse, CommandFailure> {
        let payload: Value = serde_json::from_str(&request.payload)
            .map_err(|error| CommandFailure::Fault(error.to_string()))?;
        if payload["fault"] == true {
            return Err(CommandFailure::Fault("the fixture broke".into()));
        }
        if payload["reject"] == true {
            return Err(CommandFailure::Rejected(CommandRejection {
                code: "client_rejected".into(),
                message: if request.locale.starts_with("pl") {
                    "NIP jest nieprawidłowy".into()
                } else {
                    "NIP is invalid".into()
                },
            }));
        }
        Ok(CommandResponse {
            payload: json!({"handler": request.handler, "locale": request.locale}).to_string(),
        })
    }
}

impl event_handler::Guest for Component {
    fn handle_event(
        _event: attricat::host::api::Event,
        _delivery: Delivery,
    ) -> Result<(), outcomes::Failure> {
        Err(outcomes::Failure::Fault(UNUSED.into()))
    }
}

impl deferrable_operations::Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Err(UNUSED.into())
    }
    fn start(_request: OperationRequest) -> Result<String, String> {
        Err(UNUSED.into())
    }
    fn process_batch(_request: OperationRequest) -> Result<BatchOutcome, outcomes::Failure> {
        Err(outcomes::Failure::Fault(UNUSED.into()))
    }
    fn checkpoint(_request: OperationRequest) -> Result<(), String> {
        Err(UNUSED.into())
    }
    fn finish(_request: OperationRequest) -> Result<(), String> {
        Err(UNUSED.into())
    }
    fn cancel(_request: OperationRequest) -> Result<(), String> {
        Err(UNUSED.into())
    }
}

impl webhook_receiver::Guest for Component {
    fn receive(_request: WebhookRequest) -> Result<WebhookOutcome, String> {
        Err(UNUSED.into())
    }
}

export!(Component);
