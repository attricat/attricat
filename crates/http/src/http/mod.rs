mod agents;
mod audit;
mod audit_events;
mod auth;
mod blueprints;
mod contexts;
mod data_health;
mod entities;
mod entity_reads;
mod error;
mod event_deliveries;
mod extension_registries;
mod extensions;
mod extractors;
mod files;
mod members;
mod presentation_assets;
mod reusable_attributes;
mod roles;
mod rules;
mod saved_views;
mod sessions;
mod solution_packs;
mod tokens;
mod workflows;
mod workspace_navigation;

use self::error::ApiError;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    agents::AgentProviderConfig,
    extension_registry::{GitHubRegistry, GitHubRepository},
    extension_runtime::ExtensionRuntime,
    extensions::MAX_EXTENSION_ARCHIVE_BYTES,
    file_access::FileAccessPolicy,
    mail::MailDelivery,
    repository::CatalogRepository,
    solution_packs::MAX_SOLUTION_PACK_ARCHIVE_BYTES,
    storage::ObjectStore,
    telemetry::{register_request_timing, unregister_request_timing},
};
use axum::{
    Router,
    extract::State,
    http::{HeaderName, HeaderValue},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use metrics_exporter_prometheus::PrometheusHandle;
use serde_json::Value;
use tokio::sync::{Mutex, Semaphore};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{Instrument, field::Empty};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub repository: CatalogRepository,
    pub agent_provider: Option<AgentProviderConfig>,
    pub registry: Arc<GitHubRegistry>,
    pub official_registry: GitHubRepository,
    /// Storage is injected at startup so future file routes never construct a
    /// provider client from request data.
    pub object_store: Arc<dyn ObjectStore>,
    pub extension_runtime: ExtensionRuntime,
    pub file_access_policy: Arc<dyn FileAccessPolicy>,
    pub mail_delivery: Arc<dyn MailDelivery>,
    pub password_reset_url: String,
    pub workspace_invitation_url: String,
    pub workspace_onboarding_url: String,
    pub metrics: PrometheusHandle,
    // Preview expansion is request-controlled, so these limits keep cyclic or
    // high-cardinality relationship graphs from turning one read into an
    // unbounded amount of database work.
    pub max_preview_relationship_depth: u8,
    pub max_preview_relationship_items: u32,
    pub max_entity_page_size: u32,
    pub max_incoming_relationship_page_size: u32,
    pub max_relationship_facet_nodes: u32,
    /// Independently enforced while multipart fields stream to temporary storage.
    pub max_upload_file_bytes: u64,
    pub max_upload_files: usize,
    pub data_health_cache_ttl_seconds: u64,
    pub data_health_cache: DataHealthCache,
    pub session_cookie_secure: bool,
    pub allow_trusted_headers: bool,
    /// Bounds in-flight requests before expensive extractors or handlers run.
    pub request_permits: Arc<Semaphore>,
    pub request_timeout: Duration,
    pub default_body_limit: usize,
    /// Enables sanitized development-only timing phases for the Explorer.
    pub devtools_enabled: bool,
}

pub type DataHealthCache = Arc<Mutex<HashMap<String, (Instant, Value)>>>;

const TIMING_PHASES: [&str; 4] = ["candidate", "page", "related", "serialize"];
/// Only Explorer requests publish development SQL and phase breakdowns.
/// Other API responses retain the standard aggregate `app` timing.
const EXPLORER_TIMING_ROUTES: [&str; 2] = [
    "/v1/entities/search",
    "/v1/entities/facets/relationship-tree/children",
];
/// SQL breakdowns are limited to Explorer result and facet retrieval.
const SQL_TIMING_LABELS: [&str; 3] = ["relationship-facet", "entities-page", "related-hydrate"];

/// Request-local, aggregate timings. Its API only permits fixed metric names,
/// so headers cannot accidentally contain SQL, identifiers, parameters, or bodies.
#[derive(Clone)]
pub(crate) struct RequestTiming {
    enabled: bool,
    phases: Arc<std::sync::Mutex<Vec<(&'static str, f64)>>>,
    sql: Arc<std::sync::Mutex<SqlTiming>>,
}

#[derive(Default)]
struct SqlTiming {
    duration_ms: f64,
    queries: u32,
    labels: HashMap<&'static str, (f64, u32)>,
}

impl RequestTiming {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            phases: Arc::new(std::sync::Mutex::new(Vec::new())),
            sql: Arc::new(std::sync::Mutex::new(SqlTiming::default())),
        }
    }

    pub(super) fn record(&self, phase: &'static str, started: Instant) {
        if !self.enabled || !TIMING_PHASES.contains(&phase) {
            return;
        }
        self.phases
            .lock()
            .expect("timing lock is not poisoned")
            .push((phase, started.elapsed().as_secs_f64() * 1_000.0));
    }

    pub(crate) fn record_sql(&self, label: Option<&str>, duration_ms: f64) {
        if !self.enabled {
            return;
        }
        let Some(label) = label.and_then(|request_label| {
            SQL_TIMING_LABELS
                .iter()
                .find(|allowed| **allowed == request_label)
        }) else {
            return;
        };
        let mut sql = self.sql.lock().expect("timing lock is not poisoned");
        sql.duration_ms += duration_ms;
        sql.queries += 1;
        let entry = sql.labels.entry(*label).or_default();
        entry.0 += duration_ms;
        entry.1 += 1;
    }

    fn server_timing(&self) -> String {
        let mut timings = self
            .phases
            .lock()
            .expect("timing lock is not poisoned")
            .iter()
            .map(|(phase, duration)| format!("{phase};dur={duration:.2}"))
            .collect::<Vec<_>>();
        let sql = self.sql.lock().expect("timing lock is not poisoned");
        if sql.queries > 0 {
            timings.push(format!(
                "sql;dur={:.2};desc=queries-{}",
                sql.duration_ms, sql.queries
            ));
            for label in SQL_TIMING_LABELS {
                if let Some((duration, queries)) = sql.labels.get(label) {
                    timings.push(format!(
                        "sql-{label};dur={duration:.2};desc=queries-{queries}"
                    ));
                }
            }
        }
        timings.join(", ")
    }
}

async fn request_limits(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Ok(_permit) = state.request_permits.clone().try_acquire_owned() else {
        return ApiError::service_unavailable("server is at request capacity").into_response();
    };
    match tokio::time::timeout(state.request_timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (axum::http::StatusCode::REQUEST_TIMEOUT, "request timed out").into_response(),
    }
}

fn canonical_route(route: &str) -> &str {
    route
        .strip_prefix("/api")
        .filter(|suffix| suffix.starts_with('/'))
        .unwrap_or(route)
}

fn request_id(request: &axum::extract::Request) -> Uuid {
    request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Uuid>().ok())
        .unwrap_or_else(Uuid::new_v4)
}

async fn server_timing(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| canonical_route(path.as_str()).to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    // Use one validated ID for the response, request logs, and mutation audits.
    let request_id = request_id(&request);
    let timing = RequestTiming::new(
        state.devtools_enabled && EXPLORER_TIMING_ROUTES.contains(&route.as_str()),
    );
    request.extensions_mut().insert(request_id);
    request.extensions_mut().insert(timing.clone());
    let span = tracing::info_span!(
        "http.request",
        method = %method,
        route = %route,
        request_id = %request_id,
        status = Empty,
        duration_ms = Empty
    );
    // Spans have no ID when tracing is disabled (including some integration-test
    // subscribers). Timing remains optional in that case.
    let timing_span = span.id();
    if let Some(id) = &timing_span {
        register_request_timing(id.clone(), timing.clone());
    }
    let started_at = Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let status = response.status().as_u16();
    let duration_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
    span.record("status", status)
        .record("duration_ms", duration_ms);
    metrics::counter!("catalog_http_requests_total", "method" => method.clone(), "route" => route.clone(), "status" => status.to_string()).increment(1);
    metrics::histogram!("catalog_http_request_duration_seconds", "method" => method, "route" => route).record(duration_ms / 1_000.0);
    if status >= 500 {
        tracing::error!(parent: &span, status, duration_ms, "request failed");
    } else {
        tracing::info!(parent: &span, status, duration_ms, "request completed");
    }
    let duration_ms = started_at.elapsed().as_secs_f64() * 1_000.0;
    let server_timing = HeaderName::from_static("server-timing");
    let phases = timing.server_timing();
    let value = match response
        .headers()
        .get(&server_timing)
        .and_then(|value| value.to_str().ok())
    {
        Some(existing) if phases.is_empty() => format!("{existing}, app;dur={duration_ms:.2}"),
        Some(existing) => format!("{existing}, {phases}, app;dur={duration_ms:.2}"),
        None if phases.is_empty() => format!("app;dur={duration_ms:.2}"),
        None => format!("{phases}, app;dur={duration_ms:.2}"),
    };
    response.headers_mut().insert(
        HeaderName::from_static("x-request-id"),
        HeaderValue::from_str(&request_id.to_string()).expect("UUID is a valid header value"),
    );
    response.headers_mut().insert(
        server_timing,
        HeaderValue::from_str(&value).expect("server timing values are valid header values"),
    );
    if let Some(id) = timing_span {
        unregister_request_timing(id);
    }
    response
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/metrics", get(metrics))
        .route(
            "/agent/conversations",
            get(agents::list_conversations).post(agents::create_conversation),
        )
        .route(
            "/agent/conversations/search",
            get(agents::search_conversations),
        )
        .route(
            "/agent/conversations/{conversation_id}",
            get(agents::get_conversation)
                .patch(agents::update_conversation)
                .delete(agents::delete_conversation),
        )
        .route(
            "/agent/conversations/{conversation_id}/messages",
            get(agents::list_messages).post(agents::send_message),
        )
        .route(
            "/agent/conversations/{conversation_id}/uploads",
            post(files::upload_conversation),
        )
        .route(
            "/agent/conversations/{conversation_id}/runs",
            get(agents::list_runs),
        )
        .route("/agent/runs/{run_id}/events", get(agents::stream_events))
        .route("/agent/approvals", get(agents::list_pending_approvals))
        .route(
            "/agent/tool-calls/{tool_call_id}/approve",
            post(agents::approve),
        )
        .route(
            "/agent/tool-calls/{tool_call_id}/reject",
            post(agents::reject),
        )
        // `/health` remains the compatibility liveness probe. Readiness is
        // separate so load balancers withdraw an unhealthy dependency graph
        // without restarting an otherwise live process.
        .route("/health", get(data_health::liveness))
        .route("/health/live", get(data_health::liveness))
        .route("/health/ready", get(data_health::readiness))
        .route(
            "/extension-registries",
            get(extension_registries::list).post(extension_registries::create),
        )
        .route(
            "/extension-registries/discover",
            get(extension_registries::discover),
        )
        .route(
            "/extension-registries/extensions/{owner}/{repository}",
            get(extension_registries::extension_details),
        )
        .route(
            "/extension-registries/{id}",
            axum::routing::delete(extension_registries::remove),
        )
        .route(
            "/extensions",
            get(extensions::list).post(extensions::install),
        )
        .route(
            "/extensions/sideload",
            post(extensions::sideload).layer(axum::extract::DefaultBodyLimit::max(
                MAX_EXTENSION_ARCHIVE_BYTES,
            )),
        )
        .route("/presentation-assets", get(presentation_assets::list))
        .route(
            "/presentation-assets/{asset_id}",
            get(presentation_assets::get),
        )
        .route(
            "/presentation-assets/{asset_id}/content",
            get(presentation_assets::content),
        )
        .route(
            "/solution-packs/inspect",
            post(solution_packs::inspect).layer(axum::extract::DefaultBodyLimit::max(
                MAX_SOLUTION_PACK_ARCHIVE_BYTES,
            )),
        )
        .route(
            "/solution-packs/plans",
            post(solution_packs::create_plan).layer(axum::extract::DefaultBodyLimit::max(
                MAX_SOLUTION_PACK_ARCHIVE_BYTES + 128 * 1024,
            )),
        )
        .route(
            "/solution-packs/plans/{plan_id}",
            get(solution_packs::get_plan),
        )
        .route(
            "/solution-packs/plans/{plan_id}/apply",
            post(solution_packs::apply_plan),
        )
        .route(
            "/solution-packs/applications",
            get(solution_packs::list_applications),
        )
        .route(
            "/solution-packs/applications/{application_id}",
            get(solution_packs::get_application),
        )
        .route(
            "/solution-packs/applications/{application_id}/abandon",
            post(solution_packs::abandon_application),
        )
        .route(
            "/solution-packs/applications/{application_id}/checks",
            get(solution_packs::list_check_runs).post(solution_packs::rerun_checks),
        )
        .route(
            "/solution-packs/applications/{application_id}/checks/{run_id}",
            get(solution_packs::get_check_run),
        )
        .route("/extensions/runtime", get(extensions::runtime))
        .route(
            "/extension-operation-runs",
            get(extensions::list_operation_runs),
        )
        .route(
            "/extension-operation-runs/{id}",
            get(extensions::get_operation_run),
        )
        .route(
            "/extension-operation-runs/{run_id}/artifacts",
            get(extensions::list_operation_artifacts),
        )
        .route(
            "/extension-operation-runs/{run_id}/deliveries",
            get(extensions::list_operation_deliveries),
        )
        .route(
            "/extension-operation-runs/{run_id}/artifacts/{artifact_id}/download",
            get(extensions::download_operation_artifact),
        )
        .route(
            "/extension-operation-runs/{id}/cancel",
            post(extensions::cancel_operation),
        )
        .route(
            "/extension-operation-runs/{id}/replay",
            post(extensions::replay_operation),
        )
        .route(
            "/workspace/extensions-mode",
            put(extensions::set_workspace_mode),
        )
        .route(
            "/workspace/extension-secrets",
            get(extensions::list_workspace_secrets),
        )
        .route(
            "/workspace/extension-secrets/{name}",
            put(extensions::put_workspace_secret).delete(extensions::delete_workspace_secret),
        )
        .route(
            "/extensions/{extension_id}",
            get(extensions::detail).delete(extensions::remove),
        )
        .route(
            "/extensions/{extension_id}/upgrade",
            post(extensions::upgrade),
        )
        .route(
            "/extensions/{extension_id}/configure",
            put(extensions::configure),
        )
        .route("/extensions/{extension_id}/grants", post(extensions::grant))
        .route(
            "/extensions/{extension_id}/grants/{grant_kind}/{grant_id}",
            axum::routing::delete(extensions::revoke),
        )
        .route(
            "/extensions/{extension_id}/enable",
            post(extensions::enable),
        )
        .route(
            "/extensions/{extension_id}/disable",
            post(extensions::disable),
        )
        .route(
            "/extensions/{extension_id}/quarantine",
            post(extensions::quarantine),
        )
        .route(
            "/extensions/{extension_id}/operations",
            post(extensions::start_operation),
        )
        .route(
            "/extensions/{extension_id}/operation-schedules",
            post(extensions::create_operation_schedule),
        )
        .route(
            "/blueprints/{blueprint_id}/connector-jobs",
            get(extensions::list_blueprint_connector_jobs),
        )
        .route(
            "/blueprint-connector-jobs/{id}/run",
            post(extensions::run_blueprint_connector_job),
        )
        .route(
            "/extension-operation-schedules",
            get(extensions::list_operation_schedules),
        )
        .route(
            "/extension-operation-schedules/{id}",
            axum::routing::patch(extensions::update_operation_schedule),
        )
        .route(
            "/extensions/{extension_id}/{contribution_id}/command",
            post(extensions::command),
        )
        .route(
            "/extensions/{extension_id}/{contribution_id}/storage/{release_id}",
            post(extensions::storage),
        )
        .route(
            "/extensions/{extension_id}/{contribution_id}/artifact",
            get(extensions::artifact),
        )
        .route("/auth/discover", post(sessions::discover))
        .route("/auth/login", post(sessions::login))
        .route(
            "/auth/password-reset",
            post(sessions::request_password_reset),
        )
        .route(
            "/auth/password-reset/confirm",
            post(sessions::confirm_password_reset),
        )
        .route("/auth/session", get(sessions::current_session))
        .route("/auth/logout", post(sessions::logout))
        .route("/auth/renew", post(sessions::renew))
        .route(
            "/data-health/background-processing",
            get(data_health::background_processing_status),
        )
        .route(
            "/data-health/summary",
            get(data_health::data_health_summary),
        )
        .route(
            "/data-health/blueprints",
            get(data_health::data_health_blueprints),
        )
        .route(
            "/data-health/freshness",
            get(data_health::data_health_freshness),
        )
        .route(
            "/data-health/completeness",
            get(data_health::data_health_completeness),
        )
        .route(
            "/data-health/contexts",
            get(data_health::data_health_contexts),
        )
        .route(
            "/data-health/relationships",
            get(data_health::data_health_relationships),
        )
        .route(
            "/data-health/storage",
            get(data_health::data_health_storage),
        )
        .route(
            "/data-health/refresh",
            post(data_health::refresh_data_health),
        )
        .route("/audit-events", get(audit_events::list))
        .route(
            "/saved-views",
            get(saved_views::list).post(saved_views::create),
        )
        .route(
            "/saved-views/{id}",
            get(saved_views::get)
                .put(saved_views::update)
                .delete(saved_views::delete),
        )
        .route("/view-state-links", post(saved_views::create_link))
        .route("/view-state-links/{id}", get(saved_views::get_link))
        .route(
            "/event-deliveries/dead-letters",
            get(event_deliveries::list),
        )
        .route(
            "/event-deliveries/{consumer_id}/{event_id}/replay",
            post(event_deliveries::replay),
        )
        .route("/workspace/roles", get(roles::list).post(roles::create))
        .route(
            "/workspace/navigation",
            get(workspace_navigation::configured).put(workspace_navigation::update),
        )
        .route(
            "/workspace/extension-layout",
            get(extensions::workspace_extension_layout)
                .put(extensions::update_workspace_extension_layout),
        )
        .route(
            "/workspace/navigation/sidebar",
            get(workspace_navigation::sidebar),
        )
        .route(
            "/workspace/assignable-roles",
            get(roles::list_assignable_roles),
        )
        .route("/workspace/permissions", get(roles::list_permissions))
        .route(
            "/workspace/token-permissions",
            get(roles::list_token_permissions),
        )
        .route(
            "/workspace/grant-targets/{scope_type}",
            get(roles::list_grant_targets),
        )
        .route("/workspace/roles/{role_id}", put(roles::update))
        .route(
            "/workspace/roles/{role_id}/duplicate",
            post(roles::duplicate),
        )
        .route("/workspace/roles/{role_id}/retire", post(roles::retire))
        .route("/workspace/members", get(members::list_members))
        .route(
            "/workspace/members/{member_id}",
            put(members::update_member),
        )
        .route(
            "/workspace/members/{member_id}/grants",
            post(members::grant_role),
        )
        .route(
            "/workspace/members/{member_id}/grants/{grant_id}",
            axum::routing::delete(members::revoke_role),
        )
        .route(
            "/workspace/members/{member_id}/transfer-ownership",
            post(members::transfer_ownership),
        )
        .route(
            "/workspace/invitations",
            get(members::list_invitations).post(members::create_invitation),
        )
        .route("/workspace/users", post(members::create_workspace_user))
        .route("/onboarding/complete", post(members::complete_onboarding))
        .route(
            "/workspace/invitations/{invitation_id}",
            axum::routing::delete(members::revoke_invitation),
        )
        .route(
            "/workspace/invitations/accept",
            post(members::accept_invitation),
        )
        .route(
            "/personal-access-tokens",
            get(tokens::list).post(tokens::create),
        )
        .route(
            "/personal-access-tokens/{token_id}",
            axum::routing::delete(tokens::revoke),
        )
        .route(
            "/blueprints",
            get(blueprints::list_entity_blueprints).post(blueprints::create_blueprint),
        )
        .route("/rules", get(rules::list).post(rules::create))
        .route("/rules/validate", post(rules::validate))
        .route("/rules/{rule_id}", get(rules::get))
        .route("/rules/{rule_id}/versions", post(rules::create_revision))
        .route(
            "/rules/{rule_id}/versions/{version}/publish",
            post(rules::publish),
        )
        .route(
            "/rules/{rule_id}/versions/{version}/enable",
            post(rules::enable),
        )
        .route("/rules/{rule_id}/disable", post(rules::disable))
        .route("/rules/{rule_id}/run-now", post(rules::run_now))
        .route("/rule-runs", get(rules::list_runs))
        .route("/rule-runs/{run_id}/replay", post(rules::replay_run))
        .route("/rule-findings", get(rules::findings))
        .route(
            "/rule-findings/{finding_id}/acknowledge",
            post(rules::acknowledge),
        )
        .route("/workflows", get(workflows::list).post(workflows::create))
        .route("/workflows/validate", post(workflows::validate))
        .route("/workflows/{workflow_id}", get(workflows::get))
        .route(
            "/workflows/{workflow_id}/versions",
            get(workflows::list_versions).post(workflows::create_revision),
        )
        .route(
            "/workflows/{workflow_id}/versions/{version}",
            get(workflows::get_version),
        )
        .route(
            "/workflows/{workflow_id}/versions/{version}/publish",
            post(workflows::publish),
        )
        .route(
            "/workflows/{workflow_id}/versions/{version}/enable",
            post(workflows::enable),
        )
        .route("/workflows/{workflow_id}/run-now", post(workflows::run_now))
        .route("/workflows/{workflow_id}/disable", post(workflows::disable))
        .route("/workflow-runs", get(workflows::list_runs))
        .route(
            "/workflow-runs/{run_id}/replay",
            post(workflows::replay_run),
        )
        .route(
            "/reusable-attributes",
            get(reusable_attributes::list).post(reusable_attributes::create),
        )
        .route(
            "/reusable-attributes/{definition_id}/versions",
            post(reusable_attributes::create_revision),
        )
        .route(
            "/reusable-attribute-revisions/{revision_id}/publish",
            post(reusable_attributes::publish_revision),
        )
        .route(
            "/reusable-attribute-groups",
            get(reusable_attributes::list_groups).post(reusable_attributes::create_group),
        )
        .route(
            "/v1/entities/{entity_id}/reusable-attributes",
            post(reusable_attributes::attach),
        )
        .route(
            "/v1/entities/{entity_id}/reusable-attribute-groups/{group_id}",
            post(reusable_attributes::attach_group),
        )
        .route("/blueprints/catalogue", get(blueprints::list_blueprints))
        .route(
            "/blueprints/{blueprint_id}/versions",
            get(blueprints::list_blueprint_revisions).post(blueprints::create_blueprint_revision),
        )
        .route("/blueprints/{blueprint_id}", get(blueprints::get_blueprint))
        .route(
            "/blueprints/{blueprint_id}/versions/{version}",
            get(blueprints::get_blueprint_revision),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/publish",
            post(blueprints::publish_blueprint_revision),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/entity-publications",
            post(blueprints::publish_blueprint_entities),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/entity-publications/publish-all",
            post(blueprints::publish_blueprint_entities_all_channels),
        )
        .route(
            "/blueprints/{blueprint_id}/migration-batches",
            get(blueprints::list_blueprint_migration_batches),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/safe-migration-impact",
            get(blueprints::safe_blueprint_migration_impact),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/safe-migration-batches",
            post(blueprints::start_safe_blueprint_migration_batch),
        )
        .route(
            "/blueprints/by-code/{code}",
            get(blueprints::get_blueprint_by_code),
        )
        .route(
            "/blueprints/by-code/{code}/versions/{version}",
            get(blueprints::get_blueprint_by_code_and_version),
        )
        .route(
            "/contexts",
            get(contexts::list_contexts).post(contexts::create_context),
        )
        .route(
            "/publication-channels",
            get(contexts::list_publication_channels),
        )
        .route(
            "/publication-channels/{context_id}",
            put(contexts::set_publication_channel),
        )
        .route("/contexts/{code}", get(contexts::get_context))
        .route(
            "/contexts/id/{id}",
            put(contexts::update_context).delete(contexts::delete_context),
        )
        .route(
            "/v1/entities/search",
            post(entity_reads::search_entity_previews),
        )
        .route(
            "/v1/entities/facets/relationship-tree/children",
            post(entity_reads::relationship_tree_facet_children),
        )
        .route("/v1/entities", post(entities::create_entity_form))
        .route(
            "/v1/entities/{entity_id}",
            get(entities::get_entity_form).put(entities::update_entity_form),
        )
        .route(
            "/v1/entities/{entity_id}/duplicate",
            post(entities::duplicate_entity),
        )
        .route("/agent/smart-fill", post(entities::smart_fill_entity_form))
        .route(
            "/v1/entities/{entity_id}/incoming-relationships",
            post(entities::list_incoming_relationships),
        )
        .route(
            "/v1/entities/{entity_id}/publications",
            get(entities::list_entity_publications).post(entities::publish_entity),
        )
        .route(
            "/v1/entities/{entity_id}/publications/unpublish",
            post(entities::unpublish_entity),
        )
        .route(
            "/v1/entities/{entity_id}/publications/publish-all",
            post(entities::publish_entity_all_channels),
        )
        .route(
            "/v1/entities/{entity_id}/blueprint-migration/preview",
            post(entities::preview_entity_migration),
        )
        .route(
            "/v1/entities/{entity_id}/blueprint-migration",
            post(entities::migrate_entity_to_latest),
        )
        .route(
            "/entities/{entity_id}/file-attributes/{attribute_code}/uploads",
            post(files::upload).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/files/{file_id}", get(files::metadata))
        .route("/files/{file_id}/download", get(files::download_original))
        .route(
            "/files/{file_id}/variants/{kind}/download",
            get(files::download_variant),
        )
        .route("/entities", get(entity_reads::list_previews))
        .route(
            "/entities/{entity_id}",
            get(entity_reads::get_entity).delete(entities::delete_entity),
        )
        .route(
            "/entities/{entity_id}/preview",
            get(entity_reads::get_preview),
        )
        .route(
            "/entities/{entity_id}/resolved-preview",
            get(entity_reads::get_resolved_preview),
        )
        .route(
            "/entities/{entity_id}/hierarchy",
            get(entity_reads::get_entity_hierarchy),
        )
        .route(
            "/entities/{entity_id}/values",
            post(entities::append_values),
        )
        .route(
            "/entities/{entity_id}/changes",
            get(entities::get_entity_changes),
        )
        .route(
            "/entities/{entity_id}/values/history",
            get(entities::get_value_history),
        )
        .route(
            "/entities/{entity_id}/values/history/{history_id}/restore",
            post(entities::restore_value),
        )
        .route(
            "/entities/{entity_id}/relationships/replace",
            post(entities::replace_relationships),
        )
        .route(
            "/entities/{entity_id}/relationships/remove",
            post(entities::remove_relationships),
        )
        .route(
            "/entities/{entity_id}/values/current",
            get(entities::get_current_values),
        )
        .layer(middleware::from_fn_with_state(state.clone(), audit::record))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::authorize,
        ))
        .with_state(state.clone())
        .layer(axum::extract::DefaultBodyLimit::max(
            state.default_body_limit,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_limits,
        ))
        .layer(middleware::from_fn_with_state(state, server_timing));

    // Production images set WEB_DIST_DIR to the compiled Vite output. Keep the
    // bare API routes for clients and probes while also exposing them below
    // `/api`, which is the browser application's stable origin-relative base.
    let Ok(web_dist) = std::env::var("WEB_DIST_DIR") else {
        return api;
    };
    let web_dist = web_dist.trim();
    if web_dist.is_empty() {
        return api;
    }
    let index = format!("{web_dist}/index.html");
    Router::new()
        .nest("/api", api.clone())
        .merge(api)
        .fallback_service(ServeDir::new(web_dist).not_found_service(ServeFile::new(index)))
}

async fn metrics(State(state): State<AppState>) -> Response {
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics.render(),
    )
        .into_response()
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    #[test]
    fn api_aliases_use_the_canonical_route_for_middleware() {
        assert_eq!(
            canonical_route("/api/blueprints/{blueprint_id}"),
            "/blueprints/{blueprint_id}"
        );
        assert_eq!(
            canonical_route("/blueprints/{blueprint_id}"),
            "/blueprints/{blueprint_id}"
        );
        assert_eq!(canonical_route("/apiary"), "/apiary");
    }

    #[test]
    fn request_ids_accept_only_uuids_and_generate_missing_ids() {
        let id = Uuid::new_v4();
        let request = axum::http::Request::builder()
            .header("x-request-id", id.to_string())
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(request_id(&request), id);

        let invalid = axum::http::Request::builder()
            .header("x-request-id", "not-a-uuid")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_ne!(request_id(&invalid), Uuid::nil());
    }

    #[test]
    fn development_phases_are_gated_and_allowlisted() {
        let disabled = RequestTiming::new(false);
        disabled.record("candidate", Instant::now());
        assert!(disabled.server_timing().is_empty());

        let enabled = RequestTiming::new(true);
        enabled.record("candidate", Instant::now());
        enabled.record("unknown", Instant::now());
        enabled.record_sql(Some("entities-page"), 12.345);
        let header = enabled.server_timing();
        assert!(header.starts_with("candidate;dur="));
        assert!(header.contains("sql;dur=12.35;desc=queries-1"));
        assert!(header.contains("sql-entities-page;dur=12.35;desc=queries-1"));
        assert!(!header.contains("SELECT"));
    }
}
