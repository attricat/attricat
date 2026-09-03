use std::{net::SocketAddr, str::FromStr, sync::Arc};

use api::{
    MIGRATOR,
    account::{Password, hash_password},
    agent_worker,
    agents::AgentProviderConfig,
    constants::{
        DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS, DEFAULT_ENTITY_PAGE_SIZE,
        DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE, DEFAULT_PREVIEW_RELATIONSHIP_DEPTH,
        DEFAULT_PREVIEW_RELATIONSHIP_ITEMS, DEFAULT_RELATIONSHIP_FACET_NODES,
        MAINTENANCE_POOL_CONNECTIONS, REQUEST_POOL_CONNECTIONS,
    },
    file_access::AllowFileAccess,
    http::{AppState, router},
    mail::SmtpMailDelivery,
    repository::CatalogRepository,
    storage::{ObjectStore, S3ObjectStore, StorageConfig},
    telemetry::{init_metrics, init_tracing},
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing()?;
    let metrics = init_metrics()?;
    let agent_provider = AgentProviderConfig::from_env()
        .map_err(|error| format!("invalid agent provider configuration: {error}"))?;
    if agent_provider.is_some() {
        tracing::info!("agent provider configuration loaded");
    } else {
        tracing::info!("agents are unavailable: LLM_API_KEY is not configured");
    }

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to start the API")?;
    let storage_config = StorageConfig::from_env()
        .map_err(|error| format!("invalid object storage configuration: {error}"))?;
    let object_store = Arc::new(S3ObjectStore::new(storage_config).await);
    object_store.readiness().await.map_err(|_| {
        "object storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials"
    })?;
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()?;
    let max_preview_relationship_depth = std::env::var("PREVIEW_MAX_RELATIONSHIP_DEPTH")
        .unwrap_or_else(|_| DEFAULT_PREVIEW_RELATIONSHIP_DEPTH.to_string())
        .parse()?;
    let max_preview_relationship_items = std::env::var("PREVIEW_MAX_RELATIONSHIP_ITEMS")
        .unwrap_or_else(|_| DEFAULT_PREVIEW_RELATIONSHIP_ITEMS.to_string())
        .parse()?;
    let max_entity_page_size = std::env::var("ENTITY_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| DEFAULT_ENTITY_PAGE_SIZE.to_string())
        .parse()?;
    let max_incoming_relationship_page_size = std::env::var("INCOMING_RELATIONSHIP_MAX_PAGE_SIZE")
        .unwrap_or_else(|_| DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE.to_string())
        .parse()?;
    let max_relationship_facet_nodes = std::env::var("RELATIONSHIP_FACET_MAX_NODES")
        .unwrap_or_else(|_| DEFAULT_RELATIONSHIP_FACET_NODES.to_string())
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
        .max_connections(MAINTENANCE_POOL_CONNECTIONS)
        .connect(&database_url)
        .await?;
    tracing::info!("running database migrations");
    MIGRATOR.run(&maintenance_pool).await?;
    CatalogRepository::new(maintenance_pool.clone())
        .ensure_agent_permissions()
        .await?;
    CatalogRepository::new(maintenance_pool.clone())
        .ensure_audit_permissions()
        .await?;
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
    bootstrap_workspace_owner(
        &maintenance_pool,
        workspace_id,
        bootstrap_owner_id.unwrap_or_else(Uuid::new_v4),
        Uuid::new_v4(),
        Uuid::new_v4(),
        &bootstrap_owner_email,
    )
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
        bootstrap_workspace_owner(
            &maintenance_pool,
            workspace_id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &email,
        )
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

    // Public authentication uses this unscoped pool. Authorization creates a
    // separate, cached RLS-configured pool only after deriving a trusted
    // workspace from the credential or browser session.
    let connect_options = PgConnectOptions::from_str(&database_url)?;
    let pool = PgPoolOptions::new()
        .max_connections(REQUEST_POOL_CONNECTIONS)
        .connect_with(connect_options.clone())
        .await?;

    let smtp_host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let smtp_port = std::env::var("SMTP_PORT")
        .unwrap_or_else(|_| "1025".to_owned())
        .parse()?;
    let mail_from = std::env::var("MAIL_FROM")
        .unwrap_or_else(|_| "Catalog <no-reply@catalog.local>".to_owned());
    let mail_delivery = Arc::new(SmtpMailDelivery::new(
        &smtp_host,
        smtp_port,
        &mail_from,
        std::env::var("SMTP_USERNAME").ok(),
        std::env::var("SMTP_PASSWORD").ok(),
    )?);
    let password_reset_url = std::env::var("PASSWORD_RESET_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/password-reset/confirm".to_owned());
    let workspace_invitation_url = std::env::var("WORKSPACE_INVITATION_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/invitations/accept".to_owned());
    let workspace_onboarding_url = std::env::var("WORKSPACE_ONBOARDING_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/onboarding".to_owned());

    let agent_dispatcher = match agent_provider.clone() {
        Some(config) => Some(
            agent_worker::start(
                CatalogRepository::with_workspace_pool_factory(
                    pool.clone(),
                    connect_options.clone(),
                ),
                config,
                object_store.clone(),
            )
            .await,
        ),
        None => None,
    };

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "API listening");
    axum::serve(
        listener,
        router(AppState {
            repository: CatalogRepository::with_workspace_pool_factory(pool, connect_options),
            agent_provider,
            agent_dispatcher,
            object_store,
            file_access_policy: Arc::new(AllowFileAccess),
            mail_delivery,
            password_reset_url,
            workspace_invitation_url,
            workspace_onboarding_url,
            metrics,
            max_preview_relationship_depth,
            max_preview_relationship_items,
            max_entity_page_size,
            max_incoming_relationship_page_size,
            max_relationship_facet_nodes,
            max_upload_file_bytes: std::env::var("FILE_UPLOAD_MAX_BYTES")
                .unwrap_or_else(|_| (50 * 1024 * 1024).to_string())
                .parse()?,
            max_upload_files: std::env::var("FILE_UPLOAD_MAX_FILES")
                .unwrap_or_else(|_| "10".to_owned())
                .parse()?,
            data_health_cache_ttl_seconds: std::env::var("DATA_HEALTH_CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS.to_string())
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

async fn bootstrap_workspace_owner(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    membership_id: Uuid,
    grant_id: Uuid,
    email: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let email = email.trim().to_lowercase();
    sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
    let owner_exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id WHERE g.workspace_id = $1 AND g.role_id = '00000000-0000-4000-8000-000000000101'::uuid AND m.state = 'active')").bind(workspace_id).fetch_one(&mut *tx).await?;
    if !owner_exists {
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING")
            .bind(user_id)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
        let persisted_user: Uuid =
            sqlx::query_scalar("SELECT id FROM users WHERE email = $1 FOR UPDATE")
                .bind(&email)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3) ON CONFLICT (workspace_id, user_id) DO NOTHING").bind(membership_id).bind(workspace_id).bind(persisted_user).execute(&mut *tx).await?;
        let membership: Uuid = sqlx::query_scalar(
            "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(workspace_id)
        .bind(persisted_user)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000101'::uuid, 'workspace', $2) ON CONFLICT DO NOTHING").bind(grant_id).bind(workspace_id).bind(membership).execute(&mut *tx).await?;
    }
    tx.commit().await
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
    // Bootstrap passwords are supplied by trusted deployment configuration, so
    // their configured owner addresses are verified before reset links can issue.
    sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) WHERE email = $1",
    )
    .bind(email)
    .execute(pool)
    .await?;
    Ok(())
}
