//! A workspace's OpenID Connect provider, managed by its administrators and
//! by the deployment operator through the same operations.

use attricat_domain::states::PasswordSignIn;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{AppState, auth::ScopedRepository, error::ApiError, extractors::ApiJson};
use crate::repository::{AttricatRepository, ErrorCode, IdentityProvider, IdentityProviderInput};

const ISSUER_MAX_BYTES: usize = 2048;
const CLIENT_ID_MAX_CHARS: usize = 512;
const CLIENT_SECRET_MAX_BYTES: usize = 4096;
const DISPLAY_NAME_MAX_CHARS: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PutProviderRequest {
    issuer: String,
    client_id: String,
    client_secret: Option<String>,
    display_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateProviderStateRequest {
    enabled: Option<bool>,
    password_sign_in: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ProviderResponse {
    #[serde(flatten)]
    provider: IdentityProvider,
    /// The address to register with the provider, when this deployment has
    /// a callback configured; sign-in is unavailable without one.
    redirect_uri: Option<String>,
}

fn invalid(message: &str) -> ApiError {
    ApiError::unprocessable(ErrorCode::InvalidIdentityProvider, message)
}

/// Issuers are compared exactly, so the value is kept as it was given. Plain
/// HTTP is only for local development providers.
fn validate_issuer(issuer: &str, allow_http: bool) -> Result<(), ApiError> {
    let url = Url::parse(issuer).map_err(|_| invalid("issuer must be a URL"))?;
    let scheme_allowed = url.scheme() == "https" || (allow_http && url.scheme() == "http");
    if !scheme_allowed
        || issuer.len() > ISSUER_MAX_BYTES
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "issuer must be an HTTPS URL with no credentials, query or fragment",
        ));
    }
    Ok(())
}

fn response(state: &AppState, provider: IdentityProvider) -> ProviderResponse {
    ProviderResponse {
        provider,
        redirect_uri: state
            .oidc
            .as_ref()
            .map(|oidc| oidc.callback_url.to_string()),
    }
}

pub(super) async fn read_provider(
    state: &AppState,
    repository: &AttricatRepository,
) -> Result<Json<ProviderResponse>, ApiError> {
    let provider = repository
        .identity_provider()
        .await?
        .ok_or_else(|| ApiError::not_found("identity provider"))?;
    Ok(Json(response(state, provider)))
}

pub(super) async fn put_provider(
    state: &AppState,
    repository: &AttricatRepository,
    input: PutProviderRequest,
) -> Result<Json<ProviderResponse>, ApiError> {
    validate_issuer(&input.issuer, state.devtools_enabled)?;
    let client_id = input.client_id.trim();
    if client_id.is_empty() || client_id.chars().count() > CLIENT_ID_MAX_CHARS {
        return Err(invalid("client_id must be 1 to 512 characters"));
    }
    let display_name = input.display_name.trim();
    if display_name.is_empty() || display_name.chars().count() > DISPLAY_NAME_MAX_CHARS {
        return Err(invalid("display_name must be 1 to 64 characters"));
    }
    if input
        .client_secret
        .as_deref()
        .is_some_and(|secret| secret.is_empty() || secret.len() > CLIENT_SECRET_MAX_BYTES)
    {
        return Err(invalid("client_secret must be 1 to 4096 bytes"));
    }
    let provider = repository
        .put_identity_provider(&IdentityProviderInput {
            issuer: &input.issuer,
            client_id,
            client_secret: input.client_secret.as_deref(),
            display_name,
        })
        .await?;
    Ok(Json(response(state, provider)))
}

pub(super) async fn update_provider_state(
    state: &AppState,
    repository: &AttricatRepository,
    input: UpdateProviderStateRequest,
) -> Result<Json<ProviderResponse>, ApiError> {
    let password_sign_in = input
        .password_sign_in
        .as_deref()
        .map(|value| {
            PasswordSignIn::parse(value)
                .ok_or_else(|| invalid("password_sign_in must be all or owners"))
        })
        .transpose()?;
    let provider = repository
        .update_identity_provider_state(input.enabled, password_sign_in)
        .await?;
    Ok(Json(response(state, provider)))
}

pub(super) async fn delete_provider(
    repository: &AttricatRepository,
) -> Result<StatusCode, ApiError> {
    if repository.delete_identity_provider().await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("identity provider"))
    }
}

/// Demo visitors share seeded accounts, so sign-in stays as it is there.
fn reject_in_demo(state: &AppState) -> Result<(), ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    Ok(())
}

pub(super) async fn get(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<ProviderResponse>, ApiError> {
    read_provider(&state, &repository).await
}

pub(super) async fn put(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<PutProviderRequest>,
) -> Result<Json<ProviderResponse>, ApiError> {
    reject_in_demo(&state)?;
    put_provider(&state, &repository, input).await
}

pub(super) async fn patch(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<UpdateProviderStateRequest>,
) -> Result<Json<ProviderResponse>, ApiError> {
    reject_in_demo(&state)?;
    update_provider_state(&state, &repository, input).await
}

pub(super) async fn delete(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<StatusCode, ApiError> {
    reject_in_demo(&state)?;
    delete_provider(&repository).await
}

#[cfg(test)]
mod tests {
    use super::validate_issuer;

    #[test]
    fn issuers_must_be_plain_https_urls() {
        for valid in [
            "https://accounts.example.com",
            "https://idp.example.com/realms/acme/",
        ] {
            assert!(validate_issuer(valid, false).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "not a url",
            "http://idp.example.com",
            "https://user:pass@idp.example.com",
            "https://idp.example.com?x=1",
            "https://idp.example.com#x",
            "ftp://idp.example.com",
        ] {
            assert!(validate_issuer(invalid, false).is_err(), "{invalid}");
        }
        assert!(validate_issuer("http://127.0.0.1:8080/realms/a", true).is_ok());
    }
}
