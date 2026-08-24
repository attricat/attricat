use axum::{
    extract::{FromRequestParts, Request, State},
    http::{Method, request::Parts},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use super::{AppState, error::ApiError};

const USER_HEADER: &str = "x-catalog-user-id";
const WORKSPACE_HEADER: &str = "x-catalog-workspace-id";

/// Verified request identity. It is inserted only after membership and policy
/// evaluation, never constructed from an unvalidated handler argument.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct AuthenticatedPrincipal(pub Uuid);

/// The workspace selected by the authenticated request headers.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveWorkspace(pub Uuid);

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
    if path == "/health" {
        return Ok(next.run(request).await);
    }

    let principal = request
        .headers()
        .get(USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .ok_or_else(ApiError::unauthenticated)?;
    let workspace = request
        .headers()
        .get(WORKSPACE_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .ok_or_else(ApiError::unauthenticated)?;
    // Catalog repositories use the deployment-configured RLS workspace. Do
    // not accept a header that would authorize one workspace then read another
    // connection's configured tenant.
    if workspace != state.workspace_id {
        return Err(ApiError::forbidden());
    }
    let matched = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|matched| matched.as_str())
        .unwrap_or(path);
    let policy = policy(request.method(), matched).ok_or_else(ApiError::forbidden)?;
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

    request
        .extensions_mut()
        .insert(AuthenticatedPrincipal(principal));
    request.extensions_mut().insert(ActiveWorkspace(workspace));
    Ok(next.run(request).await)
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
