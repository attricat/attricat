//! Deployment configuration shared by the API, file worker and migrate
//! binaries.
//!
//! [`Config::load`] reads every setting once at startup. Plain settings come
//! from the process environment, after `.env` is loaded. Secrets come only
//! from files in the Docker secrets directory (`/run/secrets` unless
//! `ATTRICAT_SECRETS_DIR` names another), never from environment variables, so
//! they do not appear in `docker inspect`, process listings or crash dumps.

use std::{
    collections::HashMap,
    fmt, fs, io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use secrecy::SecretString;
use uuid::Uuid;

use crate::{
    agents::AgentProviderConfig,
    blueprint_migration_worker::BlueprintMigrationBatchConfig,
    constants::{
        DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS, DEFAULT_FILE_UPLOAD_MAX_BYTES,
        DEFAULT_FILE_UPLOAD_MAX_FILES, DEFAULT_HTTP_DEFAULT_BODY_BYTES,
        DEFAULT_HTTP_EVENT_STREAM_LIFETIME_SECONDS, DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
        DEFAULT_HTTP_MAX_EVENT_STREAMS, DEFAULT_HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL,
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS, DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE,
        DEFAULT_PREVIEW_RELATIONSHIP_DEPTH, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS,
        DEFAULT_RECORD_PAGE_SIZE, DEFAULT_RELATIONSHIP_FACET_NODES, REQUEST_POOL_CONNECTIONS,
        TASK_POOL_CONNECTIONS,
    },
    event_dispatcher::DispatcherConfig,
    extension_policy::ExtensionPolicy,
    extension_registry::{DEFAULT_OFFICIAL_REGISTRY, GitHubRepository},
    file_worker::WorkerConfig,
    maintenance::RetentionConfig,
    storage::StorageConfig,
    task_worker::TaskWorkerConfig,
};
use attricat_cache::CacheConfig;

pub const DEFAULT_SECRETS_DIR: &str = "/run/secrets";
const MAX_GLOBAL_POOL_CONNECTIONS: u32 = 100;
const DEFAULT_BIND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
const DEFAULT_FILE_WORKER_OPERATIONS_BIND_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3001);
const DEFAULT_FILE_WORKER_POLL_MILLISECONDS: u64 = 500;
const DEFAULT_WORKSPACE_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000002);
const DEFAULT_BOOTSTRAP_OWNER_EMAIL: &str = "owner@example.test";
// Local development defaults: a mail catcher and the Vite dev server.
const DEFAULT_SMTP_HOST: &str = "127.0.0.1";
const DEFAULT_SMTP_PORT: u16 = 1025;
const DEFAULT_SMTP_TLS_MODE: &str = "starttls";
const DEFAULT_MAIL_FROM: &str = "Attricat <no-reply@attricat.local>";
const DEFAULT_PASSWORD_RESET_URL: &str = "http://127.0.0.1:5173/password-reset/confirm";
const DEFAULT_WORKSPACE_INVITATION_URL: &str = "http://127.0.0.1:5173/invitations/accept";
const DEFAULT_WORKSPACE_ONBOARDING_URL: &str = "http://127.0.0.1:5173/onboarding";

/// Names of the files the secrets directory may hold.
pub mod secret {
    pub const DATABASE_URL: &str = "database_url";
    pub const S3_SECRET_ACCESS_KEY: &str = "s3_secret_access_key";
    pub const SMTP_PASSWORD: &str = "smtp_password";
    pub const LLM_API_KEY: &str = "llm_api_key";
    pub const REDIS_URL: &str = "redis_url";
    pub const BOOTSTRAP_OWNER_PASSWORD: &str = "bootstrap_owner_password";
    pub const E2E_FIXTURE_PASSWORD: &str = "e2e_fixture_password";
    pub const FILE_WORKER_METRICS_TOKEN: &str = "file_worker_metrics_token";
    pub const EXTENSION_CREDENTIAL_KEY: &str = "extension_credential_key";
}

/// Environment variables that used to carry a secret. Startup refuses them
/// rather than silently ignoring a credential an operator meant to apply.
/// `DATABASE_URL` is absent: SQLx tooling and tests still read it, so the
/// binaries ignore it instead.
const RETIRED_SECRET_VARIABLES: [(&str, &str); 7] = [
    ("S3_SECRET_ACCESS_KEY", secret::S3_SECRET_ACCESS_KEY),
    ("SMTP_PASSWORD", secret::SMTP_PASSWORD),
    ("LLM_API_KEY", secret::LLM_API_KEY),
    ("REDIS_URL", secret::REDIS_URL),
    (
        "ATTRICAT_BOOTSTRAP_OWNER_PASSWORD",
        secret::BOOTSTRAP_OWNER_PASSWORD,
    ),
    (
        "ATTRICAT_E2E_FIXTURE_PASSWORD",
        secret::E2E_FIXTURE_PASSWORD,
    ),
    (
        "FILE_WORKER_METRICS_TOKEN",
        secret::FILE_WORKER_METRICS_TOKEN,
    ),
];

/// Every setting the binaries read. Each binary loads all of it, so one
/// deployment environment configures and validates every role the same way.
pub struct Config {
    pub telemetry: TelemetryConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
    pub http: HttpConfig,
    pub bootstrap: BootstrapConfig,
    pub mail: MailConfig,
    pub cache: CacheConfig,
    /// `None` when no `llm_api_key` secret is configured: agents are then
    /// unavailable.
    pub agent_provider: Option<AgentProviderConfig>,
    pub extensions: ExtensionsConfig,
    pub retention: RetentionConfig,
    pub event_dispatcher: DispatcherConfig,
    pub task_worker: TaskWorkerConfig,
    pub blueprint_migration: BlueprintMigrationBatchConfig,
    pub file_worker: FileWorkerConfig,
}

pub struct TelemetryConfig {
    /// `RUST_LOG` directives.
    pub log_filter: Option<String>,
    pub otlp_traces_endpoint: Option<String>,
}

pub struct DatabaseConfig {
    pub url: SecretString,
    pub request_pool_connections: u32,
    pub task_pool_connections: u32,
    pub auto_migrate: bool,
}

pub struct HttpConfig {
    pub bind_addr: SocketAddr,
    pub web_dist_dir: Option<String>,
    pub session_cookie_secure: bool,
    pub devtools_enabled: bool,
    pub request_timeout: Duration,
    pub max_concurrent_requests: usize,
    pub default_body_limit: usize,
    pub max_event_streams: usize,
    pub max_event_streams_per_principal: usize,
    pub event_stream_lifetime: Duration,
    pub max_preview_relationship_depth: u8,
    pub max_preview_relationship_items: u32,
    pub max_record_page_size: u32,
    pub max_incoming_relationship_page_size: u32,
    pub max_relationship_facet_nodes: u32,
    pub max_upload_file_bytes: u64,
    pub max_upload_files: usize,
    pub data_health_cache_ttl_seconds: u64,
}

pub struct BootstrapConfig {
    pub workspace_id: Uuid,
    pub workspace_name: String,
    /// Trimmed and lowercased.
    pub owner_email: String,
    pub owner_id: Option<Uuid>,
    pub owner_password: Option<SecretString>,
    pub demo_mode: bool,
    /// Implied by `demo_mode`; requires `owner_password`.
    pub sample_accounts: bool,
    pub e2e_fixture: Option<E2eFixture>,
}

/// An independent principal for browser E2E fixture setup.
pub struct E2eFixture {
    /// Trimmed and lowercased.
    pub email: String,
    pub password: SecretString,
}

pub struct MailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls_mode: String,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<SecretString>,
    pub from: String,
    pub password_reset_url: String,
    pub workspace_invitation_url: String,
    pub workspace_onboarding_url: String,
}

pub struct ExtensionsConfig {
    pub official_registry: GitHubRepository,
    pub policy: ExtensionPolicy,
    /// Development only: solution packs install locally built extensions
    /// from this directory. Requires `ATTRICAT_DEVTOOLS`.
    pub local_extensions_dir: Option<String>,
    /// AES-256 key encrypting extension connection credentials; without it
    /// connections cannot be created or used.
    pub credential_key: Option<[u8; 32]>,
    /// Externally reachable base URL of extension-issued public file links
    /// (`EXTENSION_PUBLIC_FILE_BASE_URL`, ending in
    /// `/api/public/extension-files/`); without it links are unavailable.
    pub public_file_base_url: Option<url::Url>,
    /// Externally reachable URL of the public OAuth callback
    /// (`EXTENSION_OAUTH_CALLBACK_URL`, the `/api/public/extension-oauth/callback`
    /// route), which operators register as their apps' redirect URI. Without
    /// it, authorization-code connections are authorized by pasting codes.
    pub oauth_callback_url: Option<url::Url>,
    /// Where the callback sends the browser afterwards
    /// (`EXTENSION_OAUTH_RETURN_URL`, the web app's `/manage/extensions/`,
    /// ending in '/'). Defaults to that path on the callback URL's origin.
    pub oauth_return_url: Option<url::Url>,
    /// Development only: egress overrides that let the extension broker
    /// reach local marketplace mocks. Requires `ATTRICAT_DEVTOOLS`.
    pub development_egress: Option<DevelopmentEgressConfig>,
}

/// `EXTENSION_DEV_EGRESS_RESOLVE` (`host=ip:port,*.suffix=ip:port`) and
/// `EXTENSION_DEV_EGRESS_CA_FILE` (a PEM bundle).
pub struct DevelopmentEgressConfig {
    pub resolve: Vec<(String, SocketAddr)>,
    pub ca_file: Option<String>,
}

pub struct FileWorkerConfig {
    pub worker: WorkerConfig,
    pub operations_bind_addr: SocketAddr,
    /// Required when `operations_bind_addr` is not loopback.
    pub metrics_token: Option<SecretString>,
    pub poll_interval: Duration,
}

pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// `main` reports a returned error with `Debug`; show operators the message.
impl fmt::Debug for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl From<String> for ConfigError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for ConfigError {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

impl Config {
    /// Loads `.env` when present, then reads the process environment and the
    /// secrets directory.
    pub fn load() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();
        let vars = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Self::from_source(&Source::new(vars))
    }

    fn from_source(source: &Source) -> Result<Self, ConfigError> {
        source.reject_secret_variables()?;
        let get = |name: &str| source.owned(name);

        let devtools_enabled = source.boolean("ATTRICAT_DEVTOOLS", false)?;
        let demo_mode = source.boolean("ATTRICAT_DEMO_MODE", false)?;
        let mut agent_provider =
            AgentProviderConfig::from_values(get, source.secret(secret::LLM_API_KEY)?)
                .map_err(|error| format!("invalid agent provider configuration: {error}"))?;
        // Demo visitors share accounts, so the agent may only read there.
        if let Some(config) = &mut agent_provider {
            config.read_only = demo_mode;
        }

        let local_extensions_dir = source.owned("SOLUTION_PACK_LOCAL_EXTENSIONS_DIR");
        if local_extensions_dir.is_some() && !devtools_enabled {
            return Err(
                "SOLUTION_PACK_LOCAL_EXTENSIONS_DIR requires ATTRICAT_DEVTOOLS=true".into(),
            );
        }

        Ok(Self {
            telemetry: TelemetryConfig {
                log_filter: source.owned("RUST_LOG"),
                otlp_traces_endpoint: source.owned("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"),
            },
            database: DatabaseConfig {
                url: source.required_secret(secret::DATABASE_URL)?,
                request_pool_connections: source.pool_connections(
                    "DATABASE_REQUEST_POOL_CONNECTIONS",
                    REQUEST_POOL_CONNECTIONS,
                )?,
                task_pool_connections: source
                    .pool_connections("DATABASE_TASK_POOL_CONNECTIONS", TASK_POOL_CONNECTIONS)?,
                auto_migrate: source.boolean("ATTRICAT_AUTO_MIGRATE", true)?,
            },
            storage: StorageConfig::from_values(get, source.secret(secret::S3_SECRET_ACCESS_KEY)?)
                .map_err(|error| format!("invalid object storage configuration: {error}"))?,
            http: HttpConfig {
                bind_addr: source.parsed("BIND_ADDR", DEFAULT_BIND_ADDR)?,
                web_dist_dir: source.owned("WEB_DIST_DIR"),
                session_cookie_secure: source.boolean("SESSION_COOKIE_SECURE", true)?,
                devtools_enabled,
                request_timeout: Duration::from_secs(source.positive(
                    "HTTP_REQUEST_TIMEOUT_SECONDS",
                    DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS as u64,
                )?),
                max_concurrent_requests: source.positive(
                    "HTTP_MAX_CONCURRENT_REQUESTS",
                    DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
                )?,
                default_body_limit: source
                    .positive("HTTP_DEFAULT_BODY_BYTES", DEFAULT_HTTP_DEFAULT_BODY_BYTES)?,
                max_event_streams: source
                    .positive("HTTP_MAX_EVENT_STREAMS", DEFAULT_HTTP_MAX_EVENT_STREAMS)?,
                max_event_streams_per_principal: source.positive(
                    "HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL",
                    DEFAULT_HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL,
                )?,
                event_stream_lifetime: Duration::from_secs(source.positive(
                    "HTTP_EVENT_STREAM_LIFETIME_SECONDS",
                    DEFAULT_HTTP_EVENT_STREAM_LIFETIME_SECONDS,
                )?),
                max_preview_relationship_depth: source.parsed(
                    "PREVIEW_MAX_RELATIONSHIP_DEPTH",
                    DEFAULT_PREVIEW_RELATIONSHIP_DEPTH,
                )?,
                max_preview_relationship_items: source.parsed(
                    "PREVIEW_MAX_RELATIONSHIP_ITEMS",
                    DEFAULT_PREVIEW_RELATIONSHIP_ITEMS,
                )?,
                max_record_page_size: source
                    .parsed("RECORD_MAX_PAGE_SIZE", DEFAULT_RECORD_PAGE_SIZE)?,
                max_incoming_relationship_page_size: source.parsed(
                    "INCOMING_RELATIONSHIP_MAX_PAGE_SIZE",
                    DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE,
                )?,
                max_relationship_facet_nodes: source.parsed(
                    "RELATIONSHIP_FACET_MAX_NODES",
                    DEFAULT_RELATIONSHIP_FACET_NODES,
                )?,
                max_upload_file_bytes: source.positive(
                    "FILE_UPLOAD_MAX_BYTES",
                    DEFAULT_FILE_UPLOAD_MAX_BYTES as u64,
                )?,
                max_upload_files: source
                    .positive("FILE_UPLOAD_MAX_FILES", DEFAULT_FILE_UPLOAD_MAX_FILES)?,
                data_health_cache_ttl_seconds: source.parsed(
                    "DATA_HEALTH_CACHE_TTL_SECONDS",
                    DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS,
                )?,
            },
            bootstrap: bootstrap(source, demo_mode)?,
            mail: MailConfig {
                smtp_host: source.var("SMTP_HOST").unwrap_or(DEFAULT_SMTP_HOST).to_owned(),
                smtp_port: source.parsed("SMTP_PORT", DEFAULT_SMTP_PORT)?,
                smtp_tls_mode: source.var("SMTP_TLS_MODE").unwrap_or(DEFAULT_SMTP_TLS_MODE).to_owned(),
                smtp_username: get("SMTP_USERNAME"),
                smtp_password: source.secret(secret::SMTP_PASSWORD)?,
                from: source.var("MAIL_FROM").unwrap_or(DEFAULT_MAIL_FROM).to_owned(),
                password_reset_url: source.var("PASSWORD_RESET_URL").unwrap_or(DEFAULT_PASSWORD_RESET_URL).to_owned(),
                workspace_invitation_url: source.var("WORKSPACE_INVITATION_URL").unwrap_or(DEFAULT_WORKSPACE_INVITATION_URL).to_owned(),
                workspace_onboarding_url: source.var("WORKSPACE_ONBOARDING_URL").unwrap_or(DEFAULT_WORKSPACE_ONBOARDING_URL).to_owned(),
            },
            cache: CacheConfig::parse(
                source.var("CACHE_BACKEND"),
                source
                    .secret(secret::REDIS_URL)?
                    .as_ref()
                    .map(secrecy::ExposeSecret::expose_secret),
                source.var("CACHE_MAX_ENTRIES"),
                source.var("CACHE_KEY_PREFIX"),
            )?,
            agent_provider,
            extensions: ExtensionsConfig {
                official_registry: source
                    .var("EXTENSION_OFFICIAL_REGISTRY")
                    .unwrap_or(DEFAULT_OFFICIAL_REGISTRY)
                    .parse()
                    .map_err(|error| format!("invalid EXTENSION_OFFICIAL_REGISTRY: {error}"))?,
                policy: ExtensionPolicy::parse(
                    source.var("EXTENSIONS_MODE"),
                    source.var("EXTENSION_DENYLIST"),
                )?,
                local_extensions_dir,
                credential_key: credential_key(source)?,
                public_file_base_url: source
                    .var("EXTENSION_PUBLIC_FILE_BASE_URL")
                    .map(|value| {
                        plain_http_url(value)
                            .filter(|url| url.path().ends_with('/'))
                            .ok_or_else(|| {
                                ConfigError::from(
                                    "EXTENSION_PUBLIC_FILE_BASE_URL must be an HTTP(S) URL ending in '/'",
                                )
                            })
                    })
                    .transpose()?,
                oauth_callback_url: oauth_callback_url(source)?,
                oauth_return_url: oauth_return_url(source)?,
                development_egress: development_egress(source, devtools_enabled)?,
            },
            retention: RetentionConfig::from_values(get)
                .map_err(|error| format!("invalid retention configuration: {error}"))?,
            event_dispatcher: DispatcherConfig::from_values(get)
                .map_err(|error| format!("invalid event dispatcher configuration: {error}"))?,
            task_worker: TaskWorkerConfig::from_values(get)
                .map_err(|error| format!("invalid task worker configuration: {error}"))?,
            blueprint_migration: BlueprintMigrationBatchConfig::from_values(get)
                .map_err(|error| format!("invalid blueprint migration configuration: {error}"))?,
            file_worker: file_worker(source)?,
        })
    }
}

fn bootstrap(source: &Source, demo_mode: bool) -> Result<BootstrapConfig, ConfigError> {
    // A blank address is a mistake, not a request for the test default.
    let owner_email = source
        .vars
        .get("ATTRICAT_BOOTSTRAP_OWNER_EMAIL")
        .map_or(DEFAULT_BOOTSTRAP_OWNER_EMAIL, String::as_str)
        .trim()
        .to_lowercase();
    if !owner_email.contains('@') {
        return Err("ATTRICAT_BOOTSTRAP_OWNER_EMAIL must be a valid email address".into());
    }
    let owner_password = source.secret(secret::BOOTSTRAP_OWNER_PASSWORD)?;
    // A public demo signs visitors in to the bootstrap workspace with seeded
    // per-role accounts, which the web app offers to pre-fill.
    let sample_accounts = demo_mode || source.boolean("ATTRICAT_SAMPLE_ACCOUNTS", false)?;
    if sample_accounts && owner_password.is_none() {
        return Err(
            "ATTRICAT_SAMPLE_ACCOUNTS and ATTRICAT_DEMO_MODE require the bootstrap_owner_password secret"
                .into(),
        );
    }
    // Test-only, and effective only when both halves are configured.
    let e2e_fixture = source
        .var("ATTRICAT_E2E_FIXTURE_EMAIL")
        .zip(source.secret(secret::E2E_FIXTURE_PASSWORD)?)
        .map(|(email, password)| E2eFixture {
            email: email.trim().to_lowercase(),
            password,
        });
    Ok(BootstrapConfig {
        workspace_id: source.parsed("ATTRICAT_WORKSPACE_ID", DEFAULT_WORKSPACE_ID)?,
        workspace_name: source
            .owned("ATTRICAT_BOOTSTRAP_WORKSPACE_NAME")
            .unwrap_or_else(|| {
                if demo_mode {
                    "Demo"
                } else {
                    "Default workspace"
                }
                .to_owned()
            }),
        owner_email,
        owner_id: source
            .var("ATTRICAT_BOOTSTRAP_OWNER_ID")
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| "ATTRICAT_BOOTSTRAP_OWNER_ID must be a UUID")
            })
            .transpose()?,
        owner_password,
        demo_mode,
        sample_accounts,
        e2e_fixture,
    })
}

/// An absolute HTTP(S) URL without credentials, query or fragment. A
/// trailing ':' without a port is refused: URL parsing would silently use the
/// scheme's default port, as when `$API_PORT` was unset in `host:$API_PORT`.
fn plain_http_url(value: &str) -> Option<url::Url> {
    let authority = value.split_once("://").map_or(value, |(_, rest)| {
        rest.split(['/', '?', '#']).next().unwrap_or(rest)
    });
    if authority.ends_with(':') {
        return None;
    }
    url::Url::parse(value).ok().filter(|url| {
        matches!(url.scheme(), "https" | "http")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

fn oauth_callback_url(source: &Source) -> Result<Option<url::Url>, ConfigError> {
    source
        .var("EXTENSION_OAUTH_CALLBACK_URL")
        .map(|value| {
            plain_http_url(value)
                .filter(|url| url.path().ends_with("/public/extension-oauth/callback"))
                .ok_or_else(|| {
                    ConfigError::from(
                        "EXTENSION_OAUTH_CALLBACK_URL must be an HTTP(S) URL ending in /api/public/extension-oauth/callback",
                    )
                })
        })
        .transpose()
}

fn oauth_return_url(source: &Source) -> Result<Option<url::Url>, ConfigError> {
    source
        .var("EXTENSION_OAUTH_RETURN_URL")
        .map(|value| {
            plain_http_url(value)
                .filter(|url| url.path().ends_with('/'))
                .ok_or_else(|| {
                    ConfigError::from(
                        "EXTENSION_OAUTH_RETURN_URL must be an HTTP(S) URL ending in '/'",
                    )
                })
        })
        .transpose()
}

fn credential_key(source: &Source) -> Result<Option<[u8; 32]>, ConfigError> {
    use base64::Engine as _;
    let Some(secret) = source.secret(secret::EXTENSION_CREDENTIAL_KEY)? else {
        return Ok(None);
    };
    base64::engine::general_purpose::STANDARD
        .decode(secrecy::ExposeSecret::expose_secret(&secret).trim())
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .map(Some)
        .ok_or_else(|| "the extension_credential_key secret must be 32 base64-encoded bytes".into())
}

fn development_egress(
    source: &Source,
    devtools_enabled: bool,
) -> Result<Option<DevelopmentEgressConfig>, ConfigError> {
    let resolve = source.var("EXTENSION_DEV_EGRESS_RESOLVE");
    let ca_file = source.owned("EXTENSION_DEV_EGRESS_CA_FILE");
    if resolve.is_none() && ca_file.is_none() {
        return Ok(None);
    }
    if !devtools_enabled {
        return Err("EXTENSION_DEV_EGRESS_* settings require ATTRICAT_DEVTOOLS=true".into());
    }
    let resolve = resolve
        .unwrap_or_default()
        .split(',')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (host, address) = entry.trim().split_once('=').ok_or_else(|| {
                ConfigError::from("EXTENSION_DEV_EGRESS_RESOLVE entries are host=ip:port")
            })?;
            let address = address.parse::<SocketAddr>().map_err(|_| {
                ConfigError::from("EXTENSION_DEV_EGRESS_RESOLVE has an invalid address")
            })?;
            Ok((host.to_ascii_lowercase(), address))
        })
        .collect::<Result<Vec<_>, ConfigError>>()?;
    Ok(Some(DevelopmentEgressConfig { resolve, ca_file }))
}

fn file_worker(source: &Source) -> Result<FileWorkerConfig, ConfigError> {
    let operations_bind_addr = source.parsed(
        "FILE_WORKER_OPERATIONS_BIND_ADDR",
        DEFAULT_FILE_WORKER_OPERATIONS_BIND_ADDR,
    )?;
    let metrics_token = source.secret(secret::FILE_WORKER_METRICS_TOKEN)?;
    if !operations_bind_addr.ip().is_loopback() && metrics_token.is_none() {
        return Err(
            "the file_worker_metrics_token secret is required when FILE_WORKER_OPERATIONS_BIND_ADDR is not loopback"
                .into(),
        );
    }
    Ok(FileWorkerConfig {
        worker: WorkerConfig::from_values(|name| source.owned(name))?,
        operations_bind_addr,
        metrics_token,
        poll_interval: Duration::from_millis(source.positive(
            "FILE_WORKER_POLL_MILLISECONDS",
            DEFAULT_FILE_WORKER_POLL_MILLISECONDS,
        )?),
    })
}

/// The environment snapshot and secrets directory configuration is read from.
struct Source {
    vars: HashMap<String, String>,
    secrets_dir: PathBuf,
}

impl Source {
    fn new(vars: HashMap<String, String>) -> Self {
        let secrets_dir = PathBuf::from(
            vars.get("ATTRICAT_SECRETS_DIR")
                .filter(|dir| !dir.is_empty())
                .map_or(DEFAULT_SECRETS_DIR, String::as_str),
        );
        Self { vars, secrets_dir }
    }

    /// An empty value counts as unset, so `.env` templates can list a
    /// setting without choosing a value.
    fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    fn owned(&self, name: &str) -> Option<String> {
        self.var(name).map(str::to_owned)
    }

    fn reject_secret_variables(&self) -> Result<(), ConfigError> {
        for (variable, secret) in RETIRED_SECRET_VARIABLES {
            if self.var(variable).is_some() {
                return Err(format!(
                    "{variable} must not be set in the environment; provide it as the {secret} secret file in {} instead",
                    self.secrets_dir.display()
                )
                .into());
            }
        }
        Ok(())
    }

    /// Reads one secret file. A missing or empty file is an unset secret;
    /// trailing line breaks, which editors and `echo` add, are not part of
    /// the value.
    fn secret(&self, name: &str) -> Result<Option<SecretString>, ConfigError> {
        let path = self.secrets_dir.join(name);
        match fs::read_to_string(&path) {
            Ok(value) => {
                let value = value.trim_end_matches(['\r', '\n']);
                Ok((!value.is_empty()).then(|| SecretString::from(value.to_owned())))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!(
                "cannot read the {name} secret at {}: {error}",
                path.display()
            )
            .into()),
        }
    }

    fn required_secret(&self, name: &str) -> Result<SecretString, ConfigError> {
        self.secret(name)?.ok_or_else(|| {
            format!(
                "the {name} secret must be provided in {}",
                self.secrets_dir.display()
            )
            .into()
        })
    }

    fn parsed<T: FromStr>(&self, name: &str, default: T) -> Result<T, ConfigError> {
        match self.var(name) {
            Some(value) => value
                .parse()
                .map_err(|_| format!("{name} has an invalid value").into()),
            None => Ok(default),
        }
    }

    fn positive<T: FromStr + Default + PartialOrd>(
        &self,
        name: &str,
        default: T,
    ) -> Result<T, ConfigError> {
        match self.var(name) {
            Some(value) => value
                .parse()
                .ok()
                .filter(|value| *value > T::default())
                .ok_or_else(|| format!("{name} must be a positive integer").into()),
            None => Ok(default),
        }
    }

    fn boolean(&self, name: &str, default: bool) -> Result<bool, ConfigError> {
        match self.var(name) {
            None => Ok(default),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(_) => Err(format!("{name} must be `true` or `false`").into()),
        }
    }

    fn pool_connections(&self, name: &str, default: u32) -> Result<u32, ConfigError> {
        match self.var(name) {
            Some(value) => value
                .parse::<u32>()
                .ok()
                .filter(|value| (1..=MAX_GLOBAL_POOL_CONNECTIONS).contains(value))
                .ok_or_else(|| {
                    format!("{name} must be an integer between 1 and {MAX_GLOBAL_POOL_CONNECTIONS}")
                        .into()
                }),
            None => Ok(default),
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    /// A secrets directory removed when the test ends.
    struct Secrets(PathBuf);

    impl Secrets {
        fn new(files: &[(&str, &str)]) -> Self {
            let dir = std::env::temp_dir().join(format!("attricat-secrets-{}", Uuid::new_v4()));
            fs::create_dir(&dir).unwrap();
            for (name, value) in files {
                fs::write(dir.join(name), value).unwrap();
            }
            Self(dir)
        }
    }

    impl Drop for Secrets {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn required_secrets() -> Secrets {
        Secrets::new(&[
            (secret::DATABASE_URL, "postgres://u:p@db/attricat\n"),
            (secret::S3_SECRET_ACCESS_KEY, "s3-secret"),
        ])
    }

    fn load(secrets: &Secrets, vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let mut map: HashMap<String, String> = [
            ("S3_ENDPOINT", "http://127.0.0.1:9000"),
            ("S3_REGION", "us-east-1"),
            ("S3_BUCKET", "attricat-files"),
            ("S3_ACCESS_KEY_ID", "attricat-dev"),
            ("S3_FORCE_PATH_STYLE", "true"),
            ("S3_UPLOAD_TIMEOUT_SECONDS", "30"),
            ("S3_DOWNLOAD_TIMEOUT_SECONDS", "30"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        map.insert(
            "ATTRICAT_SECRETS_DIR".to_owned(),
            secrets.0.display().to_string(),
        );
        for (name, value) in vars {
            map.insert((*name).to_owned(), (*value).to_owned());
        }
        Config::from_source(&Source::new(map))
    }

    #[test]
    fn minimal_configuration_uses_defaults_and_reads_secret_files() {
        let secrets = required_secrets();
        let config = load(&secrets, &[]).unwrap();
        assert_eq!(
            config.database.url.expose_secret(),
            "postgres://u:p@db/attricat"
        );
        assert_eq!(
            config.database.request_pool_connections,
            REQUEST_POOL_CONNECTIONS
        );
        assert!(config.database.auto_migrate);
        assert_eq!(config.http.bind_addr.to_string(), "127.0.0.1:3000");
        assert!(config.http.session_cookie_secure);
        assert!(config.agent_provider.is_none());
        assert!(config.mail.smtp_password.is_none());
        assert_eq!(config.bootstrap.owner_email, "owner@example.test");
        assert!(!config.bootstrap.sample_accounts);
        assert_eq!(config.file_worker.poll_interval, Duration::from_millis(500));
    }

    #[test]
    fn blank_bootstrap_owner_email_stops_startup() {
        let secrets = required_secrets();
        let error = load(&secrets, &[("ATTRICAT_BOOTSTRAP_OWNER_EMAIL", "")])
            .err()
            .unwrap();
        assert!(error.to_string().contains("ATTRICAT_BOOTSTRAP_OWNER_EMAIL"));
    }

    #[test]
    fn required_secrets_must_be_files() {
        let secrets = Secrets::new(&[(secret::S3_SECRET_ACCESS_KEY, "s3-secret")]);
        let error = load(&secrets, &[("DATABASE_URL", "postgres://env")])
            .err()
            .unwrap();
        assert!(error.to_string().contains("database_url secret"));

        let secrets = Secrets::new(&[(secret::DATABASE_URL, "postgres://db")]);
        assert!(load(&secrets, &[]).is_err());
    }

    #[test]
    fn secrets_in_the_environment_stop_startup() {
        let secrets = required_secrets();
        for (variable, secret) in RETIRED_SECRET_VARIABLES {
            let error = load(&secrets, &[(variable, "value")]).err().unwrap();
            assert!(error.to_string().contains(secret), "{error}");
        }
        // Empty template entries are unset rather than an error.
        assert!(load(&secrets, &[("LLM_API_KEY", "")]).is_ok());
    }

    #[test]
    fn optional_secrets_configure_their_features() {
        let secrets = Secrets::new(&[
            (secret::DATABASE_URL, "postgres://db"),
            (secret::S3_SECRET_ACCESS_KEY, "s3-secret"),
            (secret::LLM_API_KEY, "llm-key\n"),
            (secret::BOOTSTRAP_OWNER_PASSWORD, "owner password"),
            (secret::SMTP_PASSWORD, "\n"),
        ]);
        let config = load(&secrets, &[("ATTRICAT_DEMO_MODE", "true")]).unwrap();
        assert!(config.agent_provider.unwrap().read_only);
        assert!(config.bootstrap.sample_accounts);
        assert_eq!(config.bootstrap.workspace_name, "Demo");
        assert_eq!(
            config.bootstrap.owner_password.unwrap().expose_secret(),
            "owner password"
        );
        // An empty file is an unset secret.
        assert!(config.mail.smtp_password.is_none());
    }

    #[test]
    fn dependent_settings_are_validated_together() {
        let secrets = required_secrets();
        assert!(load(&secrets, &[("ATTRICAT_SAMPLE_ACCOUNTS", "true")]).is_err());
        assert!(
            load(
                &secrets,
                &[("FILE_WORKER_OPERATIONS_BIND_ADDR", "0.0.0.0:3001")]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[("SOLUTION_PACK_LOCAL_EXTENSIONS_DIR", "/tmp/x")]
            )
            .is_err()
        );
        assert!(load(&secrets, &[("CACHE_BACKEND", "redis")]).is_err());
        assert!(load(&secrets, &[("EXTENSIONS_MODE", "off")]).is_err());
        // Development egress overrides bypass the public-address check.
        assert!(
            load(
                &secrets,
                &[("EXTENSION_DEV_EGRESS_RESOLVE", "*.mock.test=127.0.0.1:9443")]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[(
                    "EXTENSION_PUBLIC_FILE_BASE_URL",
                    "https://attricat.test/files"
                )]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[(
                    "EXTENSION_OAUTH_CALLBACK_URL",
                    "https://attricat.test/callback"
                )]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[(
                    "EXTENSION_OAUTH_CALLBACK_URL",
                    "http://127.0.0.1:/api/public/extension-oauth/callback"
                )]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[(
                    "EXTENSION_OAUTH_RETURN_URL",
                    "http://127.0.0.1:/manage/extensions/"
                )]
            )
            .is_err()
        );
        assert!(
            load(
                &secrets,
                &[(
                    "EXTENSION_OAUTH_RETURN_URL",
                    "https://attricat.test/manage/extensions?x=1"
                )]
            )
            .is_err()
        );
    }

    #[test]
    fn export_platform_settings_are_parsed() {
        let secrets = Secrets::new(&[
            (secret::DATABASE_URL, "postgres://db"),
            (secret::S3_SECRET_ACCESS_KEY, "s3-secret"),
            (
                secret::EXTENSION_CREDENTIAL_KEY,
                "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=\n",
            ),
        ]);
        let config = load(
            &secrets,
            &[
                ("ATTRICAT_DEVTOOLS", "true"),
                (
                    "EXTENSION_DEV_EGRESS_RESOLVE",
                    "*.mock.test=127.0.0.1:9443, api.mock.test=127.0.0.1:9444",
                ),
                (
                    "EXTENSION_PUBLIC_FILE_BASE_URL",
                    "https://attricat.test/api/public/extension-files/",
                ),
                (
                    "EXTENSION_OAUTH_CALLBACK_URL",
                    "https://attricat.test/api/public/extension-oauth/callback",
                ),
            ],
        )
        .unwrap();
        assert!(config.extensions.oauth_callback_url.is_some());
        assert!(config.extensions.oauth_return_url.is_none());
        assert_eq!(config.extensions.credential_key, Some([1; 32]));
        let egress = config.extensions.development_egress.unwrap();
        assert_eq!(egress.resolve.len(), 2);
        assert_eq!(egress.resolve[0].0, "*.mock.test");
        assert!(config.extensions.public_file_base_url.is_some());
        let short_key = Secrets::new(&[
            (secret::DATABASE_URL, "postgres://db"),
            (secret::S3_SECRET_ACCESS_KEY, "s3-secret"),
            (secret::EXTENSION_CREDENTIAL_KEY, "c2hvcnQ="),
        ]);
        assert!(load(&short_key, &[]).is_err());
    }

    #[test]
    fn boolean_configuration_is_strict() {
        let secrets = required_secrets();
        assert!(
            !load(&secrets, &[("ATTRICAT_AUTO_MIGRATE", "false")])
                .unwrap()
                .database
                .auto_migrate
        );
        assert!(load(&secrets, &[("ATTRICAT_AUTO_MIGRATE", "TRUE")]).is_err());
    }

    #[test]
    fn numeric_configuration_is_positive_and_bounded() {
        let secrets = required_secrets();
        let config = load(&secrets, &[("DATABASE_TASK_POOL_CONNECTIONS", "7")]).unwrap();
        assert_eq!(config.database.task_pool_connections, 7);
        for invalid in ["0", "-1", "101", "invalid"] {
            assert!(load(&secrets, &[("DATABASE_TASK_POOL_CONNECTIONS", invalid)]).is_err());
        }
        for invalid in ["0", "n/a"] {
            assert!(load(&secrets, &[("FILE_WORKER_POLL_MILLISECONDS", invalid)]).is_err());
        }
        assert!(load(&secrets, &[("HTTP_MAX_CONCURRENT_REQUESTS", "0")]).is_err());
    }
}
