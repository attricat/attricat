use axum::{Json, extract::State, http::StatusCode, response::Response};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    account::validate_password,
    repository::{WorkspaceInvitation, WorkspaceMember},
};

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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateWorkspaceUserRequest {
    email: String,
    display_name: Option<String>,
    role_id: Option<Uuid>,
    scope_type: Option<String>,
    scope_target_id: Option<Uuid>,
    expires_at: Option<DateTime<Utc>>,
}
#[derive(Serialize)]
pub(super) struct CreatedWorkspaceUser {
    user_id: Uuid,
    invitation: Option<InvitationResponse>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteOnboardingRequest {
    invitation_secret: String,
    onboarding_secret: String,
    password: String,
}
/// Demo visitors share seeded accounts, so membership, grants and invitations
/// are fixed there; invitations would also send mail on the visitor's behalf.
fn reject_in_demo(state: &AppState) -> Result<(), ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    Ok(())
}

pub(super) async fn create_workspace_user(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<CreateWorkspaceUserRequest>,
) -> Result<(StatusCode, Json<CreatedWorkspaceUser>), ApiError> {
    reject_in_demo(&state)?;
    let email = deliverable_email(&input.email)?;
    let invite_fields = (
        input.role_id,
        input.scope_type,
        input.scope_target_id,
        input.expires_at,
    );
    let (role_id, scope_type, scope_target_id, expires_at) = match invite_fields {
        (None, None, None, None) => {
            let user = repository
                .create_workspace_user(
                    actor,
                    workspace,
                    &email,
                    input.display_name.as_deref(),
                    None,
                    None,
                )
                .await?;
            return Ok((
                StatusCode::CREATED,
                Json(CreatedWorkspaceUser {
                    user_id: user.user_id,
                    invitation: None,
                }),
            ));
        }
        (Some(role_id), Some(scope_type), Some(scope_target_id), Some(expires_at)) => {
            (role_id, scope_type, scope_target_id, expires_at)
        }
        _ => {
            return Err(ApiError::invalid_input(
                "role_id, scope_type, scope_target_id, and expires_at must be supplied together"
                    .to_owned(),
            ));
        }
    };
    let invitation_secret = opaque_secret("cat_inv_");
    let onboarding_secret = opaque_secret("cat_onb_");
    let user = repository
        .create_workspace_user(
            actor,
            workspace,
            &email,
            input.display_name.as_deref(),
            Some((
                Uuid::new_v4(),
                role_id,
                scope_type,
                scope_target_id,
                Sha256::digest(invitation_secret.as_bytes()).to_vec(),
                expires_at,
            )),
            Some(Sha256::digest(onboarding_secret.as_bytes()).to_vec()),
        )
        .await?;
    let invitation_id = user
        .invitation_id
        .ok_or_else(|| ApiError::internal("workspace user invitation was not created"))?;
    let invitation = repository
        .workspace_invitation(actor, workspace, invitation_id)
        .await?;
    let delivery_url = if user.needs_password_setup {
        action_url(
            &state.workspace_onboarding_url,
            &[
                ("onboarding_secret", &onboarding_secret),
                ("invitation_secret", &invitation_secret),
            ],
        )?
    } else {
        action_url(
            &state.workspace_invitation_url,
            &[("secret", &invitation_secret)],
        )?
    };
    if user.needs_password_setup {
        state
            .mail_delivery
            .deliver_workspace_onboarding(&email, &delivery_url)
            .await
    } else {
        state
            .mail_delivery
            .deliver_workspace_invitation(&email, &delivery_url)
            .await
    }
    .map_err(|error| {
        tracing::error!(%error, "workspace invitation email delivery failed");
        ApiError::internal("workspace invitation email could not be delivered")
    })?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedWorkspaceUser {
            user_id: user.user_id,
            invitation: Some(InvitationResponse { invitation }),
        }),
    ))
}

pub(super) async fn complete_onboarding(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<CompleteOnboardingRequest>,
) -> Result<Response, ApiError> {
    validate_password(&input.password)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let password_hash = super::sessions::hash_password(input.password).await?;
    let onboarding = state
        .repository
        .complete_workspace_onboarding(
            &Sha256::digest(input.invitation_secret.as_bytes()),
            &Sha256::digest(input.onboarding_secret.as_bytes()),
            password_hash.as_phc(),
        )
        .await?;
    let account = state.repository.user_account(onboarding.user_id).await?;
    let credential = state
        .repository
        .local_login_credential(&account.email)
        .await?
        .ok_or_else(ApiError::unauthenticated)?;
    let (session, csrf, expires_at) = super::sessions::issue_session();
    state
        .repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            onboarding.workspace_id,
            &session.digest(),
            &csrf.digest(),
            expires_at,
        )
        .await?;
    super::sessions::onboarding_session_response(
        &state,
        onboarding.user_id,
        onboarding.workspace_id,
        onboarding.membership_id,
        &session,
        &csrf,
        state.session_cookie_secure,
        crate::constants::SESSION_LIFETIME_HOURS,
    )
    .await
}

/// Normalizes an invitee address and rejects any the mail transport could not
/// deliver to, before an invitation is committed that could never be sent.
fn deliverable_email(raw: &str) -> Result<String, ApiError> {
    let email = raw.trim().to_lowercase();
    email
        .parse::<lettre::Address>()
        .map_err(|_| ApiError::invalid_input("email must be a valid address".to_owned()))?;
    Ok(email)
}

pub(super) fn action_url(base: &str, parameters: &[(&str, &str)]) -> Result<String, ApiError> {
    let mut url =
        Url::parse(base).map_err(|_| ApiError::internal("workspace action URL is invalid"))?;
    url.query_pairs_mut()
        .extend_pairs(parameters.iter().copied());
    Ok(url.into())
}

fn opaque_secret(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!("{prefix}{}", URL_SAFE_NO_PAD.encode(bytes))
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
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateMemberRequest>,
) -> Result<StatusCode, ApiError> {
    reject_in_demo(&state)?;
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
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(member_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<GrantRoleRequest>,
) -> Result<(StatusCode, Json<GrantedRole>), ApiError> {
    reject_in_demo(&state)?;
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
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath((member_id, grant_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    reject_in_demo(&state)?;
    repository
        .revoke_workspace_member_role(actor, workspace, member_id, grant_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn transfer_ownership(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    reject_in_demo(&state)?;
    repository
        .transfer_workspace_ownership(actor, workspace, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn create_invitation(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace): ActiveWorkspace,
    ApiJson(input): ApiJson<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    reject_in_demo(&state)?;
    let email = deliverable_email(&input.email)?;
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
    let invitation = repository
        .workspace_invitation(actor, workspace, id)
        .await?;
    let delivery_url = action_url(&state.workspace_invitation_url, &[("secret", &secret)])?;
    state
        .mail_delivery
        .deliver_workspace_invitation(&email, &delivery_url)
        .await
        .map_err(|error| {
            tracing::error!(%error, "workspace invitation email delivery failed");
            ApiError::internal("workspace invitation email could not be delivered")
        })?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedInvitation {
            invitation: InvitationResponse { invitation },
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
