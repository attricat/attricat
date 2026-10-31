//! Field notes: the host asks every provider once per record, keeps what
//! belongs to the record, and leaves out a provider that breaks the contract.

use crate::support;

use std::sync::Arc;

use api::{repository::AttricatRepository, storage::FakeObjectStore};
use support::*;

const BLUEPRINT: &str = r#"
format_version: 1
code: noted_item
name: Noted item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: sku
    value_type: string
"#;
const PERMISSIONS: &[&str] = &[
    "client.commands",
    "client.record_field_notes",
    "client.action_dialog",
    // The slow fixture counts its calls in its storage.
    "storage.extension",
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
            {"id": "client", "kind": "client_frame", "path": "client.js"}
        ],
        "server": {"commands": [{
            "id": "describe-fields", "handler": handler,
            "request_schema": {
                "type": "object",
                "required": ["records", "context_id", "locale"]
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

    let ask = |record_ids: Vec<Uuid>| {
        authenticated_client()
            .post(format!("{base}/records/extension-field-notes"))
            .json(&json!({"record_ids": record_ids, "locale": "pl"}))
            .send()
    };
    // The first call compiles each provider's component, which may outlast
    // the host's wait; the response says so and a later call completes.
    let mut body = Value::Null;
    for _ in 0..60 {
        let response = ask(vec![record, Uuid::new_v4()]).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        body = response.json().await.unwrap();
        if body["incomplete"] == false {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert_eq!(body["incomplete"], false, "{body}");
    // An unknown record is left out; the provider breaking the contract is
    // left out of the known one, and the other answers.
    let records = body["records"].as_object().unwrap();
    assert_eq!(records.len(), 1, "{body}");
    let extensions = records[&record.to_string()].as_array().unwrap();
    assert_eq!(extensions.len(), 1, "{body}");
    let notes = &extensions[0];
    assert_eq!(notes["extension_id"], "acme.notes");
    assert_eq!(
        notes["attributes"]["title"],
        json!({
            "badge": {"label": "Checked", "tone": "success", "icon": "check", "tooltip": "From the fixture", "action": "explain"},
            "note": format!("Checked record {record} in pl")
        })
    );
    // A badge keeps its text but loses an action the extension never declared.
    assert_eq!(
        notes["attributes"]["sku"],
        json!({"badge": {"label": "SKU", "tone": "neutral"}})
    );
    // Notes for attributes, files and actions the extension or record lacks
    // are dropped.
    assert!(notes["attributes"].get("not_an_attribute").is_none());
    assert_eq!(notes["files"], json!({}));
    assert_eq!(
        notes["actions"],
        json!({"explain": {"disabled": "Nothing to explain"}})
    );
    assert_eq!(body["client_sources"], json!([]));

    // A cacheable answer is reused while the record is unchanged: the next
    // request is complete at once.
    let cached: Value = ask(vec![record]).await.unwrap().json().await.unwrap();
    assert_eq!(cached["incomplete"], false);
    assert_eq!(
        cached["records"][record.to_string()][0]["attributes"]["title"]["note"],
        format!("Checked record {record} in pl")
    );

    let empty = ask(Vec::new()).await.unwrap();
    assert_eq!(empty.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn field_notes_answer_nothing_without_a_provider(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let (base, server) =
        start_server_with_object_store(pool, Arc::new(FakeObjectStore::available())).await;

    let response = authenticated_client()
        .post(format!("{base}/records/extension-field-notes"))
        .json(&json!({"record_ids": [record]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"records": {}, "incomplete": false, "client_sources": []})
    );
    server.abort();
}

/// Asks for `record`'s field notes until no provider is still starting.
async fn complete_notes(base: &str, record: Uuid) -> Value {
    let mut body = Value::Null;
    for _ in 0..60 {
        body = authenticated_client()
            .post(format!("{base}/records/extension-field-notes"))
            .json(&json!({"record_ids": [record]}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if body["incomplete"] == false {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert_eq!(body["incomplete"], false, "{body}");
    body
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn attaching_a_reusable_attribute_refreshes_cached_field_notes(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        "acme.notes",
        &archive("acme.notes", "field-notes"),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let (base, server) = start_server_with_object_store(pool, store).await;
    let depth_note = |body: &Value| {
        body["records"][record.to_string()][0]["attributes"]["default:depth"].clone()
    };

    // The provider notes an attribute the record does not have yet, so the
    // host drops it, and marks the answer cacheable for an hour.
    let body = complete_notes(&base, record).await;
    assert_eq!(depth_note(&body), Value::Null, "{body}");

    let client = authenticated_client();
    let draft: Value = client
        .post(format!("{base}/reusable-attributes"))
        .json(&json!({"definition": "code: depth\nname: Depth\nvalue_type: number\n"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let published: Value = client
        .post(format!(
            "{base}/reusable-attribute-revisions/{}/publish",
            draft["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!("{base}/v1/records/{record}/reusable-attributes"))
        .json(&json!({"reusable_attribute_revision_id": published["id"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // The cached answer was filtered without the attribute, so it is not
    // reused once the attribute is attached.
    let body = complete_notes(&base, record).await;
    assert_eq!(
        depth_note(&body),
        json!({"note": "Depth is attached"}),
        "{body}"
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_slow_provider_is_called_once_and_its_late_answer_is_kept(pool: sqlx::PgPool) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let release = install_extension(
        &repository,
        store.clone(),
        "acme.slow",
        &archive("acme.slow", "field-notes-counted"),
        PERMISSIONS,
    )
    .await;
    let blueprint = published_blueprint(&repository, BLUEPRINT).await;
    let record = titled_record(&repository, blueprint, "Lamp").await;
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let ask = || async {
        authenticated_client()
            .post(format!("{base}/records/extension-field-notes"))
            .json(&json!({"record_ids": [record]}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };

    // The provider records its call in storage, which this lock holds up,
    // so the call outlasts the host's wait.
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE extension_storage_entries IN EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert_eq!(ask().await["incomplete"], true);
    // Asking again, as the web app does, waits for the running call instead
    // of starting another.
    for _ in 0..3 {
        let started = std::time::Instant::now();
        assert_eq!(ask().await["incomplete"], true);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
    lock.commit().await.unwrap();

    // The late answer is kept for the next request.
    let body = complete_notes(&base, record).await;
    assert_eq!(
        body["records"][record.to_string()][0]["attributes"]["title"],
        json!({"note": "Counted"}),
        "{body}"
    );
    let calls = repository
        .extension_storage_list("acme.slow", release, Some("call-"), None, 100)
        .await
        .unwrap()
        .entries;
    assert_eq!(
        calls.len(),
        1,
        "the provider was called {} times",
        calls.len()
    );
    server.abort();
}
