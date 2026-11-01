//! A small OpenID Connect provider for sign-in tests. It runs the real
//! authorization code flow with PKCE and signs real tokens, and each test
//! decides how it should misbehave.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use openidconnect::{
    Audience, ClientId, EmptyAdditionalClaims, EndUserEmail, IssuerUrl, JsonWebKeyId, Nonce,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreRsaPrivateSigningKey,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

pub const FAKE_CLIENT_ID: &str = "attricat-test-client";
pub const FAKE_CLIENT_SECRET: &str = "fake-client-secret";
const KEY_ID: &str = "test-key";
const SIGNING_KEY: &str = include_str!("../fixtures/oidc/signing-key.pem");
const OTHER_KEY: &str = include_str!("../fixtures/oidc/other-key.pem");

/// Who signs in, and how the next tokens are wrong, if at all.
#[derive(Clone)]
pub struct FakeLogin {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    /// Supply the email only from the userinfo endpoint.
    pub email_only_in_userinfo: bool,
    pub audience: Option<String>,
    pub issuer: Option<String>,
    pub nonce: Option<String>,
    pub expired: bool,
    pub signed_by_other_key: bool,
    pub unsigned: bool,
    pub authorized_party: Option<String>,
    pub issued_in_the_future: bool,
}

impl FakeLogin {
    pub fn new(subject: &str, email: &str) -> Self {
        Self {
            subject: subject.to_owned(),
            email: Some(email.to_owned()),
            email_verified: Some(true),
            email_only_in_userinfo: false,
            audience: None,
            issuer: None,
            nonce: None,
            expired: false,
            signed_by_other_key: false,
            unsigned: false,
            authorized_party: None,
            issued_in_the_future: false,
        }
    }
}

struct PendingCode {
    challenge: String,
    nonce: String,
    login: FakeLogin,
}

struct ProviderState {
    issuer: String,
    login: Mutex<FakeLogin>,
    codes: Mutex<HashMap<String, PendingCode>>,
    userinfo: Mutex<Option<FakeLogin>>,
    authorization_endpoint: Mutex<Option<String>>,
}

pub struct FakeOidcProvider {
    pub issuer: String,
    state: Arc<ProviderState>,
    pub server: JoinHandle<()>,
}

impl FakeOidcProvider {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(ProviderState {
            issuer: issuer.clone(),
            login: Mutex::new(FakeLogin::new("subject-1", "person@example.test")),
            codes: Mutex::default(),
            userinfo: Mutex::default(),
            authorization_endpoint: Mutex::default(),
        });
        let router = Router::new()
            .route("/.well-known/openid-configuration", get(configuration))
            .route("/jwks", get(jwks))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .route("/userinfo", get(userinfo))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            issuer,
            state,
            server,
        }
    }

    /// Publishes a different authorization endpoint in the metadata.
    pub async fn publish_authorization_endpoint(&self, endpoint: &str) {
        *self.state.authorization_endpoint.lock().await = Some(endpoint.to_owned());
    }

    /// Sets who the next browser visit signs in as.
    pub async fn sign_in_as(&self, login: FakeLogin) {
        *self.state.login.lock().await = login;
    }
}

async fn configuration(State(state): State<Arc<ProviderState>>) -> Json<Value> {
    let issuer = &state.issuer;
    let authorization_endpoint = state
        .authorization_endpoint
        .lock()
        .await
        .clone()
        .unwrap_or_else(|| format!("{issuer}/authorize"));
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": authorization_endpoint,
        "token_endpoint": format!("{issuer}/token"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/jwks"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "code_challenge_methods_supported": ["S256"],
    }))
}

fn signing_key(pem: &str) -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(pem, Some(JsonWebKeyId::new(KEY_ID.to_owned()))).unwrap()
}

async fn jwks() -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        signing_key(SIGNING_KEY).as_verification_key(),
    ]))
}

#[derive(Deserialize)]
struct AuthorizeQuery {
    redirect_uri: String,
    state: String,
    nonce: String,
    code_challenge: String,
    code_challenge_method: String,
    client_id: String,
}

async fn authorize(
    State(state): State<Arc<ProviderState>>,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    if query.code_challenge_method != "S256" || query.client_id != FAKE_CLIENT_ID {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let code = uuid::Uuid::new_v4().to_string();
    state.codes.lock().await.insert(
        code.clone(),
        PendingCode {
            challenge: query.code_challenge,
            nonce: query.nonce,
            login: state.login.lock().await.clone(),
        },
    );
    Redirect::to(&format!(
        "{}?code={code}&state={}",
        query.redirect_uri, query.state
    ))
    .into_response()
}

#[derive(Deserialize)]
struct TokenForm {
    code: String,
    code_verifier: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

fn client_authenticated(headers: &HeaderMap, form: &TokenForm) -> bool {
    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Basic "))
        .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
        .and_then(|value| String::from_utf8(value).ok());
    basic.as_deref() == Some(&format!("{FAKE_CLIENT_ID}:{FAKE_CLIENT_SECRET}"))
        || (form.client_id.as_deref() == Some(FAKE_CLIENT_ID)
            && form.client_secret.as_deref() == Some(FAKE_CLIENT_SECRET))
}

async fn token(
    State(state): State<Arc<ProviderState>>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    if !client_authenticated(&headers, &form) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid_client"})),
        )
            .into_response();
    }
    let Some(pending) = state.codes.lock().await.remove(&form.code) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    };
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(form.code_verifier.as_bytes()));
    if challenge != pending.challenge {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    }
    let login = pending.login;
    *state.userinfo.lock().await = Some(login.clone());
    let id_token = id_token(&state.issuer, &pending.nonce, &login);
    Json(json!({
        "access_token": "fake-access-token",
        "token_type": "Bearer",
        "expires_in": 3600,
        "id_token": id_token,
    }))
    .into_response()
}

fn id_token(issuer: &str, nonce: &str, login: &FakeLogin) -> String {
    let now = Utc::now();
    let (issued_at, expires_at) = if login.expired {
        (now - Duration::hours(2), now - Duration::hours(1))
    } else if login.issued_in_the_future {
        (now + Duration::days(1), now + Duration::days(2))
    } else {
        (now, now + Duration::hours(1))
    };
    let mut standard = StandardClaims::new(SubjectIdentifier::new(login.subject.clone()));
    if !login.email_only_in_userinfo {
        standard = standard
            .set_email(login.email.clone().map(EndUserEmail::new))
            .set_email_verified(login.email_verified);
    }
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(login.issuer.clone().unwrap_or_else(|| issuer.to_owned())).unwrap(),
        vec![Audience::new(
            login
                .audience
                .clone()
                .unwrap_or_else(|| FAKE_CLIENT_ID.to_owned()),
        )],
        expires_at,
        issued_at,
        standard,
        EmptyAdditionalClaims {},
    )
    .set_authorized_party(login.authorized_party.clone().map(ClientId::new))
    .set_nonce(Some(Nonce::new(
        login.nonce.clone().unwrap_or_else(|| nonce.to_owned()),
    )));
    if login.unsigned {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        return format!("{header}.{body}.");
    }
    let key = signing_key(if login.signed_by_other_key {
        OTHER_KEY
    } else {
        SIGNING_KEY
    });
    let token = CoreIdToken::new(
        claims,
        &key,
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .unwrap();
    token.to_string()
}

async fn userinfo(State(state): State<Arc<ProviderState>>) -> Response {
    let Some(login) = state.userinfo.lock().await.clone() else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(json!({
        "sub": login.subject,
        "email": login.email,
        "email_verified": login.email_verified,
    }))
    .into_response()
}
