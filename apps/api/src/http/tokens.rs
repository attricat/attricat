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
use crate::repository::PersonalApiToken;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateTokenRequest {
    label: String,
    permissions: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct CreatedToken {
    #[serde(flatten)]
    token: TokenMetadata,
    /// This is the only response that includes the secret. It is never stored.
    secret: String,
}

#[derive(Serialize)]
pub(super) struct TokenMetadata {
    id: Uuid,
    label: String,
    permissions: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<PersonalApiToken> for TokenMetadata {
    fn from(token: PersonalApiToken) -> Self {
        Self {
            id: token.id,
            label: token.label,
            permissions: token.permissions,
            expires_at: token.expires_at,
            revoked_at: token.revoked_at,
            last_used_at: token.last_used_at,
            created_at: token.created_at,
        }
    }
}

pub(super) async fn create(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiJson(input): ApiJson<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreatedToken>), ApiError> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let secret = format!("cat_pat_{}", URL_SAFE_NO_PAD.encode(bytes));
    let digest = Sha256::digest(secret.as_bytes());
    let token_id = Uuid::new_v4();
    repository
        .issue_personal_api_token(
            token_id,
            user_id,
            workspace_id,
            &input.label,
            &digest,
            &input.permissions,
            input.expires_at,
        )
        .await?;
    let token = TokenMetadata {
        id: token_id,
        label: input.label,
        permissions: input.permissions,
        expires_at: input.expires_at,
        revoked_at: None,
        last_used_at: None,
        created_at: Utc::now(),
    };
    Ok((StatusCode::CREATED, Json(CreatedToken { token, secret })))
}

pub(super) async fn list(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<Vec<TokenMetadata>>, ApiError> {
    Ok(Json(
        repository
            .list_personal_api_tokens(user_id, workspace_id)
            .await?
            .into_iter()
            .map(TokenMetadata::from)
            .collect(),
    ))
}

pub(super) async fn revoke(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ApiPath(token_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repository
        .revoke_personal_api_token(token_id, user_id, workspace_id)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("personal API token"))
    }
}
