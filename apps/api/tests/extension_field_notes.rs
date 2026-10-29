//! Field notes: the host asks every provider once per record, keeps what
//! belongs to the record, and leaves out a provider that breaks the contract.

use crate::support;

use std::sync::Arc;

use api::{repository::AttricatRepository, storage::FakeObjectStore};
use support::*;

const BLUEPRINT: &str = r#"
format_version = 1
code = "noted_item"
name = "Noted item"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "sku"
value_type = "string"
"#;
const PERMISSIONS: &[&str] = &[
    "client.commands",
    "client.record_field_notes",
    "client.action_dialog",
];

fn component() -> Vec<u8> {
    build_test_component(
        "attricat-unified-test-component",
        "unified-test.component.wasm",
    )
}

/// A provider whose `describe-fields` command runs the fixture `handler`.
fn archive(extension_id: &str, handler: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": format!("Notes {extension_id}"),
        "version": "1.0.0",
        "description": "field notes integration test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": extension_id, "host_api": ">=1.0.0, <2.0.0"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [
            {"id": "server", "kind": "server_wasm", "path": "server.wasm"},
            {"id": "client", "kind": "client_component", "path": "client.js"}
        ],
        "server": {"commands": [{
            "id": "describe-fields", "handler": handler,
            "request_schema": {
                "type": "object",
                "required": ["record_id", "blueprint_id", "blueprint_version", "context_id"]
            },
            "response_schema": {"type": "object"}
        }]},
        "record_field_notes": {"version": 1, "source": {"command": "describe-fields"}},
        "ui": [
            {"id": "explain-dialog", "version": 1, "kind": "dialog", "artifact": "client", "outlet": "action_dialog", "title": "Explain"},
            {"id": "explain", "version": 1, "kind": "action", "outlet": "record_attribute_action", "title": "Explain", "invoke": {"dialog": "explain-dialog"}}
        ]
    }))
    .unwrap();
    tar_zst(&[
        ("manifest.json", &manifest),
        ("server.wasm", &component()),
        ("client.js", b"export const mount = () => {};"),
    ])
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn field_notes_keep_what_belongs_to_the_record_from_each_provider(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    for (extension_id, handler) in [
        ("acme.notes", "field-notes"),
        ("acme.broken", "field-notes-invalid"),
    ] {
        install_extension(
            &repository,
            store.clone(),
            extension_id,
            &archive(extension_id, handler),
            PERMISSIONS,
        )
        .await;
    }
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;

    // The first call compiles each provider's component, which may outlast
    // the host's wait; the response says so and a later call completes.
    let mut body = Value::Null;
    for _ in 0..60 {
        let response = authenticated_client()
            .get(format!("{base}/records/{record}/extension-field-notes"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        body = response.json().await.unwrap();
        if body["incomplete"] == false {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert_eq!(body["incomplete"], false, "{body}");
    // The provider breaking the contract is left out; the other answers.
    let extensions = body["extensions"].as_array().unwrap();
    assert_eq!(extensions.len(), 1, "{body}");
    let notes = &extensions[0];
    assert_eq!(notes["extension_id"], "acme.notes");
    assert_eq!(
        notes["attributes"]["title"],
        json!({
            "badge": {"label": "Checked", "tone": "success", "tooltip": "From the fixture", "action": "explain"},
            "note": format!("Checked record {record}")
        })
    );
    // A badge keeps its text but loses an action the extension never declared.
    assert_eq!(
        notes["attributes"]["sku"],
        json!({"badge": {"label": "SKU", "tone": "neutral"}})
    );
    // Notes for attributes and files the record does not have are dropped.
    assert!(notes["attributes"].get("not_an_attribute").is_none());
    assert_eq!(notes["files"], json!({}));

    let missing = authenticated_client()
        .get(format!(
            "{base}/records/{}/extension-field-notes",
            Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    server.abort();
}
