use std::sync::Arc;

use api::{
    MIGRATOR, agent_worker, blueprint_migration_worker,
    config::{BootstrapConfig, Config},
    constants::{MAINTENANCE_POOL_CONNECTIONS, READINESS_PROBE_PERMITS},
    event_dispatcher, extension_policy,
    extension_registry::GitHubRegistry,
    extension_runtime::{self, ExtensionRuntime, ExtensionRuntimeConfig},
    file_access::AllowFileAccess,
    http::{AppState, BuildInfo, SampleAccount, SampleLogins, StreamControl, router},
    mail::SmtpMailDelivery,
    maintenance,
    repository::{AttricatRepository, SystemRepository},
    rule_runtime,
    solution_pack_extensions::{
        LocalExtensionReleases, OfficialExtensionRegistry, OfficialExtensionReleases,
    },
    solution_pack_housekeeping,
    storage::{ObjectStore, S3ObjectStore},
    task_worker::{self, TaskHandlerRegistry},
    telemetry::{init_metrics, init_tracing},
    workflow_runtime,
};
use attricat_cache::QueryCache;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Semaphore;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Config {
        telemetry,
        database,
        storage,
        http,
        bootstrap,
        mail,
        cache,
        agent_provider,
        extensions,
        value_history_retention_days,
        event_dispatcher: dispatcher_config,
        task_worker: task_worker_config,
        blueprint_migration: blueprint_migration_config,
        file_worker: _,
    } = Config::load()?;
    init_tracing(
        "attricat-api",
        telemetry.log_filter.as_deref(),
        telemetry.otlp_traces_endpoint.as_deref(),
    )?;
    let metrics = init_metrics()?;
    extension_policy::install(extensions.policy);
    if agent_provider.is_some() {
        tracing::info!("agent provider configuration loaded");
    } else {
        tracing::info!("agents are unavailable: the llm_api_key secret is not configured");
    }

    let database_url = database.url.expose_secret();
    let request_pool_connections = database.request_pool_connections;
    let task_pool_connections = database.task_pool_connections;
    tracing::info!(
        request_pool_connections,
        task_pool_connections,
        total_database_connections = u64::from(MAINTENANCE_POOL_CONNECTIONS)
            + u64::from(request_pool_connections)
            + u64::from(task_pool_connections),
        "configured bounded database connection pools"
    );
    let object_store = Arc::new(S3ObjectStore::new(storage).await);
    object_store.readiness().await.map_err(|_| {
        "object storage readiness failed: cannot access the configured S3 bucket; check S3_ENDPOINT, S3_BUCKET, and credentials"
    })?;
    let BootstrapConfig {
        workspace_id,
        workspace_name: bootstrap_workspace_name,
        owner_email: bootstrap_owner_email,
        owner_id: bootstrap_owner_id,
        owner_password: bootstrap_owner_password,
        demo_mode,
        sample_accounts,
        e2e_fixture,
    } = bootstrap;
    // Complete migrations and bootstrap before creating serving pools.
    // Tenant isolation is enforced by explicit repository scope/predicates;
    // no mutable per-connection role or workspace state is used.
    let maintenance_pool = PgPoolOptions::new()
        .max_connections(MAINTENANCE_POOL_CONNECTIONS)
        .connect(database_url)
        .await?;
    if database.auto_migrate {
        tracing::info!("running database migrations");
        MIGRATOR.run(&maintenance_pool).await?;
    } else {
        tracing::info!(
            "automatic migrations disabled; expecting the migrate role to have completed"
        );
    }
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_agent_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_audit_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_record_publication_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_retention_hold_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_extension_registry_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_workflow_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_solution_pack_permissions()
        .await?;
    AttricatRepository::system(maintenance_pool.clone())
        .ensure_rule_permissions()
        .await?;
    let bootstrap_repository = AttricatRepository::system(maintenance_pool.clone());
    // Repair pre-existing workspaces once at startup; scope derivation and
    // request reads never provision data or acquire an extra connection.
    for id in bootstrap_repository.active_workspace_ids().await? {
        bootstrap_repository.initialize_workspace(id).await?;
    }
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
    if demo_mode {
        bootstrap_repository
            .set_bootstrap_login_identifier(workspace_id, DEMO_LOGIN_IDENTIFIER)
            .await?;
    }
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
    if let Some(password) = &bootstrap_owner_password {
        bootstrap_repository
            .ensure_bootstrap_local_password(
                &bootstrap_owner_email,
                password.expose_secret().to_owned(),
            )
            .await?;
    }
    let sample_logins = match bootstrap_owner_password.filter(|_| sample_accounts) {
        Some(password) => Some(
            seed_sample_accounts(
                &bootstrap_repository,
                workspace_id,
                &bootstrap_owner_email,
                password.expose_secret().to_owned(),
                demo_mode,
            )
            .await?,
        ),
        None => None,
    };
    // The browser E2E harness needs an independent principal for server-side
    // fixture setup, because login rotation deliberately invalidates a user's
    // prior browser session. This is unavailable unless both test-only values
    // are explicitly configured.
    if let Some(fixture) = e2e_fixture {
        let email = fixture.email;
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
            .ensure_bootstrap_local_password(&email, fixture.password.expose_secret().to_owned())
            .await?;
    }
    // Shared cache keys are namespaced by this database's identity.
    let database_identity = bootstrap_repository.ensure_database_identity().await?;
    maintenance_pool.close().await;

    // Every workspace shares these bounded pools. Repository scope is carried
    // in explicit SQL predicates, never in mutable connection state.
    let request_pool = PgPoolOptions::new()
        .max_connections(request_pool_connections)
        .connect(database_url)
        .await?;
    let task_pool = PgPoolOptions::new()
        .max_connections(task_pool_connections)
        .connect(database_url)
        .await?;

    let mail_delivery = Arc::new(SmtpMailDelivery::new(
        &mail.smtp_host,
        mail.smtp_port,
        &mail.from,
        mail.smtp_username,
        mail.smtp_password
            .map(|password| password.expose_secret().to_owned()),
        &mail.smtp_tls_mode,
    )?);
    let official_registry = extensions.official_registry;
    let registry = Arc::new(
        GitHubRegistry::new()
            .map_err(|error| format!("cannot initialize extension registry: {error}"))?,
    );
    let official_extension_releases: Arc<dyn OfficialExtensionReleases> =
        match extensions.local_extensions_dir {
            Some(directory) => Arc::new(LocalExtensionReleases::new(directory)),
            None => Arc::new(OfficialExtensionRegistry::new(
                registry.clone(),
                official_registry.clone(),
            )),
        };

    // One query cache shared by request handlers and background workers.
    // With CACHE_BACKEND=redis, replicas also share cached values and
    // extension network rate limits; Redis failures degrade to process
    // memory while the client reconnects in the background.
    let (query_cache, rate_limiter) =
        QueryCache::from_config(cache, &database_identity.to_string()).await?;
    extension_runtime::use_network_rate_limiter(rate_limiter);
    let task_repository =
        AttricatRepository::system(task_pool.clone()).with_cache(query_cache.clone());
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
    let maintenance_worker = maintenance::start(
        task_repository.clone(),
        object_store.clone(),
        value_history_retention_days,
        shutdown_receiver.clone(),
    );
    let solution_pack_housekeeping =
        solution_pack_housekeeping::start(task_repository.clone(), shutdown_receiver.clone());
    // Registered kinds have atomically-enqueued producers and token-fenced
    // handlers. Unmigrated kinds remain unregistered and cannot be leased.
    let task_worker = task_worker::start(
        task_repository.clone(),
        TaskHandlerRegistry::new(task_handlers).expect("task handler kinds are unique"),
        task_worker_config,
        shutdown_receiver.clone(),
    );
    let extension_event_delivery_coordinator = extension_runtime::start_event_delivery_coordinator(
        task_repository.clone(),
        object_store.clone(),
        shutdown_receiver.clone(),
    );
    let workflow_repository =
        AttricatRepository::system(task_pool.clone()).with_cache(query_cache.clone());
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

    let stream_control = StreamControl::new(
        http.max_event_streams,
        http.max_event_streams_per_principal,
        http.event_stream_lifetime,
    );
    let listener = tokio::net::TcpListener::bind(http.bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "API listening");
    axum::serve(
        listener,
        router(AppState {
            repository: AttricatRepository::system(request_pool.clone())
                .with_cache(query_cache.clone()),
            agent_provider,
            official_extension_releases,
            registry,
            official_registry,
            object_store,
            extension_runtime,
            file_access_policy: Arc::new(AllowFileAccess),
            mail_delivery,
            password_reset_url: mail.password_reset_url,
            workspace_invitation_url: mail.workspace_invitation_url,
            workspace_onboarding_url: mail.workspace_onboarding_url,
            metrics,
            max_preview_relationship_depth: http.max_preview_relationship_depth,
            max_preview_relationship_items: http.max_preview_relationship_items,
            max_record_page_size: http.max_record_page_size,
            max_incoming_relationship_page_size: http.max_incoming_relationship_page_size,
            max_relationship_facet_nodes: http.max_relationship_facet_nodes,
            max_upload_file_bytes: http.max_upload_file_bytes,
            max_upload_files: http.max_upload_files,
            data_health_cache_ttl_seconds: http.data_health_cache_ttl_seconds,
            data_health_cache: Default::default(),
            session_cookie_secure: http.session_cookie_secure,
            allow_trusted_headers: false,
            request_permits: Arc::new(Semaphore::new(http.max_concurrent_requests)),
            request_timeout: http.request_timeout,
            stream_control: stream_control.clone(),
            readiness_permits: Arc::new(Semaphore::new(READINESS_PROBE_PERMITS)),
            default_body_limit: http.default_body_limit,
            devtools_enabled: http.devtools_enabled,
            demo_mode,
            sample_logins,
            build_info: BuildInfo {
                version: env!("CARGO_PKG_VERSION"),
                branch: env!("ATTRICAT_BUILD_BRANCH"),
                commit: env!("ATTRICAT_BUILD_COMMIT"),
            },
            web_dist_dir: http.web_dist_dir,
        }),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received");
        stream_control.shutdown();
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
    maintenance_worker.await?;
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

/// Sign-in identifier of the bootstrap workspace in `ATTRICAT_DEMO_MODE`.
const DEMO_LOGIN_IDENTIFIER: &str = "demo.attricat.com";
/// Built-in roles that receive a seeded account beside the bootstrap owner,
/// from least to most privileged.
const SAMPLE_ACCOUNT_ROLES: [&str; 3] = ["viewer", "editor", "admin"];

/// Seeds `<role>@<owner domain>` accounts that share the owner's password, so
/// demos and local development can sign in at each built-in role.
async fn seed_sample_accounts(
    repository: &SystemRepository,
    workspace_id: Uuid,
    owner_email: &str,
    password: String,
    demo: bool,
) -> Result<SampleLogins, Box<dyn std::error::Error + Send + Sync>> {
    let domain = owner_email
        .split_once('@')
        .map(|(_, domain)| domain)
        .ok_or("ATTRICAT_BOOTSTRAP_OWNER_EMAIL must be a valid email address")?;
    let mut accounts = Vec::new();
    for role in SAMPLE_ACCOUNT_ROLES {
        let email = format!("{role}@{domain}");
        // An owner configured as e.g. admin@ keeps only the owner role.
        if email == owner_email {
            continue;
        }
        repository
            .ensure_sample_account(workspace_id, &email, role)
            .await?;
        repository
            .ensure_bootstrap_local_password(&email, password.clone())
            .await?;
        accounts.push(SampleAccount {
            role: role.to_owned(),
            email,
        });
    }
    accounts.push(SampleAccount {
        role: "owner".to_owned(),
        email: owner_email.to_owned(),
    });
    Ok(SampleLogins {
        demo,
        login_identifier: repository.workspace_login_identifier(workspace_id).await?,
        password,
        accounts,
    })
}
