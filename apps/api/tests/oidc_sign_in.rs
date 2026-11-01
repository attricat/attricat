use crate::support;
use std::sync::Arc;

use api::{
    account::{Password, hash_password},
    http::{AppState, OidcSettings, OperatorToken},
    repository::use_identity_provider_credential_key,
};
use reqwest::{Response, Url, header::SET_COOKIE, redirect::Policy};
use support::fake_oidc::*;
use support::*;

const CALLBACK: &str = "https://attricat.test/api/auth/oidc/callback";
const OPERATOR_SECRET: &str = "operator-test-token-with-enough-bytes";
const PASSWORD: &str = "Correct-horse-battery-7-staple!";
const OWNER_EMAIL: &str = "api-test-owner@example.test";

struct Harness {
    base_url: String,
    mail: Arc<TestMailDelivery>,
    provider: FakeOidcProvider,
    pool: PgPool,
    server: JoinHandle<()>,
}

/// The cookies a browser would hold.
#[derive(Clone, Default)]
struct Browser {
    login_cookie: Option<String>,
    session: Option<String>,
    csrf: Option<String>,
}

impl Browser {
    fn cookie_header(&self) -> String {
        [&self.login_cookie, &self.session, &self.csrf]
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn absorb(&mut self, response: &Response) {
        for value in response.headers().get_all(SET_COOKIE) {
            let pair = value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned();
            let (name, content) = pair.split_once('=').unwrap();
            let slot = match name {
                "attricat_oidc" => &mut self.login_cookie,
                "attricat_session" => &mut self.session,
                "attricat_csrf" => &mut self.csrf,
                _ => continue,
            };
            *slot = (!content.is_empty()).then(|| pair.clone());
        }
    }

    fn csrf_value(&self) -> String {
        self.csrf
            .as_deref()
            .unwrap()
            .strip_prefix("attricat_csrf=")
            .unwrap()
            .to_owned()
    }
}

fn client() -> Client {
    Client::builder().redirect(Policy::none()).build().unwrap()
}

async fn start(pool: &PgPool) -> Harness {
    use_identity_provider_credential_key([9; 32]);
    let provider = FakeOidcProvider::start().await;
    let mail = Arc::new(TestMailDelivery::default());
    let (base_url, server) =
        start_configured_session_server(pool.clone(), mail.clone(), |state: &mut AppState| {
            state.devtools_enabled = true;
            state.operator_token = Some(OperatorToken::new(OPERATOR_SECRET));
            state.oidc = Some(OidcSettings {
                callback_url: CALLBACK.parse().unwrap(),
                allowed_private_hosts: vec!["127.0.0.1".to_owned()].into(),
                extra_root_certificates_pem: None,
            });
        })
        .await;
    Harness {
        base_url,
        mail,
        provider,
        pool: pool.clone(),
        server,
    }
}

impl Harness {
    /// Stores the fake provider for a workspace, through the operator API.
    async fn configure(&self, workspace: Uuid) {
        let response = client()
            .put(format!(
                "{}/operator/workspaces/{workspace}/identity-provider",
                self.base_url
            ))
            .bearer_auth(OPERATOR_SECRET)
            .json(&json!({
                "issuer": self.provider.issuer,
                "client_id": FAKE_CLIENT_ID,
                "client_secret": FAKE_CLIENT_SECRET,
                "display_name": "Fake SSO",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    async fn enable(&self, workspace: Uuid) {
        sqlx::query("UPDATE workspace_identity_providers SET verified_at = now(), enabled = true WHERE workspace_id = $1")
            .bind(workspace)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    /// Links an identity directly, for tests about what comes after linking.
    async fn link(&self, user: Uuid, subject: &str) {
        sqlx::query(
            "INSERT INTO external_identities (issuer, subject, user_id) VALUES ($1, $2, $3)",
        )
        .bind(&self.provider.issuer)
        .bind(subject)
        .bind(user)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn begin(&self, browser: &mut Browser, body: &Value) -> Response {
        let response = client()
            .post(format!("{}/auth/oidc/start", self.base_url))
            .json(body)
            .send()
            .await
            .unwrap();
        browser.absorb(&response);
        response
    }

    /// Follows the provider's redirect and returns the callback's query.
    async fn visit_provider(&self, authorization_url: &str) -> String {
        let response = client().get(authorization_url).send().await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::SEE_OTHER,
            "{authorization_url}"
        );
        let location = response.headers()["location"].to_str().unwrap();
        assert!(location.starts_with(CALLBACK), "{location}");
        Url::parse(location).unwrap().query().unwrap().to_owned()
    }

    async fn call_back(&self, browser: &mut Browser, query: &str) -> Url {
        let response = client()
            .get(format!("{}/auth/oidc/callback?{query}", self.base_url))
            .header("cookie", browser.cookie_header())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        browser.absorb(&response);
        Url::parse(response.headers()["location"].to_str().unwrap()).unwrap()
    }

    /// A whole sign-in, returning where it left the browser.
    async fn sign_in(&self, browser: &mut Browser, body: &Value) -> Url {
        let response = self.begin(browser, body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let started: Value = response.json().await.unwrap();
        let query = self
            .visit_provider(started["authorization_url"].as_str().unwrap())
            .await;
        self.call_back(browser, &query).await
    }

    async fn session_of(&self, browser: &Browser) -> Response {
        client()
            .get(format!("{}/auth/session", self.base_url))
            .header("cookie", browser.cookie_header())
            .send()
            .await
            .unwrap()
    }

    async fn sign_in_with_password(&self, identifier: &str, email: &str) -> Browser {
        let response = client()
            .post(format!("{}/auth/login", self.base_url))
            .json(&json!({
                "login_identifier": identifier,
                "email": email,
                "password": PASSWORD,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut browser = Browser::default();
        browser.absorb(&response);
        browser
    }

    async fn set_owner_password(&self) {
        let hash = hash_password(&Password::new(PASSWORD)).unwrap();
        sqlx::query(
            "INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)",
        )
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .bind(hash.as_phc())
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// A member of the bootstrap workspace with no password.
    async fn sso_member(&self, email: &str) -> (Uuid, Uuid) {
        let user = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, email_verified_at) VALUES ($1, $2, now())")
            .bind(user)
            .bind(email)
            .execute(&self.pool)
            .await
            .unwrap();
        let membership = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
        )
        .bind(membership)
        .bind(bootstrap_workspace_id())
        .bind(user)
        .execute(&self.pool)
        .await
        .unwrap();
        (user, membership)
    }
}

fn sign_in_body() -> Value {
    json!({ "login_identifier": "default.local" })
}

fn error_of(location: &Url) -> Option<String> {
    location
        .query_pairs()
        .find_map(|(name, value)| (name == "error").then(|| value.into_owned()))
}

async fn ready(pool: &PgPool) -> Harness {
    let harness = start(pool).await;
    harness.configure(bootstrap_workspace_id()).await;
    harness.enable(bootstrap_workspace_id()).await;
    harness
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_linked_identity_signs_in_without_a_password(pool: PgPool) {
    let harness = ready(&pool).await;
    let (user, _) = harness.sso_member("sso@example.test").await;
    harness.link(user, "subject-1").await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("subject-1", "sso@example.test"))
        .await;

    let mut browser = Browser::default();
    let location = harness.sign_in(&mut browser, &sign_in_body()).await;
    assert_eq!(location.path(), "/oidc/complete");
    assert_eq!(error_of(&location), None);
    assert!(browser.login_cookie.is_none());
    let session = harness.session_of(&browser).await;
    assert_eq!(session.status(), StatusCode::OK);
    let session: Value = session.json().await.unwrap();
    assert_eq!(session["email"], "sso@example.test");
    assert_eq!(session["login_identifier"], "default.local");

    let (action, actor, metadata): (String, Option<Uuid>, Value) = sqlx::query_as(
        "SELECT action, actor_user_id, metadata FROM audit_events WHERE action = 'auth.sign_in'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "auth.sign_in");
    assert_eq!(actor, Some(user));
    assert_eq!(metadata["method"], "oidc");

    // Signing out is recorded too, and ends the session.
    let signed_out = client()
        .post(format!("{}/auth/logout", harness.base_url))
        .header("cookie", browser.cookie_header())
        .header("x-attricat-csrf", browser.csrf_value())
        .send()
        .await
        .unwrap();
    assert!(signed_out.status().is_success());
    assert_eq!(
        harness.session_of(&browser).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let sign_outs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit_events WHERE action = 'auth.sign_out'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sign_outs, 1);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn discovery_offers_only_an_enabled_provider(pool: PgPool) {
    let harness = start(&pool).await;
    let discover = || async {
        let response = client()
            .post(format!("{}/auth/discover", harness.base_url))
            .json(&sign_in_body())
            .send()
            .await
            .unwrap();
        response.json::<Value>().await.unwrap()
    };
    assert_eq!(discover().await["oidc"], Value::Null);
    harness.configure(bootstrap_workspace_id()).await;
    assert_eq!(
        discover().await["sign_in_methods"],
        json!(["local_password"])
    );
    harness.enable(bootstrap_workspace_id()).await;
    let body = discover().await;
    assert_eq!(body["sign_in_methods"], json!(["local_password", "oidc"]));
    assert_eq!(body["oidc"]["display_name"], "Fake SSO");
    assert_eq!(body["password_restricted"], false);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_provider_that_is_not_enabled_cannot_start_a_sign_in(pool: PgPool) {
    let harness = start(&pool).await;
    harness.configure(bootstrap_workspace_id()).await;
    let response = harness
        .begin(&mut Browser::default(), &sign_in_body())
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_unlinked_identity_gets_no_session_even_with_a_matching_email(pool: PgPool) {
    let harness = ready(&pool).await;
    harness.sso_member("sso@example.test").await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("never-linked", "sso@example.test"))
        .await;
    let mut browser = Browser::default();
    let location = harness.sign_in(&mut browser, &sign_in_body()).await;
    assert_eq!(
        error_of(&location).as_deref(),
        Some("oidc_identity_not_linked")
    );
    assert!(browser.session.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn forged_or_mistargeted_tokens_are_rejected(pool: PgPool) {
    let harness = ready(&pool).await;
    let (user, _) = harness.sso_member("sso@example.test").await;
    harness.link(user, "subject-1").await;
    let base = FakeLogin::new("subject-1", "sso@example.test");
    let cases = [
        (
            "wrong key",
            FakeLogin {
                signed_by_other_key: true,
                ..base.clone()
            },
        ),
        (
            "unsigned",
            FakeLogin {
                unsigned: true,
                ..base.clone()
            },
        ),
        (
            "wrong audience",
            FakeLogin {
                audience: Some("another-client".to_owned()),
                ..base.clone()
            },
        ),
        (
            "wrong issuer",
            FakeLogin {
                issuer: Some("https://evil.example".to_owned()),
                ..base.clone()
            },
        ),
        (
            "wrong nonce",
            FakeLogin {
                nonce: Some("replayed-nonce".to_owned()),
                ..base.clone()
            },
        ),
        (
            "expired",
            FakeLogin {
                expired: true,
                ..base.clone()
            },
        ),
    ];
    for (name, login) in cases {
        harness.provider.sign_in_as(login).await;
        let mut browser = Browser::default();
        let location = harness.sign_in(&mut browser, &sign_in_body()).await;
        assert_eq!(
            error_of(&location).as_deref(),
            Some("oidc_sign_in_failed"),
            "{name}"
        );
        assert!(browser.session.is_none(), "{name}");
    }
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_callback_works_once_and_only_in_the_browser_that_started_it(pool: PgPool) {
    let harness = ready(&pool).await;
    let (user, _) = harness.sso_member("sso@example.test").await;
    harness.link(user, "subject-1").await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("subject-1", "sso@example.test"))
        .await;
    let mut browser = Browser::default();
    let response = harness.begin(&mut browser, &sign_in_body()).await;
    let started: Value = response.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;

    // Login CSRF: an attacker's code sent to a victim's browser does nothing,
    // and does not use the request up.
    let mut victim = Browser::default();
    let location = harness.call_back(&mut victim, &query).await;
    assert_eq!(error_of(&location).as_deref(), Some("oidc_request_expired"));
    assert!(victim.session.is_none());
    let mut other = Browser::default();
    harness.begin(&mut other, &sign_in_body()).await;
    let location = harness.call_back(&mut other, &query).await;
    assert_eq!(error_of(&location).as_deref(), Some("oidc_request_expired"));

    let mut replay = browser.clone();
    let location = harness.call_back(&mut browser, &query).await;
    assert_eq!(error_of(&location), None);
    assert!(browser.session.is_some());
    let location = harness.call_back(&mut replay, &query).await;
    assert_eq!(error_of(&location).as_deref(), Some("oidc_request_expired"));
    assert!(replay.session.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn provider_errors_and_a_mismatched_issuer_end_the_sign_in(pool: PgPool) {
    let harness = ready(&pool).await;
    let mut browser = Browser::default();
    let response = harness.begin(&mut browser, &sign_in_body()).await;
    let started: Value = response.json().await.unwrap();
    let url = Url::parse(started["authorization_url"].as_str().unwrap()).unwrap();
    let state = url
        .query_pairs()
        .find_map(|(name, value)| (name == "state").then(|| value.into_owned()))
        .unwrap();
    let location = harness
        .call_back(
            &mut browser.clone(),
            &format!("state={state}&error=access_denied"),
        )
        .await;
    assert_eq!(error_of(&location).as_deref(), Some("oidc_sign_in_failed"));

    let mut browser = Browser::default();
    let response = harness.begin(&mut browser, &sign_in_body()).await;
    let started: Value = response.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;
    let location = harness
        .call_back(
            &mut browser,
            &format!("{query}&iss=https%3A%2F%2Fevil.example"),
        )
        .await;
    assert_eq!(error_of(&location).as_deref(), Some("oidc_sign_in_failed"));
    assert!(browser.session.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_identity_signs_in_only_where_its_issuer_is_the_provider_and_it_is_a_member(
    pool: PgPool,
) {
    let harness = ready(&pool).await;
    let (user, _) = harness.sso_member("sso@example.test").await;
    harness.link(user, "subject-1").await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("subject-1", "sso@example.test"))
        .await;

    // Same provider, but this person is not a member there.
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'other', 'Other', 'other.example')")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    harness.configure(other).await;
    harness.enable(other).await;
    let mut browser = Browser::default();
    let location = harness
        .sign_in(
            &mut browser,
            &json!({ "login_identifier": "other.example" }),
        )
        .await;
    assert_eq!(
        error_of(&location).as_deref(),
        Some("oidc_identity_not_linked")
    );
    assert!(browser.session.is_none());

    // A member of a workspace that trusts a different issuer: the identity
    // above proves nothing there.
    let second = FakeOidcProvider::start().await;
    let moved = client()
        .put(format!(
            "{}/operator/workspaces/{}/identity-provider",
            harness.base_url,
            bootstrap_workspace_id()
        ))
        .bearer_auth(OPERATOR_SECRET)
        .json(&json!({
            "issuer": second.issuer,
            "client_id": FAKE_CLIENT_ID,
            "client_secret": FAKE_CLIENT_SECRET,
            "display_name": "Second",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status(), StatusCode::OK);
    harness.enable(bootstrap_workspace_id()).await;
    second
        .sign_in_as(FakeLogin::new("subject-1", "sso@example.test"))
        .await;
    let mut browser = Browser::default();
    let location = harness.sign_in(&mut browser, &sign_in_body()).await;
    assert_eq!(
        error_of(&location).as_deref(),
        Some("oidc_identity_not_linked")
    );
    assert!(browser.session.is_none());
    second.server.abort();
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_unsafe_provider_address_is_refused(pool: PgPool) {
    use_identity_provider_credential_key([9; 32]);
    let provider = FakeOidcProvider::start().await;
    let (base_url, server) = start_configured_session_server(
        pool.clone(),
        Arc::new(TestMailDelivery::default()),
        |state: &mut AppState| {
            state.devtools_enabled = true;
            state.oidc = Some(OidcSettings {
                callback_url: CALLBACK.parse().unwrap(),
                allowed_private_hosts: Vec::new().into(),
                extra_root_certificates_pem: None,
            });
        },
    )
    .await;
    let response = api::repository::AttricatRepository::new(pool.clone(), bootstrap_workspace_id())
        .put_identity_provider(&api::repository::IdentityProviderInput {
            issuer: &provider.issuer,
            client_id: FAKE_CLIENT_ID,
            client_secret: Some(FAKE_CLIENT_SECRET),
            display_name: "Loopback",
        })
        .await;
    assert!(response.is_ok());
    sqlx::query("UPDATE workspace_identity_providers SET verified_at = now(), enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    let started = client()
        .post(format!("{base_url}/auth/oidc/start"))
        .json(&sign_in_body())
        .send()
        .await
        .unwrap();
    assert_eq!(started.status(), StatusCode::BAD_GATEWAY);
    let body: Value = started.json().await.unwrap();
    assert_eq!(body["error"]["code"], "oidc_provider_unavailable");
    server.abort();
    provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_test_sign_in_links_the_admin_and_verifies_the_provider(pool: PgPool) {
    let harness = start(&pool).await;
    harness.configure(bootstrap_workspace_id()).await;
    harness.set_owner_password().await;
    let mut browser = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("owner-subject", "owner@corp.example"))
        .await;

    let status = client()
        .get(format!("{}/auth/oidc/link", harness.base_url))
        .header("cookie", browser.cookie_header())
        .send()
        .await
        .unwrap();
    let status: Value = status.json().await.unwrap();
    assert_eq!(status["linked"], false);
    assert_eq!(status["display_name"], "Fake SSO");

    // A cross-site request cannot start a link.
    let no_csrf = client()
        .post(format!("{}/auth/oidc/link", harness.base_url))
        .header("cookie", browser.cookie_header())
        .send()
        .await
        .unwrap();
    assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);

    let started = client()
        .post(format!("{}/auth/oidc/link", harness.base_url))
        .header("cookie", browser.cookie_header())
        .header("x-attricat-csrf", browser.csrf_value())
        .send()
        .await
        .unwrap();
    assert_eq!(started.status(), StatusCode::OK);
    browser.absorb(&started);
    let started: Value = started.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;
    let location = harness.call_back(&mut browser, &query).await;
    assert_eq!(error_of(&location), None);
    assert!(location.query().unwrap().contains("linked=true"));

    let verified: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT verified_at FROM workspace_identity_providers")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified.is_some());
    let linked: Uuid = sqlx::query_scalar(
        "SELECT user_id FROM external_identities WHERE subject = 'owner-subject'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap());

    // Now it can be enabled, and the owner can sign in with it.
    let enabled = client()
        .patch(format!("{}/workspace/identity-provider", harness.base_url))
        .header("cookie", browser.cookie_header())
        .header("x-attricat-csrf", browser.csrf_value())
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(enabled.status(), StatusCode::OK);
    let mut fresh = Browser::default();
    let location = harness.sign_in(&mut fresh, &sign_in_body()).await;
    assert_eq!(error_of(&location), None);
    assert_eq!(harness.session_of(&fresh).await.status(), StatusCode::OK);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_member_link_does_not_verify_the_provider_and_needs_the_same_session(pool: PgPool) {
    let harness = start(&pool).await;
    harness.configure(bootstrap_workspace_id()).await;
    let viewer = member_with_role(&pool, VIEWER_ROLE_ID).await;
    let hash = hash_password(&Password::new(PASSWORD)).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(viewer)
        .bind(hash.as_phc())
        .execute(&pool)
        .await
        .unwrap();
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(viewer)
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut browser = harness.sign_in_with_password("default.local", &email).await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("viewer-subject", "viewer@corp.example"))
        .await;
    let start_link = |browser: &Browser| {
        client()
            .post(format!("{}/auth/oidc/link", harness.base_url))
            .header("cookie", browser.cookie_header())
            .header("x-attricat-csrf", browser.csrf_value())
            .send()
    };

    // Signing out before the provider returns leaves nothing to link to.
    let started = start_link(&browser).await.unwrap();
    browser.absorb(&started);
    let started: Value = started.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;
    let mut signed_out = browser.clone();
    signed_out.session = None;
    let location = harness.call_back(&mut signed_out, &query).await;
    assert_eq!(error_of(&location).as_deref(), Some("unauthenticated"));

    let started = start_link(&browser).await.unwrap();
    browser.absorb(&started);
    let started: Value = started.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;
    let location = harness.call_back(&mut browser, &query).await;
    assert_eq!(error_of(&location), None);
    let verified: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT verified_at FROM workspace_identity_providers")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_identity_linked_to_someone_else_is_never_moved(pool: PgPool) {
    let harness = start(&pool).await;
    harness.configure(bootstrap_workspace_id()).await;
    let (someone, _) = harness.sso_member("someone@example.test").await;
    harness.link(someone, "taken").await;
    harness.set_owner_password().await;
    let mut browser = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("taken", "someone@example.test"))
        .await;
    let started = client()
        .post(format!("{}/auth/oidc/link", harness.base_url))
        .header("cookie", browser.cookie_header())
        .header("x-attricat-csrf", browser.csrf_value())
        .send()
        .await
        .unwrap();
    browser.absorb(&started);
    let started: Value = started.json().await.unwrap();
    let query = harness
        .visit_provider(started["authorization_url"].as_str().unwrap())
        .await;
    let location = harness.call_back(&mut browser, &query).await;
    assert_eq!(
        error_of(&location).as_deref(),
        Some("oidc_identity_linked_elsewhere")
    );
    let owner: Uuid =
        sqlx::query_scalar("SELECT user_id FROM external_identities WHERE subject = 'taken'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owner, someone);
    let verified: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT verified_at FROM workspace_identity_providers")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(verified.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

/// Creates an owner invitation through the operator API and returns the
/// invitation secret from its email.
async fn invited_workspace(harness: &Harness, email: &str) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let response = client()
        .put(format!("{}/operator/workspaces/{id}", harness.base_url))
        .bearer_auth(OPERATOR_SECRET)
        .json(&json!({
            "name": "Acme",
            "login_identifier": "acme.example",
            "owner_email": email,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    harness.configure(id).await;
    harness.enable(id).await;
    let url = harness.mail.workspace_onboarding_url(email).await.unwrap();
    let secret = Url::parse(&url)
        .unwrap()
        .query_pairs()
        .find_map(|(name, value)| (name == "invitation_secret").then(|| value.into_owned()))
        .unwrap();
    (id, secret)
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_invitation_links_the_identity_and_needs_no_password(pool: PgPool) {
    let harness = start(&pool).await;
    let email = "new-owner@corp.example";
    let (workspace, secret) = invited_workspace(&harness, email).await;
    harness
        .provider
        .sign_in_as(FakeLogin::new(
            "new-owner-subject",
            "New-Owner@corp.example",
        ))
        .await;
    let body = json!({ "login_identifier": "acme.example", "invitation_secret": secret });
    let mut browser = Browser::default();
    let location = harness.sign_in(&mut browser, &body).await;
    assert_eq!(error_of(&location), None);
    let session: Value = harness.session_of(&browser).await.json().await.unwrap();
    assert_eq!(session["email"], email);
    assert_eq!(session["login_identifier"], "acme.example");

    let credentials: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM local_password_credentials c JOIN users u ON u.id = c.user_id WHERE u.email = $1",
    )
    .bind(email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(credentials, 0);
    let current: Value = client()
        .get(format!(
            "{}/operator/workspaces/{workspace}",
            harness.base_url
        ))
        .bearer_auth(OPERATOR_SECRET)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current["has_active_owner"], true);

    // The password onboarding link stopped working, and the invitation is spent.
    let live_actions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_lifecycle_action_tokens a JOIN users u ON u.id = a.user_id WHERE u.email = $1 AND a.revoked_at IS NULL AND a.consumed_at IS NULL",
    )
    .bind(email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live_actions, 0);
    let mut again = Browser::default();
    let location = harness.sign_in(&mut again, &body).await;
    assert_eq!(error_of(&location).as_deref(), Some("invitation_invalid"));
    assert!(again.session.is_none());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_invitation_refuses_a_different_or_unproven_address(pool: PgPool) {
    let harness = start(&pool).await;
    let email = "new-owner@corp.example";
    let (workspace, secret) = invited_workspace(&harness, email).await;
    let body = json!({ "login_identifier": "acme.example", "invitation_secret": secret });
    let cases = [
        (
            FakeLogin::new("someone-else", "someone-else@corp.example"),
            "oidc_email_mismatch",
        ),
        (
            FakeLogin {
                email_verified: Some(false),
                ..FakeLogin::new("unverified", email)
            },
            "oidc_email_missing",
        ),
        (
            FakeLogin {
                email: None,
                ..FakeLogin::new("no-email", email)
            },
            "oidc_email_missing",
        ),
    ];
    for (login, code) in cases {
        harness.provider.sign_in_as(login).await;
        let mut browser = Browser::default();
        let location = harness.sign_in(&mut browser, &body).await;
        assert_eq!(error_of(&location).as_deref(), Some(code));
        assert!(browser.session.is_none());
    }
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM external_identities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 0);

    // The invitation survives those attempts, and userinfo can supply the email.
    harness
        .provider
        .sign_in_as(FakeLogin {
            email_only_in_userinfo: true,
            ..FakeLogin::new("right-person", email)
        })
        .await;
    let mut browser = Browser::default();
    let location = harness.sign_in(&mut browser, &body).await;
    assert_eq!(error_of(&location), None);
    assert!(browser.session.is_some());
    let _ = workspace;
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn unlinking_ends_provider_sessions_here_and_blocks_sign_in(pool: PgPool) {
    let harness = ready(&pool).await;
    let (user, membership) = harness.sso_member("sso@example.test").await;
    harness.link(user, "subject-1").await;
    harness
        .provider
        .sign_in_as(FakeLogin::new("subject-1", "sso@example.test"))
        .await;
    let mut member = Browser::default();
    harness.sign_in(&mut member, &sign_in_body()).await;
    assert_eq!(harness.session_of(&member).await.status(), StatusCode::OK);

    harness.set_owner_password().await;
    let admin = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    let unlink = |membership: Uuid| {
        client()
            .delete(format!(
                "{}/workspace/members/{membership}/identity",
                harness.base_url
            ))
            .header("cookie", admin.cookie_header())
            .header("x-attricat-csrf", admin.csrf_value())
            .send()
    };
    assert_eq!(
        unlink(membership).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        unlink(membership).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        harness.session_of(&member).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let mut again = Browser::default();
    let location = harness.sign_in(&mut again, &sign_in_body()).await;
    assert_eq!(
        error_of(&location).as_deref(),
        Some("oidc_identity_not_linked")
    );
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn abandoned_requests_are_swept(pool: PgPool) {
    let harness = ready(&pool).await;
    harness
        .begin(&mut Browser::default(), &sign_in_body())
        .await;
    sqlx::query("UPDATE oidc_login_requests SET expires_at = now() - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();
    let deleted = api::repository::AttricatRepository::system(pool.clone())
        .purge_rate_limit_windows_batch(100)
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    harness.server.abort();
    harness.provider.server.abort();
}

async fn restrict_passwords(pool: &PgPool, workspace: Uuid) {
    sqlx::query("UPDATE workspace_identity_providers SET password_sign_in = 'owners' WHERE workspace_id = $1")
        .bind(workspace)
        .execute(pool)
        .await
        .unwrap();
}

async fn password_attempt(harness: &Harness, email: &str, password: &str) -> Response {
    client()
        .post(format!("{}/auth/login", harness.base_url))
        .json(&json!({
            "login_identifier": "default.local",
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .unwrap()
}

async fn set_password(pool: &PgPool, user: Uuid) {
    let hash = hash_password(&Password::new(PASSWORD)).unwrap();
    sqlx::query("INSERT INTO local_password_credentials (user_id, password_hash) VALUES ($1, $2)")
        .bind(user)
        .bind(hash.as_phc())
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn restricting_passwords_leaves_only_owners_and_reveals_nothing(pool: PgPool) {
    let harness = ready(&pool).await;
    harness.set_owner_password().await;
    let (member, _) = harness.sso_member("member@example.test").await;
    set_password(&pool, member).await;
    assert_eq!(
        password_attempt(&harness, "member@example.test", PASSWORD)
            .await
            .status(),
        StatusCode::OK
    );
    restrict_passwords(&pool, bootstrap_workspace_id()).await;

    let refused = password_attempt(&harness, "member@example.test", PASSWORD).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let body: Value = refused.json().await.unwrap();
    assert_eq!(body["error"]["code"], "password_sign_in_disabled");
    // Only someone who already knows the password learns about the restriction.
    let wrong = password_attempt(&harness, "member@example.test", "Wrong-password-1!").await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    // An owner keeps the way back in if the provider fails.
    assert_eq!(
        password_attempt(&harness, OWNER_EMAIL, PASSWORD)
            .await
            .status(),
        StatusCode::OK
    );

    // Other workspaces decide for themselves.
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'other', 'Other', 'other.example')")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(other)
    .bind(member)
    .execute(&pool)
    .await
    .unwrap();
    let elsewhere = client()
        .post(format!("{}/auth/login", harness.base_url))
        .json(&json!({
            "login_identifier": "other.example",
            "email": "member@example.test",
            "password": PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(elsewhere.status(), StatusCode::OK);

    let discovered: Value = client()
        .post(format!("{}/auth/discover", harness.base_url))
        .json(&sign_in_body())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(discovered["password_restricted"], true);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn invitees_need_no_password_where_only_owners_keep_one(pool: PgPool) {
    let harness = ready(&pool).await;
    harness.set_owner_password().await;
    let admin = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    let invite = |email: &'static str, role: Uuid| {
        client()
            .post(format!("{}/workspace/users", harness.base_url))
            .header("cookie", admin.cookie_header())
            .header("x-attricat-csrf", admin.csrf_value())
            .json(&json!({
                "email": email,
                "role_id": role,
                "scope_type": "workspace",
                "scope_target_id": bootstrap_workspace_id(),
                "expires_at": chrono::Utc::now() + chrono::Duration::days(3),
            }))
            .send()
    };
    // Before restricting, a new person gets a password link. It stops working
    // once only owners may hold one.
    let early = "early@example.test";
    assert_eq!(
        invite(early, VIEWER_ROLE_ID).await.unwrap().status(),
        StatusCode::CREATED
    );
    let early_url = harness.mail.workspace_onboarding_url(early).await.unwrap();
    restrict_passwords(&pool, bootstrap_workspace_id()).await;
    let query = |url: &str, name: &str| {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
            .unwrap()
    };
    let refused = client()
        .post(format!("{}/onboarding/complete", harness.base_url))
        .json(&json!({
            "invitation_secret": query(&early_url, "invitation_secret"),
            "onboarding_secret": query(&early_url, "onboarding_secret"),
            "password": PASSWORD,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // Now a new person gets an acceptance link, and can use it through the provider.
    let late = "late@example.test";
    assert_eq!(
        invite(late, VIEWER_ROLE_ID).await.unwrap().status(),
        StatusCode::CREATED
    );
    assert!(harness.mail.workspace_onboarding_url(late).await.is_none());
    let url = harness.mail.workspace_invitation_url(late).await.unwrap();
    let secret = query(&url, "secret");
    let options: Value = client()
        .post(format!("{}/auth/oidc/invitation", harness.base_url))
        .json(&json!({ "secret": secret }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(options["login_identifier"], "default.local");
    assert_eq!(options["display_name"], "Fake SSO");
    let unknown = client()
        .post(format!("{}/auth/oidc/invitation", harness.base_url))
        .json(&json!({ "secret": "not-a-real-secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::UNPROCESSABLE_ENTITY);

    harness
        .provider
        .sign_in_as(FakeLogin::new("late-subject", late))
        .await;
    let mut browser = Browser::default();
    let location = harness
        .sign_in(
            &mut browser,
            &json!({ "login_identifier": "default.local", "invitation_secret": secret }),
        )
        .await;
    assert_eq!(error_of(&location), None);
    let session: Value = harness.session_of(&browser).await.json().await.unwrap();
    assert_eq!(session["email"], late);

    // A new owner still gets a password, as the way back in.
    let owner = "new-owner@example.test";
    assert_eq!(
        invite(
            owner,
            "00000000-0000-4000-8000-000000000101".parse().unwrap()
        )
        .await
        .unwrap()
        .status(),
        StatusCode::CREATED
    );
    assert!(harness.mail.workspace_onboarding_url(owner).await.is_some());
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_invitation_can_create_the_account_it_addresses(pool: PgPool) {
    let harness = ready(&pool).await;
    harness.set_owner_password().await;
    let admin = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    let email = "brand-new@example.test";
    let created = client()
        .post(format!("{}/workspace/invitations", harness.base_url))
        .header("cookie", admin.cookie_header())
        .header("x-attricat-csrf", admin.csrf_value())
        .json(&json!({
            "email": email,
            "role_id": VIEWER_ROLE_ID,
            "scope_type": "workspace",
            "scope_target_id": bootstrap_workspace_id(),
            "expires_at": chrono::Utc::now() + chrono::Duration::days(3),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let url = harness.mail.workspace_invitation_url(email).await.unwrap();
    let secret = Url::parse(&url)
        .unwrap()
        .query_pairs()
        .find_map(|(key, value)| (key == "secret").then(|| value.into_owned()))
        .unwrap();
    harness
        .provider
        .sign_in_as(FakeLogin::new("brand-new-subject", email))
        .await;
    let mut browser = Browser::default();
    let location = harness
        .sign_in(
            &mut browser,
            &json!({ "login_identifier": "default.local", "invitation_secret": secret }),
        )
        .await;
    assert_eq!(error_of(&location), None);
    assert_eq!(harness.session_of(&browser).await.status(), StatusCode::OK);
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn deactivating_a_member_revokes_their_tokens_for_good(pool: PgPool) {
    let harness = ready(&pool).await;
    harness.set_owner_password().await;
    let (member, membership) = harness.sso_member("member@example.test").await;
    grant_role(&pool, membership, VIEWER_ROLE_ID, GrantScope::Workspace).await;
    let secret = "cat_pat_offboarding_test";
    let digest = {
        use sha2::{Digest, Sha256};
        Sha256::digest(secret.as_bytes()).to_vec()
    };
    let token = Uuid::new_v4();
    sqlx::query("INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest) VALUES ($1, $2, $3, 'cli', $4)")
        .bind(token)
        .bind(member)
        .bind(bootstrap_workspace_id())
        .bind(&digest)
        .execute(&pool)
        .await
        .unwrap();
    let session_with_token = || {
        client()
            .get(format!("{}/auth/session", harness.base_url))
            .bearer_auth(secret)
            .send()
    };
    assert_eq!(session_with_token().await.unwrap().status(), StatusCode::OK);
    let admin = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    let set_state = |state: &'static str| {
        client()
            .put(format!(
                "{}/workspace/members/{membership}",
                harness.base_url
            ))
            .header("cookie", admin.cookie_header())
            .header("x-attricat-csrf", admin.csrf_value())
            .json(&json!({ "state": state }))
            .send()
    };
    assert!(set_state("inactive").await.unwrap().status().is_success());
    assert_eq!(
        session_with_token().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(set_state("active").await.unwrap().status().is_success());
    assert_eq!(
        session_with_token().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    harness.server.abort();
    harness.provider.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn only_workspace_managers_are_told_they_can_manage_sign_in(pool: PgPool) {
    let harness = start(&pool).await;
    harness.set_owner_password().await;
    let owner = harness
        .sign_in_with_password("default.local", OWNER_EMAIL)
        .await;
    let session: Value = harness.session_of(&owner).await.json().await.unwrap();
    assert_eq!(session["capabilities"]["workspace_manage"], true);

    let viewer = member_with_role(&pool, VIEWER_ROLE_ID).await;
    set_password(&pool, viewer).await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(viewer)
        .fetch_one(&pool)
        .await
        .unwrap();
    let browser = harness.sign_in_with_password("default.local", &email).await;
    let session: Value = harness.session_of(&browser).await.json().await.unwrap();
    assert_eq!(session["capabilities"]["workspace_manage"], false);
    harness.server.abort();
    harness.provider.server.abort();
}
