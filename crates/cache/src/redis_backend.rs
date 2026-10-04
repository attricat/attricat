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
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use fred::prelude::{
    Builder, Client, ClientLike, Config, EventInterface, Expiration, KeysInterface,
    ReconnectPolicy, TlsConnector,
};
use fred::types::Value;

use crate::{CacheError, RemoteStore, RemoteValue};

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
/// failures. After the cooldown the next command is a trial: success closes
/// the breaker, failure opens it again.
struct CircuitBreaker {
    origin: Instant,
    failures: AtomicU32,
    /// Milliseconds since `origin` until which Redis is skipped.
    open_until: AtomicU64,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            failures: AtomicU32::new(0),
            open_until: AtomicU64::new(0),
        }
    }

    fn now(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    fn is_open(&self) -> bool {
        self.now() < self.open_until.load(Ordering::Relaxed)
    }

    fn succeeded(&self) {
        if self.failures.swap(0, Ordering::Relaxed) >= FAILURE_THRESHOLD {
            tracing::info!("Redis responds again; the shared cache tier is back in use");
        }
    }

    fn failed(&self) {
        let failures = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= FAILURE_THRESHOLD {
            self.open_until
                .store(self.now() + COOLDOWN.as_millis() as u64, Ordering::Relaxed);
            if failures == FAILURE_THRESHOLD {
                tracing::warn!(
                    cooldown_ms = COOLDOWN.as_millis() as u64,
                    "Redis keeps failing; skipping the shared cache tier"
                );
            }
            metrics::counter!("catalog_query_cache_redis_circuit_opened_total").increment(1);
        }
    }
}

/// One Redis server, shared by the cache tier and the rate limiter. Keys are
/// namespaced by the configured prefix and the database identity, so
/// deployments or databases that share a Redis never read each other's
/// entries.
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
        client.on_reconnect(|server| async move {
            tracing::info!(%server, "connected to Redis");
            Ok(())
        });
        client.on_error(|(error, server)| async move {
            tracing::debug!(%error, ?server, "Redis connection error");
            Ok(())
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
    /// `attricat:<database>:cache:<key>`.
    pub(crate) fn key(&self, kind: &str, key: &str) -> String {
        format!("{}:{kind}:{key}", self.namespace)
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    /// Runs one bounded command, unless the client is disconnected or the
    /// breaker is open, in which case it fails at once with
    /// [`CacheError::Unavailable`].
    pub(crate) async fn run<T>(
        &self,
        command: impl std::future::Future<Output = Result<T, fred::error::Error>>,
    ) -> Result<T, CacheError> {
        if self.breaker.is_open() || !self.client.is_connected() {
            return Err(CacheError::Unavailable);
        }
        let result = match tokio::time::timeout(COMMAND_TIMEOUT, command).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(failure)) => Err(error(failure)),
            Err(_) => Err(CacheError::Failed("redis command timed out".into())),
        };
        match &result {
            Ok(_) => self.breaker.succeeded(),
            Err(_) => self.breaker.failed(),
        }
        result
    }

    #[cfg(test)]
    pub(crate) async fn wait_until_connected(&self) {
        let _ = tokio::time::timeout(CONNECT_TIMEOUT, self.client.wait_for_connect()).await;
    }
}

/// Parses `url`. `rediss://` uses rustls with the ring provider, like the
/// database and HTTP clients, and the platform's root certificates.
fn redis_config(url: &str) -> Result<Config, CacheError> {
    let Some(rest) = url.strip_prefix("rediss://") else {
        return Config::from_url(url).map_err(error);
    };
    // Parse as plain Redis: fred's own TLS default needs a process-wide
    // rustls provider, which this process does not install.
    let mut config = Config::from_url(&format!("redis://{rest}")).map_err(error)?;
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
        let key = self.key("cache", key);
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
            self.key("cache", key),
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
        let breaker = CircuitBreaker::new();
        for _ in 1..FAILURE_THRESHOLD {
            breaker.failed();
        }
        assert!(!breaker.is_open());
        breaker.failed();
        assert!(breaker.is_open());
        breaker.open_until.store(0, Ordering::Relaxed);
        // The trial after the cooldown fails: open again at once.
        breaker.failed();
        assert!(breaker.is_open());
        breaker.open_until.store(0, Ordering::Relaxed);
        breaker.succeeded();
        breaker.failed();
        assert!(!breaker.is_open());
    }

    #[test]
    fn tls_urls_are_supported() {
        let config = redis_config("rediss://user:secret@cache.example:6380/2").unwrap();
        assert!(config.uses_tls());
        assert!(!redis_config("redis://cache.example").unwrap().uses_tls());
    }
}
