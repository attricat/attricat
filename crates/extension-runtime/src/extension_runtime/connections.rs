//! Host-side OAuth 2.0 for credential connections (attricat:host 1.1).
//!
//! The host runs every token request itself: client credentials, refresh
//! tokens (rotating or not), the device authorization grant and an operator's
//! authorization-code exchange. A refresh holds the connection's row lock, so
//! a rotating refresh token is never used twice, and re-checks the cached
//! token after taking the lock in case another worker refreshed it first.
//! Token endpoints are reached through the connection kind's host-permission
//! rule with the same DNS pinning, TLS and no-redirect policy as every other
//! mediated request. Tokens never reach a component, a log or an error.

use attricat_extension_manifest::extensions::{ClientAuthentication, ConnectionKind};
use chrono::{DateTime, TimeDelta, Utc};

use super::*;
use crate::repository::{
    ConnectionCredentials, DeviceAuthorization, ExtensionConnection, IssuedTokens,
};

/// A cached access token is refreshed this long before it expires, or half
/// its lifetime before for a shorter-lived token.
const TOKEN_EXPIRY_SKEW: TimeDelta = TimeDelta::seconds(60);
/// How often a caller waiting for another worker's refresh rechecks it.
const REFRESH_WAIT_STEP: std::time::Duration = std::time::Duration::from_millis(200);
/// How long a caller waits for another worker's refresh.
const REFRESH_WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// The largest token endpoint response read.
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;

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
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) | Self::Temporary(message) | Self::Invalid(message) => {
                formatter.write_str(message)
            }
            Self::Pending { .. } => formatter.write_str("authorization is still pending"),
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
    let (token, kind) = access_token(&host.repository, &host.installation, connection).await?;
    if !kind.api_host_permissions.iter().any(|id| id == rule_id) {
        return Err(TokenError::Invalid(
            "this connection's token may not be sent to that host permission".into(),
        ));
    }
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
    if cached.status != "active" {
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
        tokio::time::sleep(REFRESH_WAIT_STEP).await;
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
    if refresh.credentials.status != "active" {
        let status = refresh.credentials.status.clone();
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
    let credentials = repository
        .extension_connection_credentials(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
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
    let body = token_endpoint_call(installation, &kind, &credentials, &url, form).await?;
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
    let authorization = DeviceAuthorization {
        device_code: text("device_code")?,
        user_code: text("user_code")?,
        verification_uri: text("verification_uri_complete")
            .or_else(|_| text("verification_uri"))?,
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
            .fail("the device authorization expired; start it again", true)
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
    let kind = connection_kind(installation, &credentials.kind)?.clone();
    let form = vec![
        ("grant_type", DEVICE_CODE_GRANT.to_owned()),
        ("device_code", device_code),
    ];
    match token_request(
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
        Err(TokenError::Pending { slow_down }) => refresh
            .pending(slow_down)
            .await
            .map_err(|error| TokenError::Temporary(error.to_string()))?,
        Err(error) => {
            let _ = refresh
                .fail(&error.to_string(), matches!(error, TokenError::Rejected(_)))
                .await;
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
        .ok_or_else(|| {
            TokenError::Temporary("the connection is being refreshed; try again".into())
        })?;
    let credentials = refresh.credentials.clone();
    if credentials.grant_type != "authorization_code" {
        return Err(TokenError::Invalid(
            "the connection does not use the authorization code grant".into(),
        ));
    }
    let kind = connection_kind(installation, &credentials.kind)?.clone();
    let mut form = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
    ];
    if let Some(verifier) = code_verifier {
        form.push(("code_verifier", verifier.to_owned()));
    }
    match token_request(
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
    installation: &ExtensionRuntimeInstallation,
    kind: &ConnectionKind,
    credentials: &ConnectionCredentials,
    url: &str,
    form: Vec<(&str, String)>,
) -> Result<IssuedTokens, TokenError> {
    let body = token_endpoint_call(installation, kind, credentials, url, form).await?;
    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| TokenError::Temporary("token response lacks an access token".into()))?
        .to_owned();
    let expires_at = body
        .get("expires_in")
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .map(|seconds| Utc::now() + TimeDelta::seconds(seconds.clamp(0, 365 * 86_400)));
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
        _ if status.as_u16() == 429 || status.is_server_error() => {
            TokenError::Temporary(format!("token endpoint returned HTTP {}", status.as_u16()))
        }
        _ if status.as_u16() == 401 => {
            TokenError::Rejected("the token endpoint rejected the client".into())
        }
        _ => TokenError::Temporary(format!("token endpoint returned HTTP {}", status.as_u16())),
    })
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
