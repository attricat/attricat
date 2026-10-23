//! Mediated bulk transfer: `transfer.fetch-input`/`deliver-output` (1.0)
//! and their 1.1 variants, plus `transfer.open-record-file`.
//!
//! Every transfer uses the same DNS pinning, no-proxy, TLS and no-redirect
//! policy as `network.request`, and never copies file bytes into JSON or
//! exposes a socket to WASM. The 1.1 variants add what presigned object-store
//! URLs need: rule-approved query strings, caller-controlled signed headers
//! (an exact `Content-Type`, no forced `Idempotency-Key`) and multipart form
//! POSTs for form-style staged uploads.

use super::network_v2::{admit_request, granted_rule, https_url};
use super::*;
use crate::extension_runtime::connections;

/// Headers a caller may not set on a v2 transfer: the host owns them.
const HOST_TRANSFER_HEADERS: [&str; 2] = ["range", "if-match"];
const MAX_TRANSFER_HEADERS: usize = 32;
const MAX_MULTIPART_FIELDS: usize = 32;
const MAX_MULTIPART_VALUE_BYTES: usize = 8 * 1024;
/// The largest record file `open-record-file` copies into a run.
const MAX_RECORD_FILE_BYTES: i64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferFetchV2 {
    host_permission_id: String,
    url: String,
    transfer_key: String,
    offset: u64,
    max_bytes: u32,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    secret_headers: Vec<SecretHeader>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    connection: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferDeliveryV2 {
    host_permission_id: String,
    url: String,
    method: String,
    artifact_id: Uuid,
    delivery_key: String,
    #[serde(default)]
    secret_headers: Vec<SecretHeader>,
    /// Signed headers sent exactly; `content-type` replaces the output's
    /// media type.
    #[serde(default)]
    headers: HashMap<String, String>,
    /// Sent as `Idempotency-Key` only when given.
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(default)]
    multipart: Option<Multipart>,
    #[serde(default)]
    connection: Option<String>,
}

/// A `multipart/form-data` body: the form fields in order, then the output
/// as the file field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Multipart {
    #[serde(default)]
    fields: Vec<MultipartField>,
    file_field: String,
    #[serde(default)]
    filename: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MultipartField {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenRecordFile {
    record_id: Uuid,
    file_id: Uuid,
    /// An image variant kind instead of the original.
    #[serde(default)]
    variant: Option<String>,
}

/// One transfer's resolved destination.
struct TransferTarget {
    client: Client,
    url: Url,
    headers: reqwest::header::HeaderMap,
    max_transfer_bytes: u64,
    /// The connection and the access token sent with the transfer, so a 401
    /// discards exactly that token.
    connection: Option<(String, String)>,
}

/// Why a transfer did not reach its destination.
enum TargetRefusal {
    /// The rule's request budget is spent for now.
    RateLimited(Duration),
    /// The connection's token endpoint is temporarily unavailable.
    ConnectionUnavailable(String),
    Error(String),
}

impl From<String> for TargetRefusal {
    fn from(message: String) -> Self {
        Self::Error(message)
    }
}

impl From<&str> for TargetRefusal {
    fn from(message: &str) -> Self {
        Self::Error(message.to_owned())
    }
}

impl TargetRefusal {
    /// The released 1.0 transfers report every refusal as an error.
    fn into_error(self) -> String {
        match self {
            Self::RateLimited(_) => "network request rate limit exceeded".into(),
            Self::ConnectionUnavailable(message) | Self::Error(message) => message,
        }
    }

    /// The 1.1 transfers report a spent budget and an unavailable connection
    /// as retryable outcomes, like `network.request.v2`.
    fn into_outcome(self) -> Result<String, String> {
        match self {
            Self::RateLimited(wait) => bounded_serialize(&json!({
                "outcome": "rate_limited",
                "retry_after_ms": wait.as_millis().max(1) as u64,
            })),
            Self::ConnectionUnavailable(message) => bounded_serialize(&json!({
                "outcome": "connection_unavailable",
                "message": message,
            })),
            Self::Error(message) => Err(message),
        }
    }
}

/// The outcome of a recorded delivery; `None` for one still to be sent.
fn delivery_outcome(state: &DeliveryState) -> Option<Value> {
    match state {
        DeliveryState::Succeeded(status) => Some(json!({"outcome":"succeeded","status":status})),
        DeliveryState::Failed(status) => Some(json!({"outcome":"failed","status":status})),
        DeliveryState::Uncertain => Some(json!({"outcome":"uncertain"})),
        DeliveryState::Send(_) => None,
    }
}

/// Inserts caller headers, rejecting those the host owns.
fn caller_headers(
    headers: &mut reqwest::header::HeaderMap,
    supplied: HashMap<String, String>,
) -> Result<(), String> {
    if supplied.len() > MAX_TRANSFER_HEADERS {
        return Err("too many transfer headers".into());
    }
    for (name, value) in supplied {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "invalid transfer header")?;
        if RESERVED_REQUEST_HEADERS.contains(&name.as_str())
            || HOST_TRANSFER_HEADERS.contains(&name.as_str())
            || name.as_str() == "idempotency-key"
            || value.len() > MAX_REQUEST_HEADER_VALUE_BYTES
            || headers.contains_key(&name)
        {
            return Err("unsafe transfer header".into());
        }
        headers.insert(
            name,
            reqwest::header::HeaderValue::from_str(&value)
                .map_err(|_| "invalid transfer header")?,
        );
    }
    Ok(())
}

/// Checks caller headers before a transfer spends its request budget,
/// including clashes with the request's secret headers.
fn checked_caller_headers(
    secrets: &[SecretHeader],
    supplied: HashMap<String, String>,
) -> Result<reqwest::header::HeaderMap, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    let secret_names: Vec<reqwest::header::HeaderName> = secrets
        .iter()
        .filter_map(|secret| reqwest::header::HeaderName::from_bytes(secret.header.as_bytes()).ok())
        .collect();
    for name in &secret_names {
        headers.insert(name.clone(), reqwest::header::HeaderValue::from_static(""));
    }
    caller_headers(&mut headers, supplied)?;
    for name in &secret_names {
        headers.remove(name);
    }
    Ok(headers)
}

/// Adds checked caller headers to a resolved transfer's headers.
fn merge_caller_headers(
    headers: &mut reqwest::header::HeaderMap,
    caller: &reqwest::header::HeaderMap,
) -> Result<(), String> {
    for (name, value) in caller {
        if headers.contains_key(name) {
            return Err("unsafe transfer header".into());
        }
        headers.insert(name.clone(), value.clone());
    }
    Ok(())
}

impl OperationState {
    /// Discards the access token a destination answered with 401, so the
    /// next transfer refreshes it; a token refreshed meanwhile is kept.
    async fn discard_rejected_token(
        repository: &AttricatRepository,
        extension_id: &str,
        status: u16,
        connection: Option<&(String, String)>,
    ) {
        if status == 401
            && let Some((connection, token)) = connection
        {
            let _ = repository
                .invalidate_extension_connection_token(extension_id, connection, token)
                .await;
        }
    }

    /// Resolves the granted rule, budget, secrets and pinned client of one
    /// transfer. A URL query needs a rule with `allow_query`.
    async fn transfer_target(
        host: &HostState,
        permission: &str,
        method: &str,
        url: Url,
        secrets: &[SecretHeader],
        connection: Option<&str>,
    ) -> Result<TransferTarget, TargetRefusal> {
        let rule = granted_rule(&host.installation, permission, &url, method)
            .ok()
            .filter(|rule| rule.max_transfer_bytes > 0)
            .ok_or_else(|| "destination is not granted for bulk transfer".to_owned())?;
        if method == "POST" && !rule.idempotent_delivery {
            return Err(
                "POST bulk delivery requires an explicit idempotent destination declaration".into(),
            );
        }
        let address = url.host_str().ok_or("HTTPS URL needs a host")?.to_owned();
        let port = url
            .port_or_known_default()
            .ok_or("HTTPS URL needs a port")?;
        let timeout = Duration::from_millis(rule.timeout_ms);
        if secrets.len() > MAX_TRANSFER_SECRET_HEADERS {
            return Err("too many secret headers".into());
        }
        let mut names = Vec::with_capacity(secrets.len());
        for secret in secrets {
            valid_secret_name(&secret.secret)?;
            let name = reqwest::header::HeaderName::from_bytes(secret.header.as_bytes())
                .map_err(|_| "invalid secret header")?;
            if RESERVED_REQUEST_HEADERS.contains(&name.as_str())
                || RESERVED_TRANSFER_HEADERS.contains(&name.as_str())
                || secret.prefix.len() > MAX_SECRET_HEADER_PREFIX_BYTES
                || names.contains(&name)
            {
                return Err("unsafe secret header".into());
            }
            names.push(name);
        }
        // The budget is spent only by a transfer that passed the checks
        // above, but before DNS is resolved, so failing or slow lookups are
        // throttled too, and before the connection's token is read, so a
        // throttled transfer never causes a token refresh.
        if let Err(wait) = admit_request(&host.repository, &host.installation, rule).await {
            return Err(TargetRefusal::RateLimited(wait));
        }
        let resolved = resolve_public_destination(&address, port, timeout).await?;
        let mut headers = reqwest::header::HeaderMap::new();
        for (secret, name) in secrets.iter().zip(names) {
            let value = host
                .repository
                .workspace_extension_secret_value(&secret.secret)
                .await
                .map_err(|_| "secret lookup failed")?
                .ok_or("secret reference unavailable")?;
            headers.insert(
                name,
                reqwest::header::HeaderValue::from_str(&(secret.prefix.clone() + &value))
                    .map_err(|_| "invalid secret header value")?,
            );
        }
        let mut sent = None;
        if let Some(connection) = connection {
            let token = connections::insert_access_token(host, connection, &rule.id, &mut headers)
                .await
                .map_err(|error| match error {
                    connections::TokenError::Temporary(message) => {
                        TargetRefusal::ConnectionUnavailable(message)
                    }
                    error => TargetRefusal::Error(error.to_string()),
                })?;
            sent = Some((connection.to_owned(), token));
        }
        let client = pinned_client(&address, &resolved, timeout)?;
        Ok(TransferTarget {
            client,
            url,
            headers,
            max_transfer_bytes: rule.max_transfer_bytes,
            connection: sent,
        })
    }

    /// The recorded result of a fetch this run already made with the same
    /// transfer key and request digest. It is checked before the target is
    /// resolved, so a replay spends no token refresh or request budget.
    async fn replayed_fetch(
        repository: &AttricatRepository,
        run_id: Uuid,
        transfer_key: &str,
        digest: &str,
    ) -> Result<Option<String>, String> {
        let previous = repository
            .previous_extension_http_input(run_id, transfer_key, digest)
            .await
            .map_err(|error| error.to_string())?;
        previous
            .map(|(id, etag)| {
                bounded_serialize(&json!({"artifact_id":id,"etag":etag,"replayed":true}))
            })
            .transpose()
    }

    /// Fetches one byte range into a run-bound input artifact. `digest`
    /// identifies the request for replays of the same transfer key.
    #[allow(clippy::too_many_arguments)]
    async fn fetch_range(
        &mut self,
        host: &HostState,
        target: TransferTarget,
        transfer_key: &str,
        offset: u64,
        max_bytes: u32,
        expected_etag: Option<&str>,
        digest: &str,
    ) -> Result<String, String> {
        if u64::from(max_bytes) > target.max_transfer_bytes {
            return Err("range exceeds granted bulk transfer limit".into());
        }
        let TransferTarget {
            client,
            url,
            mut headers,
            connection,
            ..
        } = target;
        let end = offset + u64::from(max_bytes) - 1;
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            reqwest::header::HeaderValue::from_static("identity"),
        );
        headers.insert(
            reqwest::header::RANGE,
            reqwest::header::HeaderValue::from_str(&format!("bytes={offset}-{end}"))
                .map_err(|_| "invalid range")?,
        );
        if let Some(etag) = expected_etag {
            if etag.len() > MAX_ETAG_BYTES {
                return Err("ETag is too long".into());
            }
            headers.insert(
                reqwest::header::IF_MATCH,
                reqwest::header::HeaderValue::from_str(etag).map_err(|_| "invalid ETag")?,
            );
        }
        let response = client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|_| "source transfer failed")?;
        if response.status().is_redirection() {
            return Err("redirect responses are denied".into());
        }
        let status = response.status().as_u16();
        Self::discard_rejected_token(
            &self.repository,
            &self.installation.extension_id,
            status,
            connection.as_ref(),
        )
        .await;
        // A connection's rejected token was discarded above, so the
        // component may retry with a fresh one, as after a delivery.
        if status == 401 && connection.is_some() {
            return bounded_serialize(&json!({"outcome":"failed","status":status}));
        }
        if response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
        {
            return Err("compressed range responses are not supported".into());
        }
        if status != 200 && status != 206 {
            return Err(format!("source returned HTTP {status}"));
        }
        if offset > 0 && status != 206 {
            return Err("source ignored range request".into());
        }
        if status == 206 {
            let expected = format!("bytes {offset}-");
            if !response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with(&expected))
            {
                return Err("source returned a mismatched range".into());
            }
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= MAX_ETAG_BYTES)
            .map(str::to_owned);
        if expected_etag.is_some_and(|expected| etag.as_deref() != Some(expected)) {
            return Err("source identity changed".into());
        }
        if response
            .content_length()
            .is_some_and(|v| v > u64::from(max_bytes))
        {
            return Err("source range exceeds limit".into());
        }
        let media = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty() && v.len() <= MAX_MEDIA_TYPE_BYTES && v.is_ascii())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let stream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|_| "source transfer interrupted".to_owned()));
        self.store_input(
            host,
            stream,
            transfer_key,
            digest,
            etag.as_deref(),
            i64::from(max_bytes),
            &media,
            offset,
        )
        .await
    }

    /// Streams `stream` into a new completed input artifact of this run.
    #[allow(clippy::too_many_arguments)]
    async fn store_input(
        &mut self,
        host: &HostState,
        mut stream: impl futures_util::Stream<Item = Result<bytes::Bytes, String>> + Unpin,
        transfer_key: &str,
        digest: &str,
        etag: Option<&str>,
        max_bytes: i64,
        media: &str,
        offset: u64,
    ) -> Result<String, String> {
        let artifact = self
            .repository
            .begin_extension_http_input(
                self.run_id,
                &host.installation.extension_id,
                host.installation.installed_release_id,
                max_bytes,
                media,
            )
            .await
            .map_err(|error| error.to_string())?;
        let path = std::env::temp_dir().join(format!("attricat-http-input-{}", artifact.id));
        let result = async {
            let mut file =
                File::create(&path).map_err(|_| "temporary transfer unavailable".to_owned())?;
            let mut hash = Sha256::new();
            let mut length = 0usize;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                length = length
                    .checked_add(chunk.len())
                    .ok_or("source range exceeds limit")?;
                if length > max_bytes as usize {
                    return Err("source range exceeds limit".into());
                }
                file.write_all(&chunk)
                    .map_err(|_| "temporary transfer unavailable".to_owned())?;
                hash.update(&chunk);
            }
            drop(file);
            if length == 0 {
                return Err("source returned an empty range".into());
            }
            let object_key = artifact
                .object_key
                .as_deref()
                .ok_or("source artifact has no key")?;
            self.object_store
                .put_file(object_key, &path, Some(media))
                .await
                .map_err(|_| "transfer storage unavailable".to_owned())?;
            self.repository
                .complete_extension_http_input(
                    artifact.id,
                    self.run_id,
                    crate::repository::CompletedHttpInput {
                        transfer_key,
                        request_digest: digest,
                        source_etag: etag,
                        length: length as i64,
                        sha256: &format!("{:x}", hash.finalize()),
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            bounded_serialize(&json!({"artifact_id":artifact.id,"etag":etag,"offset":offset,"length":length,"replayed":false}))
        }
        .await;
        let _ = std::fs::remove_file(&path);
        if result.is_err() {
            let _ = self
                .repository
                .abort_extension_http_input(artifact.id, self.run_id)
                .await;
            if let Some(key) = &artifact.object_key {
                let _ = self.object_store.delete(key).await;
            }
        }
        result
    }

    pub(super) async fn transfer_fetch_input(&mut self, request: String) -> Result<String, String> {
        let input: TransferFetch = parse_host_request(&request, "transfer request")?;
        validate_range(
            &input.transfer_key,
            input.offset,
            input.max_bytes,
            input.etag.as_deref(),
        )?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("network.request").await?;
        let url = transfer_url(&input.url)?;
        let digest = format!("{:x}", Sha256::digest(request.as_bytes()));
        if let Some(replayed) =
            Self::replayed_fetch(&self.repository, self.run_id, &input.transfer_key, &digest)
                .await?
        {
            return Ok(replayed);
        }
        let target = Self::transfer_target(
            &host,
            &input.host_permission_id,
            "GET",
            url,
            &input.secret_headers,
            None,
        )
        .await
        .map_err(TargetRefusal::into_error)?;
        self.fetch_range(
            &host,
            target,
            &input.transfer_key,
            input.offset,
            input.max_bytes,
            input.etag.as_deref(),
            &digest,
        )
        .await
    }

    pub(super) async fn transfer_fetch_input_v2(
        &mut self,
        request: String,
    ) -> Result<String, String> {
        let input: TransferFetchV2 = parse_host_request(&request, "transfer request")?;
        validate_range(
            &input.transfer_key,
            input.offset,
            input.max_bytes,
            input.etag.as_deref(),
        )?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("network.request").await?;
        let url = https_url(&input.url)?;
        let caller = checked_caller_headers(&input.secret_headers, input.headers)?;
        let digest = format!("{:x}", Sha256::digest(request.as_bytes()));
        if let Some(replayed) =
            Self::replayed_fetch(&self.repository, self.run_id, &input.transfer_key, &digest)
                .await?
        {
            return Ok(replayed);
        }
        let mut target = match Self::transfer_target(
            &host,
            &input.host_permission_id,
            "GET",
            url,
            &input.secret_headers,
            input.connection.as_deref(),
        )
        .await
        {
            Ok(target) => target,
            Err(refusal) => return refusal.into_outcome(),
        };
        merge_caller_headers(&mut target.headers, &caller)?;
        self.fetch_range(
            &host,
            target,
            &input.transfer_key,
            input.offset,
            input.max_bytes,
            input.etag.as_deref(),
            &digest,
        )
        .await
    }

    pub(super) async fn transfer_deliver_output(
        &mut self,
        request: String,
    ) -> Result<String, String> {
        let input: TransferDelivery = parse_host_request(&request, "delivery request")?;
        let url = transfer_url(&input.url)?;
        let digest = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}:{}:{}:{}",
                    input.host_permission_id, input.url, input.method, input.artifact_id
                )
                .as_bytes()
            )
        );
        // The released contract always sends a stable Idempotency-Key.
        let idempotency = format!("{}:{}", self.run_id, input.delivery_key);
        self.deliver(Delivery {
            host_permission_id: input.host_permission_id,
            url,
            method: input.method,
            artifact_id: input.artifact_id,
            delivery_key: input.delivery_key,
            secret_headers: input.secret_headers,
            headers: HashMap::new(),
            idempotency_key: Some(idempotency),
            multipart: None,
            connection: None,
            digest,
            any_artifact: false,
        })
        .await
    }

    pub(super) async fn transfer_deliver_output_v2(
        &mut self,
        request: String,
    ) -> Result<String, String> {
        let input: TransferDeliveryV2 = parse_host_request(&request, "delivery request")?;
        let url = https_url(&input.url)?;
        // Query strings are presigned credentials; only their digest is kept.
        let digest = format!("{:x}", Sha256::digest(request.as_bytes()));
        if input
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.is_empty() || key.len() > MAX_IDEMPOTENCY_KEY_BYTES)
        {
            return Err("invalid idempotency key".into());
        }
        self.deliver(Delivery {
            host_permission_id: input.host_permission_id,
            url,
            method: input.method,
            artifact_id: input.artifact_id,
            delivery_key: input.delivery_key,
            secret_headers: input.secret_headers,
            headers: input.headers,
            idempotency_key: input.idempotency_key,
            multipart: input.multipart,
            connection: input.connection,
            digest,
            any_artifact: true,
        })
        .await
    }

    async fn deliver(&mut self, input: Delivery) -> Result<String, String> {
        if !transfer_key_valid(&input.delivery_key)
            || !matches!(input.method.as_str(), "PUT" | "POST")
        {
            return Err("invalid delivery method or key".into());
        }
        if input.multipart.is_some() && input.method != "POST" {
            return Err("multipart deliveries use POST".into());
        }
        // The input is checked before the transfer spends its request budget
        // or refreshes a token.
        let content_type = input
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.clone());
        let caller = checked_caller_headers(
            &input.secret_headers,
            input
                .headers
                .into_iter()
                .filter(|(name, _)| !name.eq_ignore_ascii_case("content-type"))
                .collect(),
        )?;
        if let Some(multipart) = &input.multipart
            && (multipart.fields.len() > MAX_MULTIPART_FIELDS
                || multipart.file_field.is_empty()
                || multipart.file_field.len() > 256
                || multipart.fields.iter().any(|field| {
                    field.name.is_empty()
                        || field.name.len() > 256
                        || field.value.len() > MAX_MULTIPART_VALUE_BYTES
                }))
        {
            return Err("invalid multipart form".into());
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("network.request").await?;
        // A replay of a recorded delivery returns its outcome before it
        // spends a request budget, resolves DNS or refreshes a token.
        if let Some(state) = self
            .repository
            .previous_extension_http_delivery(self.run_id, &input.delivery_key, &input.digest)
            .await
            .map_err(|error| error.to_string())?
            && let Some(outcome) = delivery_outcome(&state)
        {
            return bounded_serialize(&outcome);
        }
        let TransferTarget {
            client,
            url,
            mut headers,
            max_transfer_bytes,
            connection,
        } = match Self::transfer_target(
            &host,
            &input.host_permission_id,
            &input.method,
            input.url,
            &input.secret_headers,
            input.connection.as_deref(),
        )
        .await
        {
            Ok(target) => target,
            // Only the 1.1 delivery has retryable outcomes.
            Err(refusal) if input.any_artifact => return refusal.into_outcome(),
            Err(refusal) => return Err(refusal.into_error()),
        };
        merge_caller_headers(&mut headers, &caller)?;
        // The released contract delivers outputs only; v2 also delivers a
        // completed input of the run, such as an opened record file.
        let artifact = if input.any_artifact {
            self.repository
                .extension_run_artifact(self.run_id, input.artifact_id)
                .await
        } else {
            self.repository
                .extension_output_artifact(self.run_id, input.artifact_id)
                .await
        }
        .map_err(|_| "output is not authorized")?;
        if artifact.state != "completed"
            || artifact.content_length <= 0
            || artifact.content_length as u64 > max_transfer_bytes
        {
            return Err("output exceeds granted delivery limit or is not finalized".into());
        }
        // Every header is checked before the durable uncertain marker is
        // written: a value that cannot be sent is an input error, not a
        // delivery that may have happened.
        let media_type = content_type.unwrap_or_else(|| artifact.media_type.clone());
        let content_type_valid = if input.multipart.is_some() {
            reqwest::multipart::Part::text("")
                .mime_str(&media_type)
                .is_ok()
        } else {
            reqwest::header::HeaderValue::from_str(&media_type).is_ok()
        };
        if !content_type_valid {
            return Err("invalid content type".into());
        }
        if let Some(key) = &input.idempotency_key {
            headers.insert(
                "idempotency-key",
                reqwest::header::HeaderValue::from_str(key)
                    .map_err(|_| "invalid idempotency key")?,
            );
        }
        let attempt = self
            .repository
            .begin_extension_http_delivery(
                self.run_id,
                &input.delivery_key,
                input.artifact_id,
                &input.digest,
            )
            .await
            .map_err(|error| error.to_string())?;
        let id = match attempt {
            DeliveryState::Send(id) => id,
            state => {
                return bounded_serialize(
                    &delivery_outcome(&state).expect("a recorded delivery has an outcome"),
                );
            }
        };
        // Once the durable uncertain marker exists we must not resend on any
        // failure path, including a process crash before the HTTP request.
        let object_key = artifact.object_key.ok_or("output object unavailable")?;
        let stream = self
            .object_store
            .get_stream(&object_key)
            .await
            .map_err(|_| "output storage unavailable")?;
        let method =
            Method::from_bytes(input.method.as_bytes()).map_err(|_| "invalid delivery method")?;
        let request = client.request(method, url).headers(headers);
        let request = match input.multipart {
            Some(multipart) => {
                let mut form = reqwest::multipart::Form::new();
                for field in multipart.fields {
                    form = form.text(field.name, field.value);
                }
                let mut part = reqwest::multipart::Part::stream_with_length(
                    reqwest::Body::wrap_stream(stream.stream),
                    artifact.content_length as u64,
                )
                .mime_str(&media_type)
                .map_err(|_| "invalid content type")?;
                if let Some(filename) = multipart.filename {
                    part = part.file_name(filename);
                }
                request.multipart(form.part(multipart.file_field, part))
            }
            None => request
                .header(reqwest::header::CONTENT_TYPE, media_type)
                .header(reqwest::header::CONTENT_LENGTH, artifact.content_length)
                .body(reqwest::Body::wrap_stream(stream.stream)),
        };
        let Ok(response) = request.send().await else {
            return bounded_serialize(&json!({"outcome":"uncertain"}));
        };
        if response.status().is_redirection() {
            return bounded_serialize(&json!({"outcome":"uncertain"}));
        }
        let status = response.status().as_u16();
        Self::discard_rejected_token(
            &self.repository,
            &self.installation.extension_id,
            status,
            connection.as_ref(),
        )
        .await;
        // A 401 with a connection's token applied nothing; the same key is
        // sent again once the token is refreshed.
        if status == 401 && connection.is_some() {
            self.repository
                .forget_extension_http_delivery(id)
                .await
                .map_err(|_| "delivery state could not be persisted")?;
            return bounded_serialize(&json!({"outcome":"failed","status":status}));
        }
        self.repository
            .complete_extension_http_delivery(id, status)
            .await
            .map_err(|_| "delivery confirmation could not be persisted")?;
        bounded_serialize(
            &json!({"outcome": if (200..300).contains(&status) { "succeeded" } else { "failed" },"status":status}),
        )
    }

    /// Copies a file referenced by a record in the run's scope into a
    /// run-bound input artifact, for direct binary uploads.
    pub(super) async fn transfer_open_record_file(
        &mut self,
        request: String,
    ) -> Result<String, String> {
        let input: OpenRecordFile = parse_host_request(&request, "record file request")?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_all_active(["attricat.read", "attricat.files.read", "artifacts.read"])
            .await?;
        let targets = self
            .repository
            .file_read_targets(input.file_id)
            .await
            .map_err(|error| error.to_string())?;
        if !targets
            .iter()
            .any(|target| target.record_id == input.record_id)
        {
            return Err("the record does not reference that file".into());
        }
        ensure_run_reaches_record(
            &self.repository,
            self.run_id,
            input.record_id,
            RunReach::Read,
        )
        .await?;
        let file = self
            .repository
            .file_object(input.file_id, input.variant.as_deref())
            .await
            .map_err(|error| error.to_string())?;
        if file.status != "ready" {
            return Err("the file is not ready".into());
        }
        if file.byte_size <= 0 || file.byte_size > MAX_RECORD_FILE_BYTES {
            return Err(format!(
                "record files up to {MAX_RECORD_FILE_BYTES} bytes can be opened"
            ));
        }
        let transfer_key = format!(
            "record-file:{}:{}",
            input.file_id,
            input.variant.as_deref().unwrap_or("original")
        );
        // The artifact holds the file's bytes whichever record referenced
        // it, so another record of the run sharing the file reuses it.
        let digest = format!("{:x}", Sha256::digest(transfer_key.as_bytes()));
        if let Some((id, _)) = self
            .repository
            .previous_extension_http_input(self.run_id, &transfer_key, &digest)
            .await
            .map_err(|error| error.to_string())?
        {
            return bounded_serialize(&json!({"artifact_id": id, "replayed": true}));
        }
        let stream = self
            .object_store
            .get_stream(&file.object_key)
            .await
            .map_err(|_| "file storage unavailable")?;
        let stream = stream
            .stream
            .map(|chunk| chunk.map_err(|_| "file read interrupted".to_owned()));
        self.store_input(
            &host,
            Box::pin(stream),
            &transfer_key,
            &digest,
            None,
            file.byte_size,
            &file.mime_type,
            0,
        )
        .await
    }
}

/// A delivery of either released contract.
struct Delivery {
    host_permission_id: String,
    url: Url,
    method: String,
    artifact_id: Uuid,
    delivery_key: String,
    secret_headers: Vec<SecretHeader>,
    headers: HashMap<String, String>,
    idempotency_key: Option<String>,
    multipart: Option<Multipart>,
    connection: Option<String>,
    digest: String,
    /// Whether an input artifact may be delivered too (v2).
    any_artifact: bool,
}

fn validate_range(
    transfer_key: &str,
    offset: u64,
    max_bytes: u32,
    etag: Option<&str>,
) -> Result<(), String> {
    if !transfer_key_valid(transfer_key)
        || max_bytes == 0
        || max_bytes > MAX_TRANSFER_RANGE_BYTES
        || offset.checked_add(u64::from(max_bytes)).is_none()
    {
        return Err("invalid bounded transfer range".into());
    }
    if offset > 0 && etag.is_none() {
        return Err("resumed input requires an ETag".into());
    }
    Ok(())
}

/// What a run does with a record it is checked for.
#[derive(Clone, Copy)]
pub(super) enum RunReach {
    /// Reads it: a connector run serves only its selection and, for a
    /// channel export, published records.
    Read,
    /// Writes its export mapping: a connector run writes any record of its
    /// blueprint, including ones it withdraws.
    Map,
}

/// Refuses a record outside a run's scope for `reach`. An interactive run
/// reaches only the records it was started on.
pub(super) async fn ensure_run_reaches_record(
    repository: &AttricatRepository,
    run_id: Uuid,
    record_id: Uuid,
    reach: RunReach,
) -> Result<(), String> {
    if repository
        .connector_run_scope(run_id)
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        let reached = match reach {
            RunReach::Read => {
                repository
                    .connector_run_covers_record(run_id, record_id)
                    .await
            }
            RunReach::Map => {
                repository
                    .connector_run_may_map_record(run_id, record_id)
                    .await
            }
        }
        .map_err(|error| error.to_string())?;
        if !reached {
            return Err("the record is outside the run's scope".into());
        }
    }
    if let Some(scope) = repository
        .interactive_run_scope(run_id)
        .await
        .map_err(|error| error.to_string())?
        && !scope.record_ids.contains(&record_id)
    {
        return Err("the record is outside the run selection".into());
    }
    Ok(())
}
