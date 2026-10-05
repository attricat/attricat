#![allow(dead_code, unused_imports)]

mod database;
mod extension_fixtures;
mod members;
mod requests;

pub use database::*;
pub use extension_fixtures::*;
pub use members::*;
pub use requests::*;

use std::{net::SocketAddr, sync::Arc};

use tokio::sync::Mutex;

use api::{
    extension_registry::{
        DiscoveredRelease, GitHubRegistry, GitHubRepository, RegistryError, ReleaseAsset,
    },
    extension_runtime::{ExtensionRuntime, ExtensionRuntimeConfig},
    file_access::{AllowFileAccess, FileAccessPolicy},
    http::{AppState, BuildInfo, router},
    mail::{MailDelivery, MailError},
    repository::CatalogRepository,
    solution_pack_extensions::OfficialExtensionReleases,
    storage::{FakeObjectStore, ObjectStore},
    telemetry::init_metrics,
};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
pub use reqwest::{Client, StatusCode};
pub use serde_json::{Value, json};
pub use sqlx::PgPool;
pub use tokio::{net::TcpListener, task::JoinHandle};
pub use uuid::Uuid;

/// An in-memory official extension registry. It lists no extensions until a
/// test publishes releases, so tests never reach GitHub.
#[derive(Default)]
pub struct FakeOfficialExtensions {
    releases: std::sync::Mutex<Vec<(String, DiscoveredRelease, Vec<u8>)>>,
    unavailable: std::sync::atomic::AtomicBool,
}

impl FakeOfficialExtensions {
    pub fn publish(&self, extension_id: &str, tag_name: &str, archive: Vec<u8>) {
        let mut releases = self.releases.lock().unwrap();
        let id = releases.len() as u64 + 1;
        releases.push((
            extension_id.to_owned(),
            DiscoveredRelease {
                source: format!("github:attricat/{extension_id}"),
                release_id: id,
                tag_name: tag_name.to_owned(),
                name: tag_name.to_owned(),
                published_at: None,
                asset: ReleaseAsset {
                    id,
                    name: format!("{extension_id}.tar.zst"),
                    download_url: format!(
                        "https://github.com/attricat/{extension_id}/releases/download/{tag_name}/{extension_id}.tar.zst"
                    ),
                },
            },
            archive,
        ));
    }

    /// Replaces the asset behind an existing release, as a re-uploaded asset would.
    pub fn replace_archive(&self, extension_id: &str, tag_name: &str, archive: Vec<u8>) {
        let mut releases = self.releases.lock().unwrap();
        let release = releases
            .iter_mut()
            .find(|(id, release, _)| id == extension_id && release.tag_name == tag_name)
            .expect("published release");
        release.2 = archive;
    }

    pub fn set_unavailable(&self, unavailable: bool) {
        self.unavailable
            .store(unavailable, std::sync::atomic::Ordering::SeqCst);
    }

    fn check_available(&self) -> Result<(), RegistryError> {
        if self.unavailable.load(std::sync::atomic::Ordering::SeqCst) {
            Err(RegistryError::Unavailable)
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl OfficialExtensionReleases for FakeOfficialExtensions {
    async fn releases(&self, extension_id: &str) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        self.check_available()?;
        Ok(self
            .releases
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _, _)| id == extension_id)
            .map(|(_, release, _)| release.clone())
            .collect())
    }

    async fn download(&self, release: &DiscoveredRelease) -> Result<Vec<u8>, RegistryError> {
        self.check_available()?;
        self.releases
            .lock()
            .unwrap()
            .iter()
            .find(|(_, candidate, _)| candidate.release_id == release.release_id)
            .map(|(_, _, archive)| archive.clone())
            .ok_or(RegistryError::Unavailable)
    }
}

#[derive(Default)]
pub struct TestMailDelivery {
    workspace_invitation_urls: Mutex<Vec<(String, String)>>,
}

impl TestMailDelivery {
    pub async fn workspace_invitation_url(&self, recipient: &str) -> Option<String> {
        self.workspace_invitation_urls
            .lock()
            .await
            .iter()
            .rev()
            .find_map(|(delivered_to, url)| (delivered_to == recipient).then(|| url.clone()))
    }
}

#[async_trait]
impl MailDelivery for TestMailDelivery {
    async fn deliver_password_reset(&self, _: &str, _: &str) -> Result<(), MailError> {
        Ok(())
    }
    async fn deliver_workspace_invitation(
        &self,
        recipient: &str,
        invitation_url: &str,
    ) -> Result<(), MailError> {
        self.workspace_invitation_urls
            .lock()
            .await
            .push((recipient.to_owned(), invitation_url.to_owned()));
        Ok(())
    }
    async fn deliver_workspace_onboarding(&self, _: &str, _: &str) -> Result<(), MailError> {
        Ok(())
    }
}

pub async fn start_server(pool: PgPool) -> (String, JoinHandle<()>) {
    start_server_with_data_health_cache_ttl(pool, 0).await
}

pub async fn start_server_with_devtools(pool: PgPool) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store_with_devtools(
        pool,
        0,
        true,
        Arc::new(FakeObjectStore::available()),
        Arc::new(AllowFileAccess),
        Arc::new(TestMailDelivery::default()),
        true,
    )
    .await
}

pub async fn start_server_with_test_mail(
    pool: PgPool,
    mail_delivery: Arc<TestMailDelivery>,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store(
        pool,
        0,
        true,
        Arc::new(FakeObjectStore::available()),
        Arc::new(AllowFileAccess),
        mail_delivery,
    )
    .await
}

pub async fn start_server_with_object_store(
    pool: PgPool,
    object_store: Arc<FakeObjectStore>,
) -> (String, JoinHandle<()>) {
    start_server_with_custom_object_store(pool, object_store).await
}

pub async fn start_server_with_custom_object_store(
    pool: PgPool,
    object_store: Arc<dyn ObjectStore>,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store(
        pool,
        0,
        true,
        object_store,
        Arc::new(AllowFileAccess),
        Arc::new(TestMailDelivery::default()),
    )
    .await
}

pub async fn start_server_with_file_access_policy(
    pool: PgPool,
    object_store: Arc<FakeObjectStore>,
    file_access_policy: Arc<dyn FileAccessPolicy>,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store(
        pool,
        0,
        true,
        object_store,
        file_access_policy,
        Arc::new(TestMailDelivery::default()),
    )
    .await
}

pub async fn start_session_server(pool: PgPool) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode(pool, 0, false).await
}

pub const BOOTSTRAP_WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
pub const BOOTSTRAP_OWNER_ID: &str = "00000000-0000-4000-8000-000000000201";

pub fn bootstrap_workspace_id() -> Uuid {
    BOOTSTRAP_WORKSPACE_ID.parse().unwrap()
}

pub async fn start_server_with_data_health_cache_ttl(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode(pool, data_health_cache_ttl_seconds, true).await
}

async fn start_server_with_auth_mode(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
    allow_trusted_headers: bool,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store(
        pool,
        data_health_cache_ttl_seconds,
        allow_trusted_headers,
        Arc::new(FakeObjectStore::available()),
        Arc::new(AllowFileAccess),
        Arc::new(TestMailDelivery::default()),
    )
    .await
}

async fn start_server_with_auth_mode_and_store(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
    allow_trusted_headers: bool,
    object_store: Arc<dyn ObjectStore>,
    file_access_policy: Arc<dyn FileAccessPolicy>,
    mail_delivery: Arc<dyn MailDelivery>,
) -> (String, JoinHandle<()>) {
    start_server_with_auth_mode_and_store_with_devtools(
        pool,
        data_health_cache_ttl_seconds,
        allow_trusted_headers,
        object_store,
        file_access_policy,
        mail_delivery,
        false,
    )
    .await
}

async fn start_server_with_auth_mode_and_store_with_devtools(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
    allow_trusted_headers: bool,
    object_store: Arc<dyn ObjectStore>,
    file_access_policy: Arc<dyn FileAccessPolicy>,
    mail_delivery: Arc<dyn MailDelivery>,
    devtools_enabled: bool,
) -> (String, JoinHandle<()>) {
    start_configured_server(
        pool,
        data_health_cache_ttl_seconds,
        allow_trusted_headers,
        object_store,
        file_access_policy,
        mail_delivery,
        devtools_enabled,
        |_| {},
    )
    .await
}

pub async fn start_server_with_config(
    pool: PgPool,
    configure: impl FnOnce(&mut AppState),
) -> (String, JoinHandle<()>) {
    start_configured_server(
        pool,
        0,
        true,
        Arc::new(FakeObjectStore::available()),
        Arc::new(AllowFileAccess),
        Arc::new(TestMailDelivery::default()),
        false,
        configure,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn start_configured_server(
    pool: PgPool,
    data_health_cache_ttl_seconds: u64,
    allow_trusted_headers: bool,
    object_store: Arc<dyn ObjectStore>,
    file_access_policy: Arc<dyn FileAccessPolicy>,
    mail_delivery: Arc<dyn MailDelivery>,
    devtools_enabled: bool,
    configure: impl FnOnce(&mut AppState),
) -> (String, JoinHandle<()>) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let owner_id = BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap();
    let system = CatalogRepository::system(pool.clone());
    for workspace in system.active_workspace_ids().await.unwrap() {
        system.initialize_workspace(workspace).await.unwrap();
    }
    let membership_id = Uuid::from_u128(0x00000000000040008000000000000202);
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'api-test-owner@example.test') ON CONFLICT (id) DO NOTHING")
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3) ON CONFLICT (workspace_id, user_id) DO NOTHING")
        .bind(membership_id)
        .bind(workspace_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    let membership_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000101', 'workspace', $2) ON CONFLICT DO NOTHING")
        .bind(Uuid::from_u128(0x00000000000040008000000000000203))
        .bind(workspace_id)
        .bind(membership_id)
        .execute(&pool)
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_agent_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_audit_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_extension_registry_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_workflow_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_entity_publication_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_retention_hold_permissions()
        .await
        .unwrap();
    CatalogRepository::system(pool.clone())
        .ensure_solution_pack_permissions()
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let mut state = AppState {
        repository: CatalogRepository::system(pool.clone()),
        agent_provider: None,
        registry: Arc::new(GitHubRegistry::new().unwrap()),
        official_registry: "attricat/attricat-extensions"
            .parse::<GitHubRepository>()
            .unwrap(),
        official_extension_releases: Arc::new(FakeOfficialExtensions::default()),
        extension_runtime: ExtensionRuntime::new(
            object_store.clone(),
            ExtensionRuntimeConfig::default(),
        )
        .unwrap(),
        object_store,
        file_access_policy,
        mail_delivery,
        password_reset_url: "http://127.0.0.1/password-reset/confirm".to_owned(),
        workspace_invitation_url: "http://127.0.0.1/invitations/accept".to_owned(),
        workspace_onboarding_url: "http://127.0.0.1/onboarding".to_owned(),
        metrics: init_metrics().unwrap(),
        max_preview_relationship_depth: 3,
        max_preview_relationship_items: 10,
        max_entity_page_size: 100,
        max_incoming_relationship_page_size: 50,
        max_relationship_facet_nodes: 100,
        max_upload_file_bytes: 50 * 1024 * 1024,
        max_upload_files: 10,
        data_health_cache_ttl_seconds,
        data_health_cache: Default::default(),
        session_cookie_secure: false,
        allow_trusted_headers,
        request_permits: Arc::new(tokio::sync::Semaphore::new(256)),
        request_timeout: std::time::Duration::from_secs(30),
        stream_control: Default::default(),
        readiness_permits: Arc::new(tokio::sync::Semaphore::new(2)),
        default_body_limit: 2 * 1024 * 1024,
        devtools_enabled,
        demo_mode: false,
        sample_logins: None,
        build_info: BuildInfo {
            version: env!("CARGO_PKG_VERSION"),
            branch: env!("ATTRICAT_BUILD_BRANCH"),
            commit: env!("ATTRICAT_BUILD_COMMIT"),
        },
    };
    configure(&mut state);
    let router = router(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    (format!("http://{address}"), server)
}

pub fn authenticated_client() -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_static(BOOTSTRAP_OWNER_ID),
    );
    headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

pub async fn create_blueprint(client: &Client, base_url: &str, definition: &str) -> Value {
    let blueprint: Value = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{}/versions/{}/publish",
            blueprint["blueprint"]["id"].as_str().unwrap(),
            blueprint["blueprint"]["version"].as_i64().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn create_entity(client: &Client, base_url: &str, blueprint: &Value) -> Value {
    client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"],
            },
            "values": [],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
