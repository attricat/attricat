//! The deployment operator's API. It stands outside every workspace, so these
//! routes bypass member authorization and carry their own bearer check. They
//! do not exist unless the deployment configures an operator token.

use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::{
    AppState,
    auth::bearer_credentials,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    members::{deliver_invitation, deliverable_email, new_invitation_secrets},
    pagination::{ArrayPage, array_response},
};
use crate::repository::{
    NewOperatorWorkspace, OperatorWorkspace, OwnerInvitation, OwnerInvitationPolicy,
};

const OWNER_INVITATION_LIFETIME_DAYS: i64 = 7;
const WORKSPACE_NAME_MAX_CHARS: usize = 120;

/// The configured operator token. Only its digest stays in memory, and
/// comparing digests does not reveal the token's length.
#[derive(Clone)]
pub struct OperatorToken([u8; 32]);

impl OperatorToken {
    pub fn new(secret: &str) -> Self {
        Self(Sha256::digest(secret.as_bytes()).into())
    }

    fn matches(&self, supplied: &str) -> bool {
        Sha256::digest(supplied.as_bytes())
            .as_slice()
            .ct_eq(&self.0)
            .into()
    }
}

pub(super) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/operator/workspaces", get(list_workspaces))
        .route(
            "/operator/workspaces/{workspace_id}",
            put(put_workspace).get(get_workspace),
        )
        .route(
            "/operator/workspaces/{workspace_id}/owner-invitation",
            post(replace_owner_invitation),
        )
        .route_layer(middleware::from_fn_with_state(state, require_operator))
}

async fn require_operator(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(token) = &state.operator_token else {
        return Err(ApiError::not_found("operator API"));
    };
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    if !bearer_credentials(request.headers()).is_some_and(|supplied| token.matches(supplied)) {
        return Err(ApiError::unauthenticated());
    }
    Ok(next.run(request).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PutWorkspaceRequest {
    name: String,
    login_identifier: String,
    owner_email: String,
    owner_display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplaceOwnerInvitationRequest {
    owner_email: Option<String>,
    owner_display_name: Option<String>,
}

/// Creates a workspace, or finishes and confirms an earlier identical request.
/// Every step is safe to repeat, so a client retries with the same ID.
async fn put_workspace(
    State(state): State<AppState>,
    ApiPath(workspace_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PutWorkspaceRequest>,
) -> Result<(StatusCode, Json<OperatorWorkspace>), ApiError> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > WORKSPACE_NAME_MAX_CHARS {
        return Err(ApiError::invalid_input(format!(
            "name must be 1 to {WORKSPACE_NAME_MAX_CHARS} characters"
        )));
    }
    let login_identifier = input.login_identifier.trim().to_lowercase();
    let owner_email = deliverable_email(&input.owner_email)?;
    let created = state
        .repository
        .create_operator_workspace(&NewOperatorWorkspace {
            id: workspace_id,
            name,
            login_identifier: &login_identifier,
            owner_email: &owner_email,
        })
        .await?;
    invite_owner(
        &state,
        workspace_id,
        &owner_email,
        input.owner_display_name.as_deref(),
        OwnerInvitationPolicy::KeepPending,
    )
    .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(load_workspace(&state, workspace_id).await?)))
}

async fn get_workspace(
    State(state): State<AppState>,
    ApiPath(workspace_id): ApiPath<Uuid>,
) -> Result<Json<OperatorWorkspace>, ApiError> {
    Ok(Json(load_workspace(&state, workspace_id).await?))
}

async fn list_workspaces(
    State(state): State<AppState>,
    ApiQuery(page): ApiQuery<ArrayPage>,
) -> Result<Response, ApiError> {
    let (limit, offset) = page.bounds()?;
    Ok(array_response(
        state
            .repository
            .list_operator_workspaces(limit, offset)
            .await?,
        (limit, offset),
    ))
}

/// Sends a new invitation while the workspace has no owner, optionally to a
/// different address. The earlier link stops working.
async fn replace_owner_invitation(
    State(state): State<AppState>,
    ApiPath(workspace_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<ReplaceOwnerInvitationRequest>,
) -> Result<Json<OperatorWorkspace>, ApiError> {
    let workspace = load_workspace(&state, workspace_id).await?;
    let owner_email = input
        .owner_email
        .or(workspace.owner_email)
        .ok_or_else(|| ApiError::invalid_input("owner_email is required".to_owned()))?;
    let owner_email = deliverable_email(&owner_email)?;
    invite_owner(
        &state,
        workspace_id,
        &owner_email,
        input.owner_display_name.as_deref(),
        OwnerInvitationPolicy::ReplacePending,
    )
    .await?;
    Ok(Json(load_workspace(&state, workspace_id).await?))
}

async fn load_workspace(
    state: &AppState,
    workspace_id: Uuid,
) -> Result<OperatorWorkspace, ApiError> {
    state
        .repository
        .operator_workspace(workspace_id)
        .await?
        .ok_or_else(|| ApiError::not_found("workspace"))
}

/// Finishes the workspace's setup and sends its owner invitation. Secrets are
/// kept only as digests, so an email that cannot be sent cannot be retried:
/// the invitation is withdrawn and a repeated request issues a new one.
async fn invite_owner(
    state: &AppState,
    workspace_id: Uuid,
    email: &str,
    display_name: Option<&str>,
    policy: OwnerInvitationPolicy,
) -> Result<(), ApiError> {
    state.repository.initialize_workspace(workspace_id).await?;
    let secrets = new_invitation_secrets();
    let Some(needs_password_setup) = state
        .repository
        .issue_owner_invitation(
            workspace_id,
            OwnerInvitation {
                id: Uuid::new_v4(),
                email,
                display_name,
                invitation_digest: secrets.invitation_digest(),
                action_digest: secrets.onboarding_digest(),
                expires_at: Utc::now() + Duration::days(OWNER_INVITATION_LIFETIME_DAYS),
            },
            policy,
        )
        .await?
    else {
        return Ok(());
    };
    if deliver_invitation(state, email, needs_password_setup, &secrets)
        .await
        .is_err()
    {
        state
            .repository
            .revoke_pending_owner_invitations(workspace_id)
            .await?;
        return Err(ApiError::invitation_delivery_failed());
    }
    Ok(())
}
