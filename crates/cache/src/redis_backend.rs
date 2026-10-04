//! The Redis connection behind the shared cache tier and rate limits.
//!
//! Every operation has a short deadline and every failure is reported to the
//! caller as an error that [`crate::QueryCache`] logs and ignores, so an
//! unavailable Redis degrades to L1 and the database instead of failing
//! requests. The client connects and reconnects in the background, including
//! when Redis is unreachable at startup, and a circuit breaker skips Redis
//! for a short cooldown after repeated failures.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use fred::prelude::{
    Builder, Client, ClientLike, Config, EventInterface, Expiration, KeysInterface,
    ReconnectPolicy, Server, TlsConnector,
};
use fred::{error::ErrorKind, types::Value};

use crate::{CACHE_FORMAT, CacheError, RemoteStore, RemoteValue};

/// Deadline for one Redis command; a slow Redis must not slow requests.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(250);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long startup waits for the first connection before it serves from
/// memory and leaves the connection to the background task.
const STARTUP_WAIT: Duration = Duration::from_secs(1);
/// Consecutive failures after which Redis is skipped for [`COOLDOWN`].
const FAILURE_THRESHOLD: u32 = 5;
const COOLDOWN: Duration = Duration::from_secs(5);

fn error(error: impl std::fmt::Display) -> CacheError {
    CacheError::Failed(error.to_string())
}

/// Skips Redis for [`COOLDOWN`] after [`FAILURE_THRESHOLD`] consecutive
/// failures. After the cooldown one command is a trial while concurrent
/// commands keep skipping Redis: success closes the breaker, failure opens it
/// for another cooldown. The failure count and the open state change together
/// under one lock, so a success racing a failure cannot reopen a breaker it
/// just closed.
struct CircuitBreaker {
    origin: Instant,
    state: Mutex<BreakerState>,
}

#[derive(Default)]
struct BreakerState {
    failures: u32,
    /// Milliseconds since `origin` until which Redis is skipped; `None`
    /// while the breaker is closed.
    open_until: Option<u64>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            state: Mutex::default(),
        }
    }

    fn now(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    fn reopen_at(&self) -> u64 {
        self.now() + COOLDOWN.as_millis() as u64
    }

    fn state(&self) -> std::sync::MutexGuard<'_, BreakerState> {
        // The state stays consistent even if a holder panicked.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether a command may use Redis. Once the cooldown has passed, the
    /// first caller claims the trial by starting another cooldown, so the
    /// others keep skipping Redis until the trial succeeds. A trial that
    /// never reports back is retried after that cooldown.
    fn admits(&self) -> bool {
        let now = self.now();
        let mut state = self.state();
        match state.open_until {
            None => true,
            Some(until) if now < until => false,
            Some(_) => {
                state.open_until = Some(now + COOLDOWN.as_millis() as u64);
                true
            }
        }
    }

    fn succeeded(&self) {
        let mut state = self.state();
        state.failures = 0;
        if state.open_until.take().is_some() {
            tracing::info!("Redis responds again; the shared cache tier is back in use");
        }
    }

    fn failed(&self) {
        let reopen_at = self.reopen_at();
        let mut state = self.state();
        state.failures = state.failures.saturating_add(1);
        if state.failures < FAILURE_THRESHOLD {
            return;
        }
        match state.open_until {
            // Already open: a failed trial starts another cooldown.
            Some(until) => state.open_until = Some(until.max(reopen_at)),
            None => {
                state.open_until = Some(reopen_at);
                drop(state);
                tracing::warn!(
                    cooldown_ms = COOLDOWN.as_millis() as u64,
                    "Redis keeps failing; skipping the shared cache tier"
                );
                metrics::counter!("catalog_query_cache_redis_circuit_opened_total").increment(1);
            }
        }
    }
}

/// Whether a command failure says Redis is unreachable or overloaded, rather
/// than that it rejected this command (such as `NOPERM` or a script error).
/// Only the former counts toward the circuit breaker.
fn is_transient(failure: &fred::error::Error) -> bool {
    matches!(
        failure.kind(),
        ErrorKind::IO
            | ErrorKind::Timeout
            | ErrorKind::Canceled
            | ErrorKind::Backpressure
            | ErrorKind::Tls
    )
}

/// Tracks the connection state that fred reports in its events, for the
/// `catalog_query_cache_redis_connected` gauge and the logs.
#[derive(Default)]
struct ConnectionState {
    connected: AtomicBool,
}

impl ConnectionState {
    fn connected(&self, server: &Server) {
        metrics::gauge!("catalog_query_cache_redis_connected").set(1.0);
        if self.connected.swap(true, Ordering::AcqRel) {
            tracing::debug!(%server, "connected to Redis");
        } else {
            tracing::info!(%server, "connected to Redis");
        }
    }

    fn failed(&self, failure: &fred::error::Error, server: Option<&Server>) {
        metrics::gauge!("catalog_query_cache_redis_connected").set(0.0);
        if self.connected.swap(false, Ordering::AcqRel) {
            tracing::warn!(
                error = %failure,
                ?server,
                "lost the Redis connection; the query cache uses process memory until it reconnects"
            );
        } else {
            tracing::debug!(error = %failure, ?server, "Redis connection error");
        }
    }
}

/// One Redis server, shared by the cache tier and the rate limiter. Keys are
/// namespaced by the configured prefix and the database identity, so
/// different databases that share a Redis never read each other's entries;
/// a cloned database needs its own prefix (see [`crate::QueryCache::from_config`]).
#[derive(Clone)]
pub struct Redis {
    client: Client,
    breaker: Arc<CircuitBreaker>,
    namespace: Arc<str>,
}

impl Redis {
    /// Creates a client for `url` and starts connecting in the background.
    /// Only an invalid URL is an error: an unreachable server is retried
    /// with a capped exponential backoff for the life of the process.
    pub async fn connect(url: &str, namespace: &str) -> Result<Self, CacheError> {
        let mut config = redis_config(url)?;
        // Retry the first connection like any later one.
        config.fail_fast = false;
        let mut builder = Builder::from_config(config);
        builder
            .with_performance_config(|performance| {
                performance.default_command_timeout = COMMAND_TIMEOUT;
            })
            .with_connection_config(|connection| {
                connection.connection_timeout = CONNECT_TIMEOUT;
                // A command interrupted by a disconnect fails instead of
                // waiting for the reconnection; callers fall back to the
                // database.
                connection.max_command_attempts = 1;
            })
            // Zero attempts: keep reconnecting, between 100 ms and 5 s apart.
            .set_policy(ReconnectPolicy::new_exponential(0, 100, 5_000, 2));
        let client = builder.build().map_err(error)?;
        let state = Arc::new(ConnectionState::default());
        metrics::gauge!("catalog_query_cache_redis_connected").set(0.0);
        client.on_reconnect({
            let state = state.clone();
            move |server| {
                state.connected(&server);
                async { Ok(()) }
            }
        });
        // fred reports a dropped connection and every failed reconnection
        // attempt as an error event.
        client.on_error(move |(failure, server)| {
            state.failed(&failure, server.as_ref());
            async { Ok(()) }
        });
        client.connect();
        if tokio::time::timeout(STARTUP_WAIT, client.wait_for_connect())
            .await
            .is_err()
        {
            tracing::warn!(
                "Redis is not reachable yet; the query cache uses process memory until it connects"
            );
        }
        Ok(Self {
            client,
            breaker: Arc::new(CircuitBreaker::new()),
            namespace: namespace.into(),
        })
    }

    /// `kind` under this deployment's namespace, such as
    /// `attricat:<database>:ratelimit:<key>`.
    pub(crate) fn key(&self, kind: &str, key: &str) -> String {
        format!("{}:{kind}:{key}", self.namespace)
    }

    /// A cache entry's key, such as `attricat:<database>:cache:v1:<key>`.
    /// The [`CACHE_FORMAT`] segment keeps entries written by a release with
    /// other cached shapes or loaders apart.
    fn cache_key(&self, key: &str) -> String {
        self.key(&format!("cache:v{CACHE_FORMAT}"), key)
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    /// Runs one bounded command, unless the client is disconnected or the
    /// breaker is open, in which case it fails at once with
    /// [`CacheError::Unavailable`]. Only timeouts and connection failures
    /// count toward the breaker; an error Redis returns for the command
    /// itself does not.
    pub(crate) async fn run<T>(
        &self,
        command: impl std::future::Future<Output = Result<T, fred::error::Error>>,
    ) -> Result<T, CacheError> {
        if !self.client.is_connected() || !self.breaker.admits() {
            return Err(CacheError::Unavailable);
        }
        match tokio::time::timeout(COMMAND_TIMEOUT, command).await {
            Ok(Ok(value)) => {
                self.breaker.succeeded();
                Ok(value)
            }
            Ok(Err(failure)) => {
                if is_transient(&failure) {
                    self.breaker.failed();
                } else {
                    // Redis answered, so it is reachable.
                    self.breaker.succeeded();
                }
                Err(error(failure))
            }
            Err(_) => {
                self.breaker.failed();
                Err(CacheError::Failed("redis command timed out".into()))
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn wait_until_connected(&self) {
        let _ = tokio::time::timeout(CONNECT_TIMEOUT, self.client.wait_for_connect()).await;
    }
}

/// Parses `url`. Every TLS scheme fred accepts (`rediss`, `valkeys` and
/// their `-cluster` and `-sentinel` variants) uses rustls with the ring
/// provider, like the database and HTTP clients, and the platform's root
/// certificates.
fn redis_config(url: &str) -> Result<Config, CacheError> {
    let Some((scheme, rest)) = url.split_once("://") else {
        return Config::from_url(url).map_err(error);
    };
    // fred enables TLS for any scheme that starts with `rediss` or
    // `valkeys`, and its own TLS default needs a process-wide rustls
    // provider, which this process does not install. Parse such a URL as its
    // plain counterpart, then add the TLS connector.
    let lowercase = scheme.to_ascii_lowercase();
    let plain = if let Some(suffix) = lowercase.strip_prefix("rediss") {
        format!("redis{suffix}")
    } else if let Some(suffix) = lowercase.strip_prefix("valkeys") {
        format!("valkey{suffix}")
    } else {
        return Config::from_url(url).map_err(error);
    };
    let mut config = Config::from_url(&format!("{plain}://{rest}")).map_err(error)?;
    let mut roots = rustls::RootCertStore::empty();
    let certificates = rustls_native_certs::load_native_certs();
    if certificates.certs.is_empty()
        && let Some(failure) = certificates.errors.first()
    {
        return Err(error(failure));
    }
    for certificate in certificates.certs {
        // A platform certificate rustls cannot use is skipped, not fatal.
        let _ = roots.add(certificate);
    }
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(error)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.tls = Some(TlsConnector::from(tls).into());
    Ok(config)
}

#[async_trait]
impl RemoteStore for Redis {
    async fn get(&self, key: &str) -> Result<Option<RemoteValue>, CacheError> {
        let key = self.cache_key(key);
        let pipeline = self.client.pipeline();
        let _: () = pipeline.get(&key).await.map_err(error)?;
        let _: () = pipeline.pttl(&key).await.map_err(error)?;
        let (value, ttl_ms): (Option<Bytes>, i64) = self.run(pipeline.all()).await?;
        Ok(value.map(|bytes| RemoteValue {
            bytes,
            // -1: no expiry; -2: expired between the two commands.
            remaining: u64::try_from(ttl_ms).ok().map(Duration::from_millis),
        }))
    }

    async fn set(&self, key: &str, value: Bytes, ttl: Duration) -> Result<(), CacheError> {
        let ttl_ms = ttl.as_millis().max(1) as i64;
        self.run(self.client.set::<Value, _, _>(
            self.cache_key(key),
            value,
            Some(Expiration::PX(ttl_ms)),
            None,
            false,
        ))
        .await
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_breaker_opens_after_repeated_failures_and_closes_on_success() {
        // Started a while ago, so a deadline of 1 ms has passed.
        let breaker = CircuitBreaker {
            origin: Instant::now() - COOLDOWN,
            ..CircuitBreaker::new()
        };
        for _ in 1..FAILURE_THRESHOLD {
            breaker.failed();
        }
        assert!(breaker.admits());
        breaker.failed();
        assert!(!breaker.admits());
        // Failures while open keep it open without reopening it.
        breaker.failed();
        assert!(!breaker.admits());

        // After the cooldown one caller runs the trial; the others still skip.
        breaker.state().open_until = Some(1);
        assert!(breaker.admits());
        assert!(!breaker.admits());
        // The trial fails: open for another cooldown.
        breaker.failed();
        assert!(!breaker.admits());

        breaker.state().open_until = Some(1);
        assert!(breaker.admits());
        breaker.succeeded();
        assert!(breaker.admits());
        assert!(breaker.admits());
        // Closed again: one failure does not open it.
        breaker.failed();
        assert!(breaker.admits());
    }

    #[test]
    fn only_unreachable_redis_counts_as_a_failure() {
        use fred::error::Error;
        assert!(is_transient(&Error::new(ErrorKind::IO, "reset")));
        assert!(is_transient(&Error::new(ErrorKind::Timeout, "slow")));
        assert!(!is_transient(&Error::new(ErrorKind::Auth, "NOPERM")));
        assert!(!is_transient(&Error::new(ErrorKind::Unknown, "ERR script")));
    }

    #[test]
    fn every_tls_scheme_uses_the_ring_connector() {
        for url in [
            "rediss://user:secret@cache.example:6380/2",
            "REDISS://cache.example:6380",
            "valkeys://cache.example:6380",
            "rediss-cluster://cache.example:6380?node=other.example:6381",
            "valkeys-cluster://cache.example:6380",
            "rediss-sentinel://cache.example:26379/0?sentinelServiceName=main",
            "valkeys-sentinel://cache.example:26379?sentinelServiceName=main",
        ] {
            assert!(redis_config(url).unwrap().uses_tls(), "{url}");
        }
        for url in [
            "redis://cache.example",
            "valkey://cache.example",
            "redis-cluster://cache.example:6380",
        ] {
            assert!(!redis_config(url).unwrap().uses_tls(), "{url}");
        }
        // fred's own TLS default would have installed a process-wide
        // provider; the API binary has none and two candidates, so it would
        // panic there instead.
        assert!(rustls::crypto::CryptoProvider::get_default().is_none());
    }
}
