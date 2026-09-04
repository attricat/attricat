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
mod roles;
mod sessions;
mod tokens;

use std::{collections::HashMap, sync::Arc, time::Instant};

use crate::{
    agent_worker::AgentDispatcher,
    agents::AgentProviderConfig,
    extension_registry::{GitHubRegistry, GitHubRepository},
    file_access::FileAccessPolicy,
    mail::MailDelivery,
    repository::CatalogRepository,
    storage::ObjectStore,
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
use tokio::sync::Mutex;
use tracing::{Instrument, field::Empty};

#[derive(Clone)]
pub struct AppState {
    pub repository: CatalogRepository,
    pub agent_provider: Option<AgentProviderConfig>,
    pub agent_dispatcher: Option<AgentDispatcher>,
    pub registry: Arc<GitHubRegistry>,
    pub official_registry: GitHubRepository,
    /// Storage is injected at startup so future file routes never construct a
    /// provider client from request data.
    pub object_store: Arc<dyn ObjectStore>,
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
}

pub type DataHealthCache = Arc<Mutex<HashMap<String, (Instant, Value)>>>;

async fn server_timing(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let span = tracing::info_span!(
        "http.request",
        method = %method,
        route = %route,
        status = Empty,
        duration_ms = Empty
    );
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
    let value = match response
        .headers()
        .get(&server_timing)
        .and_then(|value| value.to_str().ok())
    {
        Some(existing) => format!("{existing}, app;dur={duration_ms:.2}"),
        None => format!("app;dur={duration_ms:.2}"),
    };
    response.headers_mut().insert(
        server_timing,
        HeaderValue::from_str(&value).expect("server timing values are valid header values"),
    );
    response
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .route(
            "/agent/conversations",
            get(agents::list_conversations).post(agents::create_conversation),
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
        .route(
            "/agent/schedules",
            get(agents::list_schedules).post(agents::create_schedule),
        )
        .route(
            "/agent/schedules/{schedule_id}",
            put(agents::update_schedule).delete(agents::delete_schedule),
        )
        .route(
            "/agent/schedules/{schedule_id}/run-now",
            post(agents::run_schedule_now),
        )
        // Keep the short form for clients built during the backend rollout.
        .route(
            "/agent/schedules/{schedule_id}/run",
            post(agents::run_schedule_now),
        )
        .route("/health", get(data_health::health))
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
        .route("/extensions/runtime", get(extensions::runtime))
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
            "/event-deliveries/dead-letters",
            get(event_deliveries::list),
        )
        .route(
            "/event-deliveries/{consumer_id}/{event_id}/replay",
            post(event_deliveries::replay),
        )
        .route("/workspace/roles", get(roles::list).post(roles::create))
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
            "/v1/entities/{entity_id}/incoming-relationships",
            post(entities::list_incoming_relationships),
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
        .with_state(state)
        .layer(middleware::from_fn(server_timing))
}

async fn metrics(State(state): State<AppState>) -> Response {
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics.render(),
    )
        .into_response()
}
