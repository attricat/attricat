use axum::{Json, extract::State, http::StatusCode};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::repository::{WorkspaceInvitation, WorkspaceMember};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInvitationRequest {
    email: String,
    role_id: Uuid,
    scope_type: String,
    scope_target_id: Uuid,
    expires_at: DateTime<Utc>,
}
#[derive(Serialize)]
pub(super) struct CreatedInvitation {
    #[serde(flatten)]
    invitation: InvitationResponse,
    secret: String,
}
#[derive(Serialize)]
pub(super) struct InvitationResponse {
    #[serde(flatten)]
    invitation: WorkspaceInvitation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AcceptInvitationRequest {
    secret: String,
}
#[derive(Serialize)]
pub(super) struct AcceptedInvitation {
    membership_id: Uuid,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateMemberRequest {
    state: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrantRoleRequest {
    role_id: Uuid,
    scope_type: String,
    scope_target_id: Uuid,
}
#[derive(Serialize)]
pub(super) struct GrantedRole {
    id: Uuid,
}

pub(super) async fn list_members(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<WorkspaceMember>>, ApiError> {
    Ok(Json(
        repository.list_workspace_members(actor, workspace).await?,
    ))
}

pub(super) async fn update_member(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateMemberRequest>,
) -> Result<StatusCode, ApiError> {
    if repository
        .set_workspace_membership_state(id, actor, workspace, &input.state)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("workspace member"))
    }
}

pub(super) async fn grant_role(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(member_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<GrantRoleRequest>,
) -> Result<(StatusCode, Json<GrantedRole>), ApiError> {
    let id = repository
        .grant_workspace_member_role(
            actor,
            workspace,
            member_id,
            input.role_id,
            &input.scope_type,
            input.scope_target_id,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(GrantedRole { id })))
}

pub(super) async fn revoke_role(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath((_, grant_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    repository
        .revoke_workspace_member_role(actor, workspace, grant_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn transfer_ownership(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository
        .transfer_workspace_ownership(actor, workspace, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn create_invitation(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    let email = input.email.trim().to_lowercase();
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let secret = format!("cat_inv_{}", URL_SAFE_NO_PAD.encode(bytes));
    let id = Uuid::new_v4();
    repository
        .create_workspace_invitation(
            id,
            actor,
            workspace,
            &email,
            input.role_id,
            &input.scope_type,
            input.scope_target_id,
            &Sha256::digest(secret.as_bytes()),
            input.expires_at,
        )
        .await?;
    // The digest is the only durable representation of this one-time secret.
    let invitations = repository
        .list_workspace_invitations(actor, workspace)
        .await?;
    let invitation = invitations
        .into_iter()
        .find(|item| item.id == id)
        .expect("new invitation is visible");
    Ok((
        StatusCode::CREATED,
        Json(CreatedInvitation {
            invitation: InvitationResponse { invitation },
            secret,
        }),
    ))
}

pub(super) async fn list_invitations(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
) -> Result<Json<Vec<WorkspaceInvitation>>, ApiError> {
    Ok(Json(
        repository
            .list_workspace_invitations(actor, workspace)
            .await?,
    ))
}

pub(super) async fn revoke_invitation(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repository
        .revoke_workspace_invitation(id, actor, workspace)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("workspace invitation"))
    }
}

pub(super) async fn accept_invitation(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(user, _): AuthenticatedPrincipal,
    ApiJson(input): ApiJson<AcceptInvitationRequest>,
) -> Result<Json<AcceptedInvitation>, ApiError> {
    let digest = Sha256::digest(input.secret.as_bytes());
    Ok(Json(AcceptedInvitation {
        membership_id: repository
            .accept_workspace_invitation(&digest, user)
            .await?,
    }))
}
