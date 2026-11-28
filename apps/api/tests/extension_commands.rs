//! 1.3 client commands (`attricat:host@1.3.0` `commands`) through the HTTP
//! router and the production Wasmtime runtime: a rejection reaches the
//! person with the extension's own message, in their language, and a fault
//! stays a generic error.

use crate::support;

use std::sync::Arc;

use api::{repository::AttricatRepository, storage::FakeObjectStore};
use support::*;

const EXTENSION: &str = "acme.commands";
const BLUEPRINT: &str = r#"
format_version: 1
code: command_item
name: Command item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    extensions: [acme.commands/check]
attributes:
  - code: title
    value_type: string
"#;
const PERMISSIONS: &[&str] = &["client.commands", "client.explorer_bulk_action"];

fn archive() -> Vec<u8> {
    static COMPONENT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let component = COMPONENT.get_or_init(|| {
        build_test_component(
            "attricat-command-test-component",
            "command-test.component.wasm",
        )
    });
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Command fixture",
        "version": "1.0.0",
        "description": "attricat:host 1.3 command test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": EXTENSION, "host_api": "^1.3"},
        "permissions": PERMISSIONS,
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "commands": [{
                "id": "probe", "handler": "probe",
                "request_schema": {"type": "object"},
                "response_schema": {"type": "object"}
            }]
        },
        "ui": [{
            "id": "check", "version": 1, "kind": "action", "outlet": "explorer_bulk_action",
            "view": "table", "title": "Check", "invoke": {"command": "probe"}
        }]
    }))
    .unwrap();
    tar_zst(&[("manifest.json", &manifest), ("server.wasm", component)])
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_command_rejection_reaches_the_person_in_their_language(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let command = |payload: Value, locale: Option<&str>| {
        let mut body = json!({"release_id": release, "command_id": "probe", "payload": {
            "blueprint_id": blueprint.0, "blueprint_version": blueprint.1, "record_ids": []
        }});
        for (key, value) in payload.as_object().unwrap() {
            body["payload"][key] = value.clone();
        }
        if let Some(locale) = locale {
            body["locale"] = json!(locale);
        }
        authenticated_client()
            .post(format!("{base}/extensions/{EXTENSION}/check/command"))
            .json(&body)
            .send()
    };

    // The command receives the person's language.
    let answered = command(json!({}), Some("pl")).await.unwrap();
    assert_eq!(answered.status(), StatusCode::OK);
    assert_eq!(
        answered.json::<Value>().await.unwrap(),
        json!({"handler": "probe", "locale": "pl"})
    );

    // A rejection is a 422 carrying the extension's code and message.
    for (locale, reason) in [
        (Some("pl"), "NIP jest nieprawidłowy"),
        (None, "NIP is invalid"),
    ] {
        let rejected = command(json!({"reject": true}), locale).await.unwrap();
        assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: Value = rejected.json().await.unwrap();
        assert_eq!(body["error"]["code"], "extension_command_rejected");
        assert_eq!(
            body["error"]["details"],
            json!({"extension_id": EXTENSION, "rejection_code": "client_rejected", "reason": reason})
        );
    }

    // A fault reveals nothing of the component's text.
    let faulted = command(json!({"fault": true}), None).await.unwrap();
    assert_eq!(faulted.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!faulted.text().await.unwrap().contains("the fixture broke"));

    // A locale that is not a short language tag is refused.
    let invalid = command(json!({}), Some("pl; DROP")).await.unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    server.abort();
}
