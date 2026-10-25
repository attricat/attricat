//! Export platform routes (#424): per-record export status and retry,
//! credential connections and their OAuth flows, request budgets, connector
//! resyncs, payload previews and public file links.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    AppState,
    auth::{self, ActiveWorkspace, AuthenticatedPrincipal, AuthenticatedSession, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    extension_runtime::connections::{self, TokenError},
    extensions::{
        MAX_RATE_LIMIT_REQUESTS, MAX_RATE_LIMIT_SECONDS, Manifest, RateLimit, validate_schema,
    },
    repository::{
        AttricatRepository, AuthorizationActor, EXPORT_STATUSES, ExportConnectionLabel,
        ExportDestinationDescriptor, ExportMapping, ExportRetrySelection, ExportStatusCount,
        ExtensionConnection, ExtensionConnectionPatch, ExtensionRuntimeInstallation,
        HandlerFailure, MAX_CONNECTOR_RECORD_FILTER, MAX_EXPORT_MAPPING_PAGE,
        NewExtensionConnection, RepositoryError, valid_connection_key,
    },
};

/// The most failed handler deliveries an export status page lists.
const HANDLER_FAILURE_LIMIT: i64 = 20;
const MAX_CONNECTION_TEXT_BYTES: usize = 2048;
/// Column limits of `extension_connections`.
const MAX_CONNECTION_NAME_CHARS: usize = 200;
const MAX_CLIENT_ID_CHARS: usize = 512;
const MAX_CONNECTION_URL_CHARS: usize = 2048;
const MAX_SECRET_INPUT_BYTES: usize = 8 * 1024;
const MAX_CONNECTION_PARAMETERS_BYTES: usize = 16 * 1024;

fn installed_manifest(value: &Value) -> Result<Manifest, ApiError> {
    serde::Deserialize::deserialize(value)
        .map_err(|_| ApiError::internal("installed extension manifest is invalid"))
}

/// The enabled, authorized runtime snapshot of an extension.
async fn runtime_installation(
    repository: &AttricatRepository,
    extension_id: &str,
) -> Result<ExtensionRuntimeInstallation, ApiError> {
    let installed = repository.installed_extension(extension_id).await?;
    repository
        .runtime_extension_installation(extension_id, installed.installed_release_id)
        .await?
        .ok_or_else(|| ApiError::conflict("the extension must be enabled and fully granted"))
}

fn token_error(error: TokenError) -> ApiError {
    match error {
        TokenError::Pending { .. } => ApiError::conflict("authorization is still pending"),
        TokenError::Rejected(message) | TokenError::Invalid(message) => {
            ApiError::invalid_input(message)
        }
        TokenError::Temporary(_) => ApiError::bad_gateway("the token endpoint is unavailable"),
        TokenError::Busy => ApiError::conflict("the connection is being refreshed; try again"),
    }
}

// ---- export status --------------------------------------------------------

#[derive(Serialize)]
pub(super) struct RecordExportStatus {
    statuses: Vec<ExportMapping>,
    destinations: Vec<ExportDestinationDescriptor>,
    /// The kind and display name of each connection a status names, so a
    /// status can be matched with its destination.
    connections: Vec<ExportConnectionLabel>,
}

/// Every extension's export status of one record, with the destinations of
/// the enabled export extensions.
pub(super) async fn record_export_status(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<RecordExportStatus>, ApiError> {
    let statuses = repository.export_mappings_for_record(record_id).await?;
    Ok(Json(RecordExportStatus {
        statuses,
        destinations: repository.enabled_export_destinations().await?,
        connections: repository
            .export_connection_labels_for_record(record_id)
            .await?,
    }))
}

#[derive(Serialize)]
pub(super) struct WorkspaceExportStatus {
    counts: Vec<ExportStatusCount>,
    destinations: Vec<ExportDestinationDescriptor>,
}

/// Status counts across every export extension, for the data-health card.
pub(super) async fn workspace_export_status(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<WorkspaceExportStatus>, ApiError> {
    Ok(Json(WorkspaceExportStatus {
        counts: repository.export_status_counts(None).await?,
        destinations: repository.enabled_export_destinations().await?,
    }))
}

#[derive(Deserialize)]
pub(super) struct ExportStatusQuery {
    #[serde(default)]
    connection: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    after: Option<Uuid>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Serialize)]
pub(super) struct ExtensionExportStatus {
    counts: Vec<ExportStatusCount>,
    mappings: Vec<ExportMapping>,
    next: Option<Uuid>,
    handler_failures: Vec<HandlerFailure>,
}

/// An extension's mappings and handler failures across the workspace. They
/// name records and carry destination IDs and errors, so the caller needs a
/// workspace-wide `records.read` grant as well as `extensions.read`.
pub(super) async fn extension_export_status(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(extension_id): ApiPath<String>,
    ApiQuery(query): ApiQuery<ExportStatusQuery>,
) -> Result<Json<ExtensionExportStatus>, ApiError> {
    if !state
        .repository
        .principal_may(principal.actor(), workspace.0, "records.read", None, None)
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let (mappings, next) = repository
        .export_mapping_list(
            &extension_id,
            query.connection.as_deref(),
            query.status.as_deref(),
            query.after,
            query.limit.unwrap_or(50).clamp(1, MAX_EXPORT_MAPPING_PAGE),
            true,
        )
        .await?;
    // Counts and failures describe the whole extension, so only the first
    // page computes them; later pages carry only mappings.
    let first = query.after.is_none();
    Ok(Json(ExtensionExportStatus {
        counts: if first {
            repository.export_status_counts(Some(&extension_id)).await?
        } else {
            Vec::new()
        },
        mappings,
        next,
        handler_failures: if first {
            repository
                .extension_handler_failures(&extension_id, HANDLER_FAILURE_LIMIT)
                .await?
        } else {
            Vec::new()
        },
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RetryRequest {
    #[serde(default)]
    connection: Option<String>,
    #[serde(default)]
    record_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    status: Option<String>,
    /// Also start a resync of just these records on every export job of the
    /// extension that targets the connection.
    #[serde(default)]
    run: bool,
    /// Makes the request idempotent: sent again as it was, it retries
    /// nothing twice and finds the runs it started.
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// Marks records for another push and notifies the extension; optionally
/// resyncs just those records at once, without rerunning whole jobs.
/// Like the status view, it names records selected by status, so it needs a
/// workspace-wide `records.read` grant too.
pub(super) async fn retry_export(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(extension_id): ApiPath<String>,
    ApiJson(input): ApiJson<RetryRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !state
        .repository
        .principal_may(principal.actor(), workspace.0, "records.read", None, None)
        .await?
    {
        return Err(ApiError::forbidden());
    }
    if input
        .idempotency_key
        .as_ref()
        .is_some_and(|key| key.is_empty() || key.len() > 40 || !key.is_ascii())
    {
        return Err(ApiError::invalid_input(
            "idempotency_key must be 1-40 ASCII bytes".into(),
        ));
    }
    // With a key, the request as sent (not the records a status resolves
    // to now) names the retry. A request sent again finds the records the
    // first one retried instead of retrying anything twice, and resyncs
    // them under the same run keys, which start each job's run only once.
    let request_selection = input.idempotency_key.as_ref().map(|_| {
        format!(
            "retry:{}:{}:{}",
            input.connection.as_deref().unwrap_or(""),
            input.status.as_deref().unwrap_or(""),
            record_ids_key(input.record_ids.as_deref().unwrap_or_default()).1
        )
    });
    let request_key = input
        .idempotency_key
        .as_ref()
        .zip(request_selection.as_ref())
        .map(|(key, selection)| {
            format!(
                "{:x}",
                Sha256::digest(format!("{extension_id}\0{key}\0{selection}").as_bytes())
            )
        });
    let record_ids = repository
        .request_export_retry(
            &extension_id,
            ExportRetrySelection {
                connection: input.connection.clone(),
                record_ids: input.record_ids,
                status: input.status,
                request_key,
            },
        )
        .await?;
    // The retry is recorded above. Each job resyncs only its blueprint's
    // records, and a job that cannot run is reported rather than failing a
    // retry that already happened.
    let mut run_ids = Vec::new();
    let mut job_errors = Vec::new();
    if input.run && !record_ids.is_empty() {
        let key = input
            .idempotency_key
            .clone()
            .unwrap_or_else(|| format!("retry-{}", Uuid::new_v4().simple()));
        for job in repository
            .extension_export_connector_jobs(&extension_id)
            .await?
            .into_iter()
            .filter(|job| job.connection == input.connection)
        {
            let ids = repository
                .records_of_blueprint(job.blueprint_id, &record_ids)
                .await?;
            if ids.is_empty() {
                continue;
            }
            match repository
                .resync_blueprint_connector_job(
                    job.id,
                    &key,
                    request_selection.as_deref(),
                    Some(ids),
                )
                .await
            {
                Ok(runs) => run_ids.extend(runs),
                Err(error) => {
                    tracing::warn!(job = %job.id, %error, "export retry resync could not start");
                    job_errors
                        .push(json!({"job_id": job.id, "message": resync_error_message(&error)}));
                }
            }
        }
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"record_ids": record_ids, "run_ids": run_ids, "job_errors": job_errors})),
    ))
}

/// Record IDs in one order without repeats, and how an idempotency key names
/// them, so a request sent again maps to the same run key.
fn record_ids_key(ids: &[Uuid]) -> (Vec<Uuid>, String) {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let key = ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(",");
    (ids, key)
}

/// What a client is told about a job whose retry resync could not start.
fn resync_error_message(error: &RepositoryError) -> String {
    match error {
        RepositoryError::InvalidExtension(message) => message.clone(),
        RepositoryError::NotFound(what) => format!("{what} not found"),
        _ => "the job could not start a resync".to_owned(),
    }
}

// ---- connections ----------------------------------------------------------

pub(super) async fn list_connections(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<Json<Vec<ExtensionConnection>>, ApiError> {
    let mut connections = repository.list_extension_connections(&extension_id).await?;
    // A pending device code lets whoever enters it bind the connection to
    // their own account, so only operators who may authorize see it.
    if !state
        .repository
        .principal_may(
            principal.actor(),
            workspace.0,
            "extensions.manage",
            None,
            None,
        )
        .await?
    {
        for connection in &mut connections {
            connection.device_user_code = None;
            connection.device_verification_uri = None;
        }
    }
    Ok(Json(
        connections.into_iter().map(with_redirect_uri).collect(),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateConnectionRequest {
    connection: String,
    kind: String,
    display_name: String,
    grant_type: String,
    token_url: String,
    #[serde(default)]
    device_authorization_url: Option<String>,
    /// An authorization-code connection's provider authorization endpoint,
    /// for browser authorization.
    #[serde(default)]
    authorization_url: Option<String>,
    client_id: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    parameters: Option<Value>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn bounded_text(value: &str, label: &str) -> Result<(), ApiError> {
    bounded_chars(value, label, MAX_CONNECTION_TEXT_BYTES)
}

/// Checks a connection field against its column's length limit, in
/// characters as the database counts them.
fn bounded_chars(value: &str, label: &str, max: usize) -> Result<(), ApiError> {
    if value.trim().is_empty() || value.chars().count() > max {
        return Err(ApiError::invalid_input(format!(
            "{label} must be 1-{max} characters"
        )));
    }
    Ok(())
}

fn bounded_secret(value: Option<&String>, label: &str) -> Result<(), ApiError> {
    if value.is_some_and(|value| value.is_empty() || value.len() > MAX_SECRET_INPUT_BYTES) {
        return Err(ApiError::invalid_input(format!(
            "{label} must be 1-{MAX_SECRET_INPUT_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Checks a provider authorization endpoint. Only the operator's browser
/// visits it, so it needs no host permission, but it must be a plain HTTPS
/// URL: Attricat appends the request's query itself.
fn check_authorization_url(grant_type: &str, url: &str) -> Result<(), ApiError> {
    if grant_type != "authorization_code" {
        return Err(ApiError::invalid_input(
            "authorization_url applies only to the authorization code grant".into(),
        ));
    }
    bounded_chars(url, "authorization_url", MAX_CONNECTION_URL_CHARS)?;
    url::Url::parse(url)
        .ok()
        .filter(|url| {
            url.scheme() == "https"
                && url.host().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        })
        .map(drop)
        .ok_or_else(|| {
            ApiError::invalid_input(
                "authorization_url must be an HTTPS URL without query or fragment".into(),
            )
        })
}

/// Adds the redirect URI an operator registers with the provider to an
/// authorization-code connection, when browser authorization is configured.
fn with_redirect_uri(mut connection: ExtensionConnection) -> ExtensionConnection {
    if connection.grant_type == "authorization_code" {
        connection.redirect_uri =
            connections::oauth_callback().map(|callback| callback.redirect_uri.to_string());
    }
    connection
}

/// Checks a token or device endpoint against the kind's host permission.
fn check_endpoint(manifest: &Manifest, rule_id: &str, url: &str) -> Result<(), ApiError> {
    bounded_chars(url, "endpoint URL", MAX_CONNECTION_URL_CHARS)?;
    let parsed = url::Url::parse(url)
        .map_err(|_| ApiError::invalid_input("endpoint URLs must be HTTPS URLs".into()))?;
    if !manifest
        .host_permission(rule_id)
        .is_some_and(|rule| rule.allows_request_with_query(&parsed, "POST"))
    {
        return Err(ApiError::invalid_input(
            "the endpoint is not covered by the connection kind's host permission".into(),
        ));
    }
    Ok(())
}

pub(super) async fn create_connection(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
    ApiJson(input): ApiJson<CreateConnectionRequest>,
) -> Result<(StatusCode, Json<ExtensionConnection>), ApiError> {
    let installed = repository.installed_extension(&extension_id).await?;
    let manifest = installed_manifest(&installed.manifest)?;
    let kind = manifest
        .server
        .as_ref()
        .and_then(|server| server.connections.iter().find(|kind| kind.id == input.kind))
        .ok_or_else(|| ApiError::invalid_input("unknown connection kind".into()))?;
    if !valid_connection_key(&input.connection) {
        return Err(ApiError::invalid_input(
            "connection names are 1-128 letters, digits, '.', '_' or '-'".into(),
        ));
    }
    if !kind
        .grant_types
        .iter()
        .any(|grant| grant.as_str() == input.grant_type)
    {
        return Err(ApiError::invalid_input(
            "the connection kind does not support that grant type".into(),
        ));
    }
    bounded_chars(
        input.display_name.trim(),
        "display_name",
        MAX_CONNECTION_NAME_CHARS,
    )?;
    bounded_chars(&input.client_id, "client_id", MAX_CLIENT_ID_CHARS)?;
    if let Some(scope) = &input.scope {
        bounded_text(scope, "scope")?;
    }
    bounded_secret(input.client_secret.as_ref(), "client_secret")?;
    bounded_secret(input.refresh_token.as_ref(), "refresh_token")?;
    check_endpoint(&manifest, &kind.host_permission, &input.token_url)?;
    match (&input.device_authorization_url, input.grant_type.as_str()) {
        (Some(url), "device_code") => check_endpoint(&manifest, &kind.host_permission, url)?,
        (None, "device_code") => {
            return Err(ApiError::invalid_input(
                "the device code grant needs device_authorization_url".into(),
            ));
        }
        (Some(_), _) => {
            return Err(ApiError::invalid_input(
                "device_authorization_url applies only to the device code grant".into(),
            ));
        }
        (None, _) => {}
    }
    if let Some(url) = &input.authorization_url {
        check_authorization_url(&input.grant_type, url)?;
    }
    if input.grant_type == "client_credentials" && input.client_secret.is_none() {
        return Err(ApiError::invalid_input(
            "the client credentials grant needs client_secret".into(),
        ));
    }
    let parameters = input.parameters.unwrap_or_else(|| json!({}));
    bounded_parameters(&parameters)?;
    if let Some(schema) = &kind.parameters_schema {
        validate_schema(schema, &parameters)
            .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    }
    let created = repository
        .create_extension_connection(NewExtensionConnection {
            extension_id,
            connection: input.connection,
            kind: input.kind,
            display_name: input.display_name.trim().to_owned(),
            grant_type: input.grant_type,
            host_permission_id: kind.host_permission.clone(),
            token_url: input.token_url,
            device_authorization_url: input.device_authorization_url,
            authorization_url: input.authorization_url,
            client_id: input.client_id,
            scope: input.scope,
            parameters,
            client_secret: input.client_secret,
            refresh_token: input.refresh_token,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(with_redirect_uri(created))))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateConnectionRequest {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    token_url: Option<String>,
    /// A device-code connection's authorization endpoint.
    #[serde(default)]
    device_authorization_url: Option<String>,
    /// An authorization-code connection's browser authorization endpoint;
    /// an empty string clears it.
    #[serde(default)]
    authorization_url: Option<String>,
    /// The requested scope; an empty string clears it.
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    parameters: Option<Value>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

/// Connection parameters are an object of at most 16 KiB, so they always fit
/// the host calls that return them.
fn bounded_parameters(parameters: &Value) -> Result<(), ApiError> {
    if !parameters.is_object()
        || serde_json::to_vec(parameters)
            .map_or(true, |bytes| bytes.len() > MAX_CONNECTION_PARAMETERS_BYTES)
    {
        return Err(ApiError::invalid_input(
            "parameters must be an object of at most 16 KiB".into(),
        ));
    }
    Ok(())
}

pub(super) async fn update_connection(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<UpdateConnectionRequest>,
) -> Result<Json<ExtensionConnection>, ApiError> {
    let current = repository
        .extension_connection(&extension_id, &connection)
        .await?;
    let installed = repository.installed_extension(&extension_id).await?;
    let manifest = installed_manifest(&installed.manifest)?;
    if let Some(name) = &input.display_name {
        bounded_chars(name.trim(), "display_name", MAX_CONNECTION_NAME_CHARS)?;
    }
    if let Some(scope) = input.scope.as_ref().filter(|scope| !scope.is_empty()) {
        bounded_text(scope, "scope")?;
    }
    if let Some(url) = &input.token_url {
        check_endpoint(&manifest, &current.host_permission_id, url)?;
    }
    if let Some(url) = &input.device_authorization_url {
        if current.grant_type != "device_code" {
            return Err(ApiError::invalid_input(
                "device_authorization_url applies only to the device code grant".into(),
            ));
        }
        check_endpoint(&manifest, &current.host_permission_id, url)?;
    }
    if let Some(url) = input
        .authorization_url
        .as_ref()
        .filter(|url| !url.is_empty())
    {
        check_authorization_url(&current.grant_type, url)?;
    }
    bounded_secret(input.client_secret.as_ref(), "client_secret")?;
    bounded_secret(input.refresh_token.as_ref(), "refresh_token")?;
    if let Some(parameters) = &input.parameters {
        bounded_parameters(parameters)?;
        if let Some(schema) = manifest.server.as_ref().and_then(|server| {
            server
                .connections
                .iter()
                .find(|kind| kind.id == current.kind)
                .and_then(|kind| kind.parameters_schema.as_ref())
        }) {
            validate_schema(schema, parameters)
                .map_err(|error| ApiError::invalid_input(error.to_string()))?;
        }
    }
    Ok(Json(
        repository
            .update_extension_connection(
                &extension_id,
                &connection,
                ExtensionConnectionPatch {
                    display_name: input.display_name.map(|name| name.trim().to_owned()),
                    token_url: input.token_url,
                    device_authorization_url: input.device_authorization_url.map(Some),
                    authorization_url: input
                        .authorization_url
                        .map(|url| (!url.is_empty()).then_some(url)),
                    scope: input
                        .scope
                        .map(|scope| (!scope.is_empty()).then_some(scope)),
                    parameters: input.parameters,
                    client_secret: input.client_secret,
                    refresh_token: input.refresh_token,
                },
            )
            .await?,
    ))
}

pub(super) async fn delete_connection(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    if repository
        .delete_extension_connection(&extension_id, &connection)
        .await?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("extension connection"))
    }
}

pub(super) async fn start_device_authorization(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
) -> Result<Json<ExtensionConnection>, ApiError> {
    let installation = runtime_installation(&repository, &extension_id).await?;
    Ok(Json(
        connections::start_device_authorization(&repository, &installation, &connection)
            .await
            .map_err(token_error)?,
    ))
}

pub(super) async fn poll_device_authorization(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
) -> Result<Json<ExtensionConnection>, ApiError> {
    let installation = runtime_installation(&repository, &extension_id).await?;
    Ok(Json(
        connections::poll_device_authorization(&repository, &installation, &connection)
            .await
            .map_err(token_error)?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AuthorizationCodeRequest {
    code: String,
    redirect_uri: String,
    #[serde(default)]
    code_verifier: Option<String>,
}

pub(super) async fn exchange_authorization_code(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<AuthorizationCodeRequest>,
) -> Result<Json<ExtensionConnection>, ApiError> {
    bounded_text(&input.code, "code")?;
    bounded_text(&input.redirect_uri, "redirect_uri")?;
    if let Some(verifier) = &input.code_verifier {
        bounded_text(verifier, "code_verifier")?;
    }
    let installation = runtime_installation(&repository, &extension_id).await?;
    Ok(Json(
        connections::exchange_authorization_code(
            &repository,
            &installation,
            &connection,
            &input.code,
            &input.redirect_uri,
            input.code_verifier.as_deref(),
        )
        .await
        .map_err(token_error)?,
    ))
}

#[derive(Serialize)]
pub(super) struct BrowserAuthorization {
    /// Where to send the operator to approve the app at the provider.
    authorization_url: String,
}

/// Starts a browser authorization of an authorization-code connection. The
/// provider sends the operator back to the public OAuth callback, which
/// completes it only in this same signed-in browser session, so it cannot be
/// started with an API token.
pub(super) async fn start_browser_authorization(
    ScopedRepository(repository): ScopedRepository,
    session: Result<AuthenticatedSession, ApiError>,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
) -> Result<Json<BrowserAuthorization>, ApiError> {
    let AuthenticatedSession(session) = session.map_err(|_| {
        ApiError::invalid_input("start browser authorization from a signed-in browser".into())
    })?;
    let installation = runtime_installation(&repository, &extension_id).await?;
    let url = connections::start_browser_authorization(
        &repository,
        &installation,
        &connection,
        &connections::authorization_session_binding(session.as_bytes()),
    )
    .await
    .map_err(token_error)?;
    Ok(Json(BrowserAuthorization {
        authorization_url: url.into(),
    }))
}

/// The provider's redirect after an operator approved or declined an app.
/// Providers may add parameters of their own, such as `iss` or `scope`.
#[derive(Deserialize)]
pub(super) struct OAuthCallbackQuery {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// `GET /public/extension-oauth/callback`, a public route. The single-use
/// `state` names the pending authorization and its workspace. The browser
/// must still carry the session that started it, valid for that workspace
/// and allowed to manage extensions; only then is the request taken and
/// the code exchanged with its PKCE verifier. The browser is sent back to
/// the extension's management page with `authorization=connected` or
/// `failed`, or to the extension list with `expired` (unknown, used, expired
/// or another session's state) or `signed_out` (no valid session). Failure
/// details are stored as the connection's last error, never put in the URL.
pub(super) async fn oauth_callback(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<OAuthCallbackQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let callback =
        connections::oauth_callback().ok_or_else(|| ApiError::not_found("OAuth callback"))?;
    let redirect = |extension_id: Option<&str>, connection: Option<&str>, outcome: &str| {
        let mut url = callback.return_base.clone();
        if let Some(extension_id) = extension_id {
            // Extension IDs are URL-safe: letters, digits, '.', '_' and '-'.
            url = url.join(extension_id).unwrap_or(url);
        }
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(connection) = connection {
                pairs.append_pair("connection", connection);
            }
            pairs.append_pair("authorization", outcome);
        }
        Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(axum::http::header::LOCATION, url.as_str())
            .header(axum::http::header::CACHE_CONTROL, "no-store")
            .header("referrer-policy", "no-referrer")
            .body(axum::body::Body::empty())
            .map_err(|_| ApiError::internal("OAuth callback redirect"))
    };
    let Some(oauth_state) = query
        .state
        .as_deref()
        .filter(|value| !value.is_empty() && value.len() <= 128)
    else {
        return redirect(None, None, "expired");
    };
    let digest = connections::authorization_state_digest(oauth_state);
    let Some((workspace_id, extension_id, connection)) = state
        .repository
        .resolve_extension_authorization_state(&digest)
        .await?
    else {
        return redirect(None, None, "expired");
    };
    // The callback is public, so the session is checked here as the
    // authorization layer would: live, for this workspace, and still allowed
    // to manage extensions. Nothing is changed for a browser without one.
    let Some(session) = auth::session_cookie_digest(&headers) else {
        return redirect(None, None, "signed_out");
    };
    let operator = match state.repository.validate_browser_session(&session).await? {
        Some(operator) if operator.workspace_id == workspace_id => operator,
        _ => return redirect(None, None, "signed_out"),
    };
    if !state
        .repository
        .principal_may(
            AuthorizationActor {
                user_id: operator.user_id,
                token_id: None,
            },
            workspace_id,
            "extensions.manage",
            None,
            None,
        )
        .await?
    {
        return redirect(None, None, "signed_out");
    }
    let binding = connections::authorization_session_binding(session.as_bytes());
    let repository = state.repository.for_workspace(workspace_id);
    let code = query
        .code
        .as_deref()
        .filter(|code| !code.is_empty() && code.len() <= MAX_CONNECTION_TEXT_BYTES);
    let failure = match (code, query.error.as_deref()) {
        (Some(code), None) => match runtime_installation(&repository, &extension_id).await {
            Ok(installation) => match connections::complete_browser_authorization(
                &repository,
                &installation,
                &connection,
                oauth_state,
                &binding,
                code,
            )
            .await
            {
                Ok(Some(_)) => {
                    return redirect(Some(&extension_id), Some(&connection), "connected");
                }
                // Not pending for this session: change nothing.
                Ok(None) => return redirect(None, None, "expired"),
                Err(error) => error.to_string(),
            },
            Err(_) => "the extension must be enabled and fully granted".to_owned(),
        },
        (_, error) => {
            // The request is used up either way, so a replayed redirect
            // cannot complete it later. Another session's request stays.
            if repository
                .take_extension_authorization_request(&extension_id, &connection, &digest, &binding)
                .await?
                .is_none()
            {
                return redirect(None, None, "expired");
            }
            // Only a short, plain error code from the provider is kept.
            let reason = error
                .filter(|error| {
                    error.len() <= 64
                        && error
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                })
                .unwrap_or("invalid_response");
            format!("the provider did not authorize the app ({reason})")
        }
    };
    repository
        .fail_extension_authorization(&extension_id, &connection, &failure)
        .await?;
    redirect(Some(&extension_id), Some(&connection), "failed")
}

/// Obtains (or reuses) an access token, to check a connection.
pub(super) async fn test_connection(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, connection)): ApiPath<(String, String)>,
) -> Result<Json<ExtensionConnection>, ApiError> {
    let installation = runtime_installation(&repository, &extension_id).await?;
    connections::access_token(&repository, &installation, &connection)
        .await
        .map_err(token_error)?;
    Ok(Json(
        repository
            .extension_connection(&extension_id, &connection)
            .await?,
    ))
}

// ---- request budgets ------------------------------------------------------

#[derive(Serialize)]
pub(super) struct HostPermissionBudget {
    host_permission_id: String,
    declared: Option<RateLimit>,
    lowered: Option<LoweredBudget>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoweredBudget {
    requests: i32,
    per_seconds: i32,
}

pub(super) async fn list_budgets(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<Json<Vec<HostPermissionBudget>>, ApiError> {
    let installed = repository.installed_extension(&extension_id).await?;
    let manifest = installed_manifest(&installed.manifest)?;
    let lowered = repository
        .extension_host_permission_budgets(&extension_id)
        .await?;
    Ok(Json(
        manifest
            .host_permissions
            .iter()
            .chain(&manifest.optional_host_permissions)
            .map(|rule| HostPermissionBudget {
                host_permission_id: rule.id.clone(),
                declared: rule.rate_limit,
                lowered: lowered.iter().find(|(id, ..)| id == &rule.id).map(
                    |(_, requests, per_seconds)| LoweredBudget {
                        requests: *requests,
                        per_seconds: *per_seconds,
                    },
                ),
            })
            .collect(),
    ))
}

/// Lowers a rule's budget; it can never exceed the declared one.
pub(super) async fn set_budget(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, host_permission_id)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<LoweredBudget>,
) -> Result<StatusCode, ApiError> {
    let installed = repository.installed_extension(&extension_id).await?;
    let manifest = installed_manifest(&installed.manifest)?;
    let rule = manifest
        .host_permission(&host_permission_id)
        .ok_or_else(|| ApiError::not_found("host permission"))?;
    let Some(declared) = rule.rate_limit else {
        return Err(ApiError::invalid_input(
            "only a rule with a declared budget can be lowered".into(),
        ));
    };
    if input.requests < 1
        || input.requests as u32 > MAX_RATE_LIMIT_REQUESTS
        || input.per_seconds < 1
        || input.per_seconds as u32 > MAX_RATE_LIMIT_SECONDS
        || u64::from(input.requests as u32) * u64::from(declared.per_seconds)
            > u64::from(declared.requests) * u64::from(input.per_seconds as u32)
    {
        return Err(ApiError::invalid_input(
            "a lowered budget must be positive and no higher than the declared one".into(),
        ));
    }
    repository
        .set_extension_host_permission_budget(
            &extension_id,
            &host_permission_id,
            Some((input.requests, input.per_seconds)),
        )
        .await?;
    crate::extension_runtime::forget_lowered_budget(
        repository.extension_workspace(),
        &extension_id,
        &host_permission_id,
    );
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn clear_budget(
    ScopedRepository(repository): ScopedRepository,
    ApiPath((extension_id, host_permission_id)): ApiPath<(String, String)>,
) -> Result<StatusCode, ApiError> {
    repository
        .set_extension_host_permission_budget(&extension_id, &host_permission_id, None)
        .await?;
    crate::extension_runtime::forget_lowered_budget(
        repository.extension_workspace(),
        &extension_id,
        &host_permission_id,
    );
    Ok(StatusCode::NO_CONTENT)
}

// ---- resync ---------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResyncRequest {
    idempotency_key: String,
    /// Only these records; otherwise every published record.
    #[serde(default)]
    record_ids: Option<Vec<Uuid>>,
    /// Only records with this export status on the job's connection.
    #[serde(default)]
    status: Option<String>,
}

/// Re-exports every published record of a job, or a filtered set, to its
/// destination. The component skips records whose content hash is unchanged.
pub(super) async fn resync_connector_job(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<ResyncRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    // The idempotency key covers the request's selection, not the records
    // a status resolves to now, so a retried request finds its run.
    let selection = match (&input.record_ids, &input.status) {
        (Some(ids), None) => Some(format!("records:{}", record_ids_key(ids).1)),
        (None, Some(status)) => Some(format!("status:{status}")),
        _ => None,
    };
    let record_ids = match (input.record_ids, input.status) {
        (Some(_), Some(_)) => {
            return Err(ApiError::invalid_input(
                "filter a resync by record IDs or by status, not both".into(),
            ));
        }
        // The run selects each record once, as its idempotency key does.
        (Some(ids), None) => Some(record_ids_key(&ids).0),
        (None, Some(status)) => {
            if !EXPORT_STATUSES.contains(&status.as_str()) {
                return Err(ApiError::invalid_input("unknown export status".into()));
            }
            let job = repository
                .blueprint_connector_job(id)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint connector job"))?;
            let ids = repository
                .export_status_record_ids(
                    &job.extension_id,
                    job.connection.as_deref(),
                    &status,
                    job.blueprint_id,
                    MAX_CONNECTOR_RECORD_FILTER as i64,
                )
                .await?
                .ok_or_else(|| {
                    ApiError::invalid_input(format!(
                        "more than {MAX_CONNECTOR_RECORD_FILTER} records have this status; resync the whole job instead"
                    ))
                })?;
            if ids.is_empty() {
                return Ok((StatusCode::OK, Json(json!({"run_ids": []}))));
            }
            Some(ids)
        }
        (None, None) => None,
    };
    let runs = repository
        .resync_blueprint_connector_job(
            id,
            &input.idempotency_key,
            selection.as_deref(),
            record_ids,
        )
        .await?;
    Ok((StatusCode::ACCEPTED, Json(json!({"run_ids": runs}))))
}

// ---- preview --------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewRequest {
    record_id: Uuid,
    #[serde(default)]
    connection: Option<String>,
}

/// Returns the destination payload an export would push for one record,
/// with any validation issues, without pushing it. The preview command runs
/// with the extension's catalog access, so it needs a caller who can read the
/// whole workspace, and only a connection of the export's kind.
pub(super) async fn preview_export(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath((extension_id, export_id)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<PreviewRequest>,
) -> Result<Json<Value>, ApiError> {
    // Without a target only a workspace-wide grant matches, so the command
    // cannot read anything the caller could not.
    if !state
        .repository
        .principal_may(principal.actor(), workspace.0, "records.read", None, None)
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let installation = runtime_installation(&repository, &extension_id).await?;
    // A preview is a client command, so it needs the same grant.
    if !installation.capability_grants.contains("client.commands") {
        return Err(ApiError::forbidden());
    }
    let server = installation
        .manifest
        .server
        .as_ref()
        .ok_or_else(|| ApiError::not_found("export destination"))?;
    let export = server
        .exports
        .iter()
        .find(|export| export.id == export_id)
        .ok_or_else(|| ApiError::not_found("export destination"))?;
    let command_id = export
        .preview_command
        .as_ref()
        .ok_or_else(|| ApiError::not_found("export preview"))?;
    let command = server
        .commands
        .iter()
        .find(|command| &command.id == command_id)
        .ok_or_else(|| ApiError::not_found("export preview"))?;
    if let Some(connection) = &input.connection {
        let found = repository
            .extension_connection(&extension_id, connection)
            .await
            .map_err(|_| ApiError::not_found("extension connection"))?;
        if export
            .connection
            .as_ref()
            .is_none_or(|kind| *kind != found.kind)
        {
            return Err(ApiError::invalid_input(
                "the connection is not a destination of this export".into(),
            ));
        }
    }
    let payload = json!({
        "export_id": export_id,
        "record_id": input.record_id,
        "connection": input.connection,
    });
    let response = state
        .extension_runtime
        .invoke_command(
            &installation,
            repository.for_extension(&extension_id),
            &command.handler,
            &payload.to_string(),
            command.max_response_bytes,
        )
        .await
        .map_err(|_| {
            tracing::warn!(extension = %extension_id, export = %export_id, "export preview failed");
            ApiError::service_unavailable("export preview failed")
        })?;
    let response: Value = serde_json::from_str(&response)
        .map_err(|_| ApiError::service_unavailable("export preview returned invalid JSON"))?;
    validate_schema(&command.response_schema, &response)
        .map_err(|_| ApiError::service_unavailable("export preview returned invalid data"))?;
    Ok(Json(response))
}

// ---- public file links ----------------------------------------------------

/// Serves a file through an extension-issued public link. The link stays
/// valid only while it is unexpired and unrevoked, the extension is enabled
/// with `attricat.files.public_links` and `attricat.files.read`, and the live record it was issued for
/// still references the file.
pub(super) async fn public_file(
    State(state): State<AppState>,
    ApiPath(token): ApiPath<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if token.is_empty() || token.len() > 128 {
        return Err(ApiError::not_found("file"));
    }
    let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
    let Some((workspace_id, file_id, extension_id, record_id)) = state
        .repository
        .resolve_extension_file_link(&digest)
        .await?
    else {
        return Err(ApiError::not_found("file"));
    };
    let repository = state.repository.for_workspace(workspace_id);
    let installed = match repository.installed_extension(&extension_id).await {
        Ok(installed) => installed,
        Err(RepositoryError::NotFound(_)) => return Err(ApiError::not_found("file")),
        Err(error) => return Err(error.into()),
    };
    let allowed = repository
        .runtime_extension_installation(&extension_id, installed.installed_release_id)
        .await?
        // A link keeps needing every grant it was issued under.
        .is_some_and(|installation| {
            crate::repository::PUBLIC_FILE_LINK_GRANTS
                .iter()
                .all(|grant| installation.capability_grants.contains(*grant))
        });
    if !allowed
        || !repository
            .file_read_targets(file_id)
            .await?
            .iter()
            .any(|target| target.record_id == record_id)
    {
        return Err(ApiError::not_found("file"));
    }
    let file = repository.file_object(file_id, None).await?;
    super::files::download(&state, file, &headers).await
}
