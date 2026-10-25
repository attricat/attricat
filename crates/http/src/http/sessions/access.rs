use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    error::ApiError,
    extractors::ApiJson,
};
use crate::repository::AuthorizationTargetKind;
use attricat_domain::permissions::Permission;

/// Targets one access check may name; a page of explorer rows fits.
const MAX_ACCESS_TARGETS: usize = 500;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in super::super) enum AccessTargetKind {
    Record,
    Blueprint,
    Context,
}

impl AccessTargetKind {
    /// The permissions the API checks against targets of this kind.
    fn scoped_permissions(self) -> &'static [Permission] {
        match self {
            Self::Record => &[
                Permission::RecordsRead,
                Permission::RecordsWrite,
                Permission::RecordsDelete,
                Permission::RecordsPublish,
                Permission::RecordsUnlock,
            ],
            Self::Blueprint => &[
                Permission::BlueprintsWrite,
                Permission::BlueprintsPublish,
                Permission::RecordsPublish,
            ],
            Self::Context => &[Permission::ContextsWrite],
        }
    }
}

impl From<AccessTargetKind> for AuthorizationTargetKind {
    fn from(kind: AccessTargetKind) -> Self {
        match kind {
            AccessTargetKind::Record => Self::Record,
            AccessTargetKind::Blueprint => Self::Blueprint,
            AccessTargetKind::Context => Self::Context,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct AccessRequest {
    permission: Permission,
    kind: AccessTargetKind,
    ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub(in super::super) struct AccessResponse {
    /// The requested IDs the caller may act on, in request order.
    granted: Vec<Uuid>,
}

/// Which of the named targets the caller holds `permission` on, counting
/// record, blueprint-family and context-subtree grants as the API does.
pub(in super::super) async fn check_access(
    State(state): State<AppState>,
    principal: AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiJson(request): ApiJson<AccessRequest>,
) -> Result<Json<AccessResponse>, ApiError> {
    if !request
        .kind
        .scoped_permissions()
        .contains(&request.permission)
    {
        return Err(ApiError::invalid_input(format!(
            "permission {} is not checked against this kind of target",
            request.permission
        )));
    }
    if request.ids.len() > MAX_ACCESS_TARGETS {
        return Err(ApiError::invalid_input(format!(
            "at most {MAX_ACCESS_TARGETS} targets can be checked at once"
        )));
    }
    let AuthenticatedPrincipal(user_id, _) = principal;
    let repository = &state.repository;
    if !repository
        .principal_token_permits(principal.actor(), workspace_id, request.permission)
        .await?
    {
        return Ok(Json(AccessResponse {
            granted: Vec::new(),
        }));
    }
    let authorized = repository
        .authorized_target_ids(
            user_id,
            workspace_id,
            request.permission,
            request.kind.into(),
            &request.ids,
        )
        .await?;
    let mut granted = Vec::with_capacity(authorized.len());
    for id in request.ids {
        if authorized.contains(&id) && !granted.contains(&id) {
            granted.push(id);
        }
    }
    Ok(Json(AccessResponse { granted }))
}
