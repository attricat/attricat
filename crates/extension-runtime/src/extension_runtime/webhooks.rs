//! Inbound webhooks (`attricat:host@1.2.0` `webhook-receiver`, #279).
//!
//! The public route calls `receive` while the sender waits. Anyone who knows
//! an endpoint URL can make it run, so the invocation is short, can reach only
//! configuration, secrets, extension storage reads, the clock and logging, and
//! never quarantines the installation: its failures are answered with a 500.

use std::time::Duration;

use super::*;

/// How long one `receive` may run, including waiting for an invocation slot.
/// Senders typically give up after 5 to 10 seconds.
pub const WEBHOOK_RECEIVE_TIMEOUT: Duration = Duration::from_secs(5);
/// Fuel for one `receive`: enough to check a signature over the largest body
/// many times over, far less than an event handler gets.
const WEBHOOK_RECEIVE_FUEL: u64 = 200_000_000;
/// The largest reply body a component may send back.
pub const MAX_WEBHOOK_REPLY_BYTES: usize = 64 * 1024;
/// The most reply headers a component may set.
pub const MAX_WEBHOOK_REPLY_HEADERS: usize = 16;
const MAX_WEBHOOK_HEADER_VALUE_BYTES: usize = 8192;
/// Reply headers the host owns: framing, cookies, and caching and security
/// headers it sets itself.
const RESERVED_REPLY_HEADERS: &[&str] = &[
    "connection",
    "content-length",
    "transfer-encoding",
    "set-cookie",
    "cache-control",
    "referrer-policy",
    "x-content-type-options",
    "strict-transport-security",
    "content-security-policy",
];
/// Request headers a component never sees: browser and proxy credentials.
const WITHHELD_REQUEST_HEADERS: &[&str] = &["cookie", "authorization", "proxy-authorization"];

/// The host API operations available during `receive`.
const RECEIVE_OPERATIONS: &[&str] = &[
    "configuration.get.v1",
    "secrets.get.v1",
    "storage.get.v1",
    "storage.list.v1",
];

/// Requests one endpoint accepts per [`WEBHOOK_RATE_WINDOW`] before it
/// answers 429. Counted per API process, or across replicas with
/// `CACHE_BACKEND=redis`.
pub const WEBHOOK_RATE_LIMIT: u32 = 600;
pub const WEBHOOK_RATE_WINDOW: Duration = Duration::from_secs(60);
/// The public route's path below the API mount.
pub const WEBHOOK_ROUTE_PREFIX: &str = "/api/public/extension-webhooks/";

/// Where senders reach the public route (`EXTENSION_WEBHOOK_BASE_URL`).
static WEBHOOK_BASE_URL: OnceLock<Url> = OnceLock::new();

/// Installs the externally reachable base URL of webhook endpoints, ending in
/// [`WEBHOOK_ROUTE_PREFIX`]. Later calls are ignored.
pub fn use_webhook_base_url(base: Url) {
    let _ = WEBHOOK_BASE_URL.set(base);
}

/// An endpoint's path, which operators prefix with their public origin when
/// no base URL is configured.
pub fn webhook_path(token: &str) -> String {
    format!("{WEBHOOK_ROUTE_PREFIX}{token}")
}

/// An endpoint's public URL, when the base URL is configured.
pub fn webhook_url(token: &str) -> Option<Url> {
    WEBHOOK_BASE_URL.get()?.join(token).ok()
}

/// Admits one public request to an endpoint.
pub async fn allow_webhook_request(endpoint_id: Uuid) -> bool {
    NETWORK_RATE_LIMITER
        .get_or_init(|| Arc::new(LocalRateLimiter::default()))
        .allow(
            &format!("webhook:{endpoint_id}"),
            WEBHOOK_RATE_LIMIT,
            WEBHOOK_RATE_WINDOW,
        )
        .await
}

/// A public request to an endpoint.
#[derive(Clone, Debug)]
pub struct WebhookRequest {
    pub webhook: String,
    pub endpoint: String,
    pub method: String,
    pub query: String,
    /// Lower-case names. Withheld credentials are dropped by
    /// [`WebhookRequest::new`].
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl WebhookRequest {
    /// Builds the request a component sees, withholding cookies and
    /// credentials and headers whose values are not text.
    pub fn new(
        webhook: String,
        endpoint: String,
        method: String,
        query: String,
        headers: impl IntoIterator<Item = (String, Vec<u8>)>,
        body: Vec<u8>,
    ) -> Self {
        let headers = headers
            .into_iter()
            .filter_map(|(name, value)| {
                let name = name.to_ascii_lowercase();
                if WITHHELD_REQUEST_HEADERS.contains(&name.as_str()) {
                    return None;
                }
                String::from_utf8(value).ok().map(|value| (name, value))
            })
            .collect();
        Self {
            webhook,
            endpoint,
            method,
            query,
            headers,
            body,
        }
    }
}

/// The response the sender receives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// A payload the component kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptPayload {
    pub payload: String,
    pub dedupe_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookOutcome {
    pub reply: WebhookReply,
    pub keep: Option<KeptPayload>,
}

/// Checks what a component answered: a final status, safe headers and
/// bounded bodies. `max_payload_bytes` is the webhook's `max_body_bytes`.
pub(super) fn validate_outcome(
    outcome: &WebhookOutcome,
    max_payload_bytes: u64,
) -> Result<(), String> {
    let reply = &outcome.reply;
    if !(200..=599).contains(&reply.status) {
        return Err("reply status must be between 200 and 599".into());
    }
    if reply.body.len() > MAX_WEBHOOK_REPLY_BYTES {
        return Err(format!(
            "reply body exceeds {MAX_WEBHOOK_REPLY_BYTES} bytes"
        ));
    }
    if reply.headers.len() > MAX_WEBHOOK_REPLY_HEADERS {
        return Err(format!(
            "at most {MAX_WEBHOOK_REPLY_HEADERS} reply headers may be set"
        ));
    }
    for (name, value) in &reply.headers {
        let lower = name.to_ascii_lowercase();
        if reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
            || reqwest::header::HeaderValue::from_str(value).is_err()
            || value.len() > MAX_WEBHOOK_HEADER_VALUE_BYTES
        {
            return Err(format!("reply header '{name}' is not a valid header"));
        }
        if RESERVED_REPLY_HEADERS.contains(&lower.as_str()) {
            return Err(format!("reply header '{name}' is set by the host"));
        }
    }
    if let Some(kept) = &outcome.keep {
        if kept.payload.len() as u64 > max_payload_bytes {
            return Err(format!(
                "kept payload exceeds the webhook's {max_payload_bytes} bytes"
            ));
        }
        if kept.dedupe_key.as_ref().is_some_and(|key| {
            key.is_empty() || key.len() > crate::repository::MAX_WEBHOOK_DEDUPE_KEY_BYTES
        }) {
            return Err(format!(
                "a dedupe key has 1 to {} bytes",
                crate::repository::MAX_WEBHOOK_DEDUPE_KEY_BYTES
            ));
        }
    }
    Ok(())
}

impl HostState {
    /// Refuses host operations `receive` may not use.
    pub(super) fn allow_during_receive(operation: &str) -> Result<(), String> {
        if RECEIVE_OPERATIONS.contains(&operation) {
            Ok(())
        } else {
            Err(format!(
                "'{operation}' is not available while receiving a webhook"
            ))
        }
    }

    /// `webhooks.receipt.get.v1`: a payload this extension kept, by the
    /// `receipt_id` of its `webhook.received.v1` event; `null` once retention
    /// has removed it.
    pub(super) async fn webhook_receipt_call(&self, request: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ReceiptGet {
            receipt_id: String,
        }
        let input: ReceiptGet = parse_host_request(request, "webhook receipt request")?;
        let receipt_id = parse_uuid(&input.receipt_id, "receipt ID")?;
        let receipt = self
            .repository
            .extension_webhook_receipt(&self.installation.extension_id, receipt_id)
            .await
            .map_err(|_| "webhook receipt lookup failed".to_owned())?;
        let serialized = serde_json::to_string(&receipt).expect("receipt serializes");
        if serialized.len() > MAX_HOST_RESPONSE_JSON_BYTES {
            return Err("webhook receipt exceeds the host response limit".into());
        }
        Ok(serialized)
    }
}

impl ExtensionRuntime {
    /// Lets the component answer one webhook request. The installation is a
    /// fresh enabled snapshot; the caller has checked that its release
    /// declares the webhook and that `webhooks.receive` is granted.
    pub async fn receive_webhook(
        &self,
        installation: &ExtensionRuntimeInstallation,
        repository: AttricatRepository,
        request: WebhookRequest,
        max_payload_bytes: u64,
    ) -> Result<WebhookOutcome, ExtensionRuntimeError> {
        let outcome = tokio::time::timeout(
            WEBHOOK_RECEIVE_TIMEOUT,
            self.invoke_unified_webhook(
                installation,
                repository,
                request,
                WEBHOOK_RECEIVE_FUEL.min(self.config.fuel),
                WEBHOOK_RECEIVE_TIMEOUT.min(self.config.invocation_timeout),
            ),
        )
        .await
        .map_err(|_| runtime_error("webhook receive timed out"))??;
        validate_outcome(&outcome, max_payload_bytes).map_err(ExtensionRuntimeError::Runtime)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(status: u16, headers: &[(&str, &str)]) -> WebhookOutcome {
        WebhookOutcome {
            reply: WebhookReply {
                status,
                headers: headers
                    .iter()
                    .map(|(name, value)| ((*name).into(), (*value).into()))
                    .collect(),
                body: String::new(),
            },
            keep: None,
        }
    }

    #[test]
    fn replies_are_bounded_and_keep_host_headers() {
        assert!(validate_outcome(&outcome(200, &[("content-type", "text/plain")]), 10).is_ok());
        assert!(validate_outcome(&outcome(101, &[]), 10).is_err());
        assert!(validate_outcome(&outcome(600, &[]), 10).is_err());
        assert!(validate_outcome(&outcome(200, &[("Set-Cookie", "a=b")]), 10).is_err());
        assert!(validate_outcome(&outcome(200, &[("bad name", "x")]), 10).is_err());
        assert!(validate_outcome(&outcome(200, &[("x-ok", "line\nbreak")]), 10).is_err());
        let mut kept = outcome(202, &[]);
        kept.keep = Some(KeptPayload {
            payload: "12345678901".into(),
            dedupe_key: None,
        });
        assert!(validate_outcome(&kept, 10).is_err());
        assert!(validate_outcome(&kept, 11).is_ok());
        kept.keep.as_mut().unwrap().dedupe_key = Some(String::new());
        assert!(validate_outcome(&kept, 11).is_err());
    }

    #[test]
    fn requests_withhold_credentials() {
        let request = WebhookRequest::new(
            "orders".into(),
            "shop".into(),
            "POST".into(),
            String::new(),
            [
                ("Cookie".to_owned(), b"session=1".to_vec()),
                ("Authorization".to_owned(), b"Bearer x".to_vec()),
                ("X-Signature".to_owned(), b"abc".to_vec()),
                ("X-Binary".to_owned(), vec![0xff]),
            ],
            Vec::new(),
        );
        assert_eq!(
            request.headers,
            [("x-signature".to_owned(), "abc".to_owned())]
        );
    }

    #[test]
    fn receive_reaches_only_read_operations() {
        assert!(HostState::allow_during_receive("secrets.get.v1").is_ok());
        for operation in [
            "storage.set.v1",
            "attricat.command.v1",
            "attricat.read.v1",
            "events.emit.v1",
            "network.request.v2",
            "webhooks.receipt.get.v1",
        ] {
            assert!(
                HostState::allow_during_receive(operation).is_err(),
                "{operation}"
            );
        }
    }
}
