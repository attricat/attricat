mod support;

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use api::{
    extension_installer::{ExtensionInstaller, installed_artifact_key},
    model::{CreateBlueprint, NewAttributeValue},
    repository::{
        CatalogRepository, ExtensionCatalogBatch, ExtensionCatalogIntent,
        ExtensionCatalogIntentStatus,
    },
    storage::{FakeObjectStore, ObjectStore},
};
use async_trait::async_trait;
use support::{Value, authenticated_client, json, start_server_with_object_store};
use uuid::Uuid;

const ARTIFACT_BYTES: &[u8] = b"server bytes";
const SYNC_BLUEPRINT: &str = r#"
format_version = 1
code = "extension_sync_item"
name = "Extension sync item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["external_id"]
[[attributes]]
code = "external_id"
value_type = "string"
[[attributes]]
code = "title"
value_type = "string"
"#;

#[derive(Clone, Copy)]
enum ArtifactUploadFault {
    FailUpload,
    DeleteMetadataBeforeCompletion,
}

struct FaultingArtifactStore {
    inner: Arc<FakeObjectStore>,
    pool: sqlx::PgPool,
    fault: tokio::sync::Mutex<ArtifactUploadFault>,
}

impl FaultingArtifactStore {
    fn new(inner: Arc<FakeObjectStore>, pool: sqlx::PgPool, fault: ArtifactUploadFault) -> Self {
        Self {
            inner,
            pool,
            fault: tokio::sync::Mutex::new(fault),
        }
    }

    async fn set_fault(&self, fault: ArtifactUploadFault) {
        *self.fault.lock().await = fault;
    }
}

#[async_trait]
impl ObjectStore for FaultingArtifactStore {
    async fn put(
        &self,
        key: &str,
        object: api::storage::StoredObject,
    ) -> Result<(), api::storage::ObjectStoreError> {
        self.inner.put(key, object).await
    }
    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), api::storage::ObjectStoreError> {
        match *self.fault.lock().await {
            ArtifactUploadFault::FailUpload => {
                Err(api::storage::ObjectStoreError::Operation("fault"))
            }
            ArtifactUploadFault::DeleteMetadataBeforeCompletion => {
                self.inner.put_file(key, path, content_type).await?;
                let id = key
                    .rsplit('/')
                    .next()
                    .and_then(|id| id.parse::<Uuid>().ok())
                    .expect("output key ends in artifact UUID");
                sqlx::query("DELETE FROM extension_operation_artifacts WHERE id=$1")
                    .bind(id)
                    .execute(&self.pool)
                    .await
                    .map_err(|_| api::storage::ObjectStoreError::Operation("fault"))?;
                Ok(())
            }
        }
    }
    async fn get(
        &self,
        key: &str,
    ) -> Result<api::storage::StoredObject, api::storage::ObjectStoreError> {
        self.inner.get(key).await
    }
    async fn get_stream(
        &self,
        key: &str,
    ) -> Result<api::storage::StoredObjectStream, api::storage::ObjectStoreError> {
        self.inner.get_stream(key).await
    }
    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<api::storage::StoredObject, api::storage::ObjectStoreError> {
        self.inner.get_range(key, range).await
    }
    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<api::storage::StoredObjectStream, api::storage::ObjectStoreError> {
        self.inner.get_range_stream(key, range).await
    }
    async fn delete(&self, key: &str) -> Result<(), api::storage::ObjectStoreError> {
        self.inner.delete(key).await
    }
    async fn readiness(&self) -> Result<(), api::storage::ObjectStoreError> {
        self.inner.readiness().await
    }
}

fn release_archive(version: &str, dependencies: Value, artifact_bytes: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Acme extension",
        "version": version,
        "description": "extension integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": "acme.extension", "host_api": "^1.0"},
        "permissions": ["network.request"],
        "host_permissions": [{
            "id": "api-read",
            "matches": ["https://api.acme.example/v1/*"],
            "methods": ["GET"]
        }],
        "artifacts": [{
            "id": "server",
            "kind": "server_wasm",
            "path": "server.wasm"
        }],
        "configuration": {
            "version": 1,
            "schema": {"type": "object", "required": ["endpoint"], "properties": {"endpoint": {"type": "string"}}, "additionalProperties": false}
        },
        "dependencies": dependencies
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", artifact_bytes);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn artifact_stream_component() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("API crate is below workspace root")
        .to_owned();
    assert!(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "build",
                "-p",
                "catalog-artifact-test-component",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .expect("cargo must be available for the component fixture")
            .success()
    );
    let core =
        root.join("target/wasm32-unknown-unknown/release/catalog_artifact_test_component.wasm");
    let component = root.join("target/artifact-stream-test.component.wasm");
    assert!(
        Command::new("wasm-tools")
            .args(["component", "new"])
            .arg(core)
            .args(["-o"])
            .arg(&component)
            .status()
            .expect("wasm-tools must be available for the component fixture")
            .success()
    );
    std::fs::read(component).expect("component fixture must be readable")
}

fn transfer_test_component() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_owned();
    assert!(
        Command::new("cargo")
            .current_dir(&root)
            .args([
                "build",
                "-p",
                "catalog-extension-transfer-test-component",
                "--target",
                "wasm32-unknown-unknown",
                "--release",
            ])
            .status()
            .unwrap()
            .success()
    );
    let core = root.join(
        "target/wasm32-unknown-unknown/release/catalog_extension_transfer_test_component.wasm",
    );
    let component = root.join("target/transfer-test.component.wasm");
    assert!(
        Command::new("wasm-tools")
            .args(["component", "new"])
            .arg(core)
            .args(["-o"])
            .arg(&component)
            .status()
            .unwrap()
            .success()
    );
    std::fs::read(component).unwrap()
}

fn transfer_test_archive(component: &[u8]) -> Vec<u8> {
    let operations: Vec<Value> = ["probe", "redirect", "fetch", "deliver"]
        .iter().map(|id| json!({"id":id,"handler":id,"request_schema":{"type":"object"},"max_request_bytes":1024,"max_checkpoint_bytes":1024}))
        .collect();
    let manifest = serde_json::to_vec(&json!({
        "manifest_version":1,"name":"Transfer probe","version":"1.0.0",
        "description":"v1.4 real host transfer policy fixture","icons":{"48":"icon.png"},
        "catalog":{"id":"acme.transfer-probe","host_api":">=1.4.0, <2.0.0"},
        "permissions":["network.request","artifacts.read","artifacts.write"],
        "host_permissions":[
            {"id":"api","matches":["https://api.example.com/v1/*"],"methods":["GET"],"max_transfer_bytes":16777216},
            {"id":"redirect","matches":["https://httpbin.org/redirect/*"],"methods":["GET"],"max_transfer_bytes":16777216,"timeout_ms":30000},
            {"id":"source","matches":["https://httpbin.org/bytes/*"],"methods":["GET"],"max_transfer_bytes":16777216,"timeout_ms":30000},
            {"id":"target","matches":["https://httpbin.org/*"],"methods":["PUT"],"max_transfer_bytes":16777216,"timeout_ms":30000}
        ],
        "artifacts":[{"id":"server","kind":"server_wasm","path":"server.wasm"}],
        "configuration":{"version":1,"schema":{"type":"object","additionalProperties":false}},
        "server":{"operations":operations}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", component);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn artifact_operation_release_archive(extension_id: &str, component: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Artifact operation extension",
        "version": "1.0.0",
        "description": "artifact WIT runtime integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": ">=1.3.0, <2.0.0"},
        "permissions": ["artifacts.read", "artifacts.write"],
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "server": {"operations": [{
            "id": "copy", "handler": "copy", "request_schema": {"type": "object"},
            "max_request_bytes": 1024, "max_checkpoint_bytes": 1024
        }]}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", component);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn operation_release_archive(extension_id: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Operation extension",
        "version": "1.0.0",
        "description": "durable operation integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": ">=1.2.0, <2.0.0"},
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "server": {"operations": [{
            "id": "import", "handler": "import", "request_schema": {"type": "object"},
            "max_request_bytes": 1024, "max_checkpoint_bytes": 1024
        }]}
    })).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", ARTIFACT_BYTES);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn event_contract_release_archive(extension_id: &str, producer: bool) -> Vec<u8> {
    let contracts = if producer {
        json!({"exports": [{"id": "inventory", "version": "1.0.0", "event_type": "plugin.acme.producer.inventory_changed.v1", "schema": {"type": "object"}, "max_payload_bytes": 1024}], "consumes": []})
    } else {
        json!({"exports": [], "consumes": [{"provider": "acme.producer", "contract": "inventory", "version": "^1.0"}]})
    };
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Event contract extension",
        "version": "1.0.0",
        "description": "extension event contract integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": "^1.0"},
        "permissions": if producer { json!(["events.emit"]) } else { json!(["events.subscribe"]) },
        "event_contracts": contracts,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": if producer { json!({"event_handlers": []}) } else { json!({"event_handlers": [{"id": "consume-inventory", "event_types": ["plugin.acme.producer.inventory_changed.v1"], "handler": "handle-event"}]})}
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "server.wasm", ARTIFACT_BYTES);
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn storage_client_release_archive(version: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Storage client extension",
        "version": version,
        "description": "extension storage integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": "acme.storage", "host_api": "^1.0"},
        "permissions": ["storage.extension"],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "client", "kind": "client_component", "path": "client.js"}],
        "ui": [{"id": "panel", "version": 1, "kind": "embedded", "artifact": "client", "outlet": "entity_preview_panel"}]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "client.js", b"export {}");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn client_release_archive_for_outlet(
    extension_id: &str,
    contribution_id: &str,
    outlet: &str,
) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Client extension",
        "version": "1.0.0",
        "description": "client runtime integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": "^1.0"},
        "permissions": ["configuration.read"],
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "attribute_types": [{
            "id": "money", "version": "1.0.0", "primitive": "number",
            "value_schema": {"minimum": 0},
            "configuration_schema": {"type": "object", "properties": {"currency": {"type": "string"}}, "required": ["currency"]}
        }],
        "artifacts": [{
            "id": "client",
            "kind": "client_component",
            "path": "client.js"
        }],
        "ui": [{
            "id": contribution_id,
            "version": 1,
            "kind": "embedded",
            "artifact": "client",
            "outlet": outlet
        }]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "client.js", b"export {}");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn client_release_archive(extension_id: &str) -> Vec<u8> {
    client_release_archive_for_outlet(extension_id, "panel", "entity_preview_panel")
}

fn client_release_archive_with_navigation(extension_id: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Navigable client extension",
        "version": "1.0.0",
        "description": "client runtime integration test",
        "icons": {"48": "icon.png"},
        "catalog": {"id": extension_id, "host_api": "^1.0"},
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "client", "kind": "client_component", "path": "client.js"}],
        "ui": [
            {"id": "workbench", "version": 1, "kind": "route", "artifact": "client", "title": "Workbench"},
            {"id": "workbench-nav", "version": 1, "kind": "navigation", "route": "workbench", "title": "Workbench"}
        ]
    }))
    .unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut tar_bytes);
        append_file(&mut tar, "manifest.json", &manifest);
        append_file(&mut tar, "client.js", b"export {}");
        tar.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn append_file(tar: &mut tar::Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes).unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_catalog_upsert_is_idempotent_and_emits_a_change_feed(pool: sqlx::PgPool) {
    let repository = CatalogRepository::system(pool)
        .for_workspace(Uuid::from_u128(0x00000000000040008000000000000002))
        .await
        .unwrap()
        .for_extension("acme.sync");
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: SYNC_BLUEPRINT.into(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, blueprint.blueprint.version)
        .await
        .unwrap();
    let attributes = &blueprint.attributes;
    let external_id = attributes
        .iter()
        .find(|attribute| attribute.code == "external_id")
        .unwrap()
        .id;
    let batch = |key: &str, title: &str| ExtensionCatalogBatch {
        batch_key: key.into(),
        dry_run: false,
        intents: vec![ExtensionCatalogIntent::Upsert {
            intent_key: "item-1".into(),
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: blueprint.blueprint.version,
            lookup_attribute_id: external_id,
            lookup_value: "external-1".into(),
            relationships: vec![],
            system_tags: vec![],
            system_metadata: json!({}),
            values: vec![
                NewAttributeValue::Scalar {
                    attribute_id: Some(external_id),
                    attribute_code: None,
                    context_id: None,
                    value: json!("external-1"),
                },
                NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: Some("title".into()),
                    context_id: None,
                    value: json!(title),
                },
            ],
        }],
    };
    let created = repository
        .execute_extension_catalog_batch(batch("batch-1", "first"))
        .await
        .unwrap();
    assert_eq!(
        created[0].status,
        ExtensionCatalogIntentStatus::Applied,
        "{:?}",
        created[0].error
    );
    let entity_id = created[0].entity_id.unwrap();
    let replay = repository
        .execute_extension_catalog_batch(batch("batch-1", "first"))
        .await
        .unwrap();
    assert_eq!(
        replay[0].status,
        ExtensionCatalogIntentStatus::AlreadyApplied
    );
    assert_eq!(replay[0].entity_id, Some(entity_id));
    let updated = repository
        .execute_extension_catalog_batch(batch("batch-2", "second"))
        .await
        .unwrap();
    assert_eq!(updated[0].entity_id, Some(entity_id));

    let entity_version = repository
        .get_entity(entity_id)
        .await
        .unwrap()
        .unwrap()
        .blueprint_version;
    let snapshot = repository
        .extension_catalog_page(api::repository::ExtensionCatalogPageRequest {
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: entity_version,
            context_id: None,
            publication_context_id: None,
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(snapshot.entities.len(), 1);
    let changes = repository
        .extension_catalog_changes(blueprint.blueprint.id, entity_version, None, 1)
        .await
        .unwrap();
    assert_eq!(changes.events.len(), 1);
    assert_eq!(changes.events[0].aggregate_id, entity_id);
    let cursor = changes
        .next_cursor
        .expect("the initial create and update are paged");
    repository
        .execute_extension_catalog_batch(batch("batch-3", "third"))
        .await
        .unwrap();
    let stable_tail = repository
        .extension_catalog_changes(blueprint.blueprint.id, entity_version, Some(cursor), 1)
        .await
        .unwrap();
    assert_eq!(stable_tail.events.len(), 1);
    assert!(
        stable_tail.next_cursor.is_none(),
        "new events cannot enter an established cursor"
    );
    assert_eq!(
        repository
            .extension_catalog_changes(blueprint.blueprint.id, entity_version, None, 10)
            .await
            .unwrap()
            .events
            .len(),
        3
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sideload_installs_a_validated_local_archive(pool: sqlx::PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool, store.clone()).await;
    let response = authenticated_client()
        .post(format!("{base}/extensions/sideload"))
        .header("content-type", "application/zstd")
        .body(release_archive("1.0.0", json!([]), ARTIFACT_BYTES))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let installation: Value = response.json().await.unwrap();
    assert_eq!(installation["extension_id"], "acme.extension");
    assert_eq!(installation["state"], "disabled");
    assert_eq!(store.object_count().await, 1);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn enabled_event_types_follow_authorized_consumption_contracts(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    installer
        .install(
            "test",
            &event_contract_release_archive("acme.producer", true),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.producer", "capability", "events.emit")
        .await
        .unwrap();
    repository
        .grant_extension("acme.producer", "event_publish", "inventory")
        .await
        .unwrap();
    let producer = repository.enable_extension("acme.producer").await.unwrap();

    installer
        .install(
            "test",
            &event_contract_release_archive("acme.consumer", false),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.consumer", "capability", "events.subscribe")
        .await
        .unwrap();
    repository
        .grant_extension(
            "acme.consumer",
            "event_subscribe",
            "acme.producer:inventory",
        )
        .await
        .unwrap();
    repository.enable_extension("acme.consumer").await.unwrap();

    let event_types = repository.enabled_extension_event_types().await.unwrap();
    assert_eq!(
        event_types,
        vec!["plugin.acme.producer.inventory_changed.v1"]
    );
    repository
        .emit_extension_event(
            "acme.producer",
            producer.installed_release_id,
            "inventory",
            "inventory_item",
            Uuid::new_v4(),
            json!({}),
        )
        .await
        .unwrap();
    repository
        .ensure_event_consumer("catalog.extensions.wasm", &[])
        .await
        .unwrap();
    repository
        .materialize_event_delivery_tasks("catalog.extensions.wasm", &event_types)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_deliveries")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    repository.disable_extension("acme.consumer").await.unwrap();
    assert!(
        repository
            .enabled_extension_event_types()
            .await
            .unwrap()
            .is_empty()
    );
    repository
        .revoke_extension_grant(
            "acme.consumer",
            "event_subscribe",
            "acme.producer:inventory",
        )
        .await
        .unwrap();
    assert!(
        repository
            .enabled_extension_event_types()
            .await
            .unwrap()
            .is_empty()
    );
    repository
        .grant_extension(
            "acme.consumer",
            "event_subscribe",
            "acme.producer:inventory",
        )
        .await
        .unwrap();
    repository.enable_extension("acme.consumer").await.unwrap();
    repository
        .quarantine_extension("acme.consumer", "test")
        .await
        .unwrap();
    assert!(
        repository
            .enabled_extension_event_types()
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn navigation_contributions_target_same_release_routes(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    installer
        .install(
            "test",
            &client_release_archive_with_navigation("acme.navigation"),
        )
        .await
        .unwrap();
    repository
        .enable_extension("acme.navigation")
        .await
        .unwrap();

    let contributions = repository.client_extension_contributions().await.unwrap();
    let navigation = contributions
        .iter()
        .find(|item| item.id == "workbench-nav")
        .unwrap();
    assert_eq!(navigation.route.as_deref(), Some("workbench"));
    assert_eq!(
        navigation.outlet,
        Some(api::extensions::UiOutlet::Navigation)
    );
    assert_eq!(navigation.navigation_group.as_deref(), Some("grouped"));
    assert_eq!(navigation.artifact_key, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn enabled_client_contributions_are_hidden_after_state_changes(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));

    installer
        .install(
            "github:acme/client@v1.0.0",
            &client_release_archive("acme.client"),
        )
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );

    repository
        .grant_extension("acme.client", "capability", "configuration.read")
        .await
        .unwrap();
    repository.enable_extension("acme.client").await.unwrap();
    let contributions = repository.client_extension_contributions().await.unwrap();
    assert_eq!(contributions.len(), 1);
    assert_eq!(contributions[0].id, "panel");
    assert_eq!(
        contributions[0].artifact_key,
        Some(format!(
            "extensions/v1/{}/client",
            contributions[0].installed_release_id
        ))
    );
    assert_eq!(contributions[0].contribution_key, "acme.client:panel");

    // Layout hides are evaluated after the enabled/grant runtime gate. The
    // stored key survives removal so toggling visibility restores the same
    // contribution without an installation-time ordering contract.
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": [],
                    "hidden": ["acme.client:panel"]
                }
            }
        }))
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": { "order": [], "hidden": [] }
            }
        }))
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .len(),
        1
    );

    repository.disable_extension("acme.client").await.unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    repository.enable_extension("acme.client").await.unwrap();
    assert_eq!(
        repository.client_extension_contributions().await.unwrap()[0].contribution_key,
        "acme.client:panel"
    );
    repository
        .quarantine_extension("acme.client", "test")
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_layout_replacement_rejects_non_object_roots_and_repairs_malformed_layout(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::new(pool.clone(), workspace);
    for malformed in [json!("scalar"), json!([])] {
        sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
            .bind(workspace)
            .bind(malformed.clone())
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            repository
                .update_workspace_extension_layout(json!({"version":1,"outlets":{}}))
                .await
                .is_err()
        );
        let stored: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, malformed);
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id=$1 AND action='workspace.extension_layout.set'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);

    sqlx::query("UPDATE workspaces SET settings=$2 WHERE id=$1")
        .bind(workspace)
        .bind(json!({"theme":"dark","extension_layout":{"malformed":true}}))
        .execute(&pool)
        .await
        .unwrap();
    let replacement = json!({
        "version":1,
        "outlets": {
            "navigation": {"order":[],"hidden":[],"promoted":[]}
        }
    });
    repository
        .update_workspace_extension_layout(replacement.clone())
        .await
        .unwrap();
    let repaired: Value = sqlx::query_scalar("SELECT settings FROM workspaces WHERE id=$1")
        .bind(workspace)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(repaired["theme"], "dark");
    assert_eq!(repaired["extension_layout"], replacement);
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id=$1 AND action='workspace.extension_layout.set'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_layout_order_is_stable_and_host_owned(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    for extension_id in ["acme.zebra", "acme.alpha"] {
        installer
            .install("test", &client_release_archive(extension_id))
            .await
            .unwrap();
        repository
            .grant_extension(extension_id, "capability", "configuration.read")
            .await
            .unwrap();
        repository.enable_extension(extension_id).await.unwrap();
    }
    let keys = || async {
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.contribution_key)
            .collect::<Vec<_>>()
    };
    assert_eq!(keys().await, ["acme.alpha:panel", "acme.zebra:panel"]);
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.zebra:panel"],
                    "hidden": []
                }
            }
        }))
        .await
        .unwrap();
    assert_eq!(keys().await, ["acme.zebra:panel", "acme.alpha:panel"]);
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND action = 'workspace.extension_layout.set' AND metadata ? 'extension_layout'",
    )
    .bind(workspace)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    for invalid in [
        json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.alpha:panel"],
                    "hidden": ["acme.alpha:panel"]
                }
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {"order": [], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "entity_action": {"order": ["malformed"], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "entity_action": {"order": ["acme.shared:item"], "hidden": []},
                "entity_preview_panel": {"order": ["acme.shared:item"], "hidden": []}
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": [],
                    "hidden": [],
                    "promoted": ["acme.shared:item"]
                }
            }
        }),
        json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": [],
                    "hidden": [],
                    "promoted": ["acme.shared:item"]
                },
                "entity_action": {
                    "order": ["acme.shared:item"],
                    "hidden": []
                }
            }
        }),
    ] {
        assert!(
            repository
                .update_workspace_extension_layout(invalid)
                .await
                .is_err()
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn blueprint_layout_overlays_owned_outlets_and_preserves_global_workspace_rules(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    for extension_id in ["acme.alpha", "acme.zebra"] {
        installer
            .install("test", &client_release_archive(extension_id))
            .await
            .unwrap();
        repository
            .grant_extension(extension_id, "capability", "configuration.read")
            .await
            .unwrap();
        repository.enable_extension(extension_id).await.unwrap();
    }
    installer
        .install(
            "test",
            &client_release_archive_for_outlet("acme.navigation", "entry", "navigation"),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.navigation", "capability", "configuration.read")
        .await
        .unwrap();
    repository
        .enable_extension("acme.navigation")
        .await
        .unwrap();
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": ["acme.navigation:entry"],
                    "hidden": [],
                    "promoted": ["acme.navigation:entry"]
                }
            }
        }))
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.contribution_key == "acme.navigation:entry")
            .unwrap()
            .navigation_group
            .as_deref(),
        Some("promoted")
    );
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": ["acme.alpha:panel", "acme.zebra:panel"],
                    "hidden": []
                },
                "navigation": {
                    "order": [],
                    "hidden": ["acme.navigation:entry"],
                    "promoted": []
                }
            }
        }))
        .await
        .unwrap();
    let definition = r#"
format_version = 1
code = "extension_layout_product"
name = "Extension layout product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.zebra:panel"]
hidden = ["acme.alpha:panel"]

[[attributes]]
code = "title"
value_type = "string"
"#;
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap();

    let scoped = repository
        .client_extension_contributions_for_blueprint(Some((blueprint.blueprint.id, 1)))
        .await
        .unwrap();
    assert_eq!(
        scoped
            .iter()
            .map(|item| item.contribution_key.as_str())
            .collect::<Vec<_>>(),
        ["acme.zebra:panel"]
    );
    assert!(
        scoped
            .iter()
            .all(|item| item.contribution_key != "acme.navigation:entry")
    );
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.contribution_key)
            .collect::<Vec<_>>(),
        ["acme.alpha:panel", "acme.zebra:panel"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn operation_runs_keep_a_batch_key_across_crash_reclaim_and_fence_stale_checkpoints(
    pool: sqlx::PgPool,
) {
    use api::repository::StartExtensionOperation;
    use catalog_domain::task_queue::TaskKind;
    use std::time::Duration;

    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()))
        .install("test", &operation_release_archive("acme.operations"))
        .await
        .unwrap();
    repository
        .enable_extension("acme.operations")
        .await
        .unwrap();
    let request = StartExtensionOperation {
        extension_id: "acme.operations".into(),
        expected_release_id: repository
            .installed_extension("acme.operations")
            .await
            .unwrap()
            .installed_release_id,
        operation_id: "import".into(),
        input: json!({}),
        source_reference: json!({"token": "hidden"}),
        destination_reference: json!({}),
        idempotency_key: "same-request".into(),
        schedule_id: None,

        configuration_snapshot: None,
    };
    let run_id = repository
        .start_extension_operation(request.clone())
        .await
        .unwrap();
    assert_eq!(
        run_id,
        repository.start_extension_operation(request).await.unwrap()
    );

    let first = repository
        .claim_task_for_kinds(
            "first",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let first_run = repository
        .begin_extension_operation_task(&first)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_run.batch_key, "same-request:0");
    // Simulate a process crash before its checkpoint transaction. Reclaiming
    // must reissue the same idempotency key so a prior domain commit is safe.
    sqlx::query("UPDATE tasks SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let second = repository
        .claim_task_for_kinds(
            "second",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let second_run = repository
        .begin_extension_operation_task(&second)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_run.batch_key, first_run.batch_key);
    repository
        .checkpoint_extension_operation_task(
            &second,
            &second_run,
            // `{}` is a valid durable extension checkpoint. Lifecycle state
            // must be explicit rather than inferring that this is unstarted.
            json!({}),
            json!({"written": 1}),
            false,
        )
        .await
        .unwrap();
    assert!(
        repository
            .checkpoint_extension_operation_task(
                &first,
                &first_run,
                json!({"cursor": 99}),
                json!({}),
                false
            )
            .await
            .is_err()
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT lifecycle_started FROM extension_operation_runs WHERE id=$1"
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    );

    // A cancellation which races a normal checkpoint cannot make the run
    // terminal until the worker has durably recorded the WIT cancel callback.
    // The first checkpoint returns it to pending rather than silently marking
    // it cancelled; the next lease records delivery before terminalizing it.
    repository
        .reschedule_task_at(
            second.id,
            &second.lease_owner,
            second.lease_token,
            // Keep the immediate-reclaim assertion independent of small host
            // and database-container clock skew.
            chrono::Utc::now() - chrono::Duration::seconds(1),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE extension_operation_runs SET cancellation_requested=true WHERE id=$1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let third = repository
        .claim_task_for_kinds(
            "third",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let third_run = repository
        .begin_extension_operation_task(&third)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !repository
            .checkpoint_extension_operation_task(&third, &third_run, json!({}), json!({}), true)
            .await
            .unwrap()
    );
    repository
        .reschedule_task_at(
            third.id,
            &third.lease_owner,
            third.lease_token,
            chrono::Utc::now() - chrono::Duration::seconds(1),
        )
        .await
        .unwrap();
    let fourth = repository
        .claim_task_for_kinds(
            "fourth",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let fourth_run = repository
        .begin_extension_operation_task(&fourth)
        .await
        .unwrap()
        .unwrap();
    repository
        .mark_extension_operation_cancellation_delivered(&fourth)
        .await
        .unwrap();
    assert!(
        repository
            .checkpoint_extension_operation_task(&fourth, &fourth_run, json!({}), json!({}), true)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM extension_operation_runs WHERE id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "cancelled"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn operation_http_routes_start_list_and_cancel_without_exposing_input(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()))
        .install("test", &operation_release_archive("acme.operation-http"))
        .await
        .unwrap();
    repository
        .enable_extension("acme.operation-http")
        .await
        .unwrap();

    let (base, server) =
        start_server_with_object_store(pool, Arc::new(FakeObjectStore::available())).await;
    let response = authenticated_client()
        .post(format!("{base}/extensions/acme.operation-http/operations"))
        .json(&json!({
            "operation_id": "import",
            "input": {"safe": "value", "token": "not-returned"},
            "source_reference": {"token": "redacted"},
            "destination_reference": {},
            "idempotency_key": "operation-http-1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    let id = response.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let response = authenticated_client()
        .get(format!("{base}/extension-operation-runs"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let runs = response.json::<Value>().await.unwrap();
    assert_eq!(runs[0]["id"].as_str(), Some(id.as_str()));
    assert!(runs[0].get("input").is_none());
    assert!(runs[0].get("checkpoint").is_none());

    let response = authenticated_client()
        .post(format!("{base}/extension-operation-runs/{id}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_attribute_types_are_pinned_and_survive_provider_disable(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    installer
        .install("test", &client_release_archive("acme.client"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.client", "capability", "configuration.read")
        .await
        .unwrap();
    repository.enable_extension("acme.client").await.unwrap();
    let definition = r#"
format_version = 1
code = "extension_type_product"
name = "Extension type product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["price"]

[[attributes]]
code = "price"
extension_type = "acme.client:money@^1"
extension_configuration = '{"currency":"USD"}'
"#;
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.to_owned(),
        })
        .await
        .unwrap();
    let attribute = &blueprint.attributes[0];
    assert_eq!(attribute.value_type, "number");
    assert_eq!(attribute.value_schema.as_ref().unwrap()["minimum"], 0);
    assert_eq!(
        attribute.extension_type.as_ref().unwrap()["provider"],
        "acme.client"
    );
    assert_eq!(
        attribute.extension_type.as_ref().unwrap()["primitive"],
        "number"
    );
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap();
    repository.disable_extension("acme.client").await.unwrap();
    let stored = repository
        .get_blueprint_revision(blueprint.blueprint.id, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.attributes[0].value_type, "number");
    assert_eq!(
        stored.attributes[0].extension_type.as_ref().unwrap()["type"],
        "money"
    );
    assert_eq!(
        stored.attributes[0].extension_type.as_ref().unwrap()["available"],
        false
    );
    let unavailable = repository
        .create_blueprint(CreateBlueprint {
            definition: definition.replace("extension_type_product", "unavailable_extension_type"),
        })
        .await;
    assert!(unavailable.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn hidden_contributions_remain_authorized_and_are_validated_on_publish(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    installer
        .install("test", &client_release_archive("acme.client"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.client", "capability", "configuration.read")
        .await
        .unwrap();
    repository.enable_extension("acme.client").await.unwrap();
    repository
        .update_workspace_extension_layout(json!({
            "version": 1,
            "outlets": {
                "entity_preview_panel": {
                    "order": [],
                    "hidden": ["acme.client:panel"]
                }
            }
        }))
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    let contribution = repository
        .client_extension_contribution("acme.client", "panel")
        .await
        .unwrap();
    assert!(
        contribution
            .artifact_key
            .as_deref()
            .is_some_and(|key| key.ends_with("/client"))
    );

    let incompatible = r#"
format_version = 1
code = "invalid_extension_layout"
name = "Invalid extension layout"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_action]
order = ["acme.client:panel"]
hidden = []

[[attributes]]
code = "title"
value_type = "string"
"#;
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: incompatible.to_owned(),
        })
        .await
        .unwrap();
    assert!(
        repository
            .publish_blueprint_revision(blueprint.blueprint.id, 1)
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn workspace_safe_mode_blocks_runtime_descriptors_and_storage_without_mutating_installations(
    pool: sqlx::PgPool,
) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();

    repository
        .set_workspace_extensions_enabled(false)
        .await
        .unwrap();
    assert!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repository
            .runtime_extension_installation("acme.storage", installed.installed_release_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await
            .is_err()
    );
    assert_eq!(
        repository
            .installed_extension("acme.storage")
            .await
            .unwrap()
            .state,
        "enabled"
    );

    repository
        .set_workspace_extensions_enabled(true)
        .await
        .unwrap();
    assert_eq!(
        repository
            .client_extension_contributions()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_storage_enforces_cas_bounds_quota_and_workspace_namespace(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();

    let first = repository
        .extension_storage_set(
            "acme.storage",
            installed.installed_release_id,
            "state",
            json!({"enabled": true}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first, 1);
    assert_eq!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await
            .unwrap()
            .unwrap()
            .value,
        json!({"enabled": true})
    );
    assert_eq!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "state",
                json!(2),
                Some(first)
            )
            .await
            .unwrap(),
        2
    );
    assert!(matches!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "state",
                json!(3),
                Some(first)
            )
            .await,
        Err(api::repository::ExtensionStorageError::Conflict)
    ));
    assert!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                &"x".repeat(257),
                json!(null),
                None
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "large",
                json!("x".repeat(65_535)),
                None
            )
            .await
            .is_err()
    );

    let quota_value = json!("x".repeat(65_534));
    for index in 0..79 {
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                &format!("quota-{index}"),
                quota_value.clone(),
                None,
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                "quota-overflow",
                quota_value,
                None
            )
            .await,
        Err(api::repository::ExtensionStorageError::QuotaExceeded)
    ));
    let page = repository
        .extension_storage_list(
            "acme.storage",
            installed.installed_release_id,
            Some("quota-"),
            None,
            1,
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(page.cursor.is_some());
    repository.disable_extension("acme.storage").await.unwrap();
    assert!(matches!(
        repository
            .extension_storage_get("acme.storage", installed.installed_release_id, "state")
            .await,
        Err(api::repository::ExtensionStorageError::Denied)
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn extension_storage_list_treats_like_characters_as_literal_prefixes(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000009);
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'extension-prefix', 'Extension prefix', 'extension-prefix.local')")
        .bind(workspace)
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let installer =
        ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()));
    let installed = installer
        .install("test", &storage_client_release_archive("1.0.0"))
        .await
        .unwrap();
    repository
        .grant_extension("acme.storage", "capability", "storage.extension")
        .await
        .unwrap();
    repository.enable_extension("acme.storage").await.unwrap();
    for key in ["literal%key", "literal_key", "literalXkey"] {
        repository
            .extension_storage_set(
                "acme.storage",
                installed.installed_release_id,
                key,
                json!(true),
                None,
            )
            .await
            .unwrap();
    }

    let page = repository
        .extension_storage_list(
            "acme.storage",
            installed.installed_release_id,
            Some("literal%"),
            None,
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        page.entries
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>(),
        vec!["literal%key"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn lifecycle_installs_validated_archive_artifacts_and_retains_history(pool: sqlx::PgPool) {
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let installer = ExtensionInstaller::new(repository.clone(), store.clone());

    assert!(
        installer
            .install("github:acme/extension@v1.0.0", b"not a zstd archive")
            .await
            .is_err()
    );
    assert_eq!(store.object_count().await, 0);

    let installed = installer
        .install(
            "github:acme/extension@v1.0.0",
            &release_archive("1.0.0", json!([]), ARTIFACT_BYTES),
        )
        .await
        .unwrap();
    assert_eq!(installed.state, "disabled");
    assert_eq!(
        store
            .get(&installed_artifact_key(
                installed.installed_release_id,
                "server"
            ))
            .await
            .unwrap()
            .bytes
            .as_ref(),
        ARTIFACT_BYTES
    );

    // A repository failure after S3 staging cleans up the newly staged object.
    assert!(
        installer
            .install(
                "github:acme/extension@v1.0.0",
                &release_archive("1.0.0", json!([]), ARTIFACT_BYTES),
            )
            .await
            .is_err()
    );
    assert_eq!(store.object_count().await, 1);

    assert!(repository.enable_extension("acme.extension").await.is_err());
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://api.acme.example"}),
        )
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "capability", "network.request")
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "host_permission", "api-read")
        .await
        .unwrap();
    assert_eq!(
        repository
            .enable_extension("acme.extension")
            .await
            .unwrap()
            .state,
        "enabled"
    );

    // A runtime candidate is not authorization: the invocation-time lookup
    // must observe configuration and release changes made after selection.
    let authorized = repository
        .runtime_extension_installation("acme.extension", installed.installed_release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        authorized.configuration["endpoint"],
        "https://api.acme.example"
    );
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://changed.acme.example"}),
        )
        .await
        .unwrap();
    let refreshed = repository
        .runtime_extension_installation("acme.extension", installed.installed_release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        refreshed.configuration["endpoint"],
        "https://changed.acme.example"
    );
    let upgraded = installer
        .upgrade(
            "github:acme/extension@v2.0.0",
            &release_archive("2.0.0", json!([]), ARTIFACT_BYTES),
        )
        .await
        .unwrap();
    assert_ne!(
        upgraded.installed_release_id,
        installed.installed_release_id
    );
    assert!(
        repository
            .runtime_extension_installation("acme.extension", installed.installed_release_id)
            .await
            .unwrap()
            .is_none()
    );
    repository
        .configure_extension(
            "acme.extension",
            json!({"endpoint": "https://upgraded.acme.example"}),
        )
        .await
        .unwrap();

    assert_eq!(
        repository
            .quarantine_extension("acme.extension", "timeout")
            .await
            .unwrap()
            .state,
        "quarantined"
    );
    repository
        .grant_extension("acme.extension", "capability", "network.request")
        .await
        .unwrap();
    repository
        .grant_extension("acme.extension", "host_permission", "api-read")
        .await
        .unwrap();
    assert_eq!(
        repository
            .enable_extension("acme.extension")
            .await
            .unwrap()
            .state,
        "enabled"
    );
    repository
        .disable_extension("acme.extension")
        .await
        .unwrap();
    repository.remove_extension("acme.extension").await.unwrap();

    let history = repository
        .extension_lifecycle_history("acme.extension")
        .await
        .unwrap();
    assert_eq!(history.last().unwrap().operation, "remove");
    assert!(
        history
            .iter()
            .any(|record| record.operation == "quarantine")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND target->>'type' = 'extension'",
        )
        .bind(workspace)
        .fetch_one(&pool)
        .await
        .unwrap(),
        history.len() as i64
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM extension_installations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn operation_artifacts_are_run_scoped_quota_bound_cleaned_and_downloadable(
    pool: sqlx::PgPool,
) {
    use api::{
        repository::{MAX_OPERATION_ARTIFACT_BYTES, StartExtensionOperation},
        storage::StoredObject,
    };
    use catalog_domain::task_queue::TaskKind;
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install(
            "test",
            &operation_release_archive("acme.artifact-operation"),
        )
        .await
        .unwrap();
    repository
        .enable_extension("acme.artifact-operation")
        .await
        .unwrap();
    let release_id = repository
        .installed_extension("acme.artifact-operation")
        .await
        .unwrap()
        .installed_release_id;

    // Durable operation creation, not component JSON, attaches the only
    // approved input source. The file key remains a server-only field.
    let file_id = Uuid::new_v4();
    // It is deliberately larger than the JSON/WIT chunk bound; only metadata
    // crosses operation creation and the component reads it incrementally.
    let input = vec![b'x'; 64 * 1024 + 1];
    sqlx::query("INSERT INTO files(id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status) VALUES($1,$2,'input.csv','input.csv','text/csv',$3,$4,'files/approved-input','ready')")
        .bind(file_id).bind(workspace).bind(input.len() as i64).bind(format!("{:x}", Sha256::digest(&input))).execute(&pool).await.unwrap();
    let run_id = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "acme.artifact-operation".into(),
            expected_release_id: release_id,
            operation_id: "import".into(),
            input: json!({}),
            source_reference: json!({"input_file_id": file_id}),
            destination_reference: json!({}),
            idempotency_key: "artifact-input".into(),
            schedule_id: None,

            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let claimed = repository
        .claim_task_for_kinds(
            "artifact-test",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .begin_extension_operation_task(&claimed)
        .await
        .unwrap()
        .unwrap();
    let input_artifact = repository
        .extension_operation_input_artifact(run_id, "acme.artifact-operation", release_id, "source")
        .await
        .unwrap();
    assert_eq!(input_artifact.content_length, input.len() as i64);
    assert!(input_artifact.content_length > 64 * 1024);
    assert_eq!(
        input_artifact.object_key.as_deref(),
        Some("files/approved-input")
    );
    // A worker restart reclaims the run and still resolves the durable input
    // attachment; it never relies on an in-memory handle or file key.
    sqlx::query("UPDATE tasks SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(claimed.id)
        .execute(&pool)
        .await
        .unwrap();
    let restarted = repository
        .claim_task_for_kinds(
            "artifact-restarted",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .begin_extension_operation_task(&restarted)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        repository
            .extension_operation_input_artifact(
                run_id,
                "acme.artifact-operation",
                release_id,
                "source"
            )
            .await
            .unwrap()
            .id,
        input_artifact.id
    );

    // A foreign workspace, random selector, and mismatched release cannot
    // reuse a valid opaque handle or metadata record.
    let foreign_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces(id,slug,name,login_identifier) VALUES($1,$2,'Foreign',$3)")
        .bind(foreign_workspace)
        .bind(format!(
            "foreign-{}",
            &foreign_workspace.simple().to_string()[..8]
        ))
        .bind(format!(
            "foreign-{}.local",
            &foreign_workspace.simple().to_string()[..8]
        ))
        .execute(&pool)
        .await
        .unwrap();
    let foreign = CatalogRepository::system(pool.clone())
        .for_workspace(foreign_workspace)
        .await
        .unwrap();
    assert!(
        foreign
            .extension_operation_input_artifact(
                run_id,
                "acme.artifact-operation",
                release_id,
                "source"
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .extension_operation_input_artifact(
                run_id,
                "acme.artifact-operation",
                release_id,
                &Uuid::new_v4().to_string()
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .create_extension_operation_output_artifact(
                run_id,
                "forged.extension",
                release_id,
                "text/csv"
            )
            .await
            .is_err()
    );

    let output = repository
        .create_extension_operation_output_artifact(
            run_id,
            "acme.artifact-operation",
            release_id,
            "text/csv",
        )
        .await
        .unwrap();
    // Quotas are enforced against persisted bytes rather than caller claims.
    sqlx::query("UPDATE extension_operation_artifacts SET content_length=$2 WHERE id=$1")
        .bind(output.id)
        .bind(MAX_OPERATION_ARTIFACT_BYTES)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .reserve_extension_operation_artifact_bytes(output.id, run_id, 1)
            .await
            .is_err()
    );
    assert!(
        repository
            .complete_extension_operation_artifact(
                output.id,
                run_id,
                "not-a-checksum",
                "private/key"
            )
            .await
            .is_err()
    );

    // Interrupted output is terminalized by cleanup, while completed output is
    // excluded even when old enough to be swept.
    assert!(
        repository
            .abort_extension_operation_artifact(output.id, run_id)
            .await
            .unwrap()
    );
    let completed = repository
        .create_extension_operation_output_artifact(
            run_id,
            "acme.artifact-operation",
            release_id,
            "text/csv",
        )
        .await
        .unwrap();
    let body = b"completed export";
    repository
        .reserve_extension_operation_artifact_bytes(completed.id, run_id, body.len() as i64)
        .await
        .unwrap();
    let checksum = format!("{:x}", Sha256::digest(body));
    let key = format!("extension-operation-artifacts/v1/{}", completed.id);
    store
        .put(
            &key,
            StoredObject {
                bytes: body.to_vec().into(),
                content_type: Some("text/csv".into()),
            },
        )
        .await
        .unwrap();
    repository
        .complete_extension_operation_artifact(completed.id, run_id, &checksum, &key)
        .await
        .unwrap();
    // Completion is immutable: neither a different checksum nor an object key
    // can replace a committed output.
    assert!(
        repository
            .complete_extension_operation_artifact(
                completed.id,
                run_id,
                &checksum,
                "replacement-key"
            )
            .await
            .is_err()
    );
    assert!(
        foreign
            .completed_extension_operation_artifact(run_id, completed.id)
            .await
            .is_err()
    );
    sqlx::query("UPDATE extension_operation_artifacts SET updated_at=clock_timestamp()-interval '2 hours' WHERE id=$1")
        .bind(completed.id).execute(&pool).await.unwrap();
    assert!(
        repository
            .abort_stale_extension_operation_artifacts()
            .await
            .unwrap()
            .is_empty()
    );

    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let response = authenticated_client()
        .get(format!(
            "{base}/extension-operation-runs/{run_id}/artifacts/{}/download",
            completed.id
        ))
        .send()
        .await
        .unwrap();
    // Finalized bytes remain invisible until the run itself completes.
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        authenticated_client()
            .get(format!(
                "{base}/extension-operation-runs/{run_id}/artifacts/{}/download",
                Uuid::new_v4()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );

    // Cancellation leaves a temporary output non-downloadable until the
    // resource/trap cleanup path aborts it; completed output above remains.
    let interrupted = repository
        .create_extension_operation_output_artifact(
            run_id,
            "acme.artifact-operation",
            release_id,
            "text/csv",
        )
        .await
        .unwrap();
    assert!(repository.cancel_extension_operation(run_id).await.unwrap());
    assert!(
        repository
            .abort_extension_operation_artifact(interrupted.id, run_id)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM extension_operation_artifacts WHERE id=$1"
        )
        .bind(interrupted.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "aborted"
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn artifact_wit_component_copies_a_large_approved_input_in_bounded_chunks(
    pool: sqlx::PgPool,
) {
    use api::{
        extension_runtime::{
            ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        },
        repository::StartExtensionOperation,
        storage::StoredObject,
        task_worker::TaskHandler,
    };
    use catalog_domain::task_queue::TaskKind;
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let component = artifact_stream_component();
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install(
            "test",
            &artifact_operation_release_archive("acme.artifact-wit", &component),
        )
        .await
        .unwrap();
    for capability in ["artifacts.read", "artifacts.write"] {
        repository
            .grant_extension("acme.artifact-wit", "capability", capability)
            .await
            .unwrap();
    }
    repository
        .enable_extension("acme.artifact-wit")
        .await
        .unwrap();
    let release_id = repository
        .installed_extension("acme.artifact-wit")
        .await
        .unwrap()
        .installed_release_id;

    // The guest calls read with 64 KiB, so this 128 KiB + 17-byte input requires
    // multiple bounded host calls and is never sent through operation JSON.
    let input = (0..(128 * 1024 + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let file_id = Uuid::new_v4();
    let source_key = "files/artifact-wit-input";
    store
        .put(
            source_key,
            StoredObject {
                bytes: input.clone().into(),
                content_type: Some("application/octet-stream".into()),
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO files(id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status) VALUES($1,$2,'large.bin','large.bin','application/octet-stream',$3,$4,$5,'ready')")
        .bind(file_id)
        .bind(workspace)
        .bind(input.len() as i64)
        .bind(format!("{:x}", Sha256::digest(&input)))
        .bind(source_key)
        .execute(&pool)
        .await
        .unwrap();
    let run_id = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "acme.artifact-wit".into(),
            expected_release_id: release_id,
            operation_id: "copy".into(),
            input: json!({}),
            source_reference: json!({"input_file_id": file_id}),
            destination_reference: json!({}),
            idempotency_key: "copy-large-input".into(),
            schedule_id: None,

            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let task = repository
        .claim_task_for_kinds(
            "artifact-wit-test",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    let handler = ExtensionOperationTaskHandler::new(repository.clone(), runtime);
    handler.handle(task).await.unwrap();

    let (artifact_id, key, checksum, length): (Uuid, String, String, i64) = sqlx::query_as(
        "SELECT id,object_key,checksum_sha256,content_length FROM extension_operation_artifacts WHERE operation_run_id=$1 AND direction='output' AND state='completed'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(length, input.len() as i64);
    assert_eq!(checksum, format!("{:x}", Sha256::digest(&input)));
    assert_eq!(
        store.get(&key).await.unwrap().bytes.as_ref(),
        input.as_slice()
    );
    assert!(
        repository
            .completed_extension_operation_artifact(run_id, artifact_id)
            .await
            .is_ok()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn packaged_v14_transfer_import_rejects_ssrf_without_network_io(pool: sqlx::PgPool) {
    use api::{
        extension_runtime::{
            ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        },
        repository::StartExtensionOperation,
        task_worker::TaskHandler,
    };
    use catalog_domain::task_queue::TaskKind;
    use std::time::Duration;
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(Uuid::from_u128(0x00000000000040008000000000000002))
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &transfer_test_archive(&transfer_test_component()))
        .await
        .unwrap();
    for cap in ["network.request", "artifacts.read", "artifacts.write"] {
        repository
            .grant_extension("acme.transfer-probe", "capability", cap)
            .await
            .unwrap();
    }
    for host in ["api", "redirect", "source", "target"] {
        repository
            .grant_extension("acme.transfer-probe", "host_permission", host)
            .await
            .unwrap();
    }
    repository
        .enable_extension("acme.transfer-probe")
        .await
        .unwrap();
    let release = repository
        .installed_extension("acme.transfer-probe")
        .await
        .unwrap()
        .installed_release_id;
    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "acme.transfer-probe".into(),
            expected_release_id: release,
            operation_id: "probe".into(),
            input: json!({}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "unsafe-url".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let task = repository
        .claim_task_for_kinds(
            "transfer-test",
            Duration::from_secs(60),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store, ExtensionRuntimeConfig::default()).unwrap();
    ExtensionOperationTaskHandler::new(repository.clone(), runtime)
        .handle(task)
        .await
        .unwrap();
    let (status, progress): (String, Value) =
        sqlx::query_as("SELECT status,progress FROM extension_operation_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(progress["unsafe_url_denied"], true);
}

// Opt in only when public HTTPS is reachable. No local/private address is
// allowed by the broker, so a loopback test server cannot exercise this path.
#[sqlx::test(migrations = "./migrations")]
async fn packaged_v14_public_http_transfer_and_redirect_policy(pool: sqlx::PgPool) {
    use api::{
        extension_runtime::{
            ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        },
        repository::StartExtensionOperation,
        task_worker::TaskHandler,
    };
    use catalog_domain::task_queue::TaskKind;
    use std::time::Duration;
    if std::env::var("ATTRICAT_PUBLIC_HTTP_TRANSFER_TEST").as_deref() != Ok("1") {
        return;
    }
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(Uuid::from_u128(0x00000000000040008000000000000002))
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &transfer_test_archive(&transfer_test_component()))
        .await
        .unwrap();
    for cap in ["network.request", "artifacts.read", "artifacts.write"] {
        repository
            .grant_extension("acme.transfer-probe", "capability", cap)
            .await
            .unwrap();
    }
    for host in ["api", "redirect", "source", "target"] {
        repository
            .grant_extension("acme.transfer-probe", "host_permission", host)
            .await
            .unwrap();
    }
    repository
        .enable_extension("acme.transfer-probe")
        .await
        .unwrap();
    let release = repository
        .installed_extension("acme.transfer-probe")
        .await
        .unwrap()
        .installed_release_id;
    for operation in ["redirect", "fetch", "deliver"] {
        let run = repository
            .start_extension_operation(StartExtensionOperation {
                extension_id: "acme.transfer-probe".into(),
                expected_release_id: release,
                operation_id: operation.into(),
                input: json!({}),
                source_reference: json!({}),
                destination_reference: json!({}),
                idempotency_key: format!("public-{operation}"),
                schedule_id: None,
                configuration_snapshot: None,
            })
            .await
            .unwrap();
        let task = repository
            .claim_task_for_kinds(
                "public-http",
                Duration::from_secs(60),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.subject_id, run);
        let runtime =
            ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
        ExtensionOperationTaskHandler::new(repository.clone(), runtime)
            .handle(task)
            .await
            .unwrap();
        let (status, progress): (String, Value) =
            sqlx::query_as("SELECT status,progress FROM extension_operation_runs WHERE id=$1")
                .bind(run)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "completed", "{operation}: {progress}");
        match operation {
            "redirect" => assert_eq!(progress["redirect_denied"], true),
            "fetch" => assert_eq!(progress["source_bytes"], 32),
            "deliver" => {
                let deliveries = repository
                    .list_extension_http_deliveries(run)
                    .await
                    .unwrap();
                assert_eq!(deliveries.len(), 1);
                assert_eq!(progress["delivery_outcome"], deliveries[0].state);
            }
            _ => unreachable!(),
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn delivery_attempt_is_uncertain_until_confirmed_and_never_auto_resends(pool: sqlx::PgPool) {
    use api::repository::{DeliveryState, StartExtensionOperation};
    use catalog_domain::task_queue::TaskKind;
    use std::time::Duration;
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    ExtensionInstaller::new(repository.clone(), Arc::new(FakeObjectStore::available()))
        .install("test", &operation_release_archive("acme.delivery"))
        .await
        .unwrap();
    repository.enable_extension("acme.delivery").await.unwrap();
    let release = repository
        .installed_extension("acme.delivery")
        .await
        .unwrap()
        .installed_release_id;
    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "acme.delivery".into(),
            expected_release_id: release,
            operation_id: "import".into(),
            input: json!({}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "delivery-test".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let task = repository
        .claim_task_for_kinds(
            "delivery",
            Duration::from_secs(30),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    repository
        .begin_extension_operation_task(&task)
        .await
        .unwrap();
    let scoped = repository.for_extension_operation_task(&task);
    let artifact = scoped
        .create_extension_operation_output_artifact(run, "acme.delivery", release, "text/plain")
        .await
        .unwrap();
    scoped
        .reserve_extension_operation_artifact_bytes(artifact.id, run, 3)
        .await
        .unwrap();
    scoped
        .complete_extension_operation_artifact(
            artifact.id,
            run,
            &"a".repeat(64),
            "extension-operation-artifacts/v1/test",
        )
        .await
        .unwrap();
    let digest = "b".repeat(64);
    let attempt = scoped
        .begin_extension_http_delivery(run, "stable-key", artifact.id, &digest)
        .await
        .unwrap();
    let DeliveryState::Send(id) = attempt else {
        panic!("first delivery must send");
    };
    assert!(matches!(
        scoped
            .begin_extension_http_delivery(run, "stable-key", artifact.id, &digest)
            .await
            .unwrap(),
        DeliveryState::Uncertain
    ));
    assert_eq!(
        scoped.list_extension_http_deliveries(run).await.unwrap()[0].state,
        "uncertain"
    );
    assert!(
        scoped
            .begin_extension_http_delivery(run, "stable-key", artifact.id, &"c".repeat(64))
            .await
            .is_err()
    );
    scoped
        .complete_extension_http_delivery(id, 204)
        .await
        .unwrap();
    assert_eq!(
        scoped.list_extension_http_deliveries(run).await.unwrap()[0].state,
        "succeeded"
    );
    assert!(matches!(
        scoped
            .begin_extension_http_delivery(run, "stable-key", artifact.id, &digest)
            .await
            .unwrap(),
        DeliveryState::Succeeded(204)
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn operation_schedules_are_workspace_scoped_and_occurrences_deduplicated(pool: sqlx::PgPool) {
    use api::repository::CreateExtensionOperationSchedule;
    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &operation_release_archive("acme.schedule"))
        .await
        .unwrap();
    repository.enable_extension("acme.schedule").await.unwrap();
    let release = repository
        .installed_extension("acme.schedule")
        .await
        .unwrap()
        .installed_release_id;
    let schedule = repository
        .create_extension_operation_schedule(CreateExtensionOperationSchedule {
            extension_id: "acme.schedule".into(),
            release_id: release,
            operation_id: "import".into(),
            input: json!({}),
            source_reference: json!({}),
            destination_reference: json!({}),
            interval_seconds: 60,
        })
        .await
        .unwrap();
    assert!(schedule.enabled);
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let response = authenticated_client()
        .get(format!("{base}/extension-operation-schedules"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body[0]["id"], schedule.id.to_string());
    assert!(body[0].get("input").is_none());
    let created = authenticated_client()
        .post(format!(
            "{base}/extensions/acme.schedule/operation-schedules"
        ))
        .json(&json!({"operation_id":"import","input":{},"interval_seconds":120}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), reqwest::StatusCode::CREATED);
    let second_id: Uuid = created.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let disabled = authenticated_client()
        .patch(format!("{base}/extension-operation-schedules/{second_id}"))
        .json(&json!({"enabled":false,"interval_seconds":120}))
        .send()
        .await
        .unwrap();
    assert_eq!(disabled.status(), reqwest::StatusCode::OK);
    assert_eq!(disabled.json::<Value>().await.unwrap()["enabled"], false);
    assert!(
        reqwest::Client::new()
            .get(format!("{base}/extension-operation-schedules"))
            .send()
            .await
            .unwrap()
            .status()
            .is_client_error()
    );
    server.abort();
    sqlx::query("UPDATE extension_operation_schedules SET next_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(schedule.id).execute(&pool).await.unwrap();
    let (first, second) = tokio::join!(
        repository.produce_due_extension_operation_schedules(),
        repository.produce_due_extension_operation_schedules()
    );
    assert_eq!(first.unwrap() + second.unwrap(), 1);
    let occurrences: i64 =
        sqlx::query_scalar("SELECT count(*) FROM extension_operation_runs WHERE schedule_id=$1")
            .bind(schedule.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(occurrences, 1);
    // Overlap is skipped, never queued as a second task.
    sqlx::query("UPDATE extension_operation_schedules SET next_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(schedule.id).execute(&pool).await.unwrap();
    assert_eq!(
        repository
            .produce_due_extension_operation_schedules()
            .await
            .unwrap(),
        1
    );
    let occurrences: i64 =
        sqlx::query_scalar("SELECT count(*) FROM extension_operation_runs WHERE schedule_id=$1")
            .bind(schedule.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(occurrences, 1);
    let foreign_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces(id,slug,name,login_identifier) VALUES($1,$2,'Foreign',$3)")
        .bind(foreign_id)
        .bind(format!("foreign-{}", &foreign_id.simple().to_string()[..8]))
        .bind(format!(
            "foreign-{}.local",
            &foreign_id.simple().to_string()[..8]
        ))
        .execute(&pool)
        .await
        .unwrap();
    let foreign = CatalogRepository::system(pool.clone())
        .for_workspace(foreign_id)
        .await
        .unwrap();
    assert!(
        foreign
            .list_extension_operation_schedules()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        foreign
            .update_extension_operation_schedule(schedule.id, false, 60)
            .await
            .unwrap()
            .is_none()
    );
    repository
        .update_extension_operation_schedule(schedule.id, false, 60)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE extension_operation_schedules SET next_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(schedule.id).execute(&pool).await.unwrap();
    assert_eq!(
        repository
            .produce_due_extension_operation_schedules()
            .await
            .unwrap(),
        0
    );
    let run: Uuid =
        sqlx::query_scalar("SELECT id FROM extension_operation_runs WHERE schedule_id=$1")
            .bind(schedule.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(repository.cancel_extension_operation(run).await.unwrap());
    repository
        .update_extension_operation_schedule(schedule.id, true, 60)
        .await
        .unwrap();
    repository.disable_extension("acme.schedule").await.unwrap();
    sqlx::query("UPDATE extension_operation_schedules SET next_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(schedule.id).execute(&pool).await.unwrap();
    repository
        .produce_due_extension_operation_schedules()
        .await
        .unwrap();
    let paused: Option<String> =
        sqlx::query_scalar("SELECT paused_reason FROM extension_operation_schedules WHERE id=$1")
            .bind(schedule.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(paused.as_deref(), Some("release_or_source_unavailable"));
}

/// Packaged component integration, enabled by pointing at the sibling archive.
/// This exercises the actual WIT linker, snapshot page, staging, worker and S3
/// abstraction rather than a mock component implementation.
#[sqlx::test(migrations = "./migrations")]
async fn packaged_csv_connector_exports_through_the_real_host(pool: sqlx::PgPool) {
    use api::{
        extension_runtime::{
            ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        },
        repository::StartExtensionOperation,
        task_worker::TaskHandler,
    };
    use catalog_domain::task_queue::TaskKind;
    use std::time::Duration;
    let Ok(path) = std::env::var("ATTRICAT_CONNECTOR_CSV_ARCHIVE") else {
        return;
    };
    let archive = std::fs::read(path).unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(Uuid::from_u128(0x00000000000040008000000000000002))
        .await
        .unwrap();
    let store = Arc::new(FakeObjectStore::available());
    let blueprint = repository
        .create_blueprint(CreateBlueprint {
            definition: SYNC_BLUEPRINT.into(),
        })
        .await
        .unwrap();
    repository
        .publish_blueprint_revision(blueprint.blueprint.id, blueprint.blueprint.version)
        .await
        .unwrap();
    let context = repository
        .get_context_by_code("default")
        .await
        .unwrap()
        .unwrap();
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &archive)
        .await
        .unwrap();
    for capability in [
        "catalog.read",
        "catalog.write",
        "artifacts.read",
        "artifacts.write",
    ] {
        repository
            .grant_extension("attricat-connector-csv", "capability", capability)
            .await
            .unwrap();
    }
    repository
        .enable_extension("attricat-connector-csv")
        .await
        .unwrap();
    let release = repository
        .installed_extension("attricat-connector-csv")
        .await
        .unwrap()
        .installed_release_id;
    let profile = json!({"version":1,"blueprint_id":blueprint.blueprint.id.to_string(),"blueprint_version":blueprint.blueprint.version,"context_id":context.id.to_string(),"columns":[{"header":"ID","attribute":"external_id","kind":"string"}]});
    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "attricat-connector-csv".into(),
            expected_release_id: release,
            operation_id: "export".into(),
            input: json!({"profile": profile}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "csv-export".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    let task = repository
        .claim_task_for_kinds(
            "csv-integration",
            Duration::from_secs(60),
            &[TaskKind::ExtensionOperationRunV1],
        )
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    ExtensionOperationTaskHandler::new(repository.clone(), runtime)
        .handle(task)
        .await
        .unwrap();
    let status: String =
        sqlx::query_scalar("SELECT status FROM extension_operation_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    let debug: (String, Option<String>, i32) = sqlx::query_as(
        "SELECT status,last_error_message,batch_number FROM extension_operation_runs WHERE id=$1",
    )
    .bind(run)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "completed", "{debug:?}");
    let (artifact_id, key): (Uuid,String) = sqlx::query_as("SELECT id,object_key FROM extension_operation_artifacts WHERE operation_run_id=$1 AND direction='output' AND state='completed'")
        .bind(run).fetch_one(&pool).await.unwrap();
    repository
        .completed_extension_operation_artifact(run, artifact_id)
        .await
        .unwrap();
    assert_eq!(store.get(&key).await.unwrap().bytes.as_ref(), b"ID\n");
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let outputs: Value = authenticated_client()
        .get(format!("{base}/extension-operation-runs/{run}/artifacts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(outputs[0]["id"], artifact_id.to_string());
    assert!(outputs[0].get("object_key").is_none());
    server.abort();

    // A real packaged import spans two worker leases, reopens the durable
    // input, and commits two independently deduplicated Catalog upsert batches.
    use api::{storage::StoredObject, task_worker::TaskOutcome};
    use sha2::{Digest, Sha256};
    let csv = format!(
        "ID,Title\n{}",
        (1..=17)
            .map(|n| format!("SKU-{n},Name {n}\n"))
            .collect::<String>()
    );
    let file_id = Uuid::new_v4();
    let input_key = format!("files/{file_id}");
    store
        .put(
            &input_key,
            StoredObject {
                bytes: csv.as_bytes().to_vec().into(),
                content_type: Some("text/csv".into()),
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO files(id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status) VALUES($1,$2,'input.csv','input.csv','text/csv',$3,$4,$5,'ready')")
        .bind(file_id).bind(Uuid::from_u128(0x00000000000040008000000000000002))
        .bind(csv.len() as i64).bind(format!("{:x}", Sha256::digest(csv.as_bytes()))).bind(&input_key)
        .execute(&pool).await.unwrap();
    let import_profile = json!({"version":1,"blueprint_id":blueprint.blueprint.id.to_string(),"blueprint_version":blueprint.blueprint.version,"context_id":context.id.to_string(),"business_key":"external_id","columns":[{"header":"ID","attribute":"external_id","kind":"string"},{"header":"Title","attribute":"title","kind":"string"}]});
    let import_run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "attricat-connector-csv".into(),
            expected_release_id: release,
            operation_id: "import".into(),
            input: json!({"profile":import_profile}),
            source_reference: json!({"input_file_id":file_id}),
            destination_reference: json!({}),
            idempotency_key: "csv-import".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    for batch in 0..2 {
        let task = repository
            .claim_task_for_kinds(
                "csv-restart",
                Duration::from_secs(60),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.subject_id, import_run);
        let runtime =
            ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
        let result = ExtensionOperationTaskHandler::new(repository.clone(), runtime)
            .handle(task.clone())
            .await
            .unwrap();
        if batch == 0 {
            assert!(matches!(result, TaskOutcome::Reschedule { .. }));
            repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    chrono::Utc::now(),
                )
                .await
                .unwrap();
        }
    }
    let (status, batches): (String, i32) =
        sqlx::query_as("SELECT status,batch_number FROM extension_operation_runs WHERE id=$1")
            .bind(import_run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(batches, 2);
    let created: i64 = sqlx::query_scalar("SELECT count(*) FROM entities WHERE workspace_id=$1 AND blueprint_id=$2 AND deleted_at IS NULL")
        .bind(Uuid::from_u128(0x00000000000040008000000000000002)).bind(blueprint.blueprint.id).fetch_one(&pool).await.unwrap();
    assert_eq!(created, 17);
    let second_profile = json!({"version":1,"blueprint_id":blueprint.blueprint.id.to_string(),"blueprint_version":blueprint.blueprint.version,"context_id":context.id.to_string(),"columns":[{"header":"ID","attribute":"external_id","kind":"string"}]});
    let paged_export = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: "attricat-connector-csv".into(),
            expected_release_id: release,
            operation_id: "export".into(),
            input: json!({"profile":second_profile}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "csv-export-after-import".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    for batch in 0..2 {
        let task = repository
            .claim_task_for_kinds(
                "csv-export-restart",
                Duration::from_secs(60),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.subject_id, paged_export);
        let runtime =
            ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
        let result = ExtensionOperationTaskHandler::new(repository.clone(), runtime)
            .handle(task.clone())
            .await
            .unwrap();
        if batch == 0 {
            assert!(matches!(result, TaskOutcome::Reschedule { .. }));
            // The next page must read values as of the first page's cursor,
            // even if a Catalog write lands between worker leases.
            let key_attribute = blueprint
                .attributes
                .iter()
                .find(|a| a.code == "external_id")
                .unwrap()
                .id;
            let last = repository
                .extension_catalog_lookup(
                    blueprint.blueprint.id,
                    blueprint.blueprint.version,
                    key_attribute,
                    "SKU-17",
                )
                .await
                .unwrap()
                .unwrap();
            repository
                .append_values(
                    last.id,
                    api::model::AppendAttributeValues {
                        values: vec![NewAttributeValue::Scalar {
                            attribute_id: None,
                            attribute_code: Some("external_id".into()),
                            context_id: Some(context.id),
                            value: json!("SKU-17-CHANGED"),
                        }],
                    },
                )
                .await
                .unwrap();
            repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    chrono::Utc::now(),
                )
                .await
                .unwrap();
        }
    }
    let (status, batches): (String, i32) =
        sqlx::query_as("SELECT status,batch_number FROM extension_operation_runs WHERE id=$1")
            .bind(paged_export)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(batches, 2);
    let paged_key: String = sqlx::query_scalar("SELECT object_key FROM extension_operation_artifacts WHERE operation_run_id=$1 AND state='completed' AND direction='output'")
        .bind(paged_export).fetch_one(&pool).await.unwrap();
    let csv = String::from_utf8(store.get(&paged_key).await.unwrap().bytes.to_vec()).unwrap();
    assert_eq!(csv.lines().count(), 18);
    assert!(csv.starts_with("ID\n"));
    assert!(csv.lines().any(|line| line == "SKU-17"));
    assert!(!csv.contains("SKU-17-CHANGED"));
    sqlx::query("UPDATE extension_operation_runs SET completed_at=clock_timestamp()-interval '31 days' WHERE id=$1")
        .bind(run).execute(&pool).await.unwrap();
    repository
        .expire_extension_operation_storage()
        .await
        .unwrap();
    assert!(
        repository
            .completed_extension_operation_artifact(run, artifact_id)
            .await
            .is_err()
    );
    for key in repository
        .pending_extension_operation_object_deletions()
        .await
        .unwrap()
    {
        store.delete(&key).await.unwrap();
        repository
            .confirm_extension_operation_object_deletion(&key)
            .await
            .unwrap();
    }
    assert!(store.get(&key).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn artifact_completion_faults_abort_metadata_and_delete_orphans(pool: sqlx::PgPool) {
    use api::{
        extension_runtime::{
            ExtensionOperationTaskHandler, ExtensionRuntime, ExtensionRuntimeConfig,
        },
        repository::StartExtensionOperation,
        storage::StoredObject,
        task_worker::TaskHandler,
    };
    use catalog_domain::task_queue::TaskKind;
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    let workspace = Uuid::from_u128(0x00000000000040008000000000000002);
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let inner = Arc::new(FakeObjectStore::available());
    let store = Arc::new(FaultingArtifactStore::new(
        inner.clone(),
        pool.clone(),
        ArtifactUploadFault::FailUpload,
    ));
    ExtensionInstaller::new(repository.clone(), store.clone())
        .install(
            "test",
            &artifact_operation_release_archive(
                "acme.artifact-fault",
                &artifact_stream_component(),
            ),
        )
        .await
        .unwrap();
    for capability in ["artifacts.read", "artifacts.write"] {
        repository
            .grant_extension("acme.artifact-fault", "capability", capability)
            .await
            .unwrap();
    }
    repository
        .enable_extension("acme.artifact-fault")
        .await
        .unwrap();
    let release_id = repository
        .installed_extension("acme.artifact-fault")
        .await
        .unwrap()
        .installed_release_id;
    let input = vec![9_u8; 64 * 1024 + 1];
    let file_id = Uuid::new_v4();
    inner
        .put(
            "files/fault-input",
            StoredObject {
                bytes: input.clone().into(),
                content_type: Some("application/octet-stream".into()),
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO files(id,workspace_id,original_filename,display_filename,mime_type,byte_size,sha256,original_key,status) VALUES($1,$2,'fault.bin','fault.bin','application/octet-stream',$3,$4,'files/fault-input','ready')")
        .bind(file_id).bind(workspace).bind(input.len() as i64).bind(format!("{:x}", Sha256::digest(&input))).execute(&pool).await.unwrap();

    let retained_before_failures = inner.object_count().await;
    for (idempotency_key, fault) in [
        ("upload-failure", ArtifactUploadFault::FailUpload),
        (
            "database-failure",
            ArtifactUploadFault::DeleteMetadataBeforeCompletion,
        ),
    ] {
        store.set_fault(fault).await;
        let run_id = repository
            .start_extension_operation(StartExtensionOperation {
                extension_id: "acme.artifact-fault".into(),
                expected_release_id: release_id,
                operation_id: "copy".into(),
                input: json!({}),
                source_reference: json!({"input_file_id": file_id}),
                destination_reference: json!({}),
                idempotency_key: idempotency_key.into(),
                schedule_id: None,

                configuration_snapshot: None,
            })
            .await
            .unwrap();
        let task = repository
            .claim_task_for_kinds(
                idempotency_key,
                Duration::from_secs(30),
                &[TaskKind::ExtensionOperationRunV1],
            )
            .await
            .unwrap()
            .unwrap();
        let runtime =
            ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
        ExtensionOperationTaskHandler::new(repository.clone(), runtime)
            .handle(task)
            .await
            .unwrap();
        let states: Vec<String> = sqlx::query_scalar("SELECT state FROM extension_operation_artifacts WHERE operation_run_id=$1 AND direction='output'")
            .bind(run_id).fetch_all(&pool).await.unwrap();
        if matches!(fault, ArtifactUploadFault::FailUpload) {
            assert_eq!(states, ["aborted"]);
        } else {
            // The injected delete simulates a database completion failure after
            // object upload. Runtime compensation removes the orphan object.
            assert!(states.is_empty());
        }
    }
    assert_eq!(
        inner.object_count().await,
        retained_before_failures,
        "failed outputs are never retained"
    );
}
