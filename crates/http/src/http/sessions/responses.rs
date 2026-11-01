use axum::{
    Json,
    http::{HeaderValue, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

use super::{ApiError, AppState, SessionSecret};
use crate::constants::{CSRF_COOKIE, SECONDS_PER_HOUR, SESSION_COOKIE, SESSION_LIFETIME_HOURS};
use attricat_domain::permissions::Permission;

#[derive(Serialize)]
pub(in crate::http) struct SessionResponse {
    pub(super) user_id: Uuid,
    pub(super) display_name: Option<String>,
    pub(super) email: String,
    /// Preferred IANA time zone for rendering instants; `None` follows the
    /// client's zone. Stored instants remain UTC.
    pub(super) time_zone: Option<String>,
    /// The caller's avatar in this workspace, including one still processing.
    pub(super) avatar: Option<crate::repository::OwnAvatar>,
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
    tag_colors_manage: bool,
    agents_manage: bool,
    extensions_read: bool,
    extensions_manage: bool,
    workflows_read: bool,
    workflows_manage: bool,
    rules_read: bool,
    rules_manage: bool,
    records_publish: bool,
    records_unlock: bool,
    records_delete: bool,
    blueprints_write: bool,
    records_write: bool,
    /// Workspace-wide read access, which an export preview needs.
    records_read: bool,
    blueprints_publish: bool,
    contexts_write: bool,
    agents_run: bool,
}

impl SessionResponse {
    pub(super) fn restrict_to_permissions(&mut self, permissions: &[String]) {
        let permits =
            |permission: Permission| permissions.iter().any(|value| value == permission.as_str());
        let c = &mut self.capabilities;
        c.audit_read &= permits(Permission::AuditRead);
        c.data_health_read &= permits(Permission::DataHealthRead);
        c.members_manage &= permits(Permission::MembersManage);
        c.roles_manage &= permits(Permission::RolesManage);
        c.roles_grant &= permits(Permission::RolesGrant);
        c.tokens_manage &= permits(Permission::TokensManage);
        c.workspace_navigation_manage &= permits(Permission::WorkspaceNavigationManage);
        c.tag_colors_manage &= permits(Permission::TagColorsManage);
        c.agents_manage &= permits(Permission::AgentsManage);
        c.extensions_read &= permits(Permission::ExtensionsRead);
        c.extensions_manage &= permits(Permission::ExtensionsManage);
        c.workflows_read &= permits(Permission::WorkflowsRead);
        c.workflows_manage &= permits(Permission::WorkflowsManage);
        c.rules_read &= permits(Permission::RulesRead);
        c.rules_manage &= permits(Permission::RulesManage);
        c.records_publish &= permits(Permission::RecordsPublish);
        c.records_unlock &= permits(Permission::RecordsUnlock);
        c.records_delete &= permits(Permission::RecordsDelete);
        c.blueprints_write &= permits(Permission::BlueprintsWrite);
        c.records_write &= permits(Permission::RecordsWrite);
        c.records_read &= permits(Permission::RecordsRead);
        c.blueprints_publish &= permits(Permission::BlueprintsPublish);
        c.contexts_write &= permits(Permission::ContextsWrite);
        c.agents_run &= permits(Permission::AgentsRun);
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
    // These reads are independent; run them concurrently.
    let (login_identifier, account, avatar, capabilities) = tokio::try_join!(
        async {
            Ok::<_, ApiError>(
                state
                    .repository
                    .workspace_login_identifier(workspace_id)
                    .await?,
            )
        },
        async { Ok(state.repository.user_account(user_id).await?) },
        async { Ok(state.repository.own_avatar(user_id, workspace_id).await?) },
        session_capabilities(state, user_id, workspace_id),
    )?;
    Ok(SessionResponse {
        user_id,
        display_name: account.display_name,
        email: account.email,
        time_zone: account.time_zone,
        avatar,
        workspace_id,
        login_identifier,
        capabilities,
    })
}

async fn session_response_payload_with_identifier(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    login_identifier: String,
) -> Result<SessionResponse, ApiError> {
    let (account, avatar, capabilities) = tokio::try_join!(
        async { Ok::<_, ApiError>(state.repository.user_account(user_id).await?) },
        async { Ok(state.repository.own_avatar(user_id, workspace_id).await?) },
        session_capabilities(state, user_id, workspace_id),
    )?;
    Ok(SessionResponse {
        user_id,
        display_name: account.display_name,
        email: account.email,
        time_zone: account.time_zone,
        avatar,
        workspace_id,
        login_identifier,
        capabilities,
    })
}

/// Workspace-wide permissions reported as session capability flags.
const CAPABILITY_PERMISSIONS: [Permission; 24] = [
    Permission::AuditRead,
    Permission::DataHealthRead,
    Permission::MembersManage,
    Permission::RolesManage,
    Permission::RolesGrant,
    Permission::TokensManage,
    Permission::WorkspaceNavigationManage,
    Permission::TagColorsManage,
    Permission::AgentsManage,
    Permission::ExtensionsRead,
    Permission::ExtensionsManage,
    Permission::WorkflowsRead,
    Permission::WorkflowsManage,
    Permission::RulesRead,
    Permission::RulesManage,
    Permission::RecordsPublish,
    Permission::RecordsUnlock,
    Permission::RecordsDelete,
    Permission::BlueprintsWrite,
    Permission::RecordsWrite,
    Permission::RecordsRead,
    Permission::BlueprintsPublish,
    Permission::ContextsWrite,
    Permission::AgentsRun,
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
    let has = |permission: Permission| granted.contains(permission.as_str());
    Ok(SessionCapabilities {
        audit_read: has(Permission::AuditRead),
        data_health_read: has(Permission::DataHealthRead),
        members_manage: has(Permission::MembersManage),
        roles_manage: has(Permission::RolesManage),
        roles_grant: has(Permission::RolesGrant),
        tokens_manage: has(Permission::TokensManage),
        workspace_navigation_manage: has(Permission::WorkspaceNavigationManage),
        tag_colors_manage: has(Permission::TagColorsManage),
        agents_manage: has(Permission::AgentsManage),
        extensions_read: has(Permission::ExtensionsRead),
        extensions_manage: has(Permission::ExtensionsManage),
        workflows_read: has(Permission::WorkflowsRead),
        workflows_manage: has(Permission::WorkflowsManage),
        rules_read: has(Permission::RulesRead),
        rules_manage: has(Permission::RulesManage),
        records_publish: has(Permission::RecordsPublish),
        records_unlock: has(Permission::RecordsUnlock),
        records_delete: has(Permission::RecordsDelete),
        blueprints_write: has(Permission::BlueprintsWrite),
        records_write: has(Permission::RecordsWrite),
        records_read: has(Permission::RecordsRead),
        blueprints_publish: has(Permission::BlueprintsPublish),
        contexts_write: has(Permission::ContextsWrite),
        agents_run: has(Permission::AgentsRun),
    })
}

/// Sends the browser to `location` with a new session's cookies, for a sign-in
/// that finishes in a redirect.
pub(in crate::http) fn session_redirect(
    location: &str,
    session: &SessionSecret,
    csrf: &SessionSecret,
    secure: bool,
) -> Result<Response, ApiError> {
    let mut response = Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, location)
        .header(axum::http::header::CACHE_CONTROL, "no-store")
        .header("referrer-policy", "no-referrer")
        .body(axum::body::Body::empty())
        .map_err(|_| ApiError::internal("single sign-on redirect"))?;
    append_session_cookies(&mut response, session, csrf, secure, SESSION_LIFETIME_HOURS);
    Ok(response)
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
