use axum::{
    Json,
    extract::State,
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, AuthenticatedSession},
    error::ApiError,
};
use crate::account::{Password, SessionDigest, SessionSecret};

const SESSION_COOKIE: &str = "catalog_session";
const CSRF_COOKIE: &str = "catalog_csrf";
const SESSION_LIFETIME_HOURS: i64 = 8;

#[derive(Deserialize)]
pub(super) struct LoginRequest {
    login_identifier: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub(super) struct DiscoveryRequest {
    login_identifier: String,
}

#[derive(Serialize)]
pub(super) struct DiscoveryResponse {
    login_identifier: String,
    sign_in_methods: Vec<&'static str>,
}

#[derive(Serialize)]
pub(super) struct SessionResponse {
    user_id: Uuid,
    display_name: Option<String>,
    email: String,
    /// The workspace resolved by the server and bound to this session.
    workspace_id: Uuid,
    /// The human-facing identifier for the session-bound workspace.
    login_identifier: String,
    capabilities: SessionCapabilities,
}

#[derive(Serialize)]
pub(super) struct SessionCapabilities {
    members_manage: bool,
    roles_manage: bool,
    tokens_manage: bool,
}

pub(super) async fn discover(
    State(state): State<AppState>,
    Json(request): Json<DiscoveryRequest>,
) -> Result<Json<DiscoveryResponse>, ApiError> {
    let identifier = request.login_identifier.trim().to_lowercase();
    let rate_key = digest_login_key(&identifier);
    if !state
        .repository
        .reserve_workspace_discovery_attempt(&rate_key)
        .await?
    {
        return Err(ApiError::rate_limited());
    }
    let workspace = state
        .repository
        .discover_workspace(&identifier)
        .await?
        .ok_or_else(|| ApiError::not_found("workspace"))?;
    Ok(Json(DiscoveryResponse {
        login_identifier: workspace.login_identifier,
        // This is deliberately extensible for OIDC, SAML, and passkeys.
        sign_in_methods: vec!["local_password"],
    }))
}

pub(super) async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let identifier = request.login_identifier.trim().to_lowercase();
    let workspace = state
        .repository
        .discover_workspace(&identifier)
        .await?
        .ok_or_else(ApiError::invalid_credentials)?;
    let email = request.email.trim().to_lowercase();
    let rate_key = digest_login_key(&format!("{}\0{}", workspace.id, email));
    if !state.repository.reserve_login_attempt(&rate_key).await? {
        return Err(ApiError::rate_limited());
    }
    let credential = state.repository.local_login_credential(&email).await?;
    let valid = credential.as_ref().is_some_and(|credential| {
        credential.active
            && credential
                .password_hash
                .verify(&Password::new(request.password))
                .unwrap_or(false)
    });
    if !valid {
        return Err(ApiError::invalid_credentials());
    }
    let credential = credential.expect("valid credential exists");
    state.repository.clear_login_failures(&rate_key).await?;
    let (session, csrf, expires_at) = issue_session();
    state
        .repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            workspace.id,
            &session.digest(),
            &csrf.digest(),
            expires_at,
        )
        .await?;
    session_response(
        &state,
        credential.user_id,
        workspace.id,
        workspace.login_identifier,
        &session,
        &csrf,
        state.session_cookie_secure,
        SESSION_LIFETIME_HOURS,
    )
    .await
}

pub(super) async fn current_session(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<SessionResponse>, ApiError> {
    Ok(Json(
        session_response_payload(&state, user_id, workspace_id).await?,
    ))
}

pub(super) async fn logout(
    State(state): State<AppState>,
    AuthenticatedSession(session): AuthenticatedSession,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    state
        .repository
        .revoke_browser_session(&session, workspace_id)
        .await?;
    Ok(clear_session_response(state.session_cookie_secure))
}

pub(super) async fn renew(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    AuthenticatedSession(previous): AuthenticatedSession,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    let (session, csrf, expires_at) = issue_session();
    state
        .repository
        .rotate_browser_session(
            &previous,
            Uuid::new_v4(),
            &session.digest(),
            &csrf.digest(),
            workspace_id,
            expires_at,
        )
        .await?;
    session_response(
        &state,
        user_id,
        workspace_id,
        state
            .repository
            .workspace_login_identifier(workspace_id)
            .await?,
        &session,
        &csrf,
        state.session_cookie_secure,
        SESSION_LIFETIME_HOURS,
    )
    .await
}

fn issue_session() -> (SessionSecret, SessionSecret, chrono::DateTime<Utc>) {
    (
        SessionSecret::generate(),
        SessionSecret::generate(),
        Utc::now() + Duration::hours(SESSION_LIFETIME_HOURS),
    )
}

fn digest_login_key(value: &str) -> SessionDigest {
    SessionDigest::from_slice(&Sha256::digest(value.as_bytes())).expect("sha256 is 32 bytes")
}

async fn session_response(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    login_identifier: String,
    session: &SessionSecret,
    csrf: &SessionSecret,
    secure: bool,
    lifetime_hours: i64,
) -> Result<Response, ApiError> {
    let mut response = Json(
        session_response_payload_with_identifier(state, user_id, workspace_id, login_identifier)
            .await?,
    )
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        cookie(
            SESSION_COOKIE,
            session.expose_for_delivery(),
            true,
            secure,
            lifetime_hours,
        ),
    );
    response.headers_mut().append(
        SET_COOKIE,
        cookie(
            CSRF_COOKIE,
            csrf.expose_for_delivery(),
            false,
            secure,
            lifetime_hours,
        ),
    );
    Ok(response)
}

async fn session_response_payload(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<SessionResponse, ApiError> {
    let login_identifier = state
        .repository
        .workspace_login_identifier(workspace_id)
        .await?;
    session_response_payload_with_identifier(state, user_id, workspace_id, login_identifier).await
}

async fn session_response_payload_with_identifier(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    login_identifier: String,
) -> Result<SessionResponse, ApiError> {
    let account = state.repository.user_account(user_id).await?;
    Ok(SessionResponse {
        user_id,
        display_name: account.display_name,
        email: account.email,
        workspace_id,
        login_identifier,
        capabilities: session_capabilities(state, user_id, workspace_id).await?,
    })
}

async fn session_capabilities(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<SessionCapabilities, ApiError> {
    Ok(SessionCapabilities {
        members_manage: state
            .repository
            .is_authorized(user_id, workspace_id, "members.manage", None, None)
            .await?,
        roles_manage: state
            .repository
            .is_authorized(user_id, workspace_id, "roles.manage", None, None)
            .await?,
        tokens_manage: state
            .repository
            .is_authorized(user_id, workspace_id, "tokens.manage", None, None)
            .await?,
    })
}

fn clear_session_response(secure: bool) -> Response {
    let mut response = axum::http::StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, cookie(SESSION_COOKIE, "", true, secure, 0));
    response
        .headers_mut()
        .append(SET_COOKIE, cookie(CSRF_COOKIE, "", false, secure, 0));
    response
}

fn cookie(
    name: &str,
    value: &str,
    http_only: bool,
    secure: bool,
    lifetime_hours: i64,
) -> HeaderValue {
    let mut value = format!(
        "{name}={value}; Path=/; SameSite=Lax; Max-Age={}",
        lifetime_hours * 3600
    );
    if http_only {
        value.push_str("; HttpOnly");
    }
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("base64url session cookie is a valid header")
}
