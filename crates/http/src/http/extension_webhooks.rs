//! Inbound extension webhooks (#279): endpoint management and the public
//! route senders call.
//!
//! The public route authenticates nothing itself. It resolves the endpoint by
//! its URL token, refuses oversized bodies and floods, and then lets the
//! extension's component check the request and choose the response. A payload
//! the component keeps is stored, with its `webhook.received.v1` event, before
//! the response is sent; the component's event handlers process it later.

use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use metrics::counter;
use serde::{Deserialize, Serialize};

use super::{
    AppState,
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    extension_runtime::{self, WebhookOutcome, WebhookRequest},
    extensions::{Manifest, WEBHOOKS_RECEIVE_CAPABILITY, Webhook},
    repository::{
        AttricatRepository, ExtensionWebhookEndpoint, IssuedWebhookEndpoint, KeptWebhookPayload,
        ResolvedWebhookEndpoint, webhook_token_digest,
    },
};

/// The longest URL token accepted; issued tokens have 43 characters.
const MAX_WEBHOOK_TOKEN_BYTES: usize = 128;
/// Outcomes of public webhook requests, the `outcome` label of
/// `attricat_extension_webhook_requests_total`.
#[derive(Clone, Copy)]
enum RequestOutcome {
    UnknownEndpoint,
    Unavailable,
    TooLarge,
    RateLimited,
    Failed,
    Answered,
    Kept,
    Duplicate,
}

impl RequestOutcome {
    const fn label(self) -> &'static str {
        match self {
            Self::UnknownEndpoint => "unknown_endpoint",
            Self::Unavailable => "unavailable",
            Self::TooLarge => "too_large",
            Self::RateLimited => "rate_limited",
            Self::Failed => "failed",
            Self::Answered => "answered",
            Self::Kept => "kept",
            Self::Duplicate => "duplicate",
        }
    }

    fn count(self) {
        counter!("attricat_extension_webhook_requests_total", "outcome" => self.label())
            .increment(1);
    }
}

/// An endpoint with its public URL, which only a create or rotate response
/// carries. `path` is always present; `url` needs
/// `EXTENSION_WEBHOOK_BASE_URL`.
#[derive(Serialize)]
pub(super) struct IssuedEndpointResponse {
    #[serde(flatten)]
    endpoint: ExtensionWebhookEndpoint,
    path: String,
    url: Option<String>,
}

impl From<IssuedWebhookEndpoint> for IssuedEndpointResponse {
    fn from(issued: IssuedWebhookEndpoint) -> Self {
        Self {
            path: extension_runtime::webhook_path(&issued.token),
            url: extension_runtime::webhook_url(&issued.token).map(|url| url.to_string()),
            endpoint: issued.endpoint,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateEndpointRequest {
    webhook: String,
    endpoint: String,
}

fn installed_manifest(value: &serde_json::Value) -> Result<Manifest, ApiError> {
    serde::Deserialize::deserialize(value)
        .map_err(|_| ApiError::invalid_input("installed extension manifest is invalid".into()))
}

fn declared_webhook<'a>(manifest: &'a Manifest, webhook: &str) -> Option<&'a Webhook> {
    manifest
        .server
        .as_ref()?
        .webhooks
        .iter()
        .find(|item| item.id == webhook)
}

/// `GET /extensions/{extension_id}/webhook-endpoints`
pub(super) async fn list_endpoints(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<Json<Vec<ExtensionWebhookEndpoint>>, ApiError> {
    repository.installed_extension(&extension_id).await?;
    Ok(Json(
        repository
            .list_extension_webhook_endpoints(&extension_id)
            .await?,
    ))
}

/// `POST /extensions/{extension_id}/webhook-endpoints`: creates an endpoint
/// of a webhook the installed release declares. The response carries the
/// endpoint's URL, which cannot be shown again.
pub(super) async fn create_endpoint(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
    ApiJson(input): ApiJson<CreateEndpointRequest>,
) -> Result<(StatusCode, Json<IssuedEndpointResponse>), ApiError> {
    let installed = repository.installed_extension(&extension_id).await?;
    let manifest = installed_manifest(&installed.manifest)?;
    if declared_webhook(&manifest, &input.webhook).is_none() {
        return Err(ApiError::invalid_input(
            "the installed release declares no such webhook".into(),
        ));
    }
    let issued = repository
        .create_extension_webhook_endpoint(&extension_id, &input.webhook, &input.endpoint)
        .await?;
    Ok((StatusCode::CREATED, Json(issued.into())))
}

/// `POST /extensions/{extension_id}/webhook-endpoints/{endpoint}/rotate`:
/// issues a new URL; the previous one stops working at once.
pub(super) async fn rotate_endpoint(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, endpoint)): ApiPath<(String, String)>,
) -> Result<Json<IssuedEndpointResponse>, ApiError> {
    let issued = repository
        .rotate_extension_webhook_endpoint(&extension_id, &endpoint)
        .await?;
    Ok(Json(issued.into()))
}

/// `DELETE /extensions/{extension_id}/webhook-endpoints/{endpoint}`
pub(super) async fn delete_endpoint(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, endpoint)): ApiPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    if repository
        .delete_extension_webhook_endpoint(&extension_id, &endpoint)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("webhook endpoint"))
    }
}

/// `GET|POST /public/extension-webhooks/{token}`, a public route; axum answers
/// other methods with 405.
///
/// An unknown token is a 404, an installation that is not enabled with
/// `webhooks.receive` or no longer declares the webhook a 503, a body over
/// the webhook's `max_body_bytes` a 413 and a flood a 429; none of these runs
/// the component. Otherwise the component's reply is sent as it is, after a
/// payload it kept has been stored, or a 500 when it failed.
pub(super) async fn receive(
    State(state): State<AppState>,
    ApiPath(token): ApiPath<String>,
    request: Request,
) -> Result<Response, ApiError> {
    if token.is_empty() || token.len() > MAX_WEBHOOK_TOKEN_BYTES {
        RequestOutcome::UnknownEndpoint.count();
        return Err(ApiError::not_found("webhook endpoint"));
    }
    let Some(endpoint) = state
        .repository
        .resolve_extension_webhook_endpoint(&webhook_token_digest(&token))
        .await?
    else {
        RequestOutcome::UnknownEndpoint.count();
        return Err(ApiError::not_found("webhook endpoint"));
    };
    if !extension_runtime::allow_webhook_request(endpoint.id).await {
        RequestOutcome::RateLimited.count();
        return Err(ApiError::webhook_rate_limited());
    }
    let repository = state.repository.for_workspace(endpoint.workspace_id);
    let response = answer(&state, &repository, &endpoint, request).await;
    let status = match &response {
        Ok(response) => response.status(),
        Err(error) => error.status(),
    };
    if let Err(error) = repository
        .record_extension_webhook_request(endpoint.id, status.as_u16())
        .await
    {
        tracing::warn!(endpoint = %endpoint.id, %error, "webhook request could not be noted");
    }
    response
}

async fn answer(
    state: &AppState,
    repository: &AttricatRepository,
    endpoint: &ResolvedWebhookEndpoint,
    request: Request,
) -> Result<Response, ApiError> {
    let unavailable = || {
        RequestOutcome::Unavailable.count();
        ApiError::service_unavailable("the extension cannot receive webhooks now")
    };
    // A fresh enabled, fully granted snapshot of the installed release.
    let installed = match repository.installed_extension(&endpoint.extension_id).await {
        Ok(installed) => installed,
        Err(_) => return Err(unavailable()),
    };
    let Some(installation) = repository
        .runtime_extension_installation(&endpoint.extension_id, installed.installed_release_id)
        .await?
    else {
        return Err(unavailable());
    };
    if !installation
        .capability_grants
        .iter()
        .any(|grant| grant == WEBHOOKS_RECEIVE_CAPABILITY)
    {
        return Err(unavailable());
    }
    let Some(webhook) = declared_webhook(&installation.manifest, &endpoint.webhook).cloned() else {
        return Err(unavailable());
    };

    let (parts, body) = request.into_parts();
    let declared_length = parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared_length.is_some_and(|length| length > webhook.max_body_bytes) {
        RequestOutcome::TooLarge.count();
        return Err(ApiError::payload_too_large());
    }
    let Ok(body) = axum::body::to_bytes(body, webhook.max_body_bytes as usize).await else {
        RequestOutcome::TooLarge.count();
        return Err(ApiError::payload_too_large());
    };
    let request = WebhookRequest::new(
        endpoint.webhook.clone(),
        endpoint.endpoint.clone(),
        parts.method.as_str().to_owned(),
        parts.uri.query().unwrap_or_default().to_owned(),
        parts
            .headers
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec())),
        body.to_vec(),
    );
    let outcome = match state
        .extension_runtime
        .receive_webhook(
            &installation,
            repository.for_extension(&endpoint.extension_id),
            request,
            webhook.max_body_bytes,
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            RequestOutcome::Failed.count();
            tracing::warn!(
                extension = %endpoint.extension_id,
                webhook = %endpoint.webhook,
                endpoint = %endpoint.endpoint,
                %error,
                "extension could not receive a webhook"
            );
            return Err(ApiError::internal(
                "the extension could not answer the webhook",
            ));
        }
    };
    let kept = match &outcome.keep {
        Some(kept) => Some(
            repository
                .keep_extension_webhook_payload(endpoint, &kept.payload, kept.dedupe_key.as_deref())
                .await?,
        ),
        None => None,
    };
    match kept {
        Some(KeptWebhookPayload::Stored(_)) => RequestOutcome::Kept.count(),
        Some(KeptWebhookPayload::Duplicate) => RequestOutcome::Duplicate.count(),
        None => RequestOutcome::Answered.count(),
    }
    Ok(reply(outcome))
}

/// The component's reply, which it has been checked to be valid, with the
/// host's own caching and sniffing headers.
fn reply(outcome: WebhookOutcome) -> Response {
    let reply = outcome.reply;
    let mut response = Response::new(Body::from(reply.body));
    *response.status_mut() =
        StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let headers = response.headers_mut();
    for (name, value) in reply.headers {
        if let (Ok(name), Ok(value)) = (
            header::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            headers.append(name, value);
        }
    }
    if !headers.contains_key(header::CONTENT_TYPE) {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.into_response()
}
