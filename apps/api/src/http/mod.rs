mod blueprints;
mod contexts;
mod data_health;
mod entities;
mod entity_reads;
mod error;
mod extractors;

use std::{collections::HashMap, sync::Arc, time::Instant};

use axum::{
    Router,
    routing::{get, post, put},
};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::repository::CatalogRepository;

#[derive(Clone)]
pub struct AppState {
    pub repository: CatalogRepository,
    // Preview expansion is request-controlled, so these limits keep cyclic or
    // high-cardinality relationship graphs from turning one read into an
    // unbounded amount of database work.
    pub max_preview_relationship_depth: u8,
    pub max_preview_relationship_items: u32,
    pub max_entity_page_size: u32,
    pub max_incoming_relationship_page_size: u32,
    pub max_relationship_facet_nodes: u32,
    pub data_health_cache_ttl_seconds: u64,
    pub data_health_cache: DataHealthCache,
}

pub type DataHealthCache = Arc<Mutex<HashMap<String, (Instant, Value)>>>;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(data_health::health))
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
        .with_state(state)
}
