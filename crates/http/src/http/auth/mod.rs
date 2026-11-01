use attricat_domain::permissions::Permission;
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
    repository::{AttricatRepository, AuthorizationActor, RepositoryScope},
};

mod policy;

const USER_HEADER: &str = "x-attricat-user-id";
const WORKSPACE_HEADER: &str = "x-attricat-workspace-id";
const AUTHORIZATION_HEADER: &str = "authorization";
const CSRF_HEADER: &str = "x-attricat-csrf";

/// Verified request identity. It is inserted only after membership and policy
/// evaluation, never constructed from an unvalidated handler argument.
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

    /// Fails with `403` unless the principal holds `permission` across the
    /// workspace, re-checking live grants and token revocation.
    pub(super) async fn require_workspace_permission<S: RepositoryScope>(
        self,
        repository: &AttricatRepository<S>,
        workspace_id: Uuid,
        permission: Permission,
    ) -> Result<(), ApiError> {
        if repository
            .principal_may(self.actor(), workspace_id, permission, None, None)
            .await?
        {
            Ok(())
        } else {
            Err(ApiError::forbidden())
        }
    }
}

/// The workspace selected by the verified credential or browser session.
#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveWorkspace(pub Uuid);

/// Retained for session revocation, renewal and live stream validation;
/// raw cookie credentials never reach a handler.
#[derive(Clone)]
pub(super) struct AuthenticatedSession(pub SessionDigest);

/// Repository explicitly scoped to the authenticated workspace. It is inserted
/// only by authorization after workspace selection.
#[derive(Clone)]
pub(super) struct ScopedRepository(pub AttricatRepository);

/// Handlers read each verified fact back from the extensions `authorize`
/// inserted; a route outside the authorization layer is unauthenticated.
macro_rules! authorized_extension {
    ($($fact:ty),+) => {$(
        impl FromRequestParts<AppState> for $fact {
            type Rejection = ApiError;

            async fn from_request_parts(
                parts: &mut Parts,
                _: &AppState,
            ) -> Result<Self, Self::Rejection> {
                parts
                    .extensions
                    .get::<Self>()
                    .cloned()
                    .ok_or_else(ApiError::unauthenticated)
            }
        }
    )+};
}

authorized_extension!(
    ScopedRepository,
    AuthenticatedSession,
    AuthenticatedPrincipal,
    ActiveWorkspace
);

pub(super) async fn authorize(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Policy and public-route checks use the path without the `/api` mount
    // prefix.
    let path = super::canonical_route(request.uri().path());
    if is_public_route(path) {
        return Ok(next.run(request).await);
    }

    let bearer = bearer_credentials(request.headers());
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
    // Cached permissions apply only to a live workspace; tokens and trusted
    // headers read no generations for a deleted one.
    let authorization_generations = match active_principal {
        Some(false) => None,
        _ => generations,
    };
    // A token's permissions were read with the credential in this request.
    let token_permits = |permission: Permission| {
        token_permissions
            .as_ref()
            .is_none_or(|permissions| permissions.contains(permission.as_str()))
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
        let (target_id, target_code) = policy::target(matched, path, policy.target);
        // File reads are authorized in their handlers after resolving active
        // file-to-record references. Other routes can authorize from the path.
        let handler_authorized = matches!(
            policy.target,
            policy::TargetKind::FileRead
                | policy::TargetKind::WorkspaceNavigation
                | policy::TargetKind::ExtensionRun
                | policy::TargetKind::RecordBatch
                | policy::TargetKind::RecordList
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
                    .is_authorized_at(
                        authorization_generations,
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
                    .is_authorized_at(
                        authorization_generations,
                        principal,
                        workspace,
                        extra,
                        None,
                        None,
                    )
                    .await?)
        {
            return Err(ApiError::forbidden());
        }
    } else if is_self_service_route(matched) {
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

/// The credentials of an `Authorization: Bearer` header, if the request has one.
pub(super) fn bearer_credentials(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        // The authentication scheme is case-insensitive (RFC 9110 §11.1).
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, credentials)| credentials)
}

/// Routes that read or change only the caller's own session, account or
/// inbox, so they need an active membership but no catalog permission.
fn is_self_service_route(route: &str) -> bool {
    matches!(
        route,
        "/auth/session"
            // Reports only the caller's own access to the targets it names.
            | "/auth/access"
            | "/auth/preferences"
            | "/auth/display-name"
            | "/auth/avatar"
            | "/auth/logout"
            | "/auth/renew"
            | "/auth/oidc/link"
            // Each member's own inbox: handlers only ever read and change
            // the caller's notifications, so no catalog permission applies.
            | "/notifications"
            | "/notifications/unread-count"
            | "/notifications/read-all"
            | "/notifications/{notification_id}"
    )
}

fn is_public_route(path: &str) -> bool {
    // Extension-issued public file links carry their own unguessable token.
    if path
        .strip_prefix("/public/extension-files/")
        .is_some_and(|token| !token.is_empty() && !token.contains('/'))
    {
        return true;
    }
    matches!(
        path,
        // The OAuth callback carries a single-use, unguessable state.
        "/public/extension-oauth/callback"
            | "/auth/login"
            | "/auth/discover"
            | "/auth/oidc/start"
            | "/auth/oidc/callback"
            | "/auth/sample-logins"
            | "/auth/password-reset"
            | "/auth/password-reset/confirm"
            | "/onboarding/complete"
    )
}

/// The digest of the browser session cookie a request carries, if any. Used
/// by public routes that must recognize the signed-in browser without
/// authorizing the request; callers validate the session themselves.
pub(super) fn session_cookie_digest(headers: &axum::http::HeaderMap) -> Option<SessionDigest> {
    let raw = cookie_value(
        headers.get("cookie").and_then(|value| value.to_str().ok()),
        SESSION_COOKIE,
    )?;
    SessionSecret::from_delivery_value(raw)
        .ok()
        .map(|secret| secret.digest())
}

pub(super) fn cookie_value(header: Option<&str>, name: &str) -> Option<String> {
    header?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

#[cfg(test)]
mod route_tests {
    use super::*;

    /// Every `(route, method)` the API router registers, read from its source.
    fn registered_routes() -> Vec<(String, Method)> {
        let source = include_str!("../mod.rs");
        let mut routes = Vec::new();
        for call in source.split(".route(").skip(1) {
            let Some(rest) = call.trim_start().strip_prefix('"') else {
                continue;
            };
            let Some((path, handlers)) = rest.split_once('"') else {
                continue;
            };
            // The method router runs to the call's closing parenthesis.
            let mut depth = 1;
            let end = handlers
                .char_indices()
                .find(|&(_, ch)| {
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map_or(handlers.len(), |(index, _)| index);
            let handlers = &handlers[..end];
            for (name, method) in [
                ("get(", Method::GET),
                ("post(", Method::POST),
                ("put(", Method::PUT),
                ("patch(", Method::PATCH),
                ("delete(", Method::DELETE),
            ] {
                let called = handlers.match_indices(name).any(|(index, _)| {
                    index == 0
                        || !handlers[..index]
                            .ends_with(|ch: char| ch.is_alphanumeric() || ch == '_')
                });
                if called {
                    routes.push((path.to_owned(), method));
                }
            }
        }
        routes
    }

    #[test]
    fn every_registered_route_is_authorized_explicitly() {
        let routes = registered_routes();
        assert!(routes.len() > 100, "found only {} routes", routes.len());
        let unauthorized: Vec<_> = routes
            .iter()
            .filter(|(route, method)| {
                // Health probes are mounted outside the authorization layer.
                !route.starts_with("/health")
                    && !is_public_route(route)
                    && !is_self_service_route(route)
                    && route != "/workspace/invitations/accept"
                    && policy::policy(method, route).is_none()
            })
            .collect();
        assert!(
            unauthorized.is_empty(),
            "routes without a policy are always forbidden: {unauthorized:?}"
        );
    }

    #[test]
    fn public_and_scoped_policy_paths_ignore_the_api_prefix() {
        for route in [
            "/auth/login",
            "/auth/discover",
            "/auth/sample-logins",
            "/onboarding/complete",
            "/public/extension-oauth/callback",
        ] {
            assert!(is_public_route(super::super::canonical_route(route)));
            assert!(is_public_route(super::super::canonical_route(&format!(
                "/api{route}"
            ))));
        }
        let record = Uuid::new_v4();
        for route in ["/records/{record_id}", "/v1/records/{record_id}/values"] {
            let path = route.replace("{record_id}", &record.to_string());
            assert_eq!(
                policy::target(route, &path, policy::TargetKind::RecordId).0,
                Some(record)
            );
        }
    }
}
