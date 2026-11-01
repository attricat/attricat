use crate::support;

use api::{
    http::{AppState, OidcSettings, OperatorToken},
    repository::{AttricatRepository, use_identity_provider_credential_key},
};
use reqwest::Response;
use support::*;

const OPERATOR_SECRET: &str = "operator-test-token-with-enough-bytes";
const CALLBACK: &str = "https://attricat.test/api/auth/oidc/callback";
const ENDPOINT: &str = "/workspace/identity-provider";

async fn start(pool: &PgPool) -> (String, JoinHandle<()>) {
    use_identity_provider_credential_key([9; 32]);
    start_configured_trusted_server(pool.clone(), |state: &mut AppState| {
        state.operator_token = Some(OperatorToken::new(OPERATOR_SECRET));
        state.oidc = Some(OidcSettings {
            callback_url: CALLBACK.parse().unwrap(),
            allowed_private_hosts: Vec::new().into(),
            extra_root_certificates_pem: None,
        });
    })
    .await
}

fn provider(issuer: &str) -> Value {
    json!({
        "issuer": issuer,
        "client_id": "attricat",
        "client_secret": "first-secret",
        "display_name": "Acme SSO",
    })
}

async fn put(client: &Client, base_url: &str, body: &Value) -> Response {
    client
        .put(format!("{base_url}{ENDPOINT}"))
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn patch(client: &Client, base_url: &str, body: &Value) -> Response {
    client
        .patch(format!("{base_url}{ENDPOINT}"))
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn error_code(response: Response) -> String {
    let body: Value = response.json().await.unwrap();
    body["error"]["code"].as_str().unwrap().to_owned()
}

async fn mark_verified(pool: &PgPool) {
    sqlx::query("UPDATE workspace_identity_providers SET verified_at = now()")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn the_secret_is_stored_encrypted_and_never_returned(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let client = authenticated_client();
    let missing = client
        .get(format!("{base_url}{ENDPOINT}"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let response = put(&client, &base_url, &provider("https://idp.example.com")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["issuer"], "https://idp.example.com");
    assert_eq!(body["display_name"], "Acme SSO");
    assert_eq!(body["enabled"], false);
    assert_eq!(body["password_sign_in"], "all");
    assert!(body["verified_at"].is_null());
    assert_eq!(body["redirect_uri"], CALLBACK);
    assert!(!body.to_string().contains("first-secret"));
    assert!(body.get("client_secret_ciphertext").is_none());

    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT client_secret_ciphertext FROM workspace_identity_providers")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&stored).contains("first-secret"));
    let sign_in = AttricatRepository::system(pool.clone())
        .identity_provider_for_sign_in(bootstrap_workspace_id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sign_in.client_secret, "first-secret");
    assert!(!format!("{sign_in:?}").contains("first-secret"));

    // Leaving the secret out keeps it, and a new one replaces it.
    let mut keep = provider("https://idp.example.com");
    keep.as_object_mut().unwrap().remove("client_secret");
    assert_eq!(
        put(&client, &base_url, &keep).await.status(),
        StatusCode::OK
    );
    let repository = AttricatRepository::system(pool.clone());
    let current = repository
        .identity_provider_for_sign_in(bootstrap_workspace_id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.client_secret, "first-secret");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn invalid_providers_are_refused(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let client = authenticated_client();
    let mut no_secret = provider("https://idp.example.com");
    no_secret.as_object_mut().unwrap().remove("client_secret");
    let mut no_name = provider("https://idp.example.com");
    no_name["display_name"] = json!("  ");
    let mut no_client = provider("https://idp.example.com");
    no_client["client_id"] = json!("");
    for body in [
        no_secret,
        no_name,
        no_client,
        provider("http://idp.example.com"),
        provider("https://idp.example.com?tenant=a"),
    ] {
        let response = put(&client, &base_url, &body).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}"
        );
        assert_eq!(error_code(response).await, "invalid_identity_provider");
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_provider_must_be_verified_and_enabled_before_restricting_passwords(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let client = authenticated_client();
    put(&client, &base_url, &provider("https://idp.example.com")).await;

    let early = patch(&client, &base_url, &json!({ "enabled": true })).await;
    assert_eq!(early.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(early).await, "identity_provider_not_verified");
    mark_verified(&pool).await;

    let restricted = patch(&client, &base_url, &json!({ "password_sign_in": "owners" })).await;
    assert_eq!(restricted.status(), StatusCode::CONFLICT);
    assert_eq!(
        error_code(restricted).await,
        "identity_provider_not_enabled"
    );

    let on = patch(
        &client,
        &base_url,
        &json!({ "enabled": true, "password_sign_in": "owners" }),
    )
    .await;
    assert_eq!(on.status(), StatusCode::OK);
    let on: Value = on.json().await.unwrap();
    assert_eq!(on["enabled"], true);
    assert_eq!(on["password_sign_in"], "owners");

    let unknown = patch(&client, &base_url, &json!({ "password_sign_in": "nobody" })).await;
    assert_eq!(error_code(unknown).await, "invalid_identity_provider");

    // Turning it off brings passwords back for everyone.
    let off = patch(&client, &base_url, &json!({ "enabled": false })).await;
    let off: Value = off.json().await.unwrap();
    assert_eq!(off["enabled"], false);
    assert_eq!(off["password_sign_in"], "all");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn only_a_new_issuer_or_client_discards_verification(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let client = authenticated_client();
    put(&client, &base_url, &provider("https://idp.example.com")).await;
    mark_verified(&pool).await;
    patch(
        &client,
        &base_url,
        &json!({ "enabled": true, "password_sign_in": "owners" }),
    )
    .await;

    // Renaming it and rotating its secret keep it working.
    let mut rotated = provider("https://idp.example.com");
    rotated["display_name"] = json!("Renamed");
    rotated["client_secret"] = json!("second-secret");
    let rotated: Value = put(&client, &base_url, &rotated)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(rotated["display_name"], "Renamed");
    assert_eq!(rotated["enabled"], true);
    assert_eq!(rotated["password_sign_in"], "owners");
    assert!(rotated["verified_at"].is_string());

    // A different provider is unproven, and nobody is locked out by it.
    let moved: Value = put(&client, &base_url, &provider("https://other.example.com"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(moved["enabled"], false);
    assert_eq!(moved["password_sign_in"], "all");
    assert!(moved["verified_at"].is_null());
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn managing_a_provider_requires_workspace_management(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let viewer = member_with_role(&pool, VIEWER_ROLE_ID).await;
    let client = client_for(viewer);
    let read = client
        .get(format!("{base_url}{ENDPOINT}"))
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::FORBIDDEN);
    let write = put(&client, &base_url, &provider("https://idp.example.com")).await;
    assert_eq!(write.status(), StatusCode::FORBIDDEN);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn the_operator_can_configure_any_workspace(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    let client = Client::new();
    let url = format!(
        "{base_url}/operator/workspaces/{}/identity-provider",
        bootstrap_workspace_id()
    );
    let anonymous = client
        .put(&url)
        .json(&provider("https://idp.example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let pushed = client
        .put(&url)
        .bearer_auth(OPERATOR_SECRET)
        .json(&provider("https://idp.example.com"))
        .send()
        .await
        .unwrap();
    assert_eq!(pushed.status(), StatusCode::OK);
    let read = client
        .get(&url)
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    let read: Value = read.json().await.unwrap();
    assert_eq!(read["client_id"], "attricat");

    // The operator cannot skip the test sign-in either.
    let enable = client
        .patch(&url)
        .bearer_auth(OPERATOR_SECRET)
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(enable.status(), StatusCode::CONFLICT);

    let (actor, metadata): (Option<Uuid>, Value) = sqlx::query_as(
        "SELECT actor_user_id, metadata FROM audit_events WHERE action = 'workspace.identity_provider.update'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(actor.is_none());
    assert_eq!(metadata["actor"], "operator");

    let deleted = client
        .delete(&url)
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let missing_workspace = client
        .get(format!(
            "{base_url}/operator/workspaces/{}/identity-provider",
            Uuid::new_v4()
        ))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap();
    assert_eq!(missing_workspace.status(), StatusCode::NOT_FOUND);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_member_change_is_audited_as_the_member(pool: PgPool) {
    let (base_url, server) = start(&pool).await;
    put(
        &authenticated_client(),
        &base_url,
        &provider("https://idp.example.com"),
    )
    .await;
    let actor: Option<Uuid> = sqlx::query_scalar(
        "SELECT actor_user_id FROM audit_events WHERE action = 'workspace.identity_provider.update'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, Some(BOOTSTRAP_OWNER_ID.parse().unwrap()));
    server.abort();
}
