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
use crate::account::{
    ActionTokenSecret, IssuedLifecycleAction, LifecycleActionPurpose, Password, SessionDigest,
    SessionSecret, hash_password,
};

const SESSION_COOKIE: &str = "catalog_session";
const CSRF_COOKIE: &str = "catalog_csrf";
pub(super) const SESSION_LIFETIME_HOURS: i64 = 8;
const PASSWORD_RESET_LIFETIME_MINUTES: i64 = 30;

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

#[derive(Deserialize)]
pub(super) struct PasswordResetRequest {
    email: String,
}

#[derive(Deserialize)]
pub(super) struct PasswordResetConfirmation {
    token: String,
    password: String,
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
pub(super) struct OnboardingSessionResponse {
    #[serde(flatten)]
    session: SessionResponse,
    membership_id: Uuid,
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

/// Always returns 204 so callers cannot determine whether an address has a
/// resettable local credential. Opaque token values only cross the mail boundary.
pub(super) async fn request_password_reset(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let email = request.email.trim().to_lowercase();
    let credential = state.repository.local_login_credential(&email).await?;
    let Some(credential) =
        credential.filter(|credential| credential.active && credential.email_verified)
    else {
        return Ok(axum::http::StatusCode::NO_CONTENT);
    };
    let issued = IssuedLifecycleAction::issue(
        LifecycleActionPurpose::PasswordReset,
        Utc::now() + Duration::minutes(PASSWORD_RESET_LIFETIME_MINUTES),
        crate::account::SecurityVersion::new(credential.security_version.into()),
        Some(crate::account::CredentialVersion::new(
            credential.credential_version.into(),
        )),
    );
    state
        .repository
        .issue_lifecycle_token(
            Uuid::new_v4(),
            credential.user_id,
            LifecycleActionPurpose::PasswordReset.as_str(),
            issued.action.token_digest().as_ref(),
            issued.action.expires_at(),
        )
        .await?;
    let reset_url = format!(
        "{}?token={}",
        state.password_reset_url,
        issued.secret.expose_for_delivery()
    );
    state
        .mail_delivery
        .deliver_password_reset(&email, &reset_url)
        .await
        .map_err(|_| ApiError::internal("password reset email could not be delivered"))?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(super) async fn confirm_password_reset(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetConfirmation>,
) -> Result<axum::http::StatusCode, ApiError> {
    let secret = ActionTokenSecret::from_delivery_value(request.token).map_err(|_| {
        ApiError::invalid_input("password reset link is invalid or expired".to_owned())
    })?;
    let password_hash = hash_password(&Password::new(request.password))
        .map_err(|_| ApiError::invalid_input("password could not be set".to_owned()))?;
    state
        .repository
        .consume_password_reset(&secret.digest(), &password_hash)
        .await
        .map_err(|_| {
            ApiError::invalid_input("password reset link is invalid or expired".to_owned())
        })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
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

pub(super) fn issue_session() -> (SessionSecret, SessionSecret, chrono::DateTime<Utc>) {
    (
        SessionSecret::generate(),
        SessionSecret::generate(),
        Utc::now() + Duration::hours(SESSION_LIFETIME_HOURS),
    )
}

fn digest_login_key(value: &str) -> SessionDigest {
    SessionDigest::from_slice(&Sha256::digest(value.as_bytes())).expect("sha256 is 32 bytes")
}

pub(super) async fn session_response(
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

pub(super) async fn onboarding_session_response(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    membership_id: Uuid,
    session: &SessionSecret,
    csrf: &SessionSecret,
    secure: bool,
    lifetime_hours: i64,
) -> Result<Response, ApiError> {
    let mut response = Json(OnboardingSessionResponse {
        session: session_response_payload(state, user_id, workspace_id).await?,
        membership_id,
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
