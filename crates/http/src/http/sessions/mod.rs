use axum::{Json, extract::State, response::Response};
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, AuthenticatedSession, ScopedRepository},
    error::ApiError,
    extractors::ApiJson,
};
use crate::{
    account::{
        ActionTokenSecret, IssuedLifecycleAction, LifecycleActionPurpose, SessionDigest,
        SessionSecret, validate_password,
    },
    constants::SESSION_LIFETIME_HOURS,
    repository::RepositoryError,
};

mod password;
mod responses;
pub(super) use password::hash_password;
pub(super) use responses::onboarding_session_response;
use responses::{
    SessionResponse, clear_session_response, session_response, session_response_payload,
};

const PASSWORD_RESET_LIFETIME_MINUTES: i64 = 30;

#[derive(Deserialize)]
pub(super) struct LoginRequest {
    login_identifier: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub(super) struct DiscoveryRequest {
    login_identifier: String,
}

#[derive(Deserialize)]
pub(super) struct PasswordResetRequest {
    email: String,
}

#[derive(Deserialize)]
pub(super) struct PasswordResetConfirmation {
    token: String,
    password: String,
}

/// Per-user display preferences. Every field is required so a request states
/// the full preference set; later preferences are added here.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreferencesRequest {
    time_zone: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DisplayNameRequest {
    display_name: String,
}

const DISPLAY_NAME_MIN_CHARS: usize = 2;
const DISPLAY_NAME_MAX_CHARS: usize = 64;

#[derive(serde::Serialize)]
pub(super) struct DiscoveryResponse {
    login_identifier: String,
    sign_in_methods: Vec<&'static str>,
}

/// Seeded accounts of a demo or development deployment, one per built-in
/// role and sharing one password. These values are deliberately public.
#[derive(Clone, Debug, serde::Serialize)]
pub struct SampleLogins {
    /// The deployment is a public demo with no other workspace.
    pub demo: bool,
    pub login_identifier: String,
    pub password: String,
    /// Ordered from least to most privileged.
    pub accounts: Vec<SampleAccount>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SampleAccount {
    pub role: String,
    pub email: String,
}

/// Returns `null` unless the deployment seeds sample accounts.
pub(super) async fn sample_logins(State(state): State<AppState>) -> Json<Option<SampleLogins>> {
    Json(state.sample_logins)
}

pub(super) async fn discover(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<DiscoveryRequest>,
) -> Result<Json<DiscoveryResponse>, ApiError> {
    let identifier = request.login_identifier.trim().to_lowercase();
    let rate_key = digest_login_key(&identifier);
    if !state
        .repository
        .reserve_workspace_discovery_attempt(&rate_key)
        .await?
    {
        return Err(ApiError::rate_limited());
    }
    let workspace = state
        .repository
        .discover_workspace(&identifier)
        .await?
        .ok_or_else(|| ApiError::not_found("workspace"))?;
    Ok(Json(DiscoveryResponse {
        login_identifier: workspace.login_identifier,
        // This is deliberately extensible for OIDC, SAML, and passkeys.
        sign_in_methods: vec!["local_password"],
    }))
}

/// Always returns 204 so callers cannot determine whether an address has a
/// resettable local credential. Opaque token values only cross the mail boundary.
pub(super) async fn request_password_reset(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<PasswordResetRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    let email = request.email.trim().to_lowercase();
    let rate_key = digest_login_key(&email);
    if !state
        .repository
        .reserve_password_reset_attempt(&rate_key)
        .await?
    {
        return Ok(axum::http::StatusCode::NO_CONTENT);
    }
    let credential = state.repository.local_login_credential(&email).await?;
    let Some(credential) =
        credential.filter(|credential| credential.active && credential.email_verified)
    else {
        return Ok(axum::http::StatusCode::NO_CONTENT);
    };
    let issued = IssuedLifecycleAction::issue(
        LifecycleActionPurpose::PasswordReset,
        Utc::now() + Duration::minutes(PASSWORD_RESET_LIFETIME_MINUTES),
        crate::account::SecurityVersion::new(credential.security_version.into()),
        Some(crate::account::CredentialVersion::new(
            credential.credential_version.into(),
        )),
    );
    state
        .repository
        .issue_lifecycle_token(
            Uuid::new_v4(),
            credential.user_id,
            LifecycleActionPurpose::PasswordReset.as_str(),
            issued.action.token_digest().as_ref(),
            issued.action.expires_at(),
        )
        .await?;
    let reset_url = super::members::action_url(
        &state.password_reset_url,
        &[("token", issued.secret.expose_for_delivery())],
    )?;
    // Delivery failure must not change the response: a 500 only for real,
    // verified accounts would disclose which addresses exist.
    if let Err(error) = state
        .mail_delivery
        .deliver_password_reset(&email, &reset_url)
        .await
    {
        tracing::error!(%error, "password reset email delivery failed");
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(super) async fn confirm_password_reset(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<PasswordResetConfirmation>,
) -> Result<axum::http::StatusCode, ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    let secret = ActionTokenSecret::from_delivery_value(request.token).map_err(|_| {
        ApiError::invalid_input("password reset link is invalid or expired".to_owned())
    })?;
    validate_password(&request.password)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let password_hash = hash_password(request.password).await?;
    state
        .repository
        .consume_password_reset(&secret.digest(), &password_hash)
        .await
        .map_err(|error| match error {
            RepositoryError::NotFound(_) => {
                ApiError::invalid_input("password reset link is invalid or expired".to_owned())
            }
            error => error.into(),
        })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(super) async fn login(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<LoginRequest>,
) -> Result<Response, ApiError> {
    let identifier = request.login_identifier.trim().to_lowercase();
    let workspace = state
        .repository
        .discover_workspace(&identifier)
        .await?
        .ok_or_else(ApiError::invalid_credentials)?;
    let email = request.email.trim().to_lowercase();
    // Password credentials are global to the account. Including the selected
    // workspace lets an attacker multiply guesses by cycling public workspaces,
    // even workspaces the victim has never joined.
    let rate_key = digest_login_key(&email);
    if !state.repository.reserve_login_attempt(&rate_key).await? {
        return Err(ApiError::rate_limited());
    }
    let credential = state
        .repository
        .local_login_credential(&email)
        .await?
        .filter(|credential| credential.active);
    let hash = credential
        .as_ref()
        .map(|credential| credential.password_hash.clone());
    let (true, Some(credential)) = (
        password::verify_password(hash, request.password).await?,
        credential,
    ) else {
        return Err(ApiError::invalid_credentials());
    };
    state.repository.clear_login_failures(&rate_key).await?;
    let (session, csrf, expires_at) = issue_session();
    state
        .repository
        .issue_login_session(
            Uuid::new_v4(),
            &credential,
            workspace.id,
            &session.digest(),
            &csrf.digest(),
            expires_at,
        )
        .await?;
    session_response(
        &state,
        credential.user_id,
        workspace.id,
        workspace.login_identifier,
        &session,
        &csrf,
        state.session_cookie_secure,
        SESSION_LIFETIME_HOURS,
    )
    .await
}

pub(super) async fn current_session(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, token_id): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<SessionResponse>, ApiError> {
    let mut session = session_response_payload(&state, user_id, workspace_id).await?;
    if let Some(token_id) = token_id {
        let permissions = state
            .repository
            .personal_api_token_permissions(token_id)
            .await?;
        session.restrict_to_permissions(&permissions);
    }
    Ok(Json(session))
}

pub(super) async fn update_preferences(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, token_id): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(request): ApiJson<PreferencesRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    let time_zone = request
        .time_zone
        .map(|zone| validate_time_zone(&zone))
        .transpose()?;
    repository
        .update_user_preferences(user_id, time_zone.as_deref())
        .await?;
    current_session(
        State(state),
        AuthenticatedPrincipal(user_id, token_id),
        ActiveWorkspace(workspace_id),
    )
    .await
}

pub(super) async fn update_display_name(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, token_id): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(request): ApiJson<DisplayNameRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    validate_display_name(&request.display_name)?;
    repository
        .update_user_display_name(user_id, &request.display_name)
        .await?;
    current_session(
        State(state),
        AuthenticatedPrincipal(user_id, token_id),
        ActiveWorkspace(workspace_id),
    )
    .await
}

/// Display names are letters and digits in words separated by spaces, with no
/// leading or trailing space. Input is never trimmed, so the stored value is
/// exactly what the user submitted.
fn validate_display_name(name: &str) -> Result<(), ApiError> {
    let length = name.chars().count();
    if !(DISPLAY_NAME_MIN_CHARS..=DISPLAY_NAME_MAX_CHARS).contains(&length) {
        return Err(ApiError::invalid_input(format!(
            "display name must be {DISPLAY_NAME_MIN_CHARS} to {DISPLAY_NAME_MAX_CHARS} characters"
        )));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == ' ') {
        return Err(ApiError::invalid_input(
            "display name may contain only letters, digits, and spaces".to_owned(),
        ));
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(ApiError::invalid_input(
            "display name cannot start or end with a space".to_owned(),
        ));
    }
    Ok(())
}

/// Accepts canonical IANA names (including `UTC`) and returns the
/// database's canonical spelling so equivalent inputs store identically.
fn validate_time_zone(zone: &str) -> Result<String, ApiError> {
    zone.parse::<chrono_tz::Tz>()
        .map(|zone| zone.name().to_owned())
        .map_err(|_| ApiError::invalid_input(format!("unknown IANA time zone: {zone}")))
}

pub(super) async fn logout(
    State(state): State<AppState>,
    AuthenticatedSession(session): AuthenticatedSession,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    state
        .repository
        .revoke_browser_session(&session, workspace_id)
        .await?;
    Ok(clear_session_response(state.session_cookie_secure))
}

pub(super) async fn renew(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    AuthenticatedSession(previous): AuthenticatedSession,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    let (session, csrf, expires_at) = issue_session();
    state
        .repository
        .rotate_browser_session(
            &previous,
            Uuid::new_v4(),
            &session.digest(),
            &csrf.digest(),
            workspace_id,
            expires_at,
        )
        .await?;
    session_response(
        &state,
        user_id,
        workspace_id,
        state
            .repository
            .workspace_login_identifier(workspace_id)
            .await?,
        &session,
        &csrf,
        state.session_cookie_secure,
        SESSION_LIFETIME_HOURS,
    )
    .await
}

pub(super) fn issue_session() -> (SessionSecret, SessionSecret, chrono::DateTime<Utc>) {
    (
        SessionSecret::generate(),
        SessionSecret::generate(),
        Utc::now() + Duration::hours(SESSION_LIFETIME_HOURS),
    )
}

fn digest_login_key(value: &str) -> SessionDigest {
    SessionDigest::from_slice(&Sha256::digest(value.as_bytes())).expect("sha256 is 32 bytes")
}

#[cfg(test)]
mod tests {
    use super::{validate_display_name, validate_time_zone};

    #[test]
    fn display_name_validation_accepts_letters_digits_and_inner_spaces() {
        for valid in [
            "Al",
            "Ada Lovelace",
            "R2 D2",
            "Łukasz Żółć",
            "x  y",
            &"a".repeat(64),
        ] {
            assert!(validate_display_name(valid).is_ok(), "{valid:?}");
        }
        for invalid in [
            "",
            "A",
            " Ada",
            "Ada ",
            "  ",
            "Ada\tLovelace",
            "Ada\nLovelace",
            "Ada-Lovelace",
            "ada@example.test",
            "Ada_",
            &"a".repeat(65),
        ] {
            assert!(validate_display_name(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn time_zone_validation_accepts_iana_names_and_rejects_others() {
        assert_eq!(
            validate_time_zone("Europe/Warsaw").unwrap(),
            "Europe/Warsaw"
        );
        assert_eq!(validate_time_zone("UTC").unwrap(), "UTC");
        for invalid in ["", "Mars/Olympus", "+02:00", "europe warsaw"] {
            assert!(validate_time_zone(invalid).is_err(), "{invalid}");
        }
    }
}
