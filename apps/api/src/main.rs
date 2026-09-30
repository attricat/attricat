use std::{net::SocketAddr, sync::Arc, time::Instant};

use api::{
    MIGRATOR, agent_worker,
    agents::AgentProviderConfig,
    blueprint_migration_worker,
    constants::{
        DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS, DEFAULT_ENTITY_PAGE_SIZE,
        DEFAULT_FILE_UPLOAD_MAX_BYTES, DEFAULT_FILE_UPLOAD_MAX_FILES,
        DEFAULT_HTTP_DEFAULT_BODY_BYTES, DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS, DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE,
        DEFAULT_PREVIEW_RELATIONSHIP_DEPTH, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS,
        DEFAULT_RELATIONSHIP_FACET_NODES, MAINTENANCE_POOL_CONNECTIONS, REQUEST_POOL_CONNECTIONS,
        TASK_POOL_CONNECTIONS,
    },
    event_dispatcher::{self, DispatcherConfig},
    extension_registry::{DEFAULT_OFFICIAL_REGISTRY, GitHubRegistry, GitHubRepository},
    extension_runtime::{self, ExtensionRuntime, ExtensionRuntimeConfig},
    file_access::AllowFileAccess,
    http::{AppState, BuildInfo, router},
    mail::SmtpMailDelivery,
    repository::{CatalogRepository, ValueHistoryRetentionDays},
    rule_runtime, solution_pack_housekeeping,
    storage::{ObjectStore, S3ObjectStore, StorageConfig},
    task_worker::{self, TaskHandlerRegistry, TaskWorkerConfig},
    telemetry::{init_metrics, init_tracing},
    workflow_runtime,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Semaphore;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();
    init_tracing("attricat-api")?;
    let metrics = init_metrics()?;
    // Validate before opening a maintenance connection, so destructive cleanup
    // can never receive a zero or negative retention interval.
    let history_retention_days = std::env::var("ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS")
        .unwrap_or_else(|_| "90".to_owned())
        .parse::<ValueHistoryRetentionDays>()
        .map_err(|error| format!("ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS {error}"))?;
    let agent_provider = AgentProviderConfig::from_env()
        .map_err(|error| format!("invalid agent provider configuration: {error}"))?;
    if agent_provider.is_some() {
        tracing::info!("agent provider configuration loaded");
    } else {
        tracing::info!("agents are unavailable: LLM_API_KEY is not configured");
    }

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to start the API")?;
    let request_pool_connections = pool_connections(
        "DATABASE_REQUEST_POOL_CONNECTIONS",
        REQUEST_POOL_CONNECTIONS,
    )?;
    let task_pool_connections =
        pool_connections("DATABASE_TASK_POOL_CONNECTIONS", TASK_POOL_CONNECTIONS)?;
    tracing::info!(
        request_pool_connections,
        task_pool_connections,
        total_database_connections = u64::from(MAINTENANCE_POOL_CONNECTIONS)
            + u64::from(request_pool_connections)
            + u64::from(task_pool_connections),
        "configured bounded database connection pools"
    );
    let storage_config = StorageConfig::from_env()
        .map_err(|error| format!("invalid object storage configuration: {error}"))?;
    let object_store = Arc::new(S3ObjectStore::new(storage_config).await);
    object_store.readiness().await.map_err(|_| {
        "object storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials"
    })?;
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()?;
    let devtools_enabled = std::env::var("CATALOG_DEVTOOLS").is_ok_and(|value| value == "true");
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
    let auto_migrate = boolean_env("CATALOG_AUTO_MIGRATE", true)?;
    if auto_migrate {
        tracing::info!("running database migrations");
        MIGRATOR.run(&maintenance_pool).await?;
    } else {
        tracing::info!(
            "automatic migrations disabled; expecting the migrate role to have completed"
        );
    }
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_agent_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_audit_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_entity_publication_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_extension_registry_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_workflow_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_solution_pack_permissions()
        .await?;
    CatalogRepository::system(maintenance_pool.clone())
        .ensure_rule_permissions()
        .await?;
    let bootstrap_repository = CatalogRepository::system(maintenance_pool.clone());
    // The identity/membership migration consumes this durable bootstrap owner
    // record to create the initial owner grant. It is set only by deployment
    // configuration, never by a catalog request.
    bootstrap_repository
        .configure_bootstrap_workspace(
            workspace_id,
            &bootstrap_workspace_name,
            &bootstrap_owner_email,
        )
        .await?;
    // The migration defines the identity/RBAC schema, but configuration is
    // available only after migrations. Bootstrap the configured owner here so
    // a fresh installation receives its initial durable owner grant.
    bootstrap_repository
        .ensure_bootstrap_workspace_owner(
            workspace_id,
            bootstrap_owner_id.unwrap_or_else(Uuid::new_v4),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &bootstrap_owner_email,
        )
        .await?;
    if let Some(password) = bootstrap_owner_password {
        bootstrap_repository
            .ensure_bootstrap_local_password(&bootstrap_owner_email, password)
            .await?;
    }
    // The browser E2E harness needs an independent principal for server-side
    // fixture setup, because login rotation deliberately invalidates a user's
    // prior browser session. This is unavailable unless both test-only values
    // are explicitly configured.
    if let Some((email, password)) = e2e_fixture {
        let email = email.trim().to_lowercase();
        bootstrap_repository
            .ensure_bootstrap_workspace_owner(
                workspace_id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                &email,
            )
            .await?;
        bootstrap_repository
            .ensure_bootstrap_local_password(&email, password)
            .await?;
    }
    let cleanup_started = Instant::now();
    match CatalogRepository::system(maintenance_pool.clone())
        .purge_value_history(history_retention_days)
        .await
    {
        Ok(deleted_entries) => {
            let elapsed_seconds = cleanup_started.elapsed().as_secs_f64();
            metrics::counter!("catalog_value_history_cleanup_total", "outcome" => "success")
                .increment(1);
            metrics::counter!("catalog_value_history_entries_purged_total")
                .increment(deleted_entries);
            metrics::histogram!("catalog_value_history_cleanup_duration_seconds")
                .record(elapsed_seconds);
            tracing::info!(
                retention_days = history_retention_days.get(),
                deleted_entries,
                elapsed_seconds,
                "attribute value history cleanup completed"
            );
        }
        Err(error) => {
            let elapsed_seconds = cleanup_started.elapsed().as_secs_f64();
            metrics::counter!("catalog_value_history_cleanup_total", "outcome" => "failed")
                .increment(1);
            metrics::histogram!("catalog_value_history_cleanup_duration_seconds")
                .record(elapsed_seconds);
            tracing::error!(
                retention_days = history_retention_days.get(),
                elapsed_seconds,
                error = %error,
                "attribute value history cleanup failed; refusing to report successful startup maintenance"
            );
            return Err(format!("attribute value history cleanup failed: {error}").into());
        }
    }
    maintenance_pool.close().await;

    // Every workspace shares these bounded pools. Repository scope is carried
    // in explicit SQL predicates, never in mutable connection state.
    let request_pool = PgPoolOptions::new()
        .max_connections(request_pool_connections)
        .connect(&database_url)
        .await?;
    let task_pool = PgPoolOptions::new()
        .max_connections(task_pool_connections)
        .connect(&database_url)
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
        &std::env::var("SMTP_TLS_MODE").unwrap_or_else(|_| "starttls".to_owned()),
    )?);
    let official_registry = std::env::var("EXTENSION_OFFICIAL_REGISTRY")
        .unwrap_or_else(|_| DEFAULT_OFFICIAL_REGISTRY.to_owned())
        .parse::<GitHubRepository>()
        .map_err(|error| format!("invalid EXTENSION_OFFICIAL_REGISTRY: {error}"))?;
    let registry = Arc::new(
        GitHubRegistry::new()
            .map_err(|error| format!("cannot initialize extension registry: {error}"))?,
    );
    let password_reset_url = std::env::var("PASSWORD_RESET_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/password-reset/confirm".to_owned());
    let workspace_invitation_url = std::env::var("WORKSPACE_INVITATION_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/invitations/accept".to_owned());
    let workspace_onboarding_url = std::env::var("WORKSPACE_ONBOARDING_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5173/onboarding".to_owned());

    let task_repository = CatalogRepository::system(task_pool.clone());
    // Agent provider calls are not safely resumable. On process restart mark
    // any previously running run interrupted before its task can be reclaimed.
    if agent_provider.is_some() {
        task_repository.recover_interrupted_agent_runs().await?;
    }
    let extension_runtime =
        ExtensionRuntime::new(object_store.clone(), ExtensionRuntimeConfig::default())
            .map_err(|error| format!("invalid extension runtime configuration: {error}"))?;
    let mut task_handlers: Vec<Arc<dyn api::task_worker::TaskHandler>> = agent_provider
        .clone()
        .map(|config| {
            Arc::new(agent_worker::AgentTaskHandler::new(
                task_repository.clone(),
                config,
                object_store.clone(),
            )) as Arc<dyn api::task_worker::TaskHandler>
        })
        .into_iter()
        .collect();
    task_handlers.push(Arc::new(extension_runtime::WasmExtensionTaskHandler::new(
        task_repository.clone(),
        extension_runtime.clone(),
    )));
    task_handlers.push(Arc::new(
        extension_runtime::ExtensionOperationTaskHandler::new(
            task_repository.clone(),
            extension_runtime.clone(),
        ),
    ));
    task_handlers.push(workflow_runtime::task_handler(task_repository.clone()));
    task_handlers.push(rule_runtime::task_handler(task_repository.clone()));
    let blueprint_migration_config =
        blueprint_migration_worker::BlueprintMigrationBatchConfig::from_env()
            .map_err(|error| format!("invalid blueprint migration configuration: {error}"))?;
    task_handlers.push(Arc::new(
        blueprint_migration_worker::BlueprintMigrationBatchTaskHandler::with_config(
            task_repository.clone(),
            blueprint_migration_config,
        ),
    ));
    // Reconcile only batches created before this deployment. New batches and
    // their task envelopes commit atomically in the repository.
    task_repository
        .backfill_safe_blueprint_migration_tasks()
        .await?;

    let (shutdown_sender, shutdown_receiver) = tokio::sync::watch::channel(());
    let solution_pack_housekeeping =
        solution_pack_housekeeping::start(task_repository.clone(), shutdown_receiver.clone());
    let task_worker_config = TaskWorkerConfig::from_env()
        .map_err(|error| format!("invalid task worker configuration: {error}"))?;
    // Registered kinds have atomically-enqueued producers and token-fenced
    // handlers. Unmigrated kinds remain unregistered and cannot be leased.
    let task_worker = task_worker::start(
        task_repository.clone(),
        TaskHandlerRegistry::new(task_handlers).expect("task handler kinds are unique"),
        task_worker_config,
        shutdown_receiver.clone(),
    );
    let dispatcher_config = DispatcherConfig::from_env()
        .map_err(|error| format!("invalid event dispatcher configuration: {error}"))?;
    let extension_event_delivery_coordinator = extension_runtime::start_event_delivery_coordinator(
        task_repository.clone(),
        object_store.clone(),
        shutdown_receiver.clone(),
    );
    let workflow_repository = CatalogRepository::system(task_pool.clone());
    let dispatcher_handles = event_dispatcher::start(
        workflow_repository.clone(),
        rule_runtime::add_to_registry(workflow_runtime::add_to_registry(
            api::event_dispatcher::EventHandlerRegistry::default_handlers(),
        )),
        dispatcher_config,
        shutdown_receiver.clone(),
    );
    let workflow_worker = workflow_runtime::start_schedule_coordinator(
        workflow_repository.clone(),
        shutdown_receiver.clone(),
    );
    let rule_worker =
        rule_runtime::start_schedule_coordinator(workflow_repository, shutdown_receiver);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "API listening");
    axum::serve(
        listener,
        router(AppState {
            repository: CatalogRepository::system(request_pool.clone()),
            agent_provider,
            registry,
            official_registry,
            object_store,
            extension_runtime,
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
                .unwrap_or_else(|_| DEFAULT_FILE_UPLOAD_MAX_BYTES.to_string())
                .parse()?,
            max_upload_files: std::env::var("FILE_UPLOAD_MAX_FILES")
                .unwrap_or_else(|_| DEFAULT_FILE_UPLOAD_MAX_FILES.to_string())
                .parse()?,
            data_health_cache_ttl_seconds: std::env::var("DATA_HEALTH_CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS.to_string())
                .parse()?,
            data_health_cache: Default::default(),
            session_cookie_secure: std::env::var("SESSION_COOKIE_SECURE")
                .map(|value| value != "false")
                .unwrap_or(true),
            allow_trusted_headers: false,
            request_permits: Arc::new(Semaphore::new(positive_env(
                "HTTP_MAX_CONCURRENT_REQUESTS",
                DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
            )?)),
            request_timeout: std::time::Duration::from_secs(positive_env(
                "HTTP_REQUEST_TIMEOUT_SECONDS",
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
            )? as u64),
            default_body_limit: positive_env(
                "HTTP_DEFAULT_BODY_BYTES",
                DEFAULT_HTTP_DEFAULT_BODY_BYTES,
            )?,
            devtools_enabled,
            build_info: BuildInfo {
                version: env!("CARGO_PKG_VERSION"),
                branch: env!("ATTRICAT_BUILD_BRANCH"),
                commit: env!("ATTRICAT_BUILD_COMMIT"),
            },
        }),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received");
        let _ = shutdown_sender.send(());
    })
    .await?;
    for handle in dispatcher_handles {
        handle.await?;
    }
    workflow_worker.await?;
    rule_worker.await?;
    extension_event_delivery_coordinator.await?;
    task_worker.await??;
    solution_pack_housekeeping.await?;
    request_pool.close().await;
    task_pool.close().await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            result = ctrl_c => result.expect("install Ctrl-C handler"),
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await.expect("install Ctrl-C handler");
}

const MAX_GLOBAL_POOL_CONNECTIONS: u32 = 100;

fn pool_connections(
    name: &str,
    default: u32,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let value = std::env::var(name).ok();
    parse_pool_connections(name, value.as_deref(), default).map_err(Into::into)
}

fn parse_pool_connections(name: &str, value: Option<&str>, default: u32) -> Result<u32, String> {
    match value {
        Some(value) => value
            .parse::<u32>()
            .ok()
            .filter(|value| *value > 0 && *value <= MAX_GLOBAL_POOL_CONNECTIONS)
            .ok_or_else(|| {
                format!("{name} must be an integer between 1 and {MAX_GLOBAL_POOL_CONNECTIONS}")
            }),
        None => Ok(default),
    }
}

fn boolean_env(name: &str, default: bool) -> Result<bool, String> {
    let value = std::env::var(name).ok();
    parse_boolean(name, value.as_deref(), default)
}

fn parse_boolean(name: &str, value: Option<&str>, default: bool) -> Result<bool, String> {
    match value {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(format!("{name} must be `true` or `false`")),
    }
}

fn positive_env(
    name: &str,
    default: usize,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("{name} must be a positive integer").into()),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_configuration_is_strict() {
        assert!(parse_boolean("BOOL", None, true).unwrap());
        assert!(parse_boolean("BOOL", Some("true"), false).unwrap());
        assert!(!parse_boolean("BOOL", Some("false"), true).unwrap());
        assert!(parse_boolean("BOOL", Some("TRUE"), true).is_err());
    }

    #[test]
    fn pool_connection_configuration_is_positive_and_capped() {
        assert_eq!(
            parse_pool_connections("POOL", None, REQUEST_POOL_CONNECTIONS).unwrap(),
            REQUEST_POOL_CONNECTIONS
        );
        assert_eq!(parse_pool_connections("POOL", Some("7"), 1).unwrap(), 7);
        for invalid in ["0", "-1", "101", "invalid"] {
            assert!(parse_pool_connections("POOL", Some(invalid), 1).is_err());
        }
    }
}
