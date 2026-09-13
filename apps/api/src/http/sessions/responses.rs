use axum::{
    Json,
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

use super::{ApiError, AppState, SessionSecret};
use crate::constants::{CSRF_COOKIE, SECONDS_PER_HOUR, SESSION_COOKIE};

#[derive(Serialize)]
pub(in crate::http) struct SessionResponse {
    pub(super) user_id: Uuid,
    pub(super) display_name: Option<String>,
    pub(super) email: String,
    /// The workspace resolved by the server and bound to this session.
    pub(super) workspace_id: Uuid,
    /// The human-facing identifier for the session-bound workspace.
    pub(super) login_identifier: String,
    pub(super) capabilities: SessionCapabilities,
}

#[derive(Serialize)]
struct OnboardingSessionResponse {
    #[serde(flatten)]
    session: SessionResponse,
    membership_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct SessionCapabilities {
    audit_read: bool,
    data_health_read: bool,
    members_manage: bool,
    roles_manage: bool,
    tokens_manage: bool,
    workspace_navigation_manage: bool,
    extensions_read: bool,
    extensions_manage: bool,
    workflows_read: bool,
    workflows_manage: bool,
}

// Cookie issuance intentionally keeps the response inputs explicit at this boundary.
#[allow(clippy::too_many_arguments)]
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
    append_session_cookies(&mut response, session, csrf, secure, lifetime_hours);
    Ok(response)
}

// Cookie issuance intentionally keeps the response inputs explicit at this boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn onboarding_session_response(
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
    append_session_cookies(&mut response, session, csrf, secure, lifetime_hours);
    Ok(response)
}

pub(super) async fn session_response_payload(
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
        audit_read: state
            .repository
            .is_authorized(user_id, workspace_id, "audit.read", None, None)
            .await?,
        data_health_read: state
            .repository
            .is_authorized(user_id, workspace_id, "data_health.read", None, None)
            .await?,
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
        workspace_navigation_manage: state
            .repository
            .is_authorized(
                user_id,
                workspace_id,
                "workspace_navigation.manage",
                None,
                None,
            )
            .await?,
        extensions_read: state
            .repository
            .is_authorized(user_id, workspace_id, "extensions.read", None, None)
            .await?,
        extensions_manage: state
            .repository
            .is_authorized(user_id, workspace_id, "extensions.manage", None, None)
            .await?,
        workflows_read: state
            .repository
            .is_authorized(user_id, workspace_id, "workflows.read", None, None)
            .await?,
        workflows_manage: state
            .repository
            .is_authorized(user_id, workspace_id, "workflows.manage", None, None)
            .await?,
    })
}

pub(super) fn clear_session_response(secure: bool) -> Response {
    let mut response = axum::http::StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, cookie(SESSION_COOKIE, "", true, secure, 0));
    response
        .headers_mut()
        .append(SET_COOKIE, cookie(CSRF_COOKIE, "", false, secure, 0));
    response
}

fn append_session_cookies(
    response: &mut Response,
    session: &SessionSecret,
    csrf: &SessionSecret,
    secure: bool,
    lifetime_hours: i64,
) {
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
        lifetime_hours * SECONDS_PER_HOUR
    );
    if http_only {
        value.push_str("; HttpOnly");
    }
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("base64url session cookie is a valid header")
}
