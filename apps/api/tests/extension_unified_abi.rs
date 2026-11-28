//! One release on the `attricat:host@1.0.0` ABI declares an event handler, a
//! client command, scoped configuration and an interactive operation.

use crate::support;

use std::sync::Arc;

use api::{
    extension_runtime::{ComponentCompiler, ExtensionRuntime, ExtensionRuntimeConfig},
    model::NewAttributeValue,
    repository::{AttricatRepository, StartExtensionOperation},
    storage::FakeObjectStore,
};
use reqwest::multipart::{Form, Part};
use support::*;

const EXTENSION: &str = "acme.unified";
const BLUEPRINT: &str = r#"
format_version: 1
code: unified_item
name: Unified item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    extensions: [acme.unified/bulk-action, acme.unified/check, acme.unified/probe-write]
attributes:
  - code: title
    value_type: string
"#;
/// A blueprint whose views list none of the extension's actions.
const UNLISTED_BLUEPRINT: &str = r#"
format_version: 1
code: unified_unlisted_item
name: Unified unlisted item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    extensions: [acme.unified/check]
attributes:
  - code: title
    value_type: string
"#;
/// A blueprint with a value that is not a string, to clear.
const COUNTED_BLUEPRINT: &str = r#"
format_version: 1
code: unified_counted_item
name: Unified counted item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    extensions: [acme.unified/bulk-action, acme.unified/check, acme.unified/probe-write]
attributes:
  - code: title
    value_type: string
  - code: quantity
    value_type: integer
"#;
const PERMISSIONS: &[&str] = &[
    "attricat.read",
    "attricat.write",
    "attricat.annotations.write",
    "configuration.write",
    "events.subscribe",
    "client.commands",
    "client.explorer_bulk_action",
    "client.action_dialog",
    "client.operations.start",
    "client.operations.read",
    "attricat.files.read",
    "artifacts.read",
];
/// Records that link to each other: an order is selected, its lines point
/// at it through `parent`, and `related` chains on to further records.
const LINKED_BLUEPRINT: &str = r#"
format_version: 1
code: unified_linked
name: Unified linked
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    extensions: [acme.unified/bulk-action, acme.unified/check, acme.unified/probe-write]
attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: unified_linked
  - code: related
    value_type: relationship
    target_blueprint: unified_linked
  - code: document
    value_type: file
"#;

fn unified_component() -> Vec<u8> {
    build_test_component(
        "attricat-unified-test-component",
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
        "attricat": {"id": EXTENSION, "host_api": ">=1.0.0, <2.0.0"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "scoped_configuration": {"version": 1, "schema": {"type": "object"}, "scopes": ["blueprint"]},
        "artifacts": [
            {"id": "server", "kind": "server_wasm", "path": "server.wasm"},
            {"id": "client", "kind": "client_frame", "path": "client.js"}
        ],
        "server": {
            "event_handlers": [{"id": "on-update", "event_types": ["record.updated.v1"], "handler": "handle-event"}],
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
            }, {
                "id": "inspect-selection", "handler": "inspect-selection",
                "request_schema": {"type": "object"},
                "max_request_bytes": 8192, "max_checkpoint_bytes": 65536,
                "interactive": {"version": 1, "max_selection": 50}
            }, {
                "id": "inspect", "handler": "inspect",
                "request_schema": {"type": "object"},
                "max_request_bytes": 8192, "max_checkpoint_bytes": 65536
            }]
        },
        "ui": [
            {"id": "bulk", "version": 1, "kind": "dialog", "artifact": "client", "outlet": "action_dialog", "title": "Bulk"},
            {"id": "bulk-action", "version": 1, "kind": "action", "outlet": "explorer_bulk_action", "view": "table", "title": "Tag", "invoke": {"dialog": "bulk"}},
            {"id": "check", "version": 1, "kind": "action", "outlet": "explorer_bulk_action", "view": "table", "title": "Probe", "invoke": {"command": "probe"}},
            {"id": "probe-write", "version": 1, "kind": "action", "outlet": "explorer_bulk_action", "view": "table", "title": "Probe writable", "invoke": {"command": "probe"}, "when": {"requires_write": true}}
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn one_unified_component_serves_commands_and_interactive_operations(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
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
                "",
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
        response["attricat_data_error"],
        "this interface is available only during an operation run"
    );

    // Interactive operations run through the operations export of the same
    // component, with direct api catalog access denied inside the run.
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let mut records = Vec::new();
    for title in ["First", "Second"] {
        records.push(titled_record(&repository, blueprint, title).await);
    }
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    // A host-rendered action calls only the command its manifest names.
    let command =
        |command_id: &str| {
            authenticated_client()
            .post(format!("{base}/extensions/{EXTENSION}/check/command"))
            .json(&json!({"release_id": release, "command_id": command_id, "payload": {
                "blueprint_id": blueprint.0, "blueprint_version": blueprint.1, "record_ids": []
            }}))
            .send()
        };
    let declared = command("probe").await.unwrap();
    assert_eq!(declared.status(), StatusCode::OK);
    assert_eq!(declared.json::<Value>().await.unwrap()["handler"], "probe");
    assert_eq!(
        command("other").await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    // An action that requires write access runs only for writable records.
    let probe_write = |payload: Value| {
        authenticated_client()
            .post(format!("{base}/extensions/{EXTENSION}/probe-write/command"))
            .json(&json!({"release_id": release, "command_id": "probe", "payload": payload}))
            .send()
    };
    assert_eq!(
        probe_write(json!({"record_ids": [records[0]]}))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        probe_write(json!({})).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    // An entry that is not a record id is refused, not skipped by the check.
    assert_eq!(
        probe_write(json!({"record_ids": [records[0], {"id": records[0]}]}))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // An action runs only where the blueprint's table view lists it: not for
    // a revision or record whose views do not, even when the request names a
    // revision that does. (`check` listed under detail does not count.)
    let unlisted = published_blueprint(&repository, UNLISTED_BLUEPRINT).await;
    let stranger = titled_record(&repository, unlisted, "Stranger").await;
    let check = |payload: Value| {
        authenticated_client()
            .post(format!("{base}/extensions/{EXTENSION}/check/command"))
            .json(&json!({"release_id": release, "command_id": "probe", "payload": payload}))
            .send()
    };
    for payload in [
        json!({"blueprint_id": unlisted.0, "blueprint_version": unlisted.1, "record_ids": []}),
        json!({"blueprint_id": blueprint.0, "blueprint_version": blueprint.1, "record_ids": [stranger]}),
        json!({"record_ids": []}),
    ] {
        assert_eq!(
            check(payload.clone()).await.unwrap().status(),
            StatusCode::FORBIDDEN,
            "{payload}"
        );
    }
    let run = |key: &str, blueprint: (Uuid, i64), record_ids: Vec<Uuid>| {
        authenticated_client()
            .post(format!("{base}/extensions/{EXTENSION}/bulk/operations"))
            .json(&json!({
                "release_id": release,
                "operation_id": "tag",
                "input": {},
                "idempotency_key": key,
                "selection": {
                    "blueprint_id": blueprint.0,
                    "blueprint_version": blueprint.1,
                    "context_id": null,
                    "record_ids": record_ids
                }
            }))
            .send()
    };
    // The dialog runs where an action that opens it may.
    assert_eq!(
        run("unlisted-1", unlisted, vec![stranger])
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        run("unlisted-2", blueprint, vec![records[0], stranger])
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
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
                "record_ids": records
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

/// Creates a `unified_linked` record titled `title` with the given
/// relationship values.
async fn linked_record(
    repository: &AttricatRepository,
    blueprint: (Uuid, i64),
    title: &str,
    links: &[(&str, Uuid)],
) -> Uuid {
    let mut values = vec![NewAttributeValue::Scalar {
        attribute_id: None,
        attribute_code: Some("title".into()),
        context_id: None,
        value: json!(title),
    }];
    values.extend(
        links
            .iter()
            .map(|(code, target)| NewAttributeValue::Relationship {
                attribute_id: None,
                attribute_code: Some((*code).into()),
                context_id: None,
                target_record_id: *target,
            }),
    );
    repository
        .create_record_with_values(blueprint.0, blueprint.1, values, vec![], json!({}))
        .await
        .unwrap()
        .id
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_selection_run_reads_linked_records_and_attaches_files(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(&unified_component()),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, LINKED_BLUEPRINT).await;
    // The selection is an order and a document record. A line links to the
    // order; `related` chains from the line through A, B and C to D, one hop
    // too far. P points at A but is linked to nothing selected.
    let d = linked_record(&repository, blueprint, "D", &[]).await;
    let c = linked_record(&repository, blueprint, "C", &[("related", d)]).await;
    let b = linked_record(&repository, blueprint, "B", &[("related", c)]).await;
    let a = linked_record(&repository, blueprint, "A", &[("related", b)]).await;
    let order = linked_record(&repository, blueprint, "Order", &[]).await;
    let line = linked_record(
        &repository,
        blueprint,
        "Line",
        &[("parent", order), ("related", a)],
    )
    .await;
    let p = linked_record(&repository, blueprint, "P", &[("related", a)]).await;
    let source = linked_record(&repository, blueprint, "Source", &[]).await;

    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let uploaded: Value = authenticated_client()
        .post(format!(
            "{base}/records/{source}/file-attributes/document/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"%PDF-1.4 invoice".to_vec())
                    .file_name("source.pdf")
                    .mime_str("application/pdf")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id = uploaded["files"][0]["id"].clone();
    let sha256 = uploaded["files"][0]["sha256"].clone();
    // The fake store has no file worker; mark the upload ready.
    sqlx::query("UPDATE files SET status='ready' WHERE id=$1")
        .bind(file_id.as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let read = |record: Uuid| json!({"operation": "record", "record_id": record});
    let attach = |record: Uuid| {
        json!({"open": {"record_id": source, "file_id": file_id}, "record_id": record,
            "attribute_code": "document", "filename": "invoice.pdf"})
    };
    let response = authenticated_client()
        .post(format!("{base}/extensions/{EXTENSION}/bulk/operations"))
        .json(&json!({
            "release_id": release,
            "operation_id": "inspect-selection",
            "input": {
                "reads": [
                    read(order),
                    {"operation": "incoming", "record_id": order,
                        "relationships": [{"source_blueprint": "unified_linked", "field": "parent"}],
                        "limit": 10},
                    read(line), read(a), read(b), read(c),
                    read(d), read(p),
                    {"operation": "incoming", "record_id": a,
                        "relationships": [{"source_blueprint": "unified_linked", "field": "related"}],
                        "limit": 10},
                    {"operation": "blueprint", "code": "unified_linked"},
                    {"operation": "page", "blueprint_id": blueprint.0,
                        "blueprint_version": blueprint.1, "cursor": null, "limit": 1},
                ],
                "attach": [attach(order), attach(a)],
            },
            "idempotency_key": "linked-1",
            "selection": {
                "blueprint_id": blueprint.0,
                "blueprint_version": blueprint.1,
                "context_id": null,
                "record_ids": [order, source]
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
    let (status, checkpoint): (String, Value) =
        sqlx::query_as("SELECT status, checkpoint FROM extension_operation_runs WHERE id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed", "checkpoint: {checkpoint}");
    let before = &checkpoint["before"];

    // The selection, its lines and three hops on from them are readable.
    assert_eq!(before[0]["record"]["id"], json!(order), "{before}");
    let lines: Vec<&Value> = before[1]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["id"])
        .collect();
    assert_eq!(lines, vec![&json!(line)], "{before}");
    for (index, record) in [(2, line), (3, a), (4, b), (5, c)] {
        assert_eq!(before[index]["record"]["id"], json!(record), "{before}");
    }
    // A fourth hop, and a record that only links to a linked one, are not.
    for index in [6, 7] {
        assert_eq!(
            before[index]["error"], "the record is not linked from the run selection",
            "{before}"
        );
    }
    assert_eq!(
        before[8]["error"], "incoming reads start at a record of the run selection",
        "{before}"
    );
    assert_eq!(before[9]["blueprint"]["id"], json!(blueprint.0));
    assert_eq!(
        before[10]["error"],
        "interactive runs read catalog data through their run-bound selection"
    );

    // A file opened into the run is attached to a selected record once; the
    // second attachment of the same file and name is a replay.
    let attached = &checkpoint["attachments"][0];
    assert_eq!(attached[0]["replayed"], false, "{checkpoint}");
    assert_eq!(attached[1]["replayed"], true, "{checkpoint}");
    assert_eq!(attached[0]["file"]["sha256"], sha256);
    assert_eq!(attached[0]["file"]["filename"], "invoice.pdf");
    assert_eq!(attached[1]["file"]["id"], attached[0]["file"]["id"]);
    assert_eq!(
        checkpoint["attachments"][1]["error"],
        "the record is outside the run selection"
    );
    let files: Vec<Uuid> = sqlx::query_scalar(
        "SELECT r.file_id FROM attribute_file_references r JOIN attribute_values v ON v.id=r.attribute_value_id WHERE v.record_id=$1",
    )
    .bind(order)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(json!(files[0]), attached[0]["file"]["id"]);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_run_reads_a_record_finds_codes_and_clears_a_value(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(&unified_component()),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, COUNTED_BLUEPRINT).await;
    let record_id = repository
        .create_record_with_values(
            blueprint.0,
            blueprint.1,
            vec![
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!("Counted"),
                },
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("quantity".into()),
                    context_id: None,
                    value: json!(5),
                },
            ],
            vec![],
            json!({}),
        )
        .await
        .unwrap()
        .id;
    let default_context = repository
        .get_context_by_code("default")
        .await
        .unwrap()
        .unwrap()
        .id;

    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: EXTENSION.into(),
            expected_release_id: release,
            operation_id: "inspect".into(),
            input: json!({
                "reads": [
                    {"operation": "record", "record_id": record_id, "context_id": default_context},
                    {"operation": "blueprint", "code": "unified_counted_item"},
                    {"operation": "context", "code": "default"},
                    {"operation": "blueprint", "code": "no_such_blueprint"},
                ],
                "intents": [{
                    "kind": "update", "intent_key": "clear-quantity", "record_id": record_id,
                    "remove_values": [{"attribute_code": "quantity", "context_id": default_context}]
                }],
            }),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "inspect-1".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    drain_extension_operations(&repository, store).await;
    let (status, checkpoint): (String, Value) =
        sqlx::query_as("SELECT status, checkpoint FROM extension_operation_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed", "checkpoint: {checkpoint}");
    let before = &checkpoint["before"];
    let quantity = before[1]["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attribute| attribute["code"] == "quantity")
        .unwrap()["id"]
        .clone();
    let quantity_values = |read: &Value| {
        read["direct_values"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| value["attribute_id"] == quantity)
            .map(|value| value["value"].clone())
            .collect::<Vec<_>>()
    };

    // A run reads one record as `api.read` would, resolved in a context.
    assert_eq!(before[0]["record"]["id"], json!(record_id), "{before}");
    assert_eq!(
        before[0]["blueprint"]["blueprint"]["id"],
        json!(blueprint.0)
    );
    assert_eq!(quantity_values(&before[0]), vec![json!(5)]);
    assert!(before[0]["resolved_values"].is_object(), "{before}");
    // Configuration can name a blueprint and a context by code.
    assert_eq!(before[1]["blueprint"]["id"], json!(blueprint.0));
    assert_eq!(before[1]["blueprint"]["version"], json!(blueprint.1));
    assert_eq!(before[2]["id"], json!(default_context));
    assert_eq!(before[3], Value::Null);

    // An integer value is cleared, not overwritten with an empty string.
    assert_eq!(
        checkpoint["outcomes"][0]["status"], "applied",
        "{checkpoint}"
    );
    let after = &checkpoint["after"];
    assert_eq!(quantity_values(&after[0]), Vec::<Value>::new(), "{after}");
    assert_eq!(after[0]["direct_values"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_component_compiled_in_a_child_process_runs_like_any_other(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(&unified_component()),
        PERMISSIONS,
    )
    .await;
    let installation = repository
        .runtime_extension_installation(EXTENSION, release)
        .await
        .unwrap()
        .unwrap();
    let runtime_with = |executable: &str| {
        ExtensionRuntime::new(
            store.clone(),
            ExtensionRuntimeConfig {
                compiler: ComponentCompiler::Isolated {
                    executable: executable.into(),
                },
                ..ExtensionRuntimeConfig::default()
            },
        )
        .unwrap()
    };
    let response: Value = serde_json::from_str(
        &runtime_with(env!("CARGO_BIN_EXE_api"))
            .invoke_command(
                &installation,
                repository.clone(),
                "probe",
                r#"{"n":1}"#,
                "",
                65536,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["echo"], json!({"n": 1}));

    // A compiler that fails stays failed for a while, rather than being run again
    // by every request.
    let broken = runtime_with("/nonexistent/compiler");
    for expected in ["component compilation failed", "failed to compile recently"] {
        let error = broken
            .invoke_command(&installation, repository.clone(), "probe", "{}", "", 65536)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn the_compile_command_refuses_what_is_not_a_component() {
    use std::{
        io::Write as _,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_api"))
        .arg(api::extension_runtime::COMPILE_COMPONENT_COMMAND)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"not wasm").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

const LINK_LIMIT_BLUEPRINT: &str = r#"
format_version: 1
code: link_limit
name: Link limit
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: link_limit
  - code: related
    value_type: relationship
    target_blueprint: link_limit
"#;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_selection_with_too_many_incoming_links_reads_only_itself(pool: sqlx::PgPool) {
    use api::repository::SelectionLink;
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let blueprint = published_blueprint(&repository, LINK_LIMIT_BLUEPRINT).await;
    let product = linked_record(&repository, blueprint, "Product", &[]).await;
    let order = linked_record(&repository, blueprint, "Order", &[]).await;
    for title in ["Line 1", "Line 2", "Line 3"] {
        linked_record(
            &repository,
            blueprint,
            title,
            &[("parent", order), ("related", product)],
        )
        .await;
    }
    let selection = [order];
    // Within the limit, the lines' product is linked from the order.
    assert_eq!(
        repository
            .selection_links_record_within(&selection, product, 3)
            .await
            .unwrap(),
        SelectionLink::Linked
    );
    // Past it, the walk does not start; the selection itself stays readable.
    assert_eq!(
        repository
            .selection_links_record_within(&selection, product, 2)
            .await
            .unwrap(),
        SelectionLink::TooManyLinks
    );
    assert_eq!(
        repository
            .selection_links_record_within(&selection, order, 2)
            .await
            .unwrap(),
        SelectionLink::Linked
    );
}
