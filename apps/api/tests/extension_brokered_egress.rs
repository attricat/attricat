//! Brokered egress of `attricat:host@1.1.0` (#424) against a local HTTPS
//! mock: `network.request.v2` with connection tokens, secrets in bodies,
//! presigned query strings, extra response headers and per-rule budgets;
//! host-side OAuth with serialized rotating refresh tokens; and transfer v2
//! with presigned PUTs and multipart POSTs. No component sees a credential.

mod support;

use api::states::ConnectionStatus;
use std::sync::{Arc, Mutex};

use api::{
    extension_runtime::{
        DevelopmentEgress, ExtensionRuntime, ExtensionRuntimeConfig, connections,
        use_development_egress,
    },
    repository::{
        AttricatRepository, ExtensionConnectionPatch, NewExtensionConnection,
        StartExtensionOperation,
    },
    storage::FakeObjectStore,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, post},
};
use support::*;

const EXTENSION: &str = "acme.egress";
const PERMISSIONS: &[&str] = &[
    "network.request",
    "connections.use",
    "artifacts.read",
    "artifacts.write",
    "attricat.read",
    "attricat.files.read",
    "attricat.files.public_links",
    "client.commands",
];
const HOST_PERMISSIONS: &[&str] = &["token", "admin", "lwa", "bucket"];
const OAUTH_CALLBACK: &str = "https://attricat.test/api/public/extension-oauth/callback";

/// What the mock received.
#[derive(Default)]
struct Seen {
    requests: Vec<Value>,
    refreshes: usize,
    /// Refresh tokens already exchanged; reusing one fails like Allegro.
    used_refresh_tokens: Vec<String>,
    /// The PKCE challenge of the browser authorization the test approved.
    code_challenge: Option<String>,
}

type Shared = Arc<Mutex<Seen>>;

fn header(headers: &HeaderMap, name: &str) -> Value {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map_or(Value::Null, |value| json!(value))
}

async fn device(body: String) -> Response {
    assert!(body.contains("client_id=client"));
    Json(json!({
        "device_code": "dev-1",
        "user_code": "ABCD-EFGH",
        "verification_uri": "https://shop.mock.test/device",
        "expires_in": 600,
        "interval": 1,
    }))
    .into_response()
}

async fn token(State(seen): State<Shared>, body: String) -> Response {
    let form: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect();
    let mut seen = seen.lock().unwrap();
    match form.get("grant_type").map(String::as_str) {
        Some("urn:ietf:params:oauth:grant-type:device_code") => {
            seen.requests
                .push(json!({"path": "/oauth/token", "device_code": form.get("device_code")}));
            // The operator approves after the first poll.
            let polls = seen
                .requests
                .iter()
                .filter(|request| request["device_code"] == "dev-1")
                .count();
            if polls < 2 {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "authorization_pending"})),
                )
                    .into_response()
            } else {
                Json(json!({"access_token": "tok-device", "refresh_token": "rd", "expires_in": 3600})).into_response()
            }
        }
        Some("authorization_code") => {
            use base64::Engine as _;
            use sha2::Digest as _;
            let verified = form.get("code").map(String::as_str) == Some("code-1")
                && form.get("redirect_uri").map(String::as_str) == Some(OAUTH_CALLBACK)
                && form.get("code_verifier").is_some_and(|verifier| {
                    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .encode(sha2::Sha256::digest(verifier.as_bytes()));
                    seen.code_challenge.as_deref() == Some(challenge.as_str())
                });
            if verified {
                Json(json!({"access_token": "tok-code", "refresh_token": "rc", "expires_in": 3600}))
                    .into_response()
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_grant"})),
                )
                    .into_response()
            }
        }
        Some("client_credentials")
            if form.get("client_secret").map(String::as_str) == Some("s3cret") =>
        {
            Json(json!({"access_token": "tok-cc", "expires_in": 3600})).into_response()
        }
        Some("refresh_token") => {
            let presented = form.get("refresh_token").cloned().unwrap_or_default();
            if seen.used_refresh_tokens.contains(&presented) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_grant"})),
                )
                    .into_response();
            }
            seen.used_refresh_tokens.push(presented);
            seen.refreshes += 1;
            let next = seen.refreshes + 1;
            Json(json!({
                "access_token": format!("tok-r{next}"),
                "refresh_token": format!("r{next}"),
                "expires_in": 3600,
            }))
            .into_response()
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid_client"})),
        )
            .into_response(),
    }
}

/// Echoes every request, including its query, selected headers and body.
async fn echo(State(seen): State<Shared>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
    let record = json!({
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query(),
        "token": header(&parts.headers, "x-shop-access-token"),
        "authorization": header(&parts.headers, "authorization"),
        "content_type": header(&parts.headers, "content-type"),
        "idempotency_key": header(&parts.headers, "idempotency-key"),
        "body": String::from_utf8_lossy(&body),
    });
    seen.lock().unwrap().requests.push(record.clone());
    if parts.uri.path() == "/result" {
        return (StatusCode::OK, Bytes::from_static(b"processingStatus=DONE")).into_response();
    }
    (
        StatusCode::OK,
        [("x-request-id", "req-42"), ("set-cookie", "session=leak")],
        Json(record),
    )
        .into_response()
}

fn mock(seen: Shared) -> Router {
    Router::new()
        .route("/oauth/token", post(token))
        .route("/oauth/device", post(device))
        .fallback(any(echo))
        .with_state(seen)
}

/// One mock for the whole test binary: the development egress override is
/// process-wide, so every test must reach the same server. It runs on its
/// own thread and runtime, which outlive each test's runtime.
fn shared_mock() -> (std::net::SocketAddr, Shared) {
    static MOCK: std::sync::OnceLock<(std::net::SocketAddr, Shared)> = std::sync::OnceLock::new();
    MOCK.get_or_init(|| {
        let seen: Shared = Arc::default();
        let (sender, receiver) = std::sync::mpsc::channel();
        let state = seen.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                sender.send(serve_tls(mock(state)).await).unwrap();
                std::future::pending::<()>().await;
            });
        });
        let address = receiver.recv().unwrap();
        use_development_egress(DevelopmentEgress {
            resolve: vec![("*.mock.test".into(), address)],
            extra_root_certificates_pem: mock_ca_pem(),
        });
        api::repository::use_extension_credential_key([9; 32]);
        (address, seen)
    })
    .clone()
}

fn manifest() -> Value {
    json!({
        "manifest_version": 1,
        "name": "Egress fixture",
        "version": "1.0.0",
        "description": "attricat:host 1.1 brokered egress test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": EXTENSION, "host_api": "^1.1"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object"}},
        "host_permissions": [
            {"id": "token", "matches": ["https://shop.mock.test/oauth/*"], "methods": ["POST"]},
            {"id": "admin", "matches": ["https://shop.mock.test/admin/*"], "methods": ["GET", "POST"],
             "allow_query": true, "response_headers": ["x-request-id"],
             "rate_limit": {"requests": 3, "per_seconds": 3600}},
            {"id": "lwa", "matches": ["https://lwa.mock.test/auth/*"], "methods": ["POST"]},
            {"id": "bucket", "matches": ["https://bucket.mock.test/*"], "methods": ["GET", "PUT", "POST"],
             "allow_query": true, "max_transfer_bytes": 1048576, "idempotent_delivery": true}
        ],
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {
            "commands": [{"id": "call", "handler": "call", "request_schema": {"type": "object"}, "response_schema": {"type": "object"}}],
            "operations": [{"id": "transfer", "handler": "transfer", "request_schema": {"type": "object"}, "max_request_bytes": 65536, "max_checkpoint_bytes": 65536}],
            "connections": [{
                "id": "shop", "title": "Shop",
                "grant_types": ["client_credentials", "refresh_token", "device_code", "authorization_code"],
                "host_permission": "token",
                "api_host_permissions": ["admin"],
                "token_header": {"name": "X-Shop-Access-Token"}
            }]
        }
    })
}

fn archive() -> Vec<u8> {
    let component = build_test_component(
        "attricat-export-test-component",
        "export-test.component.wasm",
    );
    let manifest = serde_json::to_vec(&manifest()).unwrap();
    tar_zst(&[("manifest.json", &manifest), ("server.wasm", &component)])
}

fn connection(name: &str, grant_type: &str) -> NewExtensionConnection {
    NewExtensionConnection {
        extension_id: EXTENSION.into(),
        connection: name.into(),
        kind: "shop".into(),
        display_name: name.into(),
        grant_type: grant_type.into(),
        host_permission_id: "token".into(),
        token_url: "https://shop.mock.test/oauth/token".into(),
        device_authorization_url: None,
        authorization_url: None,
        client_id: "client".into(),
        scope: None,
        parameters: json!({"shop": "acme"}),
        client_secret: Some("s3cret".into()),
        refresh_token: (grant_type == "refresh_token").then(|| "r1".into()),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn brokered_requests_carry_host_managed_credentials_only(pool: sqlx::PgPool) {
    let (_, seen) = shared_mock();
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    let installer =
        api::extension_installer::ExtensionInstaller::new(repository.clone(), store.clone());
    installer.install("test", &archive()).await.unwrap();
    for capability in PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "capability", capability)
            .await
            .unwrap();
    }
    for rule in HOST_PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "host_permission", rule)
            .await
            .unwrap();
    }
    repository.enable_extension(EXTENSION).await.unwrap();
    repository
        .put_workspace_extension_secret("lwa-client-secret", "lwa-s&cret")
        .await
        .unwrap();
    repository
        .create_extension_connection(connection("shop-eu", "client_credentials"))
        .await
        .unwrap();
    let release = repository
        .installed_extension(EXTENSION)
        .await
        .unwrap()
        .installed_release_id;
    let installation = repository
        .runtime_extension_installation(EXTENSION, release)
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    let call = |operation: &'static str, request: Value| {
        let runtime = runtime.clone();
        let installation = installation.clone();
        let repository = repository.clone();
        async move {
            let response = runtime
                .invoke_command(
                    &installation,
                    repository,
                    "call",
                    &json!({"operation": operation, "request": request}).to_string(),
                    65536,
                )
                .await
                .unwrap();
            serde_json::from_str::<Value>(&response).unwrap()
        }
    };

    // The host injects the connection's token and keeps the signed query.
    let response = call(
        "network.request.v2",
        json!({
            "host_permission_id": "admin",
            "method": "POST",
            "url": "https://shop.mock.test/admin/api/graphql.json?api_version=2026-10",
            "headers": {"content-type": "application/json"},
            "body": "{\"query\":\"{ shop { name } }\"}",
            "connection": "shop-eu"
        }),
    )
    .await;
    let response = &response["ok"];
    assert_eq!(response["outcome"], "response", "{response}");
    assert_eq!(response["status"], 200);
    assert_eq!(response["headers"]["x-request-id"], "req-42");
    assert!(response["headers"].get("set-cookie").is_none());
    let echoed: Value =
        serde_json::from_slice(&base64_decode(response["body_base64"].as_str().unwrap())).unwrap();
    assert_eq!(echoed["token"], "tok-cc");
    assert_eq!(echoed["query"], "api_version=2026-10");
    // The token is stored encrypted only.
    let stored: Vec<u8> = sqlx::query_scalar(
        "SELECT access_token_ciphertext FROM extension_connections WHERE connection='shop-eu'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!stored.windows(6).any(|window| window == b"tok-cc"));

    // Workspace secrets are substituted into form bodies by the host.
    let lwa = call(
        "network.request.v2",
        json!({
            "host_permission_id": "lwa",
            "method": "POST",
            "url": "https://lwa.mock.test/auth/o2/token",
            "headers": {"content-type": "application/x-www-form-urlencoded"},
            "body": "grant_type=refresh_token&client_id=amzn",
            "secret_body": {"format": "form", "fields": [{"field": "client_secret", "secret": "lwa-client-secret"}]}
        }),
    )
    .await;
    let echoed: Value =
        serde_json::from_slice(&base64_decode(lwa["ok"]["body_base64"].as_str().unwrap())).unwrap();
    assert_eq!(
        echoed["body"],
        "grant_type=refresh_token&client_id=amzn&client_secret=lwa-s%26cret"
    );

    // A connection token is never sent outside its API rules, and query
    // strings need a rule that allows them.
    let misdirected = call(
        "network.request.v2",
        json!({"host_permission_id": "lwa", "method": "POST", "url": "https://lwa.mock.test/auth/x", "connection": "shop-eu"}),
    )
    .await;
    assert!(
        misdirected["error"]
            .as_str()
            .unwrap()
            .contains("may not be sent")
    );
    let query = call(
        "network.request.v2",
        json!({"host_permission_id": "lwa", "method": "POST", "url": "https://lwa.mock.test/auth/x?a=1"}),
    )
    .await;
    assert!(query["error"].as_str().unwrap().contains("not granted"));

    // The admin rule's budget is three requests; the fourth is throttled
    // with a wait instead of failing.
    for _ in 0..2 {
        call(
            "network.request.v2",
            json!({"host_permission_id": "admin", "method": "GET", "url": "https://shop.mock.test/admin/ping"}),
        )
        .await;
    }
    let throttled = call(
        "network.request.v2",
        json!({"host_permission_id": "admin", "method": "GET", "url": "https://shop.mock.test/admin/ping"}),
    )
    .await;
    assert_eq!(throttled["ok"]["outcome"], "rate_limited");
    assert!(throttled["ok"]["retry_after_ms"].as_u64().unwrap() > 0);

    // Component-facing connection descriptions carry no credential.
    let described = call("connections.get.v1", json!({"connection": "shop-eu"})).await;
    assert_eq!(described["ok"]["parameters"], json!({"shop": "acme"}));
    assert!(!described.to_string().contains("s3cret"));
    assert!(!described.to_string().contains("tok-cc"));

    // Rotating refresh tokens are exchanged once even under concurrency.
    repository
        .create_extension_connection(connection("rotating", "refresh_token"))
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        connections::access_token(&repository, &installation, "rotating"),
        connections::access_token(&repository, &installation, "rotating"),
    );
    assert_eq!(first.unwrap().0, "tok-r2");
    assert_eq!(second.unwrap().0, "tok-r2");
    // A 401 for a token that was already replaced keeps the current one.
    repository
        .invalidate_extension_connection_token(EXTENSION, "rotating", "tok-stale")
        .await
        .unwrap();
    let (token, _) = connections::access_token(&repository, &installation, "rotating")
        .await
        .unwrap();
    assert_eq!(token, "tok-r2");
    repository
        .invalidate_extension_connection_token(EXTENSION, "rotating", "tok-r2")
        .await
        .unwrap();
    let (token, _) = connections::access_token(&repository, &installation, "rotating")
        .await
        .unwrap();
    assert_eq!(token, "tok-r3");
    assert_eq!(seen.lock().unwrap().refreshes, 2);

    // Transfer v2: presigned PUT with an exact content type and no forced
    // idempotency key, a multipart form POST, and a presigned GET.
    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: EXTENSION.into(),
            expected_release_id: release,
            operation_id: "transfer".into(),
            input: json!({"transfer": {
                "output": {"text": "sku\ttitle\nSKU-1\tShirt\n", "media_type": "text/tab-separated-values"},
                "deliver": {"host_permission_id": "bucket", "url": "https://bucket.mock.test/feeds/1?X-Amz-Signature=signed-query&X-Amz-Expires=300",
                            "method": "PUT", "delivery_key": "feed-1",
                            "headers": {"content-type": "text/tab-separated-values; charset=UTF-8"}},
                "multipart": {"host_permission_id": "bucket", "url": "https://bucket.mock.test/staged",
                              "method": "POST", "delivery_key": "image-1",
                              "multipart": {"fields": [{"name": "key", "value": "tmp/1/image.png"}, {"name": "policy", "value": "p"}],
                                            "file_field": "file", "filename": "image.png"}},
                "fetch": {"host_permission_id": "bucket", "url": "https://bucket.mock.test/result?X-Amz-Signature=def",
                          "transfer_key": "result-1", "offset": 0, "max_bytes": 1024},
                "record_file": {"record_id": Uuid::new_v4(), "file_id": Uuid::new_v4()}
            }}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "transfer".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    drain_extension_operations(&repository, store).await;
    let (status, checkpoint): (String, Value) =
        sqlx::query_as("SELECT status,checkpoint FROM extension_operation_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed", "{checkpoint}");
    let transfer = &checkpoint["transfer"];
    assert_eq!(transfer["deliver"]["outcome"], "succeeded", "{transfer}");
    assert_eq!(transfer["multipart"]["outcome"], "succeeded", "{transfer}");
    assert_eq!(transfer["fetch"]["text"], "processingStatus=DONE");
    assert!(
        transfer["record_file"]["error"]
            .as_str()
            .unwrap()
            .contains("does not reference")
    );
    let requests = seen.lock().unwrap().requests.clone();
    let put = requests
        .iter()
        .find(|request| request["path"] == "/feeds/1")
        .unwrap();
    assert_eq!(put["method"], "PUT");
    assert_eq!(
        put["query"],
        "X-Amz-Signature=signed-query&X-Amz-Expires=300"
    );
    assert_eq!(
        put["content_type"],
        "text/tab-separated-values; charset=UTF-8"
    );
    assert_eq!(put["idempotency_key"], Value::Null);
    assert_eq!(put["body"], "sku\ttitle\nSKU-1\tShirt\n");
    let form = requests
        .iter()
        .find(|request| request["path"] == "/staged")
        .unwrap();
    assert!(
        form["content_type"]
            .as_str()
            .unwrap()
            .starts_with("multipart/form-data")
    );
    let body = form["body"].as_str().unwrap();
    assert!(body.contains("tmp/1/image.png") && body.contains("filename=\"image.png\""));
    // Query values never reach the run's persisted delivery history.
    let digests: Vec<String> = sqlx::query_scalar(
        "SELECT destination_digest FROM extension_operation_http_deliveries WHERE operation_run_id=$1",
    )
    .bind(run)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        digests
            .iter()
            .all(|digest| !digest.contains("signed-query"))
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn operators_manage_connections_without_seeing_credentials(pool: sqlx::PgPool) {
    shared_mock();
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    api::extension_installer::ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &archive())
        .await
        .unwrap();
    for capability in PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "capability", capability)
            .await
            .unwrap();
    }
    for rule in HOST_PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "host_permission", rule)
            .await
            .unwrap();
    }
    repository.enable_extension(EXTENSION).await.unwrap();
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let connections = format!("{base_url}/extensions/{EXTENSION}/connections");

    // A token endpoint outside the kind's host permission is refused.
    expect_status(
        client
            .post(&connections)
            .json(&json!({"connection": "bad", "kind": "shop", "display_name": "Bad",
                          "grant_type": "client_credentials", "token_url": "https://evil.mock.test/oauth/token",
                          "client_id": "client", "client_secret": "s3cret"}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    let created = expect_status(
        client
            .post(&connections)
            .json(&json!({"connection": "allegro-pl", "kind": "shop", "display_name": "Allegro PL",
                          "grant_type": "device_code", "token_url": "https://shop.mock.test/oauth/token",
                          "device_authorization_url": "https://shop.mock.test/oauth/device",
                          "client_id": "client", "client_secret": "s3cret"}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(created["status"], "pending_authorization");
    assert_eq!(created["has_client_secret"], true);
    assert!(!created.to_string().contains("s3cret"));

    let started = expect_status(
        client
            .post(format!("{connections}/allegro-pl/device-authorization"))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(started["device_user_code"], "ABCD-EFGH");
    assert!(!started.to_string().contains("dev-1"));
    // Still pending until the operator approves at the provider.
    let pending = expect_status(
        client
            .post(format!(
                "{connections}/allegro-pl/device-authorization/poll"
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(pending["status"], "pending_authorization");
    // A poll sooner than the provider's interval does not reach the provider,
    // which would have approved it.
    let early = expect_status(
        client
            .post(format!(
                "{connections}/allegro-pl/device-authorization/poll"
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(early["status"], "pending_authorization");
    sqlx::query(
        "UPDATE extension_connections SET device_polled_at=device_polled_at - interval '1 minute'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let approved = expect_status(
        client
            .post(format!(
                "{connections}/allegro-pl/device-authorization/poll"
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(approved["status"], "active");
    assert_eq!(approved["has_refresh_token"], true);
    expect_status(
        client
            .post(format!("{connections}/allegro-pl/test"))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    let listed = get_json(&client, &connections).await;
    assert_eq!(listed[0]["connection"], "allegro-pl");
    for secret in ["s3cret", "tok-device", "dev-1", "\"rd\""] {
        assert!(!listed.to_string().contains(secret), "{secret}");
    }
    let audit: Vec<Value> = sqlx::query_scalar("SELECT metadata FROM audit_events")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!audit.is_empty());
    for metadata in &audit {
        assert!(!metadata.to_string().contains("s3cret"));
        assert!(!metadata.to_string().contains("tok-device"));
    }

    // Operators may lower a rule's budget, never raise it.
    let budgets = get_json(
        &client,
        format!("{base_url}/extensions/{EXTENSION}/host-permission-budgets"),
    )
    .await;
    let admin = budgets
        .as_array()
        .unwrap()
        .iter()
        .find(|budget| budget["host_permission_id"] == "admin")
        .unwrap();
    assert_eq!(admin["declared"]["requests"], 3);
    let budget = format!("{base_url}/extensions/{EXTENSION}/host-permission-budgets/admin");
    expect_status(
        client
            .put(&budget)
            .json(&json!({"requests": 1, "per_seconds": 3600}))
            .send()
            .await
            .unwrap(),
        StatusCode::NO_CONTENT,
    )
    .await;
    expect_status(
        client
            .put(&budget)
            .json(&json!({"requests": 10, "per_seconds": 1}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;

    expect_status(
        client
            .delete(format!("{connections}/allegro-pl"))
            .send()
            .await
            .unwrap(),
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_eq!(
        count(&pool, "SELECT count(*) FROM extension_connections").await,
        0
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn record_files_reach_marketplaces_by_public_link_or_as_run_input(pool: sqlx::PgPool) {
    shared_mock();
    api::extension_runtime::use_public_file_link_base(
        "https://attricat.example/api/public/extension-files/"
            .parse()
            .unwrap(),
    );
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    api::extension_installer::ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &archive())
        .await
        .unwrap();
    for capability in PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "capability", capability)
            .await
            .unwrap();
    }
    for rule in HOST_PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "host_permission", rule)
            .await
            .unwrap();
    }
    repository.enable_extension(EXTENSION).await.unwrap();
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "listing_with_image"
name = "Listing with image"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["image"]
[[attributes]]
code = "image"
value_type = "file"
allowed_mime_groups = ["text/plain"]
allowed_extensions = ["txt"]
max_bytes = 1024
"#,
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let uploaded: Value = client
        .post(format!(
            "{base_url}/records/{record_id}/file-attributes/image/uploads"
        ))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(b"product photo".to_vec())
                    .file_name("photo.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id: Uuid = sqlx::query_scalar("SELECT id FROM files ORDER BY created_at DESC LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(uploaded.to_string().contains(&file_id.to_string()));
    // The file worker, which is not running here, marks processed files ready.
    sqlx::query("UPDATE files SET status='ready' WHERE id=$1")
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();

    let release = repository
        .installed_extension(EXTENSION)
        .await
        .unwrap()
        .installed_release_id;
    let installation = repository
        .runtime_extension_installation(EXTENSION, release)
        .await
        .unwrap()
        .unwrap();
    let runtime = ExtensionRuntime::new(store.clone(), ExtensionRuntimeConfig::default()).unwrap();
    let link: Value = serde_json::from_str(
        &runtime
            .invoke_command(
                &installation,
                repository.clone(),
                "call",
                &json!({"operation": "files.link.v1", "request": {
                    "file_id": file_id, "record_id": record_id, "expires_in_seconds": 7200
                }})
                .to_string(),
                65536,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    let url = link["ok"]["url"].as_str().unwrap();
    let token = url
        .strip_prefix("https://attricat.example/api/public/extension-files/")
        .unwrap();
    // The marketplace fetches the file without any Attricat credential.
    let anonymous = reqwest::Client::new();
    let fetched = anonymous
        .get(format!("{base_url}/public/extension-files/{token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(fetched.bytes().await.unwrap().as_ref(), b"product photo");
    assert_eq!(
        anonymous
            .get(format!("{base_url}/public/extension-files/not-a-token"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    // Only the token's digest is stored.
    let stored: String = sqlx::query_scalar("SELECT token_sha256 FROM extension_file_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(stored, token);

    // Another record references the same file.
    let sharing = create_record(&client, &base_url, &blueprint).await;
    let sharing_id: Uuid = sharing["id"].as_str().unwrap().parse().unwrap();
    let value_id = Uuid::new_v4();
    sqlx::query("INSERT INTO attribute_values SELECT (jsonb_populate_record(NULL::attribute_values, to_jsonb(v) || jsonb_build_object('id', $3::uuid, 'record_id', $2::uuid))).* FROM attribute_values v WHERE v.record_id=$1 AND v.active")
        .bind(record_id)
        .bind(sharing_id)
        .bind(value_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attribute_file_references SELECT (jsonb_populate_record(NULL::attribute_file_references, to_jsonb(r) || jsonb_build_object('attribute_value_id', $2::uuid))).* FROM attribute_file_references r JOIN attribute_values v ON v.id=r.attribute_value_id WHERE v.record_id=$1 AND v.active")
        .bind(record_id)
        .bind(value_id)
        .execute(&pool)
        .await
        .unwrap();

    // A run reads the same file as an input artifact for a binary upload.
    let run = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id: EXTENSION.into(),
            expected_release_id: release,
            operation_id: "transfer".into(),
            // A second record sharing the file reuses its artifact instead of
            // being refused.
            input: json!({"transfer": {
                "record_file": {"record_id": record_id, "file_id": file_id},
                "record_file_again": {"record_id": sharing_id, "file_id": file_id},
            }}),
            source_reference: json!({}),
            destination_reference: json!({}),
            idempotency_key: "record-file".into(),
            schedule_id: None,
            configuration_snapshot: None,
        })
        .await
        .unwrap();
    drain_extension_operations(&repository, store).await;
    let checkpoint: Value =
        sqlx::query_scalar("SELECT checkpoint FROM extension_operation_runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        checkpoint["transfer"]["record_file"]["text"], "product photo",
        "{checkpoint}"
    );
    assert_eq!(
        checkpoint["transfer"]["record_file"]["media_type"],
        "text/plain"
    );
    let again = &checkpoint["transfer"]["record_file_again"];
    assert_eq!(again["text"], "product photo", "{checkpoint}");
    assert_eq!(again["result"]["replayed"], true);
    assert_eq!(
        again["result"]["artifact_id"],
        checkpoint["transfer"]["record_file"]["result"]["artifact_id"]
    );

    // Disabling the extension stops its links working.
    repository.disable_extension(EXTENSION).await.unwrap();
    assert_eq!(
        anonymous
            .get(format!("{base_url}/public/extension-files/{token}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    // Disabling revoked the link, so enabling again does not revive it.
    repository.enable_extension(EXTENSION).await.unwrap();
    let public_status = |token: String| {
        let anonymous = anonymous.clone();
        let base_url = base_url.clone();
        async move {
            anonymous
                .get(format!("{base_url}/public/extension-files/{token}"))
                .send()
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(public_status(token.to_owned()).await, StatusCode::NOT_FOUND);
    // Deleting the record a link was issued for stops it too.
    let link: Value = serde_json::from_str(
        &runtime
            .invoke_command(
                &installation,
                repository.clone(),
                "call",
                &json!({"operation": "files.link.v1", "request": {
                    "file_id": file_id, "record_id": record_id, "expires_in_seconds": 7200
                }})
                .to_string(),
                65536,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    let token = link["ok"]["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://attricat.example/api/public/extension-files/")
        .unwrap()
        .to_owned();
    assert_eq!(public_status(token.clone()).await, StatusCode::OK);
    client
        .delete(format!("{base_url}/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(public_status(token).await, StatusCode::NOT_FOUND);

    // Links are deleted a day after they expire.
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM extension_file_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE extension_file_links SET expires_at = clock_timestamp() - interval '2 days'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE extension_file_links SET expires_at = clock_timestamp() - interval '12 hours' WHERE id = (SELECT id FROM extension_file_links ORDER BY created_at LIMIT 1)")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        AttricatRepository::system(pool.clone())
            .purge_extension_file_links_batch(100)
            .await
            .unwrap(),
        links.unsigned_abs() - 1
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn moving_the_token_endpoint_to_another_host_needs_the_secret_and_drops_tokens(
    pool: sqlx::PgPool,
) {
    shared_mock();
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    api::extension_installer::ExtensionInstaller::new(repository.clone(), store)
        .install("test", &archive())
        .await
        .unwrap();
    repository
        .create_extension_connection(connection("rotating", "refresh_token"))
        .await
        .unwrap();
    let moved = |client_secret: Option<&str>| ExtensionConnectionPatch {
        display_name: None,
        token_url: Some("https://other.mock.test/oauth/token".into()),
        device_authorization_url: None,
        authorization_url: None,
        scope: None,
        parameters: None,
        client_secret: client_secret.map(str::to_owned),
        refresh_token: None,
    };
    // A device endpoint at the token endpoint's origin moves nothing.
    let same_origin = repository
        .update_extension_connection(
            EXTENSION,
            "rotating",
            ExtensionConnectionPatch {
                display_name: None,
                token_url: None,
                device_authorization_url: Some(Some("https://shop.mock.test/oauth/device".into())),
                authorization_url: None,
                scope: None,
                parameters: None,
                client_secret: None,
                refresh_token: None,
            },
        )
        .await
        .unwrap();
    assert!(same_origin.has_refresh_token);
    assert_eq!(same_origin.status, ConnectionStatus::Active);
    // The stored secret is never sent to another host unasked.
    assert!(
        repository
            .update_extension_connection(EXTENSION, "rotating", moved(None))
            .await
            .is_err()
    );
    let updated = repository
        .update_extension_connection(EXTENSION, "rotating", moved(Some("n3w")))
        .await
        .unwrap();
    assert!(updated.has_client_secret);
    assert!(!updated.has_refresh_token);
    assert!(!updated.has_access_token);
    assert_eq!(updated.status, ConnectionStatus::PendingAuthorization);
}

fn base64_decode(value: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .unwrap()
}

/// Signs the bootstrap owner in with a password and returns a client that
/// carries the browser session and CSRF cookies, and the cookie header alone.
async fn browser_session(base_url: &str) -> (reqwest::Client, String) {
    let login = reqwest::Client::new()
        .post(format!("{base_url}/auth/login"))
        .json(&json!({
            "login_identifier": "default.local",
            "email": "api-test-owner@example.test",
            "password": "correct horse battery staple"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let pairs: Vec<String> = login
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    let cookie = pairs.join("; ");
    let csrf = pairs
        .iter()
        .find_map(|pair| pair.strip_prefix("attricat_csrf="))
        .unwrap()
        .to_owned();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    headers.insert("x-attricat-csrf", csrf.parse().unwrap());
    (
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap(),
        cookie,
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn operators_authorize_connections_in_the_browser(pool: sqlx::PgPool) {
    let (_, seen) = shared_mock();
    connections::use_oauth_callback(connections::OAuthCallback {
        redirect_uri: OAUTH_CALLBACK.parse().unwrap(),
        return_base: "https://attricat.test/manage/extensions/".parse().unwrap(),
    });
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    api::extension_installer::ExtensionInstaller::new(repository.clone(), store.clone())
        .install("test", &archive())
        .await
        .unwrap();
    for capability in PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "capability", capability)
            .await
            .unwrap();
    }
    for rule in HOST_PERMISSIONS {
        repository
            .grant_extension(EXTENSION, "host_permission", rule)
            .await
            .unwrap();
    }
    repository.enable_extension(EXTENSION).await.unwrap();
    // Browser authorization is bound to a signed-in browser session.
    let (base_url, server) = start_session_server(pool.clone()).await;
    let hash =
        api::account::hash_password(&api::account::Password::new("correct horse battery staple"))
            .unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(BOOTSTRAP_OWNER_ID.parse::<uuid::Uuid>().unwrap())
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let (client, cookie) = browser_session(&base_url).await;
    let (_, other_cookie) = browser_session(&base_url).await;
    let connections = format!("{base_url}/extensions/{EXTENSION}/connections");
    // The provider's redirect arrives as a top-level navigation; its answer
    // is a redirect the test inspects rather than follows.
    let browser = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let callback = |query: String, cookie: Option<String>| {
        let browser = browser.clone();
        let url = format!("{base_url}/public/extension-oauth/callback?{query}");
        async move {
            let mut request = browser.get(url);
            if let Some(cookie) = cookie {
                request = request.header("cookie", cookie);
            }
            let response = request.send().await.unwrap();
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()["cache-control"], "no-store");
            response.headers()["location"].to_str().unwrap().to_owned()
        }
    };
    let returned =
        |outcome: &str| format!("https://attricat.test/manage/extensions/?authorization={outcome}");

    // The authorization URL must be plain HTTPS; Attricat adds the query.
    expect_status(
        client
            .post(&connections)
            .json(&json!({"connection": "bad", "kind": "shop", "display_name": "Bad",
                          "grant_type": "authorization_code", "token_url": "https://shop.mock.test/oauth/token",
                          "authorization_url": "https://shop.mock.test/oauth/authorize?x=1",
                          "client_id": "client", "client_secret": "s3cret"}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    let created = expect_status(
        client
            .post(&connections)
            .json(&json!({"connection": "web", "kind": "shop", "display_name": "Web shop",
                          "grant_type": "authorization_code", "token_url": "https://shop.mock.test/oauth/token",
                          "authorization_url": "https://shop.mock.test/oauth/authorize",
                          "client_id": "client", "client_secret": "s3cret", "scope": "listings"}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(created["status"], "pending_authorization");
    assert_eq!(created["redirect_uri"], OAUTH_CALLBACK);
    assert_eq!(
        get_json(&client, &connections).await[0]["redirect_uri"],
        OAUTH_CALLBACK
    );

    let start = || {
        let client = client.clone();
        let url = format!("{connections}/web/authorization");
        async move {
            let started =
                expect_status(client.post(url).send().await.unwrap(), StatusCode::OK).await;
            let url = url::Url::parse(started["authorization_url"].as_str().unwrap()).unwrap();
            assert_eq!(url.path(), "/oauth/authorize");
            url.query_pairs()
                .into_owned()
                .collect::<std::collections::HashMap<_, _>>()
        }
    };
    let request = start().await;
    assert_eq!(request["response_type"], "code");
    assert_eq!(request["client_id"], "client");
    assert_eq!(request["redirect_uri"], OAUTH_CALLBACK);
    assert_eq!(request["scope"], "listings");
    assert_eq!(request["code_challenge_method"], "S256");
    assert_eq!(request["state"].len(), 43);
    // The state and verifier are stored only as a digest and ciphertext.
    let stored: String = sqlx::query_scalar(
        "SELECT authorization_state_sha256 FROM extension_connections WHERE connection='web'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(stored, request["state"]);
    seen.lock().unwrap().code_challenge = Some(request["code_challenge"].clone());
    let query = |pairs: &[(&str, &str)]| {
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish()
    };
    let approved = query(&[
        ("state", &request["state"]),
        ("code", "code-1"),
        ("iss", "https://shop.mock.test"),
    ]);
    // A browser without a session, or with someone else's session, cannot
    // complete the authorization, nor use it up.
    assert_eq!(
        callback(approved.clone(), None).await,
        returned("signed_out")
    );
    assert_eq!(
        callback(approved.clone(), Some("attricat_session=forged".into())).await,
        returned("signed_out")
    );
    assert_eq!(
        callback(approved.clone(), Some(other_cookie.clone())).await,
        returned("expired")
    );
    assert_eq!(
        get_json(&client, &connections).await[0]["status"],
        "pending_authorization"
    );
    // The operator's own browser completes it.
    assert_eq!(
        callback(approved.clone(), Some(cookie.clone())).await,
        format!(
            "https://attricat.test/manage/extensions/{EXTENSION}?connection=web&authorization=connected"
        )
    );
    let active = get_json(&client, &connections).await;
    assert_eq!(active[0]["status"], "active");
    assert_eq!(active[0]["has_refresh_token"], true);
    assert!(active[0]["authorization_expires_at"].is_null());
    // A state works once: replaying the redirect changes nothing.
    assert_eq!(
        callback(approved, Some(cookie.clone())).await,
        returned("expired")
    );
    assert_eq!(
        callback(
            query(&[("state", "unknown"), ("code", "code-1")]),
            Some(cookie.clone())
        )
        .await,
        returned("expired")
    );

    // Declining at the provider fails the authorization, not the connection
    // that is already active; another session's decline changes nothing.
    let declined = start().await;
    let refused = query(&[("state", &declined["state"]), ("error", "access_denied")]);
    assert_eq!(
        callback(refused.clone(), Some(other_cookie)).await,
        returned("expired")
    );
    assert!(get_json(&client, &connections).await[0]["last_error"].is_null());
    assert_eq!(
        callback(refused, Some(cookie.clone())).await,
        format!(
            "https://attricat.test/manage/extensions/{EXTENSION}?connection=web&authorization=failed"
        )
    );
    let after = get_json(&client, &connections).await;
    assert_eq!(after[0]["status"], "active");
    assert!(
        after[0]["last_error"]
            .as_str()
            .unwrap()
            .contains("access_denied")
    );
    for secret in ["s3cret", "tok-code", "\"rc\"", declined["state"].as_str()] {
        assert!(!after.to_string().contains(secret), "{secret}");
    }

    // An authorization nobody completed is cleared once it has expired.
    let abandoned = start().await;
    assert_eq!(
        AttricatRepository::system(pool.clone())
            .purge_extension_authorization_requests_batch(100)
            .await
            .unwrap(),
        0
    );
    sqlx::query("UPDATE extension_connections SET authorization_expires_at=clock_timestamp() - interval '1 second' WHERE connection='web'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        AttricatRepository::system(pool.clone())
            .purge_extension_authorization_requests_batch(100)
            .await
            .unwrap(),
        1
    );
    let pending: Option<String> = sqlx::query_scalar(
        "SELECT authorization_state_sha256 FROM extension_connections WHERE connection='web'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(pending.is_none());
    let late = query(&[("state", &abandoned["state"]), ("code", "code-1")]);
    assert_eq!(callback(late, Some(cookie)).await, returned("expired"));
    server.abort();
}
