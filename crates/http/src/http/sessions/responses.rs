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
    roles_grant: bool,
    tokens_manage: bool,
    workspace_navigation_manage: bool,
    extensions_read: bool,
    extensions_manage: bool,
    workflows_read: bool,
    workflows_manage: bool,
    rules_read: bool,
    rules_manage: bool,
    entities_publish: bool,
    entities_delete: bool,
}

impl SessionResponse {
    pub(super) fn restrict_to_permissions(&mut self, permissions: &[String]) {
        let permits = |code: &str| permissions.iter().any(|value| value == code);
        let c = &mut self.capabilities;
        c.audit_read &= permits("audit.read");
        c.data_health_read &= permits("data_health.read");
        c.members_manage &= permits("members.manage");
        c.roles_manage &= permits("roles.manage");
        c.roles_grant &= permits("roles.grant");
        c.tokens_manage &= permits("tokens.manage");
        c.workspace_navigation_manage &= permits("workspace_navigation.manage");
        c.extensions_read &= permits("extensions.read");
        c.extensions_manage &= permits("extensions.manage");
        c.workflows_read &= permits("workflows.read");
        c.workflows_manage &= permits("workflows.manage");
        c.rules_read &= permits("rules.read");
        c.rules_manage &= permits("rules.manage");
        c.entities_publish &= permits("entities.publish");
        c.entities_delete &= permits("entities.delete");
    }
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

/// Workspace-wide permissions reported as session capability flags.
const CAPABILITY_PERMISSIONS: [&str; 15] = [
    "audit.read",
    "data_health.read",
    "members.manage",
    "roles.manage",
    "roles.grant",
    "tokens.manage",
    "workspace_navigation.manage",
    "extensions.read",
    "extensions.manage",
    "workflows.read",
    "workflows.manage",
    "rules.read",
    "rules.manage",
    "entities.publish",
    "entities.delete",
];

async fn session_capabilities(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
) -> Result<SessionCapabilities, ApiError> {
    // One query for every flag; this runs on each login, renewal, and
    // session read.
    let granted = state
        .repository
        .workspace_permissions(user_id, workspace_id, &CAPABILITY_PERMISSIONS)
        .await?;
    let has = |permission: &str| granted.contains(permission);
    Ok(SessionCapabilities {
        audit_read: has("audit.read"),
        data_health_read: has("data_health.read"),
        members_manage: has("members.manage"),
        roles_manage: has("roles.manage"),
        roles_grant: has("roles.grant"),
        tokens_manage: has("tokens.manage"),
        workspace_navigation_manage: has("workspace_navigation.manage"),
        extensions_read: has("extensions.read"),
        extensions_manage: has("extensions.manage"),
        workflows_read: has("workflows.read"),
        workflows_manage: has("workflows.manage"),
        rules_read: has("rules.read"),
        rules_manage: has("rules.manage"),
        entities_publish: has("entities.publish"),
        entities_delete: has("entities.delete"),
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
