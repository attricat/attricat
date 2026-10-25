//! `network.request.v2` (attricat:host 1.1) and per-rule request budgets.
//!
//! Compared with v1 it allows query strings on rules that opt in
//! (`allow_query`), binary bodies (`body_base64`), workspace secrets
//! substituted into form or JSON bodies (`secret_body`), a host-managed
//! connection's access token (`connection`) and a rule's extra response
//! headers. It reports throttling and uncertain outcomes as values instead of
//! errors, so a component can defer or reconcile, and returns redirect
//! responses without following them. URLs, query strings,
//! bodies and secrets never reach logs, traces, metrics or errors.

use attricat_extension_manifest::extensions::{HostPermission, RateLimit};

use super::*;
use crate::extension_runtime::connections::{self, TokenError};

const MAX_SECRET_BODY_FIELDS: usize = 8;
const MAX_SECRET_FIELD_BYTES: usize = 256;
/// How long an operator-lowered budget is cached before it is read again,
/// so mediated requests do not each query the database.
const LOWERED_BUDGET_TTL: Duration = Duration::from_secs(10);
const MAX_CACHED_BUDGETS: usize = 4_096;

/// Workspace, extension and host permission.
type BudgetKey = (Uuid, String, String);
/// When each lowered budget was read, and the budget (`None` when not lowered).
type BudgetCache = HashMap<BudgetKey, (Instant, Option<(i32, i32)>)>;
static LOWERED_BUDGETS: std::sync::LazyLock<std::sync::Mutex<BudgetCache>> =
    std::sync::LazyLock::new(Default::default);
/// Bumped by every invalidation, so a read that began before one does not
/// cache the value it read.
static BUDGET_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Response headers every request returns.
const DEFAULT_RESPONSE_HEADERS: [&str; 3] = ["content-type", "etag", "retry-after"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkRequestV2 {
    host_permission_id: String,
    method: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    secret_headers: Vec<SecretHeader>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    body_base64: Option<String>,
    #[serde(default)]
    secret_body: Option<SecretBody>,
    #[serde(default)]
    connection: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BodyFormat {
    Form,
    Json,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretBody {
    format: BodyFormat,
    fields: Vec<SecretField>,
}

/// One body field the host fills with a workspace secret. In a form body
/// `field` is a field name; in a JSON body it is a top-level key or a JSON
/// pointer such as `/credentials/secret`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretField {
    field: String,
    secret: String,
}

/// The rule's effective budget: its declared one, lowered by an operator.
fn effective_budget(declared: RateLimit, lowered: Option<(i32, i32)>) -> RateLimit {
    let Some((requests, per_seconds)) = lowered else {
        return declared;
    };
    let (requests, per_seconds) = (requests.max(1) as u32, per_seconds.max(1) as u32);
    // Compare rates without division: requests/per_seconds against the
    // declared rate. An override can only lower the budget.
    if u64::from(requests) * u64::from(declared.per_seconds)
        <= u64::from(declared.requests) * u64::from(per_seconds)
    {
        RateLimit {
            requests,
            per_seconds,
            burst: Some(declared.burst().min(requests)),
        }
    } else {
        declared
    }
}

/// Admits one request of `rule` against its budget. Rules without a declared
/// budget share the release's default budget of 60 requests per minute.
pub(super) async fn admit_request(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    rule: &HostPermission,
) -> Result<(), Duration> {
    let limiter = NETWORK_RATE_LIMITER.get_or_init(|| Arc::new(LocalRateLimiter::default()));
    let Some(declared) = rule.rate_limit else {
        return if allow_network_request(
            installation.installed_release_id,
            &installation.extension_id,
        )
        .await
        {
            Ok(())
        } else {
            // The shared default is a sliding window, which cannot tell when
            // its oldest request leaves; a whole window is when a slot is
            // certainly free.
            Err(super::NETWORK_RATE_WINDOW)
        };
    };
    let lowered = lowered_budget(repository, installation, rule).await;
    let budget = effective_budget(declared, lowered);
    limiter
        .take(
            &format!(
                "network:{}:{}:{}",
                repository.workspace_id(),
                installation.extension_id,
                rule.id
            ),
            budget.burst(),
            budget.requests,
            Duration::from_secs(u64::from(budget.per_seconds)),
        )
        .await
}

/// Drops this process's cached budget of a rule, so an operator's change
/// applies at once here; other replicas see it within
/// [`LOWERED_BUDGET_TTL`].
pub fn forget_lowered_budget(workspace_id: Uuid, extension_id: &str, host_permission_id: &str) {
    BUDGET_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    LOWERED_BUDGETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&(
            workspace_id,
            extension_id.to_owned(),
            host_permission_id.to_owned(),
        ));
}

/// The operator-lowered budget of `rule`, cached for [`LOWERED_BUDGET_TTL`].
async fn lowered_budget(
    repository: &AttricatRepository,
    installation: &ExtensionRuntimeInstallation,
    rule: &HostPermission,
) -> Option<(i32, i32)> {
    let key = (
        repository.workspace_id(),
        installation.extension_id.clone(),
        rule.id.clone(),
    );
    let cached = LOWERED_BUDGETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&key)
        .filter(|(read_at, _)| read_at.elapsed() < LOWERED_BUDGET_TTL)
        .map(|(_, budget)| *budget);
    if let Some(budget) = cached {
        return budget;
    }
    let generation = BUDGET_GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    // A failed read applies the declared budget without caching it.
    let budget = repository
        .extension_host_permission_budget(&installation.extension_id, &rule.id)
        .await
        .ok()?;
    let mut budgets = LOWERED_BUDGETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // An invalidation since the read began may postdate what it read.
    if BUDGET_GENERATION.load(std::sync::atomic::Ordering::SeqCst) != generation {
        return budget;
    }
    if budgets.len() >= MAX_CACHED_BUDGETS {
        budgets.retain(|_, (read_at, _)| read_at.elapsed() < LOWERED_BUDGET_TTL);
        if budgets.len() >= MAX_CACHED_BUDGETS {
            budgets.clear();
        }
    }
    budgets.insert(key, (Instant::now(), budget));
    budget
}

/// The granted rule `permission_id` when it matches `url` and `method`; a
/// query string needs a rule with `allow_query`.
pub(super) fn granted_rule<'a>(
    installation: &'a ExtensionRuntimeInstallation,
    permission_id: &str,
    url: &Url,
    method: &str,
) -> Result<&'a HostPermission, String> {
    installation
        .manifest
        .host_permission(permission_id)
        .filter(|rule| installation.host_permission_grants.contains(&rule.id))
        .filter(|rule| rule.allows_request_with_query(url, method))
        .ok_or_else(|| "destination is not granted by a host permission".to_owned())
}

/// Parses an HTTPS URL without credentials or a fragment. Query strings are
/// checked against the rule.
pub(super) fn https_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "invalid HTTPS URL")?;
    if url.scheme() != "https"
        || url.fragment().is_some()
        || url.username() != ""
        || url.password().is_some()
    {
        return Err("only HTTPS destinations without credentials or fragments are allowed".into());
    }
    Ok(url)
}

/// Applies `secret_body` to a UTF-8 body.
fn substitute_secret_body(
    body: Option<String>,
    secret_body: &SecretBody,
    secrets: &HashMap<String, String>,
) -> Result<String, String> {
    match secret_body.format {
        BodyFormat::Form => {
            let names: Vec<&str> = secret_body
                .fields
                .iter()
                .map(|field| field.field.as_str())
                .collect();
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (name, value) in url::form_urlencoded::parse(body.unwrap_or_default().as_bytes()) {
                if !names.contains(&name.as_ref()) {
                    serializer.append_pair(&name, &value);
                }
            }
            for field in &secret_body.fields {
                serializer.append_pair(&field.field, &secrets[&field.secret]);
            }
            Ok(serializer.finish())
        }
        BodyFormat::Json => {
            let mut value: Value = match body {
                Some(body) => serde_json::from_str(&body).map_err(|_| "invalid JSON body")?,
                None => json!({}),
            };
            if !value.is_object() {
                return Err("secret JSON fields need an object body".into());
            }
            for field in &secret_body.fields {
                let secret = Value::String(secrets[&field.secret].clone());
                if let Some(pointer) = field.field.strip_prefix('/') {
                    let (parent, key) = match pointer.rsplit_once('/') {
                        Some((parent, key)) => (format!("/{parent}"), key),
                        None => (String::new(), pointer),
                    };
                    let key = key.replace("~1", "/").replace("~0", "~");
                    let target = if parent.is_empty() {
                        Some(&mut value)
                    } else {
                        value.pointer_mut(&parent)
                    };
                    target
                        .and_then(Value::as_object_mut)
                        .ok_or("secret JSON pointer has no object parent")?
                        .insert(key, secret);
                } else {
                    value
                        .as_object_mut()
                        .expect("checked above")
                        .insert(field.field.clone(), secret);
                }
            }
            Ok(value.to_string())
        }
    }
}

impl HostState {
    /// Executes one brokered HTTPS request (v2). See the module docs.
    pub(super) async fn network_request_v2(&self, request: &str) -> Result<String, String> {
        let input: NetworkRequestV2 = parse_host_request(request, "network request")?;
        let method =
            Method::from_bytes(input.method.as_bytes()).map_err(|_| "invalid HTTP method")?;
        let url = https_url(&input.url)?;
        let rule = granted_rule(
            &self.installation,
            &input.host_permission_id,
            &url,
            method.as_str(),
        )?;
        if input.body.is_some() && input.body_base64.is_some() {
            return Err("send either body or body_base64".into());
        }
        if input.secret_body.is_some() && input.body_base64.is_some() {
            return Err("secret body fields need a UTF-8 body".into());
        }
        if input.secret_headers.len() > MAX_TRANSFER_SECRET_HEADERS
            || input.secret_body.as_ref().is_some_and(|body| {
                body.fields.is_empty() || body.fields.len() > MAX_SECRET_BODY_FIELDS
            })
        {
            return Err("too many secret fields".into());
        }
        // Resolve every secret first: an unavailable secret fails before any
        // budget is spent or byte is sent.
        let mut secrets = HashMap::new();
        let mut names: Vec<String> = input
            .secret_headers
            .iter()
            .map(|item| item.secret.clone())
            .collect();
        if let Some(body) = &input.secret_body {
            names.extend(body.fields.iter().map(|field| field.secret.clone()));
        }
        for name in &names {
            valid_secret_name(name)?;
            if !secrets.contains_key(name) {
                let value = self
                    .repository
                    .workspace_extension_secret_value(name)
                    .await
                    .map_err(|_| "secret lookup failed")?
                    .ok_or_else(|| "secret reference is unavailable".to_owned())?;
                secrets.insert(name.clone(), value);
            }
        }
        let body: Vec<u8> = match (&input.secret_body, input.body_base64) {
            (Some(secret_body), _) => {
                for field in &secret_body.fields {
                    if field.field.is_empty() || field.field.len() > MAX_SECRET_FIELD_BYTES {
                        return Err("invalid secret body field".into());
                    }
                }
                substitute_secret_body(input.body, secret_body, &secrets)?.into_bytes()
            }
            (None, Some(encoded)) => BASE64
                .decode(encoded.as_bytes())
                .map_err(|_| "invalid base64 body")?,
            (None, None) => input.body.unwrap_or_default().into_bytes(),
        };
        if body.len() as u64 > rule.max_request_bytes {
            return Err("request body exceeds host permission limit".into());
        }
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in input.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "invalid request header")?;
            if RESERVED_REQUEST_HEADERS.contains(&header_name.as_str())
                || value.len() > MAX_REQUEST_HEADER_VALUE_BYTES
            {
                return Err("unsafe request header".into());
            }
            headers.insert(
                header_name,
                reqwest::header::HeaderValue::from_str(&value)
                    .map_err(|_| "invalid request header")?,
            );
        }
        if let Some(key) = &input.idempotency_key {
            if key.is_empty()
                || key.len() > MAX_IDEMPOTENCY_KEY_BYTES
                || headers.contains_key("idempotency-key")
            {
                return Err("invalid idempotency key".into());
            }
            headers.insert(
                "idempotency-key",
                reqwest::header::HeaderValue::from_str(key)
                    .map_err(|_| "invalid idempotency key")?,
            );
        }
        for item in &input.secret_headers {
            let name = reqwest::header::HeaderName::from_bytes(item.header.as_bytes())
                .map_err(|_| "invalid secret header")?;
            if RESERVED_REQUEST_HEADERS.contains(&name.as_str())
                || item.prefix.len() > MAX_SECRET_HEADER_PREFIX_BYTES
            {
                return Err("unsafe secret header".into());
            }
            headers.insert(
                name,
                reqwest::header::HeaderValue::from_str(
                    &(item.prefix.clone() + &secrets[&item.secret]),
                )
                .map_err(|_| "secret cannot be used as a header")?,
            );
        }
        // The budget is admitted before the connection's token is read, so
        // a throttled request never causes a token refresh. A request refused
        // by its own validation above spends none.
        if let Err(wait) = admit_request(&self.repository, &self.installation, rule).await {
            counter!("attricat_extension_network_requests_total", "outcome" => "throttled")
                .increment(1);
            return bounded_serialize(&json!({
                "outcome": "rate_limited",
                "retry_after_ms": wait.as_millis().max(1) as u64,
            }));
        }
        let mut sent_token = None;
        if let Some(connection) = &input.connection {
            let token =
                match connections::insert_access_token(self, connection, &rule.id, &mut headers)
                    .await
                {
                    Ok(token) => token,
                    Err(TokenError::Temporary(message)) => {
                        return bounded_serialize(&json!({
                            "outcome": "connection_unavailable",
                            "message": message,
                        }));
                    }
                    Err(error) => return Err(error.to_string()),
                };
            sent_token = Some(token);
        }
        let host = url
            .host_str()
            .ok_or_else(|| "HTTPS URL needs a host".to_owned())?
            .to_owned();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| "HTTPS URL needs a port".to_owned())?;
        let timeout = Duration::from_millis(rule.timeout_ms);
        let addresses = resolve_public_destination(&host, port, timeout).await?;
        let client = pinned_client(&host, &addresses, timeout)?;
        let started = Instant::now();
        let response = match client
            .request(method, url)
            .headers(headers)
            .body(body)
            .send()
            .await
        {
            Ok(response) => response,
            // A connection or TLS handshake that failed sent nothing, so the
            // request may safely be sent again.
            Err(error) if error.is_connect() => {
                counter!("attricat_extension_network_requests_total", "outcome" => "unreachable")
                    .increment(1);
                return bounded_serialize(&json!({"outcome": "unreachable"}));
            }
            // No automatic retries: the peer may have received the request.
            Err(_) => {
                counter!("attricat_extension_network_requests_total", "outcome" => "uncertain")
                    .increment(1);
                return bounded_serialize(&json!({"outcome": "uncertain"}));
            }
        };
        // Redirects are never followed. A 3xx response is returned as it is,
        // so a component may request its Location through a granted rule
        // (Allegro answers finished commands with 303 See Other).
        let status = response.status().as_u16();
        if status == 401
            && let (Some(connection), Some(token)) = (&input.connection, &sent_token)
        {
            // The destination rejected the token; refresh it on the next call.
            let _ = self
                .repository
                .invalidate_extension_connection_token(
                    &self.installation.extension_id,
                    connection,
                    token,
                )
                .await;
        }
        let response_headers = response
            .headers()
            .iter()
            .filter(|(name, _)| {
                DEFAULT_RESPONSE_HEADERS.contains(&name.as_str())
                    || rule
                        .response_headers
                        .iter()
                        .any(|allowed| allowed == name.as_str())
            })
            .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_owned())))
            .collect::<HashMap<_, _>>();
        // The request was sent and may have been applied, so a response too
        // large to return is `uncertain`, not an error a component retries.
        let too_large = || {
            counter!("attricat_extension_network_requests_total", "outcome" => "uncertain")
                .increment(1);
            bounded_serialize(&json!({
                "outcome": "uncertain",
                "status": status,
                "reason": "response_too_large",
            }))
        };
        if response
            .content_length()
            .is_some_and(|length| length > rule.max_response_bytes)
        {
            return too_large();
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else {
                return bounded_serialize(&json!({"outcome": "uncertain", "status": status}));
            };
            if bytes.len().saturating_add(chunk.len()) > rule.max_response_bytes as usize {
                return too_large();
            }
            bytes.extend_from_slice(&chunk);
        }
        counter!("attricat_extension_network_requests_total", "outcome" => "response").increment(1);
        histogram!("attricat_extension_network_duration_seconds", "extension" => self.installation.extension_id.clone(), "host_permission" => rule.id.clone()).record(started.elapsed());
        bounded_serialize(&json!({
            "outcome": "response",
            "status": status,
            "headers": response_headers,
            "body_base64": BASE64.encode(bytes),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operators_can_only_lower_a_budget() {
        let declared = RateLimit {
            requests: 10,
            per_seconds: 1,
            burst: Some(40),
        };
        assert_eq!(effective_budget(declared, None), declared);
        let lowered = effective_budget(declared, Some((2, 1)));
        assert_eq!((lowered.requests, lowered.burst()), (2, 2));
        assert_eq!(effective_budget(declared, Some((100, 1))), declared);
    }

    #[test]
    fn secrets_are_substituted_into_form_and_json_bodies() {
        let secrets = HashMap::from([("lwa".to_owned(), "s&cret=1".to_owned())]);
        let form = SecretBody {
            format: BodyFormat::Form,
            fields: vec![SecretField {
                field: "client_secret".into(),
                secret: "lwa".into(),
            }],
        };
        let body = substitute_secret_body(
            Some("grant_type=refresh_token&client_secret=forged".into()),
            &form,
            &secrets,
        )
        .unwrap();
        assert_eq!(body, "grant_type=refresh_token&client_secret=s%26cret%3D1");
        let json_body = SecretBody {
            format: BodyFormat::Json,
            fields: vec![
                SecretField {
                    field: "/auth/secret".into(),
                    secret: "lwa".into(),
                },
                SecretField {
                    field: "top".into(),
                    secret: "lwa".into(),
                },
            ],
        };
        let body: Value = serde_json::from_str(
            &substitute_secret_body(Some(r#"{"auth":{"id":"a"}}"#.into()), &json_body, &secrets)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({"auth": {"id": "a", "secret": "s&cret=1"}, "top": "s&cret=1"})
        );
    }
}
