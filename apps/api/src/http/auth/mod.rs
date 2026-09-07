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
    repository::CatalogRepository,
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

/// The workspace selected by the verified credential or browser session.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveWorkspace(pub Uuid);

/// The session digest is retained only to revoke the current browser session;
/// raw cookie credentials never reach a handler.
#[derive(Clone)]
pub(super) struct AuthenticatedSession(pub SessionDigest);

/// Repository whose connections are pinned to the authenticated workspace's
/// RLS setting. It is inserted only by authorization after workspace selection.
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
    let path = request.uri().path();
    if path == "/health"
        || path == "/auth/login"
        || path == "/auth/discover"
        || path == "/auth/password-reset"
        || path == "/auth/password-reset/confirm"
        || path == "/onboarding/complete"
    {
        return Ok(next.run(request).await);
    }

    let bearer = request
        .headers()
        .get(AUTHORIZATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let (principal, workspace, token_id, session_digest) = if let Some(secret) = bearer {
        let digest = sha2::Sha256::digest(secret.as_bytes());
        state
            .repository
            .authenticate_personal_api_token(&digest)
            .await?
            .map(|(token_id, user_id, workspace_id)| (user_id, workspace_id, Some(token_id), None))
            .ok_or_else(ApiError::unauthenticated)?
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
        (user_id, workspace_id, None, None)
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
        )
    };
    let matched = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|matched| matched.as_str())
        .unwrap_or(path);
    // An invitee can have no membership yet, so acceptance deliberately skips
    // the active-membership policy while retaining normal credential validation.
    let accepting_invitation = matched == "/workspace/invitations/accept";
    if let Some(policy) = policy::policy(request.method(), matched) {
        let (target_id, target_code) = policy::target(path, policy.target);
        if !state.repository.is_active_user(principal).await? {
            return Err(ApiError::unauthenticated());
        }
        if !state
            .repository
            .is_active_principal(principal, workspace)
            .await?
        {
            return Err(ApiError::forbidden());
        }
        // File reads are authorized in their handlers after resolving active
        // file-to-entity references. Other routes can authorize from the path.
        if !matches!(
            policy.target,
            policy::TargetKind::FileRead | policy::TargetKind::WorkspaceNavigation
        ) && !state
            .repository
            .is_authorized(
                principal,
                workspace,
                policy.permission,
                target_id,
                target_code.as_deref(),
            )
            .await?
        {
            return Err(ApiError::forbidden());
        }
        if let Some(token_id) = token_id {
            if !state
                .repository
                .personal_api_token_permits(token_id, policy.permission)
                .await?
            {
                return Err(ApiError::forbidden());
            }
        }
    } else if !path.starts_with("/auth/") && !accepting_invitation {
        return Err(ApiError::forbidden());
    }

    let principal = AuthenticatedPrincipal(principal, token_id);
    let audit_context = audit::request_context(&request, principal);
    request.extensions_mut().insert(principal);
    request.extensions_mut().insert(ActiveWorkspace(workspace));
    request.extensions_mut().insert(ScopedRepository(
        state
            .repository
            .for_workspace(workspace)
            .await?
            .with_audit_context(audit_context),
    ));
    if let Some(session_digest) = session_digest {
        request
            .extensions_mut()
            .insert(AuthenticatedSession(session_digest));
    }
    Ok(next.run(request).await)
}

fn cookie_value(header: Option<&str>, name: &str) -> Option<String> {
    header?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}
