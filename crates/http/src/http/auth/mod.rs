use axum::{
    extract::{FromRequestParts, Request, State},
    http::{Method, request::Parts},
    middleware::Next,
    response::Response,
};
use sha2::Digest;
use uuid::Uuid;

use super::{AppState, audit, error::ApiError};
use crate::{
    account::{SessionDigest, SessionSecret},
    constants::SESSION_COOKIE,
    repository::{AuthorizationActor, CatalogRepository},
};

mod policy;

const USER_HEADER: &str = "x-catalog-user-id";
const WORKSPACE_HEADER: &str = "x-catalog-workspace-id";
const AUTHORIZATION_HEADER: &str = "authorization";
const CSRF_HEADER: &str = "x-catalog-csrf";

/// Verified request identity. It is inserted only after membership and policy
/// evaluation, never constructed from an unvalidated handler argument.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct AuthenticatedPrincipal(pub Uuid, pub Option<Uuid>);

impl AuthenticatedPrincipal {
    /// The actor that permission checks evaluate: the user, further limited
    /// by the API token's own permissions when the request used one.
    pub(super) fn actor(self) -> AuthorizationActor {
        AuthorizationActor {
            user_id: self.0,
            token_id: self.1,
        }
    }
}

/// The workspace selected by the verified credential or browser session.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveWorkspace(pub Uuid);

/// Retained for session revocation, renewal and live stream validation;
/// raw cookie credentials never reach a handler.
#[derive(Clone)]
pub(super) struct AuthenticatedSession(pub SessionDigest);

/// Repository explicitly scoped to the authenticated workspace. It is inserted
/// only by authorization after workspace selection.
#[derive(Clone)]
pub(super) struct ScopedRepository(pub CatalogRepository);

impl FromRequestParts<AppState> for ScopedRepository {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(ApiError::unauthenticated)
    }
}

impl FromRequestParts<AppState> for AuthenticatedSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(ApiError::unauthenticated)
    }
}

impl FromRequestParts<AppState> for AuthenticatedPrincipal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .copied()
            .ok_or_else(ApiError::unauthenticated)
    }
}

impl FromRequestParts<AppState> for ActiveWorkspace {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .copied()
            .ok_or_else(ApiError::unauthenticated)
    }
}

pub(super) async fn authorize(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Production mounts the same API under /api; all policy and public-route
    // checks must use the same canonical concrete path.
    let path = super::canonical_route(request.uri().path());
    if is_public_route(path) {
        return Ok(next.run(request).await);
    }

    let bearer = request
        .headers()
        .get(AUTHORIZATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        // The authentication scheme is case-insensitive (RFC 9110 §11.1).
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, credentials)| credentials);
    // `token_permissions` is the authenticated token's permission set (None
    // for sessions); `active_principal` is already known for browser sessions,
    // whose validation covers the same user, membership and workspace state.
    let (
        principal,
        workspace,
        token_id,
        session_digest,
        token_permissions,
        active_principal,
        generations,
    ) = if let Some(secret) = bearer {
        let digest = sha2::Sha256::digest(secret.as_bytes());
        let token = state
            .repository
            .authenticate_personal_api_token(&digest)
            .await?
            .ok_or_else(ApiError::unauthenticated)?;
        (
            token.user_id,
            token.workspace_id,
            Some(token.id),
            None,
            Some(token.permissions),
            None,
            token.generations,
        )
    } else if state.allow_trusted_headers {
        let user_id = request
            .headers()
            .get(USER_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .ok_or_else(ApiError::unauthenticated)?;
        let workspace_id = request
            .headers()
            .get(WORKSPACE_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .ok_or_else(ApiError::unauthenticated)?;
        let (active, generations) = state
            .repository
            .active_user_generations(user_id, workspace_id)
            .await?;
        if !active {
            return Err(ApiError::unauthenticated());
        }
        (user_id, workspace_id, None, None, None, None, generations)
    } else {
        let raw_session = cookie_value(
            request
                .headers()
                .get("cookie")
                .and_then(|value| value.to_str().ok()),
            SESSION_COOKIE,
        )
        .ok_or_else(ApiError::unauthenticated)?;
        let session_secret = SessionSecret::from_delivery_value(raw_session)
            .map_err(|_| ApiError::unauthenticated())?;
        let session_digest = session_secret.digest();
        let session = state
            .repository
            .validate_browser_session(&session_digest)
            .await?
            .ok_or_else(ApiError::unauthenticated)?;
        if !matches!(
            *request.method(),
            Method::GET | Method::HEAD | Method::OPTIONS
        ) {
            let csrf = request
                .headers()
                .get(CSRF_HEADER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| SessionSecret::from_delivery_value(value.to_owned()).ok())
                .ok_or_else(ApiError::csrf_failed)?;
            if !session.csrf_digest.matches(&csrf) {
                return Err(ApiError::csrf_failed());
            }
        }
        (
            session.user_id,
            session.workspace_id,
            None,
            Some(session_digest),
            None,
            Some(session.workspace_active),
            Some(session.generations),
        )
    };
    // A token's permissions were read with the credential in this request.
    let token_permits = |permission: &str| {
        token_permissions
            .as_ref()
            .is_none_or(|permissions| permissions.contains(permission))
    };
    let is_active_principal = || async {
        match active_principal {
            Some(active) => Ok::<bool, ApiError>(active),
            None => Ok(state
                .repository
                .is_active_principal(principal, workspace)
                .await?),
        }
    };
    let matched = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|matched| super::canonical_route(matched.as_str()))
        .unwrap_or(path);
    // An invitee can have no membership yet, so acceptance deliberately skips
    // the active-membership policy while retaining normal credential validation.
    let accepting_invitation = matched == "/workspace/invitations/accept";
    if let Some(policy) = policy::policy(request.method(), matched) {
        let (target_id, target_code) = policy::target(path, policy.target);
        // File reads are authorized in their handlers after resolving active
        // file-to-entity references. Other routes can authorize from the path.
        let handler_authorized = matches!(
            policy.target,
            policy::TargetKind::FileRead
                | policy::TargetKind::WorkspaceNavigation
                | policy::TargetKind::ExtensionRun
                | policy::TargetKind::EntityBatch
                | policy::TargetKind::EntityList
        );
        if handler_authorized && !is_active_principal().await? {
            return Err(ApiError::forbidden());
        }
        // Ordinary permission evaluation already checks the live user,
        // membership and workspace; do not repeat those database round trips.
        // Handler-authorized routes resolve their target later and only need
        // the token's permission here.
        let permitted = token_permits(policy.permission)
            && (handler_authorized
                || state
                    .repository
                    .is_authorized(
                        principal,
                        workspace,
                        policy.permission,
                        target_id,
                        target_code.as_deref(),
                    )
                    .await?);
        if !permitted {
            return Err(ApiError::forbidden());
        }
        if let Some(extra) = policy::additional_permission(matched)
            && !(token_permits(extra)
                && state
                    .repository
                    .is_authorized(principal, workspace, extra, None, None)
                    .await?)
        {
            return Err(ApiError::forbidden());
        }
    } else if matches!(
        matched,
        "/auth/session"
            | "/auth/preferences"
            | "/auth/display-name"
            | "/auth/avatar"
            | "/auth/logout"
            | "/auth/renew"
            // Each member's own inbox: handlers only ever read and change
            // the caller's notifications, so no catalog permission applies.
            | "/notifications"
            | "/notifications/unread-count"
            | "/notifications/read-all"
            | "/notifications/{notification_id}"
    ) {
        if !is_active_principal().await? {
            return Err(ApiError::forbidden());
        }
    } else if !accepting_invitation {
        return Err(ApiError::forbidden());
    }

    let user_scoped_agent = matched.starts_with("/agent/");
    let principal = AuthenticatedPrincipal(principal, token_id);
    let audit_context = audit::request_context(&request, principal);
    request.extensions_mut().insert(principal);
    request.extensions_mut().insert(ActiveWorkspace(workspace));
    let mut repository = state
        .repository
        .for_workspace(workspace)
        .await?
        .with_generations(generations)
        .with_audit_context(audit_context);
    if user_scoped_agent {
        repository = repository.with_authorization_actor(principal.actor());
    }
    request
        .extensions_mut()
        .insert(ScopedRepository(repository));
    if let Some(session_digest) = session_digest {
        request
            .extensions_mut()
            .insert(AuthenticatedSession(session_digest));
    }
    Ok(next.run(request).await)
}

fn is_public_route(path: &str) -> bool {
    matches!(
        path,
        "/health"
            | "/health/live"
            | "/health/ready"
            | "/auth/login"
            | "/auth/discover"
            | "/auth/sample-logins"
            | "/auth/password-reset"
            | "/auth/password-reset/confirm"
            | "/onboarding/complete"
    )
}

fn cookie_value(header: Option<&str>, name: &str) -> Option<String> {
    header?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn api_alias_uses_identical_public_and_scoped_policy_paths() {
        for route in [
            "/auth/login",
            "/auth/discover",
            "/auth/sample-logins",
            "/health/ready",
            "/onboarding/complete",
        ] {
            assert!(is_public_route(super::super::canonical_route(route)));
            assert!(is_public_route(super::super::canonical_route(&format!(
                "/api{route}"
            ))));
        }
        let entity = Uuid::new_v4();
        let path = format!("/entities/{entity}");
        let alias = format!("/api{path}");
        assert_eq!(
            policy::target(
                super::super::canonical_route(&alias),
                policy::TargetKind::EntityId
            )
            .0,
            Some(entity)
        );
    }
}
