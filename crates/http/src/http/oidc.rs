//! Signing in through a workspace's OpenID Connect provider, as a relying
//! party using the authorization code flow with PKCE. Tokens are validated and
//! discarded: the result is an ordinary browser session.
//!
//! A single-use `state`, a cookie holding the secrets, and stored digests of
//! those secrets keep one browser's sign-in from being used by another. A
//! redirect URI per workspace keeps one workspace's provider from receiving a
//! code meant for another's.

use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use attricat_extension_manifest::extensions::is_public_destination;
use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use openidconnect::{
    AccessTokenHash, AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    EndUserEmail, EndpointMaybeSet, EndpointNotSet, EndpointSet, HttpRequest, HttpResponse,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    SubjectIdentifier,
    core::{
        CoreAuthenticationFlow, CoreClient, CoreJwsSigningAlgorithm, CoreProviderMetadata,
        CoreResponseType, CoreTokenResponse,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    AppState, OidcSettings,
    auth::{
        ActiveWorkspace, AuthenticatedPrincipal, AuthenticatedSession, cookie_value,
        session_cookie_digest,
    },
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    members::opaque_secret,
    sessions::{discover_workspace, issue_session, session_redirect},
};
use crate::repository::{
    IdentityProviderSignIn, NewOidcLoginRequest, NewOidcSession, OidcIntent, OidcLoginRequest,
    RepositoryError,
};

const LOGIN_COOKIE: &str = "attricat_oidc";
const LOGIN_REQUEST_LIFETIME_MINUTES: i64 = 10;
const LOGIN_REQUEST_LIFETIME_SECONDS: i64 = LOGIN_REQUEST_LIFETIME_MINUTES * 60;
const COOKIE_PART_SEPARATOR: char = '.';
const SECRET_PREFIX_NONE: &str = "";
const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const PROVIDER_RESPONSE_MAX_BYTES: usize = 1024 * 1024;
/// The web page that finishes a sign-in, on the callback's origin.
const COMPLETION_PATH: &str = "/oidc/complete";
const COMPLETION_ERROR_PARAMETER: &str = "error";
const COMPLETION_LINKED_PARAMETER: &str = "linked";
const SCOPE_EMAIL: &str = "email";
const SCOPE_PROFILE: &str = "profile";
/// How far ahead of this server's clock a provider may date a token.
const ID_TOKEN_CLOCK_SKEW_SECONDS: i64 = 60;
const STARTING_METADATA_LIFETIME: Duration = Duration::from_secs(5 * 60);
const STARTING_METADATA_FAILURE_LIFETIME: Duration = Duration::from_secs(10);
/// Issuers remembered at once. Workspace administrators choose them.
const STARTING_METADATA_MAX_ISSUERS: usize = 1024;

type CachedMetadata = Arc<tokio::sync::Mutex<Option<(Instant, Option<CoreProviderMetadata>)>>>;

/// Provider metadata for starting sign-ins, which anyone may do. Without it an
/// anonymous flood becomes a flood of requests to the provider, and starting
/// needs only an address the browser is sent to. Finishing a sign-in always
/// reads fresh metadata, so a rotated signing key takes effect at once.
#[derive(Clone, Default)]
pub struct StartingMetadataCache(Arc<Mutex<HashMap<String, CachedMetadata>>>);

impl StartingMetadataCache {
    fn entry(&self, issuer: &str) -> CachedMetadata {
        let mut entries = self.0.lock().expect("metadata cache lock is not poisoned");
        if entries.len() >= STARTING_METADATA_MAX_ISSUERS && !entries.contains_key(issuer) {
            entries.clear();
        }
        entries.entry(issuer.to_owned()).or_default().clone()
    }

    /// One discovery per issuer runs at a time; the others wait for its result.
    async fn get(
        &self,
        provider: &IdentityProviderSignIn,
        http: &(impl Fn(HttpRequest) -> ProviderFuture + Sync),
    ) -> Result<CoreProviderMetadata, ApiError> {
        let entry = self.entry(&provider.issuer);
        let mut cached = entry.lock().await;
        if let Some((read_at, metadata)) = cached.as_ref() {
            let lifetime = if metadata.is_some() {
                STARTING_METADATA_LIFETIME
            } else {
                STARTING_METADATA_FAILURE_LIFETIME
            };
            if read_at.elapsed() < lifetime {
                return metadata
                    .clone()
                    .ok_or_else(ApiError::oidc_provider_unavailable);
            }
        }
        let metadata = discover_provider(provider, http).await;
        *cached = Some((Instant::now(), metadata.as_ref().ok().cloned()));
        metadata
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartRequest {
    login_identifier: String,
    /// The secret from an invitation email, to accept it by signing in.
    invitation_secret: Option<String>,
}

#[derive(Serialize)]
pub(super) struct StartResponse {
    authorization_url: String,
}

#[derive(Serialize)]
pub(super) struct LinkStatus {
    display_name: String,
    linked: bool,
}

#[derive(Default, Deserialize)]
pub(super) struct CallbackQuery {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
    iss: Option<String>,
}

/// Why a request to a provider failed. Never carries a provider's response.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct ProviderHttpError(&'static str);

/// Sends one request to a provider. Workspace administrators choose the issuer
/// and the provider's metadata chooses the rest, so every request is checked:
/// no redirects or proxies, and only a public address unless the deployment
/// allows that host. The connection is pinned to the addresses that passed.
async fn provider_request(
    settings: OidcSettings,
    allow_http: bool,
    request: HttpRequest,
) -> Result<HttpResponse, ProviderHttpError> {
    let uri = request.uri().clone();
    let host = uri
        .host()
        .ok_or(ProviderHttpError("provider URL has no host"))?
        .trim_matches(['[', ']'])
        .to_lowercase();
    let secure = uri.scheme_str() == Some("https");
    if !secure && !(allow_http && uri.scheme_str() == Some("http")) {
        return Err(ProviderHttpError("provider URL must use HTTPS"));
    }
    let port = uri.port_u16().unwrap_or(if secure { 443 } else { 80 });
    let addresses: Vec<SocketAddr> = tokio::time::timeout(
        PROVIDER_REQUEST_TIMEOUT,
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .map_err(|_| ProviderHttpError("provider DNS lookup timed out"))?
    .map_err(|_| ProviderHttpError("provider DNS lookup failed"))?
    .collect();
    let allowed = settings.allowed_private_hosts.contains(&host);
    if addresses.is_empty()
        || (!allowed
            && addresses
                .iter()
                .any(|address| !is_public_destination(address.ip())))
    {
        return Err(ProviderHttpError("provider resolves to an unsafe address"));
    }
    let mut client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&host, &addresses)
        .connect_timeout(PROVIDER_REQUEST_TIMEOUT)
        .timeout(PROVIDER_REQUEST_TIMEOUT);
    if let Some(pem) = &settings.extra_root_certificates_pem {
        for certificate in reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|_| ProviderHttpError("OIDC_CA_FILE is not valid PEM"))?
        {
            client = client.add_root_certificate(certificate);
        }
    }
    let client = client
        .build()
        .map_err(|_| ProviderHttpError("provider client is unavailable"))?;
    let (parts, body) = request.into_parts();
    let mut response = client
        .request(parts.method, uri.to_string())
        .headers(parts.headers)
        .body(body)
        .send()
        .await
        .map_err(|_| ProviderHttpError("provider request failed"))?;
    let mut builder = axum::http::Response::builder().status(response.status());
    if let Some(headers) = builder.headers_mut() {
        *headers = response.headers().clone();
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ProviderHttpError("provider response failed"))?
    {
        if bytes.len() + chunk.len() > PROVIDER_RESPONSE_MAX_BYTES {
            return Err(ProviderHttpError("provider response is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    builder
        .body(bytes)
        .map_err(|_| ProviderHttpError("provider response is invalid"))
}

type ProviderFuture = Pin<Box<dyn Future<Output = Result<HttpResponse, ProviderHttpError>> + Send>>;

/// The HTTP client the OpenID Connect library calls.
fn provider_client(
    settings: &OidcSettings,
    allow_http: bool,
) -> impl Fn(HttpRequest) -> ProviderFuture + use<> {
    let settings = settings.clone();
    move |request| Box::pin(provider_request(settings.clone(), allow_http, request))
}

fn settings(state: &AppState) -> Result<&OidcSettings, ApiError> {
    state
        .oidc
        .as_ref()
        .ok_or_else(|| ApiError::not_found("single sign-on"))
}

fn digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

/// What the browser keeps for the callback.
struct LoginCookie {
    binding: String,
    pkce_verifier: String,
    nonce: String,
}

impl LoginCookie {
    fn encode(&self) -> String {
        [
            self.binding.as_str(),
            self.pkce_verifier.as_str(),
            self.nonce.as_str(),
        ]
        .join(&COOKIE_PART_SEPARATOR.to_string())
    }

    fn decode(value: &str) -> Option<Self> {
        let mut parts = value.split(COOKIE_PART_SEPARATOR);
        let cookie = Self {
            binding: parts.next()?.to_owned(),
            pkce_verifier: parts.next()?.to_owned(),
            nonce: parts.next()?.to_owned(),
        };
        parts.next().is_none().then_some(cookie)
    }
}

fn login_cookie(value: &str, secure: bool, max_age_seconds: i64) -> HeaderValue {
    let mut cookie = format!(
        "{LOGIN_COOKIE}={value}; Path=/; SameSite=Lax; HttpOnly; Max-Age={max_age_seconds}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).expect("base64url values make a valid cookie")
}

type ProviderOidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// Reads the provider's current metadata and signing keys.
async fn discover_provider(
    provider: &IdentityProviderSignIn,
    http: &(impl Fn(HttpRequest) -> ProviderFuture + Sync),
) -> Result<CoreProviderMetadata, ApiError> {
    let issuer = IssuerUrl::new(provider.issuer.clone())
        .map_err(|_| ApiError::oidc_provider_unavailable())?;
    CoreProviderMetadata::discover_async(issuer, http)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "identity provider discovery failed");
            ApiError::oidc_provider_unavailable()
        })
}

fn provider_oidc_client(
    settings: &OidcSettings,
    workspace_id: Uuid,
    provider: &IdentityProviderSignIn,
    metadata: CoreProviderMetadata,
) -> Result<ProviderOidcClient, ApiError> {
    Ok(CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(provider.client_id.clone()),
        Some(ClientSecret::new(provider.client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(settings.redirect_uri(workspace_id))
            .map_err(|_| ApiError::internal("OIDC_CALLBACK_URL is invalid"))?,
    ))
}

/// Starts a sign-in at the provider and returns where to send the browser.
async fn begin(
    state: &AppState,
    workspace_id: Uuid,
    provider: &IdentityProviderSignIn,
    intent: OidcIntent,
) -> Result<Response, ApiError> {
    let settings = settings(state)?;
    let http = provider_client(settings, state.devtools_enabled);
    let metadata = settings.starting_metadata.get(provider, &http).await?;
    let client = provider_oidc_client(settings, workspace_id, provider, metadata)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode as AuthenticationFlow<CoreResponseType>,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new(SCOPE_EMAIL.to_owned()))
        .add_scope(Scope::new(SCOPE_PROFILE.to_owned()))
        .set_pkce_challenge(challenge)
        .url();
    // The provider's metadata chose this address and the browser is about to
    // be sent there, so a `javascript:` URL here would run on our origin.
    let scheme_allowed = authorization_url.scheme() == "https"
        || (state.devtools_enabled && authorization_url.scheme() == "http");
    if !scheme_allowed {
        tracing::warn!("identity provider authorization endpoint has an unsafe scheme");
        return Err(ApiError::oidc_provider_unavailable());
    }
    let cookie = LoginCookie {
        binding: opaque_secret(SECRET_PREFIX_NONE),
        pkce_verifier: verifier.secret().clone(),
        nonce: nonce.secret().clone(),
    };
    state
        .repository
        .insert_oidc_login_request(&NewOidcLoginRequest {
            state_digest: &digest(csrf.secret()),
            binding_digest: &digest(&cookie.binding),
            pkce_verifier_digest: &digest(&cookie.pkce_verifier),
            nonce_digest: &digest(&cookie.nonce),
            workspace_id,
            issuer: &provider.issuer,
            intent,
            expires_at: Utc::now() + chrono::Duration::minutes(LOGIN_REQUEST_LIFETIME_MINUTES),
        })
        .await?;
    let mut response = Json(StartResponse {
        authorization_url: authorization_url.to_string(),
    })
    .into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        login_cookie(
            &cookie.encode(),
            state.session_cookie_secure,
            LOGIN_REQUEST_LIFETIME_SECONDS,
        ),
    );
    Ok(response)
}

/// `POST /auth/oidc/start`, public. Signs in, or accepts an invitation, through
/// the provider of the workspace an identifier names.
pub(super) async fn start(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<StartRequest>,
) -> Result<Response, ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    settings(&state)?;
    let identifier = input.login_identifier.trim().to_lowercase();
    let workspace = discover_workspace(&state, &identifier).await?;
    let provider = state
        .repository
        .identity_provider_for_sign_in(workspace.id)
        .await?
        .filter(|provider| provider.enabled)
        .ok_or_else(|| ApiError::not_found("single sign-on"))?;
    let intent = match input.invitation_secret {
        Some(secret) => OidcIntent::Invitation {
            invitation_digest: digest(&secret),
        },
        None => OidcIntent::SignIn,
    };
    begin(&state, workspace.id, &provider, intent).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InvitationOptionsRequest {
    secret: String,
}

#[derive(Serialize)]
pub(super) struct InvitationOptions {
    login_identifier: String,
    /// The workspace's provider, when an invitee can accept by signing in.
    display_name: Option<String>,
}

/// `POST /auth/oidc/invitation`, public. Tells someone holding an invitation's
/// secret, and not yet signed in, how they can accept it. The secret is what
/// entitles them to learn which workspace it is for.
pub(super) async fn invitation_options(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<InvitationOptionsRequest>,
) -> Result<Json<InvitationOptions>, ApiError> {
    let (workspace_id, login_identifier) = state
        .repository
        .invitation_login_identifier(&digest(&input.secret))
        .await?
        .ok_or(RepositoryError::InvitationInvalid)?;
    let display_name = state
        .repository
        .sign_in_options(workspace_id)
        .await?
        .provider
        .filter(|_| state.oidc.is_some() && !state.demo_mode);
    Ok(Json(InvitationOptions {
        login_identifier,
        display_name,
    }))
}

/// `POST /auth/oidc/link`: links the signed-in account to its identity at the
/// workspace's provider. It works before the provider is enabled, because this
/// is the test sign-in that proves it works.
pub(super) async fn link(
    State(state): State<AppState>,
    AuthenticatedSession(session): AuthenticatedSession,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Response, ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    settings(&state)?;
    let provider = state
        .repository
        .identity_provider_for_sign_in(workspace_id)
        .await?
        .ok_or_else(|| ApiError::not_found("single sign-on"))?;
    begin(
        &state,
        workspace_id,
        &provider,
        OidcIntent::Link {
            session_digest: session,
        },
    )
    .await
}

/// `GET /auth/oidc/link`: whether the caller can link, and already has.
pub(super) async fn link_status(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<LinkStatus>, ApiError> {
    settings(&state)?;
    let provider = state
        .repository
        .for_workspace(workspace_id)
        .identity_provider()
        .await?
        .ok_or_else(|| ApiError::not_found("single sign-on"))?;
    Ok(Json(LinkStatus {
        linked: state
            .repository
            .has_external_identity(workspace_id, user_id, &provider.issuer)
            .await?,
        display_name: provider.display_name,
    }))
}

/// `DELETE /workspace/members/{member_id}/identity`
pub(super) async fn unlink_member(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(member_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repository.unlink_member_identity(member_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("linked identity"))
    }
}

/// The identity a callback proved.
struct VerifiedIdentity {
    subject: SubjectIdentifier,
    email: Option<EndUserEmail>,
}

async fn verify_identity(
    state: &AppState,
    workspace_id: Uuid,
    provider: &IdentityProviderSignIn,
    code: String,
    cookie: &LoginCookie,
) -> Result<VerifiedIdentity, ApiError> {
    let settings = settings(state)?;
    let http = provider_client(settings, state.devtools_enabled);
    let metadata = discover_provider(provider, &http).await?;
    let client = provider_oidc_client(settings, workspace_id, provider, metadata)?;
    let token: CoreTokenResponse = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| ApiError::oidc_sign_in_failed())?
        .set_pkce_verifier(PkceCodeVerifier::new(cookie.pkce_verifier.clone()))
        .request_async(&http)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "identity provider token exchange failed");
            ApiError::oidc_sign_in_failed()
        })?;
    let id_token =
        openidconnect::TokenResponse::id_token(&token).ok_or_else(ApiError::oidc_sign_in_failed)?;
    let verifier = client.id_token_verifier().set_allowed_algs([
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        CoreJwsSigningAlgorithm::EcdsaP256Sha256,
    ]);
    let claims = id_token
        .claims(&verifier, &Nonce::new(cookie.nonce.clone()))
        .map_err(|error| {
            tracing::warn!(%error, "identity token was rejected");
            ApiError::oidc_sign_in_failed()
        })?;
    // A token naming the party it was issued to must name this client.
    if claims
        .authorized_party()
        .is_some_and(|party| party.as_str() != provider.client_id)
    {
        return Err(ApiError::oidc_sign_in_failed());
    }
    if claims.issue_time() > Utc::now() + chrono::Duration::seconds(ID_TOKEN_CLOCK_SKEW_SECONDS) {
        return Err(ApiError::oidc_sign_in_failed());
    }
    // A token that names its access token's hash must match it.
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            token.access_token(),
            id_token
                .signing_alg()
                .map_err(|_| ApiError::oidc_sign_in_failed())?,
            id_token
                .signing_key(&verifier)
                .map_err(|_| ApiError::oidc_sign_in_failed())?,
        )
        .map_err(|_| ApiError::oidc_sign_in_failed())?;
        if actual != *expected {
            return Err(ApiError::oidc_sign_in_failed());
        }
    }
    let subject = claims.subject().clone();
    // An address the provider says it has not verified proves nothing.
    let claimed_email = |email: Option<&EndUserEmail>, verified: Option<bool>| {
        email.filter(|_| verified != Some(false)).cloned()
    };
    let mut email = claimed_email(claims.email(), claims.email_verified());
    if email.is_none()
        && let Ok(request) = client.user_info(token.access_token().clone(), Some(subject.clone()))
    {
        let info: Result<openidconnect::core::CoreUserInfoClaims, _> =
            request.request_async(&http).await;
        if let Ok(info) = info {
            email = claimed_email(info.email(), info.email_verified());
        }
    }
    Ok(VerifiedIdentity { subject, email })
}

fn completion(
    settings: &OidcSettings,
    parameter: Option<(&str, &str)>,
) -> Result<String, ApiError> {
    let mut url = settings.callback_url.clone();
    url.set_path(COMPLETION_PATH);
    url.set_query(None);
    if let Some((name, value)) = parameter {
        url.query_pairs_mut().append_pair(name, value);
    }
    Ok(url.to_string())
}

fn redirect_response(location: &str) -> Result<Response, ApiError> {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, location)
        .header(header::CACHE_CONTROL, "no-store")
        .header("referrer-policy", "no-referrer")
        .body(Body::empty())
        .map_err(|_| ApiError::internal("single sign-on redirect"))
}

/// What a finished callback did.
enum CallbackOutcome {
    SignedIn(Response),
    Linked,
}

/// Spends the request a callback answers. Anyone can send a browser to the
/// callback, so until this succeeds nothing about the sign-in may change.
async fn take_request(
    state: &AppState,
    path_workspace: Uuid,
    query: &CallbackQuery,
    headers: &HeaderMap,
) -> Result<(OidcLoginRequest, LoginCookie), ApiError> {
    let raw_cookie = cookie_value(
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
        LOGIN_COOKIE,
    );
    let cookie = raw_cookie
        .as_deref()
        .and_then(LoginCookie::decode)
        .ok_or_else(ApiError::oidc_request_expired)?;
    let oidc_state = query
        .state
        .as_deref()
        .ok_or_else(ApiError::oidc_request_expired)?;
    let request = state
        .repository
        .take_oidc_login_request(
            path_workspace,
            &digest(oidc_state),
            &digest(&cookie.binding),
            &digest(&cookie.pkce_verifier),
            &digest(&cookie.nonce),
        )
        .await?
        .ok_or_else(ApiError::oidc_request_expired)?;
    Ok((request, cookie))
}

async fn complete(
    state: &AppState,
    request: OidcLoginRequest,
    cookie: LoginCookie,
    query: CallbackQuery,
    headers: &HeaderMap,
) -> Result<CallbackOutcome, ApiError> {
    let OidcLoginRequest {
        workspace_id,
        issuer,
        intent,
    } = request;
    // RFC 9207: a response naming an issuer must name the one we asked.
    if query.iss.as_deref().is_some_and(|iss| iss != issuer) {
        return Err(ApiError::oidc_sign_in_failed());
    }
    if let Some(error) = &query.error {
        tracing::info!(%error, "identity provider refused the sign-in");
        return Err(ApiError::oidc_sign_in_failed());
    }
    let code = query.code.ok_or_else(ApiError::oidc_sign_in_failed)?;
    // The configuration may have changed while the browser was away.
    let provider = state
        .repository
        .identity_provider_for_sign_in(workspace_id)
        .await?
        .filter(|provider| provider.issuer == issuer)
        .ok_or_else(ApiError::oidc_request_expired)?;
    // Only linking may use a provider that is not on yet: it is how one gets proven.
    if !provider.enabled && !matches!(intent, OidcIntent::Link { .. }) {
        return Err(ApiError::oidc_request_expired());
    }
    let identity = verify_identity(state, workspace_id, &provider, code, &cookie).await?;
    let subject = identity.subject.as_str();
    if let OidcIntent::Link { session_digest } = &intent {
        let current = session_cookie_digest(headers)
            .filter(|current| current == session_digest)
            .ok_or_else(ApiError::unauthenticated)?;
        let session = state
            .repository
            .validate_browser_session(&current)
            .await?
            .filter(|session| session.workspace_id == workspace_id)
            .ok_or_else(ApiError::unauthenticated)?;
        state
            .repository
            .link_identity_to_session_user(workspace_id, session.user_id, &issuer, subject)
            .await?;
        return Ok(CallbackOutcome::Linked);
    }
    let (session, csrf, expires_at) = issue_session();
    let new_session = NewOidcSession {
        id: Uuid::new_v4(),
        session: &session.digest(),
        csrf: &csrf.digest(),
        expires_at,
    };
    // Each path rechecks the provider and membership as it issues the session.
    match intent {
        OidcIntent::Invitation { invitation_digest } => {
            let email = identity
                .email
                .as_ref()
                .ok_or_else(ApiError::oidc_email_missing)?;
            state
                .repository
                .accept_invitation_with_identity(
                    workspace_id,
                    &invitation_digest,
                    &issuer,
                    subject,
                    email.as_str(),
                    new_session,
                )
                .await
                .map_err(|error| match error {
                    RepositoryError::NotFound(_) => ApiError::oidc_request_expired(),
                    error => error.into(),
                })?;
        }
        _ => {
            state
                .repository
                .issue_session_for_identity(workspace_id, &issuer, subject, new_session)
                .await?
                .ok_or_else(ApiError::oidc_identity_not_linked)?;
        }
    }
    let location = completion(settings(state)?, None)?;
    Ok(CallbackOutcome::SignedIn(session_redirect(
        &location,
        &session,
        &csrf,
        state.session_cookie_secure,
    )?))
}

/// `GET /auth/oidc/callback/{workspace_id}`, public. Always answers with a redirect to the web
/// app; a failure names only its error code.
pub(super) async fn callback(
    State(state): State<AppState>,
    ApiPath(workspace_id): ApiPath<Uuid>,
    query: Result<ApiQuery<CallbackQuery>, ApiError>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let settings = settings(&state)?.clone();
    // A query that cannot be read names no request, like one with no state.
    let query = query.map_or_else(|_| CallbackQuery::default(), |ApiQuery(query)| query);
    let taken = take_request(&state, workspace_id, &query, &headers).await;
    let spent = taken.is_ok();
    let outcome = match taken {
        Ok((request, cookie)) => complete(&state, request, cookie, query, &headers).await,
        Err(error) => Err(error),
    };
    let mut response = match outcome {
        Ok(CallbackOutcome::SignedIn(response)) => response,
        Ok(CallbackOutcome::Linked) => redirect_response(&completion(
            &settings,
            Some((COMPLETION_LINKED_PARAMETER, "true")),
        )?)?,
        Err(error) => redirect_response(&completion(
            &settings,
            Some((COMPLETION_ERROR_PARAMETER, error.code().as_str())),
        )?)?,
    };
    if spent {
        response.headers_mut().append(
            header::SET_COOKIE,
            login_cookie("", state.session_cookie_secure, 0),
        );
    }
    Ok(response)
}
