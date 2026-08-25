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
    email: String,
    password: String,
}

#[derive(Serialize)]
pub(super) struct SessionResponse {
    user_id: Uuid,
    /// This is the deployment-selected workspace for this API instance.
    workspace_id: Uuid,
    capabilities: SessionCapabilities,
}

#[derive(Serialize)]
pub(super) struct SessionCapabilities {
    members_manage: bool,
    roles_manage: bool,
    tokens_manage: bool,
}

pub(super) async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let email = request.email.trim().to_lowercase();
    let rate_key = digest_login_key(&email);
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
        // The reservation is intentionally retained on a failed verification.
        return Err(ApiError::invalid_credentials());
    }
    let credential = credential.expect("valid credential exists");
    state.repository.clear_login_failures(&rate_key).await?;
    let (session, csrf, expires_at) = issue_session();
    // This locks the user and credential versions observed during verification,
    // revokes prior workspace sessions, and creates the replacement atomically.
    state
        .repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            state.workspace_id,
            &session.digest(),
            &csrf.digest(),
            expires_at,
        )
        .await?;
    session_response(
        &state,
        credential.user_id,
        state.workspace_id,
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
    Ok(Json(SessionResponse {
        user_id,
        workspace_id,
        capabilities: session_capabilities(&state, user_id, workspace_id).await?,
    }))
}

pub(super) async fn logout(
    State(state): State<AppState>,
    AuthenticatedSession(session): AuthenticatedSession,
) -> Result<Response, ApiError> {
    state
        .repository
        .revoke_browser_session(&session, state.workspace_id)
        .await?;
    Ok(clear_session_response(state.session_cookie_secure))
}

pub(super) async fn renew(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    AuthenticatedSession(previous): AuthenticatedSession,
) -> Result<Response, ApiError> {
    let (session, csrf, expires_at) = issue_session();
    state
        .repository
        .rotate_browser_session(
            &previous,
            Uuid::new_v4(),
            &session.digest(),
            &csrf.digest(),
            state.workspace_id,
            expires_at,
        )
        .await?;
    session_response(
        &state,
        user_id,
        state.workspace_id,
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

fn digest_login_key(email: &str) -> SessionDigest {
    SessionDigest::from_slice(&Sha256::digest(email.as_bytes())).expect("sha256 is 32 bytes")
}

async fn session_response(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    session: &SessionSecret,
    csrf: &SessionSecret,
    secure: bool,
    lifetime_hours: i64,
) -> Result<Response, ApiError> {
    let mut response = Json(SessionResponse {
        user_id,
        workspace_id,
        capabilities: session_capabilities(state, user_id, workspace_id).await?,
    })
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
