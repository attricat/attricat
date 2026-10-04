//! One release on the unified `catalog:host@1.6.0` ABI declares an event
//! handler, a client command, scoped configuration and an interactive
//! operation, which no legacy `host_api` range can combine.

mod support;

use std::sync::Arc;

use api::{
    extension_runtime::{ExtensionRuntime, ExtensionRuntimeConfig},
    repository::CatalogRepository,
    storage::FakeObjectStore,
};
use support::*;

const EXTENSION: &str = "acme.unified";
const BLUEPRINT: &str = r#"
format_version = 1
code = "unified_item"
name = "Unified item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#;
const PERMISSIONS: &[&str] = &[
    "catalog.read",
    "catalog.write",
    "catalog.annotations.write",
    "configuration.write",
    "events.subscribe",
    "client.commands",
    "client.explorer_bulk_action",
    "client.operations.start",
    "client.operations.read",
];

fn unified_component() -> Vec<u8> {
    build_test_component(
        "catalog-unified-test-component",
        "unified-test.component.wasm",
    )
}

fn manifest() -> Value {
    json!({
        "manifest_version": 1,
        "name": "Unified extension",
        "version": "1.0.0",
        "description": "unified host ABI integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": EXTENSION, "host_api": ">=1.6.0, <2.0.0"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "scoped_configuration": {"version": 1, "schema": {"type": "object"}, "scopes": ["blueprint"]},
        "artifacts": [
            {"id": "server", "kind": "server_wasm", "path": "server.wasm"},
            {"id": "client", "kind": "client_component", "path": "client.js"}
        ],
        "server": {
            "event_handlers": [{"id": "on-update", "event_types": ["entity.updated.v1"], "handler": "handle-event"}],
            "commands": [{
                "id": "probe", "handler": "probe",
                "request_schema": {"type": "object"},
                "response_schema": {"type": "object"}
            }],
            "operations": [{
                "id": "tag", "handler": "tag",
                "request_schema": {"type": "object", "additionalProperties": false},
                "max_request_bytes": 1024, "max_checkpoint_bytes": 8192,
                "interactive": {"version": 1, "max_selection": 50}
            }]
        },
        "ui": [
            {"id": "bulk", "version": 2, "kind": "action", "artifact": "client", "outlet": "explorer_bulk_action"}
        ]
    })
}

fn archive(server: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&manifest()).unwrap();
    tar_zst(&[
        ("manifest.json", &manifest),
        ("server.wasm", server),
        ("client.js", b"export const mount = () => {};"),
    ])
}

#[test]
fn legacy_ranges_cannot_combine_commands_and_interactive_operations() {
    for legacy in [">=1.1.0, <2.0.0", ">=1.5.0, <2.0.0"] {
        let mut value = manifest();
        value["catalog"]["host_api"] = json!(legacy);
        let manifest: api::extensions::Manifest = serde_json::from_value(value).unwrap();
        assert!(
            manifest
                .validate(api::extensions::SUPPORTED_HOST_API)
                .is_err(),
            "{legacy} must stay bound to a legacy world"
        );
    }
    let manifest: api::extensions::Manifest = serde_json::from_value(manifest()).unwrap();
    manifest
        .validate(api::extensions::SUPPORTED_HOST_API)
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn one_unified_component_serves_commands_and_interactive_operations(pool: sqlx::PgPool) {
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(bootstrap_workspace_id())
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(&unified_component()),
        PERMISSIONS,
    )
    .await;

    // Commands run through the handler export; run-bound imports are linked
    // but fail outside an operation run.
    let installation = repository
        .runtime_extension_installation(EXTENSION, release)
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    let response: Value = serde_json::from_str(
        &runtime
            .invoke_command(
                &installation,
                repository.clone(),
                "probe",
                r#"{"n":1}"#,
                65536,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["handler"], "probe");
    assert_eq!(response["echo"], json!({"n": 1}));
    assert_eq!(
        response["selection_error"],
        "this interface is available only during an operation run"
    );
    assert_eq!(
        response["catalog_data_error"],
        "this interface is available only during an operation run"
    );

    // Interactive operations run through the operations export of the same
    // component, with direct api catalog access denied inside the run.
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let mut entities = Vec::new();
    for title in ["First", "Second"] {
        entities.push(titled_entity(&repository, blueprint, title).await);
    }
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let response = authenticated_client()
        .post(format!("{base}/extensions/{EXTENSION}/bulk/operations"))
        .json(&json!({
            "release_id": release,
            "operation_id": "tag",
            "input": {},
            "idempotency_key": "unified-1",
            "selection": {
                "blueprint_id": blueprint.0,
                "blueprint_version": blueprint.1,
                "context_id": null,
                "entity_ids": entities
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let run_id: Uuid = response.json::<Value>().await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    drain_extension_operations(&repository, store.clone()).await;

    let (status, abi_version, checkpoint): (String, String, Value) = sqlx::query_as(
        "SELECT status, abi_version, checkpoint FROM extension_operation_runs WHERE id=$1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(abi_version, api::extensions::SUPPORTED_HOST_API);
    assert_eq!(status, "completed", "checkpoint: {checkpoint}");
    assert_eq!(checkpoint["seen"].as_array().unwrap().len(), 2);
    assert_eq!(
        checkpoint["direct_read_error"],
        "operation runs access catalog data through their run-scoped interfaces"
    );
    assert_eq!(
        checkpoint["direct_command_error"],
        "operation runs access catalog data through their run-scoped interfaces"
    );
    server.abort();
}
