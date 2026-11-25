//! Inbound extension webhooks (#279) against a real database, the HTTP
//! router and the production Wasmtime handlers: endpoint management, the
//! public route, the component's own signature check and reply, durable
//! payloads with deduplication, and their delivery to event handlers.

use crate::support;

use std::{sync::Arc, time::Duration};

use api::{
    extension_runtime::{ExtensionRuntime, ExtensionRuntimeConfig, WasmExtensionTaskHandler},
    repository::AttricatRepository,
    storage::FakeObjectStore,
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskOutcome},
};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use support::*;

const EXTENSION: &str = "acme.webhooks";
const WASM_CONSUMER: &str = "attricat.extensions.wasm";
const SECRET: &str = "s3cret";
const MAX_BODY_BYTES: usize = 1024;
const PERMISSIONS: &[&str] = &[
    "events.subscribe",
    "storage.extension",
    "secrets.read",
    "webhooks.receive",
];

fn manifest() -> Value {
    json!({
        "manifest_version": 1,
        "name": "Webhook fixture",
        "version": "1.0.0",
        "description": "attricat:host 1.2 inbound webhook test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": EXTENSION, "host_api": "^1.2"},
        // Optional, so the grant can be lost while the extension stays enabled.
        "permissions": &PERMISSIONS[..PERMISSIONS.len() - 1],
        "optional_permissions": ["webhooks.receive"],
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "event_handlers": [{
                "id": "orders",
                "event_types": ["webhook.received.v1"],
                "handler": "handle-event"
            }],
            "webhooks": [{"id": "orders", "title": "Orders", "max_body_bytes": MAX_BODY_BYTES}]
        }
    })
}

fn archive() -> Vec<u8> {
    static COMPONENT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let component = COMPONENT.get_or_init(|| {
        build_test_component(
            "attricat-webhook-test-component",
            "webhook-test.component.wasm",
        )
    });
    let manifest = serde_json::to_vec(&manifest()).unwrap();
    tar_zst(&[("manifest.json", &manifest), ("server.wasm", component)])
}

fn signature(body: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(SECRET.as_bytes());
    hash.update(b".");
    hash.update(body.as_bytes());
    hex::encode(hash.finalize())
}

/// Delivers every pending extension event like the production coordinator
/// and task worker, making rescheduled deliveries due at once.
async fn deliver_events(repository: &AttricatRepository, store: Arc<FakeObjectStore>) {
    let runtime = ExtensionRuntime::new(store, ExtensionRuntimeConfig::default()).unwrap();
    let handler = WasmExtensionTaskHandler::new(repository.clone(), runtime);
    let event_types = repository.enabled_extension_event_types().await.unwrap();
    repository
        .materialize_event_delivery_tasks(WASM_CONSUMER, &event_types)
        .await
        .unwrap();
    for _ in 0..50 {
        let Some(task) = repository
            .claim_task_for_kinds(
                "webhook-events-test",
                Duration::from_secs(30),
                &[TaskKind::EventDeliveryV1],
            )
            .await
            .unwrap()
        else {
            return;
        };
        match handler.handle(task.clone()).await {
            Ok(TaskOutcome::Reschedule { .. }) => repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    chrono::Utc::now() - chrono::Duration::seconds(60),
                )
                .await
                .unwrap(),
            _ => repository
                .complete_task(task.id, &task.lease_owner, task.lease_token)
                .await
                .unwrap(),
        }
    }
    panic!("event deliveries did not settle");
}

struct Fixture {
    pool: sqlx::PgPool,
    repository: AttricatRepository,
    store: Arc<FakeObjectStore>,
    base_url: String,
    client: reqwest::Client,
    release: uuid::Uuid,
    _server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(pool: sqlx::PgPool) -> Self {
        let repository =
            AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
        repository
            .ensure_event_consumer(WASM_CONSUMER)
            .await
            .unwrap();
        let store = Arc::new(FakeObjectStore::available());
        let release = install_extension(
            &repository,
            store.clone(),
            EXTENSION,
            &archive(),
            PERMISSIONS,
        )
        .await;
        repository
            .put_workspace_extension_secret("webhook-secret", SECRET)
            .await
            .unwrap();
        let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
        Self {
            pool,
            repository,
            store,
            base_url,
            client: authenticated_client(),
            release,
            _server: server,
        }
    }

    fn endpoints(&self) -> String {
        format!("{}/extensions/{EXTENSION}/webhook-endpoints", self.base_url)
    }

    /// Creates an endpoint and returns its public URL.
    async fn create_endpoint(&self, endpoint: &str) -> String {
        let created = expect_status(
            self.client
                .post(self.endpoints())
                .json(&json!({"webhook": "orders", "endpoint": endpoint}))
                .send()
                .await
                .unwrap(),
            StatusCode::CREATED,
        )
        .await;
        assert_eq!(created["endpoint"], endpoint);
        assert!(created.get("token_sha256").is_none());
        self.public_url(&created)
    }

    fn public_url(&self, issued: &Value) -> String {
        let origin = self.base_url.trim_end_matches("/api");
        format!("{origin}{}", issued["path"].as_str().unwrap())
    }

    async fn send(&self, url: &str, body: &str, delivery: Option<&str>) -> reqwest::Response {
        let mut request = reqwest::Client::new()
            .post(url)
            .header("x-signature", signature(body))
            .body(body.to_owned());
        if let Some(delivery) = delivery {
            request = request.header("x-delivery-id", delivery);
        }
        request.send().await.unwrap()
    }

    async fn receipts(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM extension_webhook_receipts WHERE extension_id=$1")
            .bind(EXTENSION)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn stored(&self, key: &str) -> Option<Value> {
        self.repository
            .extension_storage_get(EXTENSION, self.release, key)
            .await
            .unwrap()
            .map(|entry| entry.value)
    }
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn components_answer_webhooks_and_their_handlers_process_kept_payloads(pool: sqlx::PgPool) {
    let fixture = Fixture::new(pool).await;
    let url = fixture.create_endpoint("shop-eu").await;

    // Only declared webhooks get endpoints, and names are unique.
    expect_error(
        fixture
            .client
            .post(fixture.endpoints())
            .json(&json!({"webhook": "invoices", "endpoint": "x"}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    let taken = expect_error(
        fixture
            .client
            .post(fixture.endpoints())
            .json(&json!({"webhook": "orders", "endpoint": "shop-eu"}))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "code_taken",
    )
    .await;
    assert_eq!(taken["error"]["details"]["resource"], "webhook_endpoint");

    // A verification handshake is answered by the component, as it says.
    let handshake = reqwest::get(format!("{url}?challenge=abc123"))
        .await
        .unwrap();
    assert_eq!(handshake.status(), StatusCode::OK);
    assert_eq!(handshake.headers()["cache-control"], "no-store");
    assert_eq!(handshake.headers()["content-type"], "text/plain");
    assert_eq!(handshake.text().await.unwrap(), "abc123");

    // The component checks the signature; a bad one is its 401.
    let forged = reqwest::Client::new()
        .post(&url)
        .header("x-signature", "0000")
        .body(r#"{"id":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(forged.text().await.unwrap(), "bad signature");
    assert_eq!(fixture.receipts().await, 0);

    // A signed delivery is stored before it is acknowledged; a redelivery
    // with the same delivery ID is acknowledged but not stored again.
    for _ in 0..2 {
        let kept = fixture.send(&url, r#"{"id":1}"#, Some("d-1")).await;
        assert_eq!(kept.status(), StatusCode::ACCEPTED);
        assert_eq!(kept.text().await.unwrap(), "kept for shop-eu");
    }
    assert_eq!(fixture.receipts().await, 1);
    let retried = fixture
        .send(&url, r#"{"id":2,"retry":true}"#, Some("d-2"))
        .await;
    assert_eq!(retried.status(), StatusCode::ACCEPTED);
    assert_eq!(fixture.receipts().await, 2);

    // Oversized bodies never reach the component.
    let oversized = "x".repeat(MAX_BODY_BYTES + 1);
    expect_error(
        fixture.send(&url, &oversized, None).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload_too_large",
    )
    .await;

    // While receiving, the component reaches only read operations.
    for operation in [
        "storage.set.v1",
        "attricat.command.v1",
        "network.request.v2",
    ] {
        let probe = reqwest::Client::new()
            .post(&url)
            .header("x-probe", operation)
            .send()
            .await
            .unwrap();
        assert_eq!(probe.status(), StatusCode::OK);
        assert!(
            probe
                .text()
                .await
                .unwrap()
                .contains("not available while receiving a webhook"),
            "{operation}"
        );
    }
    // Its failure is a 500 and does not quarantine the installation.
    expect_error(
        reqwest::Client::new()
            .post(&url)
            .header("x-fail", "1")
            .send()
            .await
            .unwrap(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
    )
    .await;
    assert_eq!(
        fixture
            .repository
            .installed_extension(EXTENSION)
            .await
            .unwrap()
            .state
            .as_str(),
        "enabled"
    );

    // Unknown tokens are a plain 404.
    expect_error(
        reqwest::Client::new()
            .post(format!(
                "{}/public/extension-webhooks/not-a-token",
                fixture.base_url
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;

    // Handlers read the kept payloads back; one retries before it succeeds.
    deliver_events(&fixture.repository, fixture.store.clone()).await;
    let first = fixture.stored("order-1").await.expect("order 1 processed");
    assert_eq!(first["payload"], json!({"id": 1}));
    assert_eq!(first["webhook"], "orders");
    assert_eq!(first["endpoint"], "shop-eu");
    assert_eq!(first["attempt"], 1);
    let second = fixture.stored("order-2").await.expect("order 2 processed");
    assert_eq!(second["attempt"], 2);

    // Operators see when an endpoint was last called and how it answered.
    let listed = expect_status(
        fixture
            .client
            .get(fixture.endpoints())
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(listed[0]["endpoint"], "shop-eu");
    assert!(listed[0]["last_request_at"].is_string());
    assert_eq!(listed[0]["last_reply_status"], 500);

    // Rotating issues a new URL and retires the old one at once.
    let rotated = expect_status(
        fixture
            .client
            .post(format!("{}/shop-eu/rotate", fixture.endpoints()))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    let rotated_url = fixture.public_url(&rotated);
    assert_ne!(rotated_url, url);
    expect_error(
        fixture.send(&url, r#"{"id":3}"#, None).await,
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    assert_eq!(
        fixture
            .send(&rotated_url, r#"{"id":3}"#, None)
            .await
            .status(),
        StatusCode::ACCEPTED
    );

    // Deleting the endpoint retires its URL; kept payloads stay deliverable.
    assert_eq!(
        fixture
            .client
            .delete(format!("{}/shop-eu", fixture.endpoints()))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    expect_error(
        fixture.send(&rotated_url, r#"{"id":4}"#, None).await,
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    deliver_events(&fixture.repository, fixture.store.clone()).await;
    assert!(fixture.stored("order-3").await.is_some());
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn webhooks_reach_only_an_enabled_granted_extension(pool: sqlx::PgPool) {
    let fixture = Fixture::new(pool).await;
    let url = fixture.create_endpoint("shop").await;
    let unavailable = |response| {
        expect_error(
            response,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
    };

    fixture
        .repository
        .disable_extension(EXTENSION)
        .await
        .unwrap();
    unavailable(fixture.send(&url, r#"{"id":1}"#, None).await).await;
    assert_eq!(fixture.receipts().await, 0);

    fixture
        .repository
        .enable_extension(EXTENSION)
        .await
        .unwrap();
    assert_eq!(
        fixture.send(&url, r#"{"id":1}"#, None).await.status(),
        StatusCode::ACCEPTED
    );

    // Losing the grant stops both the public route and delivery.
    fixture
        .repository
        .revoke_extension_grant(EXTENSION, "capability", "webhooks.receive")
        .await
        .unwrap();
    unavailable(fixture.send(&url, r#"{"id":2}"#, None).await).await;
    assert_eq!(fixture.receipts().await, 1);

    fixture
        .repository
        .grant_extension(EXTENSION, "capability", "webhooks.receive")
        .await
        .unwrap();
    // A handler fault quarantines the installation, which then cannot
    // receive webhooks either.
    assert_eq!(
        fixture
            .send(&url, r#"{"id":3,"fault":true}"#, None)
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    deliver_events(&fixture.repository, fixture.store.clone()).await;
    assert_eq!(
        fixture
            .repository
            .installed_extension(EXTENSION)
            .await
            .unwrap()
            .state
            .as_str(),
        "quarantined"
    );
    unavailable(fixture.send(&url, r#"{"id":4}"#, None).await).await;

    // Removing the extension removes its endpoints and kept payloads.
    fixture
        .repository
        .remove_extension(EXTENSION)
        .await
        .unwrap();
    assert_eq!(fixture.receipts().await, 0);
    expect_error(
        fixture.send(&url, r#"{"id":5}"#, None).await,
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
}
