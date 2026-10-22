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

/// A cached access token is refreshed this long before it expires.
const TOKEN_EXPIRY_SKEW: TimeDelta = TimeDelta::seconds(60);
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

fn still_valid(credentials: &ConnectionCredentials, now: DateTime<Utc>) -> Option<String> {
    let token = credentials.access_token.clone()?;
    match credentials.access_token_expires_at {
        Some(expires_at) if expires_at - TOKEN_EXPIRY_SKEW <= now => None,
        _ => Some(token),
    }
}

/// A valid access token of `connection`, refreshed when it is missing or
/// about to expire.
pub async fn access_token(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    connection: &str,
) -> Result<(String, ConnectionKind), TokenError> {
    let credentials = repository
        .extension_connection_credentials(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    let kind = connection_kind(installation, &credentials.kind)?.clone();
    if credentials.status != "active" {
        return Err(TokenError::Invalid(format!(
            "connection '{connection}' is {} and needs an operator",
            credentials.status
        )));
    }
    if let Some(token) = still_valid(&credentials, Utc::now()) {
        return Ok((token, kind));
    }
    let refresh = repository
        .lock_extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Temporary(error.to_string()))?;
    // Another worker may have refreshed while this one waited for the lock.
    if let Some(token) = still_valid(&refresh.credentials, Utc::now()) {
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
            return Err(TokenError::Invalid(format!(
                "connection '{connection}' has no refresh token; authorize it again"
            )));
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
    let refresh = repository
        .lock_extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
    let credentials = refresh.credentials.clone();
    let Some(device_code) = credentials.device_code.clone() else {
        drop(refresh);
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
        Err(TokenError::Pending { .. }) => drop(refresh),
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
        .lock_extension_connection(&installation.extension_id, connection)
        .await
        .map_err(|error| TokenError::Invalid(error.to_string()))?;
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
        let credentials = |expires_at| ConnectionCredentials {
            id: Uuid::nil(),
            connection: "shop".into(),
            kind: "shopify".into(),
            grant_type: "client_credentials".into(),
            host_permission_id: "token".into(),
            token_url: "https://example.com/token".into(),
            device_authorization_url: None,
            client_id: "client".into(),
            scope: None,
            status: "active".into(),
            client_secret: None,
            refresh_token: None,
            access_token: Some("token".into()),
            access_token_expires_at: expires_at,
            device_code: None,
            device_expires_at: None,
            device_interval_seconds: None,
        };
        let now = Utc::now();
        assert!(still_valid(&credentials(None), now).is_some());
        assert!(still_valid(&credentials(Some(now + TimeDelta::minutes(10))), now).is_some());
        assert!(still_valid(&credentials(Some(now + TimeDelta::seconds(30))), now).is_none());
    }
}
