//! Extension secrets against a real database, the HTTP router and the
//! production Wasmtime webhook handler: each secret belongs to one
//! installation, is stored only as ciphertext, merges with the manifest's
//! declarations for management, and goes with the installation.

use crate::support;

use std::sync::Arc;

use api::{repository::AttricatRepository, storage::FakeObjectStore};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use support::*;

const FIRST: &str = "acme.secrets-first";
const SECOND: &str = "acme.secrets-second";
const SECRET_NAME: &str = "webhook-secret";
const SECRET: &str = "s3cret-value";
const PERMISSIONS: &[&str] = &["secrets.read", "webhooks.receive"];

/// The webhook test component under `extension_id`; it checks each request's
/// signature with its `webhook-secret`.
fn archive(extension_id: &str) -> Vec<u8> {
    static COMPONENT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let component = COMPONENT.get_or_init(|| {
        build_test_component(
            "attricat-webhook-test-component",
            "webhook-test.component.wasm",
        )
    });
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Secrets fixture",
        "version": "1.0.0",
        "description": "per-extension secrets test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": extension_id, "host_api": "^1.2"},
        "permissions": PERMISSIONS,
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "webhooks": [{"id": "orders", "title": "Orders"}],
            "secrets": [
                {"name": SECRET_NAME, "title": "Signing key", "description": "Signs webhook requests.", "required": true},
                {"name": "sandbox-key", "title": "Sandbox key"}
            ]
        }
    }))
    .unwrap();
    tar_zst(&[("manifest.json", &manifest), ("server.wasm", component)])
}

fn signature(body: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(SECRET.as_bytes());
    hash.update(b".");
    hash.update(body.as_bytes());
    hex::encode(hash.finalize())
}

struct Fixture {
    pool: sqlx::PgPool,
    repository: AttricatRepository,
    store: Arc<FakeObjectStore>,
    base_url: String,
    client: reqwest::Client,
    _server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(pool: sqlx::PgPool) -> Self {
        api::repository::use_extension_credential_key(TEST_CREDENTIAL_KEY);
        let repository =
            AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
        let store = Arc::new(FakeObjectStore::available());
        for extension_id in [FIRST, SECOND] {
            install_extension(
                &repository,
                store.clone(),
                extension_id,
                &archive(extension_id),
                PERMISSIONS,
            )
            .await;
        }
        let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
        Self {
            pool,
            repository,
            store,
            base_url,
            client: authenticated_client(),
            _server: server,
        }
    }

    fn secrets(&self, extension_id: &str) -> String {
        format!("{}/extensions/{extension_id}/secrets", self.base_url)
    }

    async fn put(&self, extension_id: &str, name: &str, value: &str) -> reqwest::Response {
        self.client
            .put(format!("{}/{name}", self.secrets(extension_id)))
            .json(&json!({"value": value}))
            .send()
            .await
            .unwrap()
    }

    async fn list(&self, extension_id: &str) -> Value {
        expect_status(
            self.client
                .get(self.secrets(extension_id))
                .send()
                .await
                .unwrap(),
            StatusCode::OK,
        )
        .await
    }

    /// Creates a webhook endpoint of `extension_id` and returns its public URL.
    async fn endpoint(&self, extension_id: &str) -> String {
        let created = expect_status(
            self.client
                .post(format!(
                    "{}/extensions/{extension_id}/webhook-endpoints",
                    self.base_url
                ))
                .json(&json!({"webhook": "orders", "endpoint": "main"}))
                .send()
                .await
                .unwrap(),
            StatusCode::CREATED,
        )
        .await;
        let origin = self.base_url.trim_end_matches("/api");
        format!("{origin}{}", created["path"].as_str().unwrap())
    }

    async fn deliver(&self, url: &str) -> StatusCode {
        let body = r#"{"id":1}"#;
        reqwest::Client::new()
            .post(url)
            .header("x-signature", signature(body))
            .body(body)
            .send()
            .await
            .unwrap()
            .status()
    }
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_component_reads_only_its_own_extensions_secrets(pool: sqlx::PgPool) {
    let fixture = Fixture::new(pool).await;
    let first = fixture.endpoint(FIRST).await;
    let second = fixture.endpoint(SECOND).await;

    // Set for the second extension only: the first cannot read it, although
    // it asks for the same name.
    expect_status(
        fixture.put(SECOND, SECRET_NAME, SECRET).await,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(fixture.deliver(&second).await, StatusCode::ACCEPTED);
    assert_eq!(
        fixture.deliver(&first).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        fixture
            .repository
            .extension_secret_value(FIRST, SECRET_NAME)
            .await
            .unwrap(),
        None
    );

    expect_status(
        fixture.put(FIRST, SECRET_NAME, SECRET).await,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(fixture.deliver(&first).await, StatusCode::ACCEPTED);

    // Values are stored only as ciphertext, each bound to its own row: one
    // copied over another's does not open.
    let stored: Vec<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT extension_id, value_ciphertext FROM extension_secrets WHERE name = $1 ORDER BY extension_id",
    )
    .bind(SECRET_NAME)
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(stored.len(), 2);
    for (_, ciphertext) in &stored {
        assert!(
            !ciphertext
                .windows(SECRET.len())
                .any(|window| window == SECRET.as_bytes())
        );
    }
    sqlx::query("UPDATE extension_secrets SET value_ciphertext = $1 WHERE extension_id = $2")
        .bind(&stored[1].1)
        .bind(FIRST)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(
        fixture
            .repository
            .extension_secret_value(FIRST, SECRET_NAME)
            .await
            .is_err()
    );
    assert_eq!(
        fixture.deliver(&first).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn management_lists_declared_and_set_secrets_without_values(pool: sqlx::PgPool) {
    let fixture = Fixture::new(pool).await;

    // Declared secrets are listed before any is set.
    let listed = fixture.list(FIRST).await;
    assert_eq!(
        listed,
        json!([
            {"name": SECRET_NAME, "declared": true, "title": "Signing key",
             "description": "Signs webhook requests.", "required": true, "set": false},
            {"name": "sandbox-key", "declared": true, "title": "Sandbox key",
             "required": false, "set": false}
        ])
    );

    expect_status(
        fixture.put(FIRST, SECRET_NAME, SECRET).await,
        StatusCode::NO_CONTENT,
    )
    .await;
    expect_status(
        fixture.put(FIRST, "endpoint-main", "other-value").await,
        StatusCode::NO_CONTENT,
    )
    .await;
    let listed = fixture.list(FIRST).await;
    let names: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|secret| secret["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, [SECRET_NAME, "sandbox-key", "endpoint-main"]);
    assert_eq!(listed[0]["set"], true);
    assert!(listed[0]["updated_at"].is_string());
    assert_eq!(listed[2]["declared"], false);
    assert!(!listed.to_string().contains(SECRET));
    assert!(!listed.to_string().contains("other-value"));
    // The other extension's list is its own.
    assert!(
        fixture
            .list(SECOND)
            .await
            .as_array()
            .unwrap()
            .iter()
            .all(|secret| secret["set"] == false)
    );

    // Invalid names and values, and extensions that are not installed.
    expect_error(
        fixture.put(FIRST, "bad:name", SECRET).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    expect_error(
        fixture.put(FIRST, SECRET_NAME, "").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    expect_error(
        fixture.put("acme.missing", SECRET_NAME, SECRET).await,
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    expect_error(
        fixture
            .client
            .get(fixture.secrets("acme.missing"))
            .send()
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;

    // Deleting is audited by name, never by value.
    expect_status(
        fixture
            .client
            .delete(format!("{}/endpoint-main", fixture.secrets(FIRST)))
            .send()
            .await
            .unwrap(),
        StatusCode::NO_CONTENT,
    )
    .await;
    expect_error(
        fixture
            .client
            .delete(format!("{}/endpoint-main", fixture.secrets(FIRST)))
            .send()
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    let audits: Vec<(String, Value, Value)> = sqlx::query_as(
        "SELECT action, target, metadata FROM audit_events WHERE action LIKE 'extension.secret.%' ORDER BY occurred_at",
    )
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    let actions: Vec<_> = audits.iter().map(|(action, ..)| action.as_str()).collect();
    assert_eq!(
        actions,
        [
            "extension.secret.put",
            "extension.secret.put",
            "extension.secret.delete"
        ]
    );
    assert_eq!(audits[2].1["extension_id"], FIRST);
    assert_eq!(audits[2].1["name"], "endpoint-main");
    for (_, target, metadata) in &audits {
        let text = format!("{target}{metadata}");
        assert!(!text.contains(SECRET) && !text.contains("other-value"));
    }
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn removing_an_extension_removes_its_secrets(pool: sqlx::PgPool) {
    let fixture = Fixture::new(pool).await;
    for extension_id in [FIRST, SECOND] {
        expect_status(
            fixture.put(extension_id, SECRET_NAME, SECRET).await,
            StatusCode::NO_CONTENT,
        )
        .await;
    }
    fixture.repository.remove_extension(FIRST).await.unwrap();
    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT extension_id FROM extension_secrets ORDER BY extension_id")
            .fetch_all(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(remaining, [SECOND]);

    // A later install of the same ID starts without them.
    install_extension(
        &fixture.repository,
        fixture.store.clone(),
        FIRST,
        &archive(FIRST),
        PERMISSIONS,
    )
    .await;
    assert_eq!(fixture.list(FIRST).await[0]["set"], false);
}
