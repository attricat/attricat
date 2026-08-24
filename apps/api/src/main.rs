use std::net::SocketAddr;

use api::{
    MIGRATOR,
    account::{Password, hash_password},
    http::{AppState, router},
    repository::CatalogRepository,
    telemetry::{init_metrics, init_tracing},
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing()?;
    let metrics = init_metrics()?;

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to start the API")?;
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()?;
    let max_preview_relationship_depth = std::env::var("PREVIEW_MAX_RELATIONSHIP_DEPTH")
        .unwrap_or_else(|_| "3".to_owned())
        .parse()?;
    let max_preview_relationship_items = std::env::var("PREVIEW_MAX_RELATIONSHIP_ITEMS")
        .unwrap_or_else(|_| "10".to_owned())
        .parse()?;
    let max_entity_page_size = std::env::var("ENTITY_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()?;
    let max_incoming_relationship_page_size = std::env::var("INCOMING_RELATIONSHIP_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| "50".to_owned())
        .parse()?;
    let max_relationship_facet_nodes = std::env::var("RELATIONSHIP_FACET_MAX_NODES")
        .unwrap_or_else(|_| "100".to_owned())
        .parse()?;
    let workspace_id = std::env::var("CATALOG_WORKSPACE_ID")
        .unwrap_or_else(|_| "00000000-0000-4000-8000-000000000002".to_owned())
        .parse::<Uuid>()?;
    let bootstrap_workspace_name = std::env::var("CATALOG_BOOTSTRAP_WORKSPACE_NAME")
        .unwrap_or_else(|_| "Default workspace".to_owned());
    let bootstrap_owner_email = std::env::var("CATALOG_BOOTSTRAP_OWNER_EMAIL")
        .unwrap_or_else(|_| "owner@example.test".to_owned())
        .trim()
        .to_lowercase();
    let bootstrap_owner_id = std::env::var("CATALOG_BOOTSTRAP_OWNER_ID")
        .ok()
        .map(|value| value.parse())
        .transpose()?;
    let bootstrap_owner_password = std::env::var("CATALOG_BOOTSTRAP_OWNER_PASSWORD").ok();
    let e2e_fixture = std::env::var("CATALOG_E2E_FIXTURE_EMAIL")
        .ok()
        .zip(std::env::var("CATALOG_E2E_FIXTURE_PASSWORD").ok());
    if bootstrap_owner_email.is_empty() || !bootstrap_owner_email.contains('@') {
        return Err("CATALOG_BOOTSTRAP_OWNER_EMAIL must be a valid email address".into());
    }
    // Migration and retention maintenance need DDL privileges. Request-serving
    // connections are deliberately created only after that work is complete
    // and switch to the non-owner role provisioned by the tenancy migration.
    let maintenance_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    tracing::info!("running database migrations");
    MIGRATOR.run(&maintenance_pool).await?;
    // The identity/membership migration consumes this durable bootstrap owner
    // record to create the initial owner grant. It is set only by deployment
    // configuration, never by a catalog request.
    let bootstrap_workspace = sqlx::query(
        "UPDATE workspaces SET name = $2, bootstrap_owner_email = $3, updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(workspace_id)
    .bind(bootstrap_workspace_name)
    .bind(&bootstrap_owner_email)
    .execute(&maintenance_pool)
    .await?;
    if bootstrap_workspace.rows_affected() != 1 {
        return Err("CATALOG_WORKSPACE_ID does not identify an active workspace".into());
    }
    // The migration defines the identity/RBAC schema, but configuration is
    // available only after migrations. Bootstrap the configured owner here so
    // a fresh installation receives its initial durable owner grant.
    sqlx::query("SELECT bootstrap_workspace_owner($1, $2, $3, $4, $5)")
        .bind(workspace_id)
        .bind(bootstrap_owner_id.unwrap_or_else(Uuid::new_v4))
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(&bootstrap_owner_email)
        .execute(&maintenance_pool)
        .await?;
    if let Some(password) = bootstrap_owner_password {
        create_bootstrap_password(&maintenance_pool, &bootstrap_owner_email, password).await?;
    }
    // The browser E2E harness needs an independent principal for server-side
    // fixture setup, because login rotation deliberately invalidates a user's
    // prior browser session. This is unavailable unless both test-only values
    // are explicitly configured.
    if let Some((email, password)) = e2e_fixture {
        let email = email.trim().to_lowercase();
        sqlx::query("SELECT bootstrap_workspace_owner($1, $2, $3, $4, $5)")
            .bind(workspace_id)
            .bind(Uuid::new_v4())
            .bind(Uuid::new_v4())
            .bind(Uuid::new_v4())
            .bind(&email)
            .execute(&maintenance_pool)
            .await?;
        create_bootstrap_password(&maintenance_pool, &email, password).await?;
    }
    let history_retention_days = std::env::var("ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS")
        .unwrap_or_else(|_| "90".to_owned())
        .parse()?;
    CatalogRepository::new(maintenance_pool.clone())
        .purge_value_history(history_retention_days)
        .await?;
    maintenance_pool.close().await;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        // RLS is configured per deployment pool. HTTP authorization rejects a
        // different requested workspace before a repository query can run.
        .after_connect(move |connection, _| {
            Box::pin(async move {
                sqlx::query("SELECT set_config('catalog.workspace_id', $1, false)")
                    .bind(workspace_id.to_string())
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET ROLE catalog_api")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await?;

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "API listening");
    axum::serve(
        listener,
        router(AppState {
            repository: CatalogRepository::new(pool),
            workspace_id,
            metrics,
            max_preview_relationship_depth,
            max_preview_relationship_items,
            max_entity_page_size,
            max_incoming_relationship_page_size,
            max_relationship_facet_nodes,
            data_health_cache_ttl_seconds: std::env::var("DATA_HEALTH_CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_owned())
                .parse()?,
            data_health_cache: Default::default(),
            session_cookie_secure: std::env::var("SESSION_COOKIE_SECURE")
                .map(|value| value != "false")
                .unwrap_or(true),
            allow_trusted_headers: false,
        }),
    )
    .await?;

    Ok(())
}

async fn create_bootstrap_password(
    pool: &sqlx::PgPool,
    email: &str,
    password: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let password_hash = hash_password(&Password::new(password))?;
    sqlx::query(
        "INSERT INTO local_password_credentials (user_id, password_hash) SELECT id, $2 FROM users WHERE email = $1 ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(email)
    .bind(password_hash.as_phc())
    .execute(pool)
    .await?;
    Ok(())
}
