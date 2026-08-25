use axum::{
    extract::{FromRequestParts, Request, State},
    http::{Method, request::Parts},
    middleware::Next,
    response::Response,
};
use sha2::Digest;
use uuid::Uuid;

use super::{AppState, error::ApiError};
use crate::{
    account::{SessionDigest, SessionSecret},
    repository::CatalogRepository,
};

const USER_HEADER: &str = "x-catalog-user-id";
const WORKSPACE_HEADER: &str = "x-catalog-workspace-id";
const AUTHORIZATION_HEADER: &str = "authorization";
const SESSION_COOKIE: &str = "catalog_session";
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

#[derive(Clone, Copy)]
struct Policy {
    permission: &'static str,
    target: TargetKind,
}

#[derive(Clone, Copy)]
enum TargetKind {
    None,
    BlueprintId,
    BlueprintCode,
    EntityId,
    ContextId,
    ContextCode,
    ContextList,
}

pub(super) async fn authorize(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let path = request.uri().path();
    if path == "/health" || path == "/auth/login" || path == "/auth/discover" {
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
    if let Some(policy) = policy(request.method(), matched) {
        let (target_id, target_code) = target(path, policy.target);
        if !state
            .repository
            .is_active_principal(principal, workspace)
            .await?
        {
            return Err(ApiError::unauthenticated());
        }
        if !state
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

    request
        .extensions_mut()
        .insert(AuthenticatedPrincipal(principal, token_id));
    request.extensions_mut().insert(ActiveWorkspace(workspace));
    request.extensions_mut().insert(ScopedRepository(
        state.repository.for_workspace(workspace).await?,
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

fn policy(method: &Method, path: &str) -> Option<Policy> {
    let read = |target| Policy {
        permission: "entities.read",
        target,
    };
    let write = |target| Policy {
        permission: "entities.write",
        target,
    };
    let blueprint = if method == Method::GET {
        "blueprints.read"
    } else if path.ends_with("/publish") {
        "blueprints.publish"
    } else {
        "blueprints.write"
    };
    if path == "/workspace/invitations/accept" {
        return None;
    }
    if path == "/workspace/assignable-roles" || path.starts_with("/workspace/grant-targets/") {
        return Some(Policy {
            permission: "members.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/token-permissions" {
        return Some(Policy {
            permission: "tokens.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/roles"
        || path == "/workspace/permissions"
        || path.starts_with("/workspace/roles/")
    {
        return Some(Policy {
            permission: "roles.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/members"
        || path.starts_with("/workspace/members/")
        || path == "/workspace/invitations"
        || path.starts_with("/workspace/invitations/")
    {
        return Some(Policy {
            permission: "members.manage",
            target: TargetKind::None,
        });
    }
    if path == "/personal-access-tokens" || path.starts_with("/personal-access-tokens/") {
        return Some(Policy {
            permission: "tokens.manage",
            target: TargetKind::None,
        });
    }
    if path == "/metrics" {
        return Some(Policy {
            permission: "data_health.read",
            target: TargetKind::None,
        });
    }
    if path.starts_with("/data-health/") {
        return Some(Policy {
            permission: "data_health.read",
            target: TargetKind::None,
        });
    }
    if path.starts_with("/blueprints/by-code/{code}") {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::BlueprintCode,
        });
    }
    if path.starts_with("/blueprints/{blueprint_id}") {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::BlueprintId,
        });
    }
    if path == "/blueprints" || path == "/blueprints/catalogue" {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::None,
        });
    }
    if path == "/contexts/{code}" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: TargetKind::ContextCode,
        });
    }
    if path == "/contexts/id/{id}" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: TargetKind::ContextId,
        });
    }
    if path == "/contexts" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: if method == Method::GET {
                TargetKind::ContextList
            } else {
                TargetKind::None
            },
        });
    }
    if path.starts_with("/v1/entities/{entity_id}") || path.starts_with("/entities/{entity_id}") {
        return Some(if method == Method::DELETE {
            Policy {
                permission: "entities.delete",
                target: TargetKind::EntityId,
            }
        } else if method == Method::GET {
            read(TargetKind::EntityId)
        } else {
            write(TargetKind::EntityId)
        });
    }
    if path == "/v1/entities" {
        return Some(write(TargetKind::None));
    }
    if path == "/v1/entities/search"
        || path == "/v1/entities/facets/relationship-tree/children"
        || path == "/entities"
    {
        return Some(read(TargetKind::None));
    }
    None
}

fn target(path: &str, kind: TargetKind) -> (Option<Uuid>, Option<String>) {
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    match kind {
        TargetKind::None => (None, None),
        TargetKind::BlueprintId => (segments.get(1).and_then(|value| value.parse().ok()), None),
        TargetKind::BlueprintCode => (None, segments.get(2).map(|value| (*value).to_owned())),
        TargetKind::EntityId => {
            let index = if segments.first() == Some(&"v1") {
                2
            } else {
                1
            };
            (
                segments.get(index).and_then(|value| value.parse().ok()),
                None,
            )
        }
        TargetKind::ContextId => (segments.get(2).and_then(|value| value.parse().ok()), None),
        TargetKind::ContextCode => (None, segments.get(1).map(|value| (*value).to_owned())),
        TargetKind::ContextList => (None, Some("__context_list__".to_owned())),
    }
}
