//! Host-side OAuth 2.0 for credential connections (attricat:host 1.1).
//!
//! The host runs every token request itself: client credentials, refresh
//! tokens (rotating or not), the device authorization grant and an operator's
//! authorization-code exchange. A refresh first takes a time-limited claim on
//! the connection, so while it lasts no other worker uses a rotating refresh
//! token too, and re-checks the cached token after claiming in case another
//! worker refreshed it first; others wait for the claim to settle. No row lock
//! is held across the token request, and tokens issued after the claim lapsed
//! are still stored, since the provider may already have rotated the refresh
//! token.
//! Token endpoints are reached through the connection kind's host-permission
//! rule with the same DNS pinning, TLS and no-redirect policy as every other
//! mediated request. Tokens never reach a component, a log or an error.

use attricat_domain::states::ConnectionStatus;
use reqwest::StatusCode;
use std::sync::OnceLock;

use attricat_extension_manifest::extensions::{ClientAuthentication, ConnectionKind};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, Utc};
use ring::rand::{SecureRandom, SystemRandom};

use super::*;
use crate::repository::{
    ConnectionCredentials, DeviceAuthorization, ExtensionConnection, IssuedTokens,
};

/// A cached access token is refreshed this long before it expires, or half
/// its lifetime before for a shorter-lived token.
const TOKEN_EXPIRY_SKEW: TimeDelta = TimeDelta::seconds(60);
/// The shortest lifetime a token response's `expires_in` is taken to state.
const MIN_TOKEN_LIFETIME_SECONDS: i64 = 30;
/// How soon a caller waiting for another worker's refresh first rechecks
/// it; the wait doubles up to [`REFRESH_WAIT_MAX_STEP`].
const REFRESH_WAIT_STEP: std::time::Duration = std::time::Duration::from_millis(200);
const REFRESH_WAIT_MAX_STEP: std::time::Duration = std::time::Duration::from_secs(1);
/// How long a caller waits for another worker's refresh before answering
/// that the connection is temporarily unavailable, so a slow token endpoint
/// never holds component invocations or HTTP requests for long.
const REFRESH_WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// The largest token endpoint response read.
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
/// How long an operator has to approve a browser authorization.
const BROWSER_AUTHORIZATION_LIFETIME: TimeDelta = TimeDelta::minutes(10);

/// Where the provider sends the browser back after an operator approves an
/// authorization-code connection, and where Attricat then sends it.
#[derive(Clone, Debug)]
pub struct OAuthCallback {
    /// The externally reachable URL of `GET /api/public/extension-oauth/callback`,
    /// registered with the provider as the app's redirect URI.
    pub redirect_uri: Url,
    /// The web app's extension management base, such as
    /// `https://attricat.example.com/manage/extensions/`. The callback sends
    /// the browser to `<base><extension-id>`.
    pub return_base: Url,
}

static OAUTH_CALLBACK: OnceLock<OAuthCallback> = OnceLock::new();

/// Installs the OAuth callback configuration. Without it, browser
/// authorization is unavailable and operators exchange codes by hand. Later
/// calls are ignored.
pub fn use_oauth_callback(callback: OAuthCallback) {
    let _ = OAUTH_CALLBACK.set(callback);
}

/// The configured OAuth callback, if any.
pub fn oauth_callback() -> Option<&'static OAuthCallback> {
    OAUTH_CALLBACK.get()
}

/// Why a token request failed.
#[derive(Debug)]
pub enum TokenError {
    /// The grant or client was rejected; an operator must reconnect.
    Rejected(String),
    /// The user has not approved a device authorization yet.
    Pending { slow_down: bool },
    /// A transport failure, throttling or server error; try again later.
    Temporary(String),
    /// The connection or its kind cannot be used as configured.
    Invalid(String),
    /// Another worker holds the connection's refresh claim; nothing was sent.
    Busy,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) | Self::Temporary(message) | Self::Invalid(message) => {
                formatter.write_str(message)
            }
            Self::Pending { .. } => formatter.write_str("authorization is still pending"),
            Self::Busy => formatter.write_str("the connection is being refreshed; try again"),
        }
    }
}

/// The manifest's declaration of a connection's kind.
pub fn connection_kind<'a>(
    installation: &'a ExtensionRuntimeInstallation,
    kind: &str,
) -> Result<&'a ConnectionKind, TokenError> {
    installation
        .manifest
        .server
        .as_ref()
        .and_then(|server| server.connections.iter().find(|item| item.id == kind))
        .ok_or_else(|| TokenError::Invalid("the connection kind is no longer declared".into()))
}

fn still_valid(
    token: Option<&String>,
    expires_at: Option<DateTime<Utc>>,
    issued_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<String> {
    let token = token?.clone();
    let Some(expires_at) = expires_at else {
        return Some(token);
    };
    // A token issued for a minute or less would otherwise always count as
    // expiring and be refreshed on every request.
    let skew = issued_at.map_or(TOKEN_EXPIRY_SKEW, |issued_at| {
        TOKEN_EXPIRY_SKEW.min((expires_at - issued_at) / 2)
    });
    (expires_at - skew > now).then_some(token)
}

/// Sets `connection`'s access token on a request admitted by host-permission
/// rule `rule_id`, in the header its kind names, and returns the token sent.
/// A kind's token goes only to the host permissions it lists.
pub(super) async fn insert_access_token(
    host: &HostState,
    connection: &str,
    rule_id: &str,
    headers: &mut reqwest::header::HeaderMap,
) -> Result<String, TokenError> {
    host.require("connections.use")
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    // A connector run sends only its own connection's token.
    if let Some(run_id) = host.run_id {
        host.run_facts(run_id)
            .await
            .map_err(TokenError::Invalid)?
            .ensure_token_connection(connection)
            .map_err(TokenError::Invalid)?;
    }
    // The rule is checked before the token is read, so a misrouted request
    // never causes a refresh (which spends a rotating refresh token).
    let described = host
        .repository
        .extension_connection(&host.installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    if !connection_kind(&host.installation, &described.kind)?
        .api_host_permissions
        .iter()
        .any(|id| id == rule_id)
    {
        return Err(TokenError::Invalid(
            "this connection's token may not be sent to that host permission".into(),
        ));
    }
    let (token, kind) = access_token(&host.repository, &host.installation, connection).await?;
    let (name, prefix) = kind.token_header.as_ref().map_or_else(
        || ("authorization".to_owned(), "Bearer ".to_owned()),
        |header| (header.name.to_ascii_lowercase(), header.prefix.clone()),
    );
    headers.insert(
        reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| TokenError::Invalid("invalid token header".into()))?,
        reqwest::header::HeaderValue::from_str(&(prefix + &token))
            .map_err(|_| TokenError::Invalid("token cannot be used as a header".into()))?,
    );
    Ok(token)
}

/// A valid access token of `connection`, refreshed when it is missing or
/// about to expire.
pub async fn access_token(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
) -> Result<(String, ConnectionKind), TokenError> {
    let cached = repository
        .extension_connection_access_token(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    let kind = connection_kind(installation, &cached.kind)?.clone();
    if cached.status != ConnectionStatus::Active {
        return Err(TokenError::Invalid(format!(
            "connection '{connection}' is {} and needs an operator",
            cached.status
        )));
    }
    if let Some(token) = still_valid(
        cached.access_token.as_ref(),
        cached.access_token_expires_at,
        cached.last_refreshed_at,
        Utc::now(),
    ) {
        return Ok((token, kind));
    }
    // One worker refreshes; the others wait for its token without holding a
    // database connection.
    let started = std::time::Instant::now();
    let mut step = REFRESH_WAIT_STEP;
    let refresh = loop {
        if let Some(refresh) = repository
            .claim_extension_connection_refresh(&installation.extension_id, connection)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?
        {
            break refresh;
        }
        if started.elapsed() >= REFRESH_WAIT_LIMIT {
            return Err(TokenError::Temporary(
                "another worker is still refreshing the token".into(),
            ));
        }
        tokio::time::sleep(step).await;
        step = (step * 2).min(REFRESH_WAIT_MAX_STEP);
        let cached = repository
            .extension_connection_access_token(&installation.extension_id, connection)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?;
        if let Some(token) = still_valid(
            cached.access_token.as_ref(),
            cached.access_token_expires_at,
            cached.last_refreshed_at,
            Utc::now(),
        ) {
            return Ok((token, kind));
        }
    };
    // Another worker may have failed the grant meanwhile; its refresh token
    // is not sent again.
    if refresh.credentials.status != ConnectionStatus::Active {
        let status = refresh.credentials.status;
        let _ = refresh.release().await;
        return Err(TokenError::Invalid(format!(
            "connection '{connection}' is {status} and needs an operator"
        )));
    }
    // Another worker may have refreshed just before this claim.
    if let Some(token) = still_valid(
        refresh.credentials.access_token.as_ref(),
        refresh.credentials.access_token_expires_at,
        refresh.credentials.last_refreshed_at,
        Utc::now(),
    ) {
        let _ = refresh.release().await;
        return Ok((token, kind));
    }
    let credentials = refresh.credentials.clone();
    let mut form: Vec<(&str, String)> = Vec::new();
    match (
        credentials.grant_type.as_str(),
        credentials.refresh_token.as_deref(),
    ) {
        ("client_credentials", _) => form.push(("grant_type", "client_credentials".into())),
        (_, Some(refresh_token)) => {
            form.push(("grant_type", "refresh_token".into()));
            form.push(("refresh_token", refresh_token.to_owned()));
        }
        _ => {
            // The provider issued no refresh token, so only a new
            // authorization helps; mark the connection failed so operators
            // see it.
            let message =
                format!("connection '{connection}' has no refresh token; authorize it again");
            let _ = refresh.fail(&message, true).await;
            counter!("attricat_extension_connection_refreshes_total", "outcome" => "rejected")
                .increment(1);
            return Err(TokenError::Invalid(message));
        }
    }
    if let Some(scope) = &credentials.scope
        && credentials.grant_type == "client_credentials"
    {
        form.push(("scope", scope.clone()));
    }
    let result = token_request(
        repository,
        installation,
        &kind,
        &credentials,
        &credentials.token_url,
        form,
    )
    .await;
    match result {
        Ok(tokens) => {
            let token = tokens.access_token.clone();
            refresh
                .complete(tokens)
                .await
                .map_err(|error| TokenError::Temporary(error.to_string()))?;
            counter!("attricat_extension_connection_refreshes_total", "outcome" => "refreshed")
                .increment(1);
            Ok((token, kind))
        }
        Err(error) => {
            let revoked = matches!(error, TokenError::Rejected(_));
            let _ = refresh.fail(&error.to_string(), revoked).await;
            counter!("attricat_extension_connection_refreshes_total", "outcome" => if revoked { "rejected" } else { "failed" })
                .increment(1);
            Err(error)
        }
    }
}

/// Starts the device authorization grant and stores the user code the
/// operator approves at the verification URI.
pub async fn start_device_authorization(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
) -> Result<ExtensionConnection, TokenError> {
    // A poll or refresh under way holds the claim; starting over meanwhile
    // would let it clear the new code when it completes.
    let refresh = repository
        .claim_extension_connection_refresh(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?
        .ok_or(TokenError::Busy)?;
    // Every outcome ends the claim before returning, so starting again
    // right away is not refused as busy.
    let result = async {
        let credentials = refresh.credentials.clone();
        if credentials.grant_type != "device_code" {
            return Err(TokenError::Invalid(
                "the connection does not use the device authorization grant".into(),
            ));
        }
        let url = credentials
            .device_authorization_url
            .clone()
            .ok_or_else(|| {
                TokenError::Invalid("the connection has no device authorization URL".into())
            })?;
        let kind = connection_kind(installation, &credentials.kind)?.clone();
        let mut form = Vec::new();
        if let Some(scope) = &credentials.scope {
            form.push(("scope", scope.clone()));
        }
        let body =
            token_endpoint_call(repository, installation, &kind, &credentials, &url, form).await?;
        let text = |name: &str| {
            body.get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| TokenError::Temporary(format!("device authorization lacks {name}")))
        };
        let expires_in = body
            .get("expires_in")
            .and_then(Value::as_i64)
            .unwrap_or(600)
            .clamp(1, 86_400);
        // The operator is shown the code and follows the link, so both must be
        // short and the link plain http(s).
        let link = |name: &str| {
            text(name).ok().filter(|uri| {
                uri.len() <= 2048
                    && url::Url::parse(uri)
                        .is_ok_and(|url| matches!(url.scheme(), "https" | "http"))
            })
        };
        let invalid =
            || TokenError::Rejected("the provider's device authorization is invalid".into());
        let user_code = text("user_code")?;
        if user_code.is_empty() || user_code.len() > 128 {
            return Err(invalid());
        }
        let authorization = DeviceAuthorization {
            device_code: text("device_code")?,
            user_code,
            verification_uri: link("verification_uri_complete")
                .or_else(|| link("verification_uri"))
                .ok_or_else(invalid)?,
            expires_at: Utc::now() + TimeDelta::seconds(expires_in),
            interval_seconds: body
                .get("interval")
                .and_then(Value::as_i64)
                .unwrap_or(5)
                .clamp(1, 3600) as i32,
        };
        repository
            .record_extension_device_authorization(
                &installation.extension_id,
                connection,
                &authorization,
            )
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))
    }
    .await;
    let _ = refresh.release().await;
    result
}

/// Polls a pending device authorization once. Returns the connection, now
/// `active` when the operator approved it.
pub async fn poll_device_authorization(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
) -> Result<ExtensionConnection, TokenError> {
    // A poll while another is under way is answered as still pending.
    let Some(refresh) = repository
        .claim_extension_connection_refresh(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?
    else {
        return repository
            .extension_connection(&installation.extension_id, connection)
            .await
            .map_err(|error| TokenError::Invalid(error.to_string()));
    };
    let credentials = refresh.credentials.clone();
    let Some(device_code) = credentials.device_code.clone() else {
        let _ = refresh.release().await;
        return repository
            .extension_connection(&installation.extension_id, connection)
            .await
            .map_err(|error| TokenError::Invalid(error.to_string()));
    };
    if credentials
        .device_expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now())
    {
        let _ = refresh
            .abandon_device_authorization("the device authorization expired; start it again")
            .await;
        return Err(TokenError::Rejected(
            "the device authorization expired; start it again".into(),
        ));
    }
    // A poll sooner than the provider's interval is answered as still
    // pending without asking the provider, which would answer `slow_down`.
    if let (Some(polled_at), Some(interval)) = (
        credentials.device_polled_at,
        credentials.device_interval_seconds,
    ) && Utc::now() < polled_at + chrono::TimeDelta::seconds(i64::from(interval))
    {
        let _ = refresh.release().await;
        return repository
            .extension_connection(&installation.extension_id, connection)
            .await
            .map_err(|error| TokenError::Invalid(error.to_string()));
    }
    let kind = match connection_kind(installation, &credentials.kind) {
        Ok(kind) => kind.clone(),
        Err(error) => {
            let _ = refresh.release().await;
            return Err(error);
        }
    };
    let form = vec![
        ("grant_type", DEVICE_CODE_GRANT.to_owned()),
        ("device_code", device_code),
    ];
    match token_request(
        repository,
        installation,
        &kind,
        &credentials,
        &credentials.token_url,
        form,
    )
    .await
    {
        Ok(tokens) => refresh
            .complete_device_authorization(tokens)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?,
        Err(TokenError::Pending { slow_down }) => refresh
            .pending(slow_down)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?,
        // A refused device grant ends the authorization; it does not revoke
        // tokens an active connection already holds.
        Err(error @ TokenError::Rejected(_)) => {
            let _ = refresh
                .abandon_device_authorization(&error.to_string())
                .await;
            return Err(error);
        }
        Err(error) => {
            let _ = refresh.fail(&error.to_string(), false).await;
            return Err(error);
        }
    }
    repository
        .extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))
}

/// Exchanges an authorization code an operator copied from the provider's
/// redirect, for apps whose redirect URI is not an Attricat route.
pub async fn exchange_authorization_code(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: Option<&str>,
) -> Result<ExtensionConnection, TokenError> {
    let refresh = repository
        .claim_extension_connection_refresh(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?
        .ok_or(TokenError::Busy)?;
    let credentials = refresh.credentials.clone();
    // Every early return ends the claim at once, so a retry is not refused
    // as busy.
    if credentials.grant_type != "authorization_code" {
        let _ = refresh.release().await;
        return Err(TokenError::Invalid(
            "the connection does not use the authorization code grant".into(),
        ));
    }
    let kind = match connection_kind(installation, &credentials.kind) {
        Ok(kind) => kind.clone(),
        Err(error) => {
            let _ = refresh.release().await;
            return Err(error);
        }
    };
    let mut form = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
    ];
    if let Some(verifier) = code_verifier {
        form.push(("code_verifier", verifier.to_owned()));
    }
    match token_request(
        repository,
        installation,
        &kind,
        &credentials,
        &credentials.token_url,
        form,
    )
    .await
    {
        Ok(tokens) => refresh
            .complete(tokens)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?,
        Err(error) => {
            let _ = refresh.fail(&error.to_string(), false).await;
            return Err(error);
        }
    }
    repository
        .extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))
}

/// Posts a token request and parses the issued tokens.
async fn token_request(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    kind: &ConnectionKind,
    credentials: &ConnectionCredentials,
    url: &str,
    form: Vec<(&str, String)>,
) -> Result<IssuedTokens, TokenError> {
    let body = token_endpoint_call(repository, installation, kind, credentials, url, form).await?;
    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| TokenError::Temporary("token response lacks an access token".into()))?
        .to_owned();
    // `expires_in: 0` or less states no usable lifetime and is treated like
    // a response without `expires_in`. A very short lifetime is treated
    // as `MIN_TOKEN_LIFETIME_SECONDS`, so it cannot make every request
    // refresh the token.
    let expires_at = body
        .get("expires_in")
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .filter(|seconds| *seconds > 0)
        .map(|seconds| {
            Utc::now() + TimeDelta::seconds(seconds.clamp(MIN_TOKEN_LIFETIME_SECONDS, 365 * 86_400))
        });
    Ok(IssuedTokens {
        access_token,
        expires_at,
        refresh_token: body
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_owned),
    })
}

/// Posts a form to a token or device endpoint with client authentication and
/// returns its JSON object. Error responses are classified by their OAuth
/// error code; their bodies are never echoed.
async fn token_endpoint_call(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    kind: &ConnectionKind,
    credentials: &ConnectionCredentials,
    url: &str,
    mut form: Vec<(&str, String)>,
) -> Result<Value, TokenError> {
    let url = Url::parse(url).map_err(|_| TokenError::Invalid("invalid token URL".into()))?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err(TokenError::Invalid("token endpoints must be HTTPS".into()));
    }
    let rule = installation
        .manifest
        .host_permission(&kind.host_permission)
        .filter(|rule| installation.host_permission_grants.contains(&rule.id))
        .filter(|rule| rule.allows_request_with_query(&url, "POST"))
        .ok_or_else(|| {
            TokenError::Invalid("the token endpoint is not granted to the connection kind".into())
        })?;
    // Token requests count against the rule's budget like any request to
    // its host, so a lowered budget caps them too.
    if let Err(wait) = super::network_v2::admit_request(repository, installation, rule).await {
        return Err(TokenError::Temporary(format!(
            "the token endpoint's request budget is spent; try again in {} s",
            wait.as_secs().max(1)
        )));
    }
    let host = url
        .host_str()
        .ok_or_else(|| TokenError::Invalid("token URL needs a host".into()))?
        .to_owned();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| TokenError::Invalid("token URL needs a port".into()))?;
    let timeout = Duration::from_millis(rule.timeout_ms);
    let addresses = resolve_public_destination(&host, port, timeout)
        .await
        .map_err(TokenError::Temporary)?;
    let client = pinned_client(&host, &addresses, timeout).map_err(TokenError::Temporary)?;
    let mut request = client
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json");
    match kind.client_authentication {
        ClientAuthentication::Basic => {
            request =
                request.basic_auth(&credentials.client_id, credentials.client_secret.as_deref());
        }
        ClientAuthentication::Body => {
            form.push(("client_id", credentials.client_id.clone()));
            if let Some(secret) = &credentials.client_secret {
                form.push(("client_secret", secret.clone()));
            }
        }
    }
    let response = request
        .form(&form)
        .send()
        .await
        .map_err(|_| TokenError::Temporary("token endpoint is unreachable".into()))?;
    let status = response.status();
    if status.is_redirection() {
        return Err(TokenError::Temporary("token endpoint redirected".into()));
    }
    // Read at most the limit, so an oversized response is never buffered.
    let too_large = || TokenError::Temporary("token response is too large".into());
    if response
        .content_length()
        .is_some_and(|length| length > MAX_TOKEN_RESPONSE_BYTES as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| TokenError::Temporary("token response failed".into()))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_TOKEN_RESPONSE_BYTES {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if status.is_success() && body.is_object() {
        return Ok(body);
    }
    let code = body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Err(match code {
        "authorization_pending" => TokenError::Pending { slow_down: false },
        "slow_down" => TokenError::Pending { slow_down: true },
        "invalid_grant"
        | "invalid_client"
        | "unauthorized_client"
        | "access_denied"
        | "expired_token" => {
            TokenError::Rejected(format!("the token endpoint rejected the grant ({code})"))
        }
        _ if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() => {
            TokenError::Temporary(format!("token endpoint returned HTTP {}", status.as_u16()))
        }
        _ if status == StatusCode::UNAUTHORIZED => {
            TokenError::Rejected("the token endpoint rejected the client".into())
        }
        _ => TokenError::Temporary(format!("token endpoint returned HTTP {}", status.as_u16())),
    })
}

/// A 256-bit random value, base64url-encoded without padding (43 characters),
/// as `state` and as an RFC 7636 code verifier.
fn random_token() -> Result<String, TokenError> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| TokenError::Temporary("random source unavailable".into()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// The SHA-256 under which a browser authorization's `state` is stored.
pub fn authorization_state_digest(state: &str) -> String {
    format!("{:x}", Sha256::digest(state.as_bytes()))
}

/// What a browser authorization stores of the session that started it:
/// a SHA-256 over the session digest, so the stored value is not itself a
/// session lookup key.
pub fn authorization_session_binding(session_digest: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"attricat.extension-oauth-session:");
    hasher.update(session_digest);
    hasher.finalize().into()
}

/// Starts a browser authorization of an authorization-code connection and
/// returns the provider URL to send the operator to. The request carries a
/// single-use `state` and an S256 PKCE challenge, and is bound to the
/// operator's browser session (`session_binding`, from
/// [`authorization_session_binding`]); the public callback completes it
/// within ten minutes, from that session only.
pub async fn start_browser_authorization(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
    session_binding: &[u8; 32],
) -> Result<Url, TokenError> {
    let callback = oauth_callback().ok_or_else(|| {
        TokenError::Invalid("browser authorization is not configured on this deployment".into())
    })?;
    let current = repository
        .extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    if current.grant_type != "authorization_code" {
        return Err(TokenError::Invalid(
            "the connection does not use the authorization code grant".into(),
        ));
    }
    connection_kind(installation, &current.kind)?;
    let mut url = current
        .authorization_url
        .as_deref()
        .and_then(|url| Url::parse(url).ok())
        .ok_or_else(|| TokenError::Invalid("the connection has no authorization URL".into()))?;
    let state = random_token()?;
    let verifier = random_token()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", &current.client_id)
            .append_pair("redirect_uri", callback.redirect_uri.as_str())
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        if let Some(scope) = &current.scope {
            query.append_pair("scope", scope);
        }
    }
    repository
        .record_extension_authorization_request(
            &installation.extension_id,
            connection,
            &authorization_state_digest(&state),
            &verifier,
            session_binding,
            Utc::now() + BROWSER_AUTHORIZATION_LIFETIME,
        )
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    Ok(url)
}

/// Completes a browser authorization from the provider's redirect: takes the
/// pending request `state` names (at most once, and only for the session it
/// is bound to) and exchanges `code` with its PKCE verifier and the
/// configured redirect URI. Returns `None`, changing nothing, when no such
/// request is pending for this session.
pub async fn complete_browser_authorization(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
    state: &str,
    session_binding: &[u8; 32],
    code: &str,
) -> Result<Option<ExtensionConnection>, TokenError> {
    let callback = oauth_callback().ok_or_else(|| {
        TokenError::Invalid("browser authorization is not configured on this deployment".into())
    })?;
    let Some(verifier) = repository
        .take_extension_authorization_request(
            &installation.extension_id,
            connection,
            &authorization_state_digest(state),
            session_binding,
        )
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?
    else {
        return Ok(None);
    };
    exchange_authorization_code(
        repository,
        installation,
        connection,
        code,
        callback.redirect_uri.as_str(),
        Some(&verifier),
    )
    .await
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_tokens_are_refreshed_before_they_expire() {
        let token = Some("token".to_owned());
        let now = Utc::now();
        assert!(still_valid(token.as_ref(), None, None, now).is_some());
        let in_ten_minutes = Some(now + TimeDelta::minutes(10));
        assert!(still_valid(token.as_ref(), in_ten_minutes, None, now).is_some());
        let in_thirty_seconds = Some(now + TimeDelta::seconds(30));
        assert!(still_valid(token.as_ref(), in_thirty_seconds, None, now).is_none());
        assert!(still_valid(None, None, None, now).is_none());
        // A token issued for 60 seconds is reused for its first 30.
        let issued = Some(now - TimeDelta::seconds(10));
        let short = Some(now + TimeDelta::seconds(50));
        assert!(still_valid(token.as_ref(), short, issued, now).is_some());
        let late = Some(now - TimeDelta::seconds(40));
        let ending = Some(now + TimeDelta::seconds(20));
        assert!(still_valid(token.as_ref(), ending, late, now).is_none());
    }
}
