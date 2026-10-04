//! Process-local query cache with an optional shared tier.
//!
//! [`QueryCache`] keeps decoded values in a bounded in-memory LRU (L1); a hit
//! is an `Arc` clone with no serialization. An optional [`RemoteStore`] (L2)
//! shares serialized values between replicas.
//!
//! Nothing is invalidated across replicas. Correctness comes from the keys:
//! callers either cache immutable data, embed a generation number they read
//! in the same request into the key, or accept a bounded [`Policy::Ttl`].
//! Remote failures degrade to L1 and the loader; they never fail a request,
//! and remote writes never delay one.

use std::{
    any::Any,
    collections::HashMap,
    fmt,
    future::Future,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;

mod rate_limit;
mod redis_backend;

pub use rate_limit::{LocalRateLimiter, RateLimiter, RedisRateLimiter};
pub use redis_backend::Redis;
use serde::{Serialize, de::DeserializeOwned};

/// The version of the shared tier's entry format, part of every Redis cache
/// key. Bump it whenever a cached value's serialized shape or what its loader
/// returns for a key changes, so a deploy never serves entries the previous
/// release wrote, which the shared tier may keep for up to a day.
pub(crate) const CACHE_FORMAT: u32 = 1;

/// A cache key. By convention `namespace:part:part`; the namespace labels
/// metrics.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheKey(Arc<str>);

impl CacheKey {
    pub fn new(namespace: &str, parts: &[&(dyn fmt::Display + Sync)]) -> Self {
        let mut key = namespace.to_owned();
        for part in parts {
            key.push(':');
            key.push_str(&part.to_string());
        }
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn namespace(&self) -> &str {
        self.0.split(':').next().unwrap_or_default()
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A group of this replica's L1 entries that [`QueryCache::invalidate_local`]
/// drops together. Tags are process-local: they do not reach the shared tier
/// or other replicas, so they must never be what keeps an entry correct.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(Arc<str>);

impl Tag {
    pub fn new(namespace: &str, parts: &[&(dyn fmt::Display + Sync)]) -> Self {
        Self(CacheKey::new(namespace, parts).0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How long an entry may be served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// The value never changes for its key; only LRU eviction removes it.
    Immutable,
    /// The key embeds a generation the caller read; a new generation is a new
    /// key, so the entry itself never goes stale.
    Generation,
    /// Served for `fresh`; for a further `stale` one caller reloads while
    /// concurrent callers keep receiving the stale value.
    Ttl { fresh: Duration, stale: Duration },
}

impl Policy {
    /// How long the shared tier keeps an entry.
    fn remote_ttl(self) -> Duration {
        match self {
            // Remote memory is shared; let unused immutable entries age out.
            Self::Immutable | Self::Generation => Duration::from_secs(24 * 60 * 60),
            Self::Ttl { fresh, stale } => fresh + stale,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// The shared tier is disconnected or skipped after repeated failures.
    #[error("the shared cache tier is unavailable")]
    Unavailable,
    #[error("{0}")]
    Failed(String),
}

/// A value read from the shared tier.
pub struct RemoteValue {
    pub bytes: Bytes,
    /// How much longer the shared tier keeps the value, when known.
    pub remaining: Option<Duration>,
}

/// A shared serialized tier, such as Redis.
#[async_trait]
pub trait RemoteStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<RemoteValue>, CacheError>;
    /// Stores `value` for `ttl`.
    async fn set(&self, key: &str, value: Bytes, ttl: Duration) -> Result<(), CacheError>;
}

/// Where shared cache state lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheBackend {
    /// Process memory only; the default.
    Memory,
    /// Process memory in front of Redis, which also holds the shared
    /// extension network rate limits.
    Redis { url: String },
}

#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub backend: CacheBackend,
    /// Maximum number of L1 entries.
    pub max_entries: u64,
    /// The first segment of every Redis key; the database identity follows
    /// it.
    pub key_prefix: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            max_entries: 20_000,
            key_prefix: "attricat".to_owned(),
        }
    }
}

impl CacheConfig {
    /// Reads `CACHE_BACKEND` (`memory` or `redis`), `REDIS_URL`,
    /// `CACHE_MAX_ENTRIES` and `CACHE_KEY_PREFIX`.
    pub fn from_env() -> Result<Self, String> {
        Self::parse(
            std::env::var("CACHE_BACKEND").ok().as_deref(),
            std::env::var("REDIS_URL").ok().as_deref(),
            std::env::var("CACHE_MAX_ENTRIES").ok().as_deref(),
            std::env::var("CACHE_KEY_PREFIX").ok().as_deref(),
        )
    }

    fn parse(
        backend: Option<&str>,
        redis_url: Option<&str>,
        max_entries: Option<&str>,
        key_prefix: Option<&str>,
    ) -> Result<Self, String> {
        let backend = match backend.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("memory") => CacheBackend::Memory,
            Some("redis") => CacheBackend::Redis {
                url: redis_url
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .ok_or("CACHE_BACKEND=redis requires REDIS_URL")?
                    .to_owned(),
            },
            Some(_) => return Err("CACHE_BACKEND must be `memory` or `redis`".to_owned()),
        };
        let max_entries = match max_entries {
            Some(value) => value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("CACHE_MAX_ENTRIES must be a positive integer")?,
            None => Self::default().max_entries,
        };
        let key_prefix = match key_prefix.map(str::trim).filter(|value| !value.is_empty()) {
            Some(prefix) if prefix.chars().any(char::is_whitespace) => {
                return Err("CACHE_KEY_PREFIX must not contain whitespace".to_owned());
            }
            Some(prefix) => prefix.to_owned(),
            None => Self::default().key_prefix,
        };
        Ok(Self {
            backend,
            max_entries,
            key_prefix,
        })
    }
}

#[derive(Clone)]
struct Entry {
    value: Arc<dyn Any + Send + Sync>,
    tags: Arc<[Tag]>,
    stored_at: Instant,
    policy: Policy,
}

enum Freshness {
    Fresh,
    Stale,
    Expired,
}

impl Entry {
    fn freshness(&self) -> Freshness {
        match self.policy {
            Policy::Immutable | Policy::Generation => Freshness::Fresh,
            Policy::Ttl { fresh, stale } => {
                let age = self.stored_at.elapsed();
                if age < fresh {
                    Freshness::Fresh
                } else if age < fresh + stale {
                    Freshness::Stale
                } else {
                    Freshness::Expired
                }
            }
        }
    }
}

struct Inner {
    l1: moka::future::Cache<CacheKey, Entry>,
    l2: Option<Arc<dyn RemoteStore>>,
    /// One loader per key at a time; waiters re-read L1 when it finishes.
    flights: Mutex<HashMap<CacheKey, Weak<tokio::sync::Mutex<()>>>>,
}

/// See the module documentation. Cloning shares the same cache.
#[derive(Clone)]
pub struct QueryCache {
    inner: Arc<Inner>,
}

impl fmt::Debug for QueryCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryCache")
            .field("entries", &self.inner.l1.entry_count())
            .field("remote", &self.inner.l2.is_some())
            .finish()
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

impl QueryCache {
    /// An in-memory cache.
    pub fn new(config: CacheConfig) -> Self {
        Self::with_remote(config, None)
    }

    pub fn with_remote(config: CacheConfig, l2: Option<Arc<dyn RemoteStore>>) -> Self {
        Self {
            inner: Arc::new(Inner {
                l1: moka::future::Cache::builder()
                    .max_capacity(config.max_entries)
                    .support_invalidation_closures()
                    .build(),
                l2,
                flights: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Builds the configured cache and the rate limiter that goes with it.
    ///
    /// `database_identity` identifies the database this process serves; Redis
    /// keys start with `<key_prefix>:<database_identity>:` so databases that
    /// share a Redis never read each other's entries. A copy of a database,
    /// such as staging cloned from production, keeps its identity, so a copy
    /// that runs alongside its source needs another `key_prefix` or Redis
    /// database.
    /// An unreachable Redis is retried in the background while the process
    /// serves from memory: the shared tier is an optimization, never a
    /// dependency. Only an invalid `REDIS_URL` is an error.
    pub async fn from_config(
        config: CacheConfig,
        database_identity: &str,
    ) -> Result<(Self, Arc<dyn RateLimiter>), CacheError> {
        Ok(match &config.backend {
            CacheBackend::Memory => (Self::new(config), Arc::new(LocalRateLimiter::default())),
            CacheBackend::Redis { url } => {
                let namespace = format!("{}:{database_identity}", config.key_prefix);
                let redis = Redis::connect(url, &namespace).await?;
                tracing::info!(%namespace, "query cache uses Redis as its shared tier");
                (
                    Self::with_remote(config.clone(), Some(Arc::new(redis.clone()))),
                    Arc::new(RedisRateLimiter::new(&redis)),
                )
            }
        })
    }

    /// Returns the cached value for `key`, or loads, stores and returns it.
    /// Concurrent misses for one key run `load` once; a failed load is not
    /// cached and its error goes to that caller only.
    pub async fn fetch<T, E, F, Fut>(
        &self,
        key: CacheKey,
        tags: &[Tag],
        policy: Policy,
        load: F,
    ) -> Result<Arc<T>, E>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut stale = None;
        if let Some((value, freshness)) = self.local::<T>(&key).await {
            match freshness {
                Freshness::Fresh => {
                    record(&key, "hit");
                    return Ok(value);
                }
                Freshness::Stale => stale = Some(value),
                Freshness::Expired => {}
            }
        }
        let flight = self.flight(&key);
        let _guard = match stale {
            // Serve the stale value unless this caller wins the reload.
            Some(value) => match flight.try_lock() {
                Ok(guard) => {
                    record(&key, "stale_reload");
                    guard
                }
                Err(_) => {
                    record(&key, "stale");
                    return Ok(value);
                }
            },
            None => flight.lock().await,
        };
        // Another caller may have filled the entry while this one waited.
        if let Some((value, Freshness::Fresh)) = self.local::<T>(&key).await {
            record(&key, "hit");
            return Ok(value);
        }
        if let Some((value, stored_at)) = self.remote::<T>(&key, policy).await {
            record(&key, "remote_hit");
            let value = Arc::new(value);
            self.store_local(&key, value.clone(), tags, policy, stored_at)
                .await;
            return Ok(value);
        }
        record(&key, "miss");
        let value = Arc::new(load().await?);
        self.store_local(&key, value.clone(), tags, policy, Instant::now())
            .await;
        self.store_remote(&key, value.as_ref(), policy);
        Ok(value)
    }

    /// The fresh L1 value for `key`, without loading.
    pub async fn get<T: Send + Sync + 'static>(&self, key: &CacheKey) -> Option<Arc<T>> {
        match self.local::<T>(key).await {
            Some((value, Freshness::Fresh)) => {
                record(key, "hit");
                Some(value)
            }
            _ => {
                record(key, "miss");
                None
            }
        }
    }

    /// Stores a value the caller loaded itself, for values whose cacheability
    /// is only known after loading (such as "published").
    pub async fn insert<T: Send + Sync + 'static>(
        &self,
        key: CacheKey,
        value: Arc<T>,
        tags: &[Tag],
        policy: Policy,
    ) {
        self.store_local(&key, value, tags, policy, Instant::now())
            .await;
    }

    /// Drops this replica's L1 entries with `tag`. The shared tier and other
    /// replicas keep theirs.
    pub fn invalidate_local(&self, tag: &Tag) {
        let tag = tag.clone();
        if let Err(error) = self
            .inner
            .l1
            .invalidate_entries_if(move |_, entry| entry.tags.contains(&tag))
        {
            tracing::error!(%error, "cache tag invalidation is unavailable");
            self.inner.l1.invalidate_all();
        }
        metrics::counter!("catalog_query_cache_invalidations_total").increment(1);
    }

    pub fn entry_count(&self) -> u64 {
        self.inner.l1.entry_count()
    }

    async fn local<T: Send + Sync + 'static>(&self, key: &CacheKey) -> Option<(Arc<T>, Freshness)> {
        let entry = self.inner.l1.get(key).await?;
        let freshness = entry.freshness();
        // A key reused for another type is a miss rather than a panic.
        let value = entry.value.downcast::<T>().ok()?;
        Some((value, freshness))
    }

    fn flight(&self, key: &CacheKey) -> Arc<tokio::sync::Mutex<()>> {
        let mut flights = self
            .inner
            .flights
            .lock()
            .expect("cache flight lock is not poisoned");
        if let Some(flight) = flights.get(key).and_then(Weak::upgrade) {
            return flight;
        }
        flights.retain(|_, flight| flight.strong_count() > 0);
        let flight = Arc::new(tokio::sync::Mutex::new(()));
        flights.insert(key.clone(), Arc::downgrade(&flight));
        flight
    }

    async fn store_local<T: Send + Sync + 'static>(
        &self,
        key: &CacheKey,
        value: Arc<T>,
        tags: &[Tag],
        policy: Policy,
        stored_at: Instant,
    ) {
        self.inner
            .l1
            .insert(
                key.clone(),
                Entry {
                    value,
                    tags: tags.into(),
                    stored_at,
                    policy,
                },
            )
            .await;
    }

    /// A fresh value from the shared tier and when it was stored there. A
    /// TTL entry keeps the age it has in the shared tier, so copying it into
    /// L1 never extends its life.
    async fn remote<T: DeserializeOwned>(
        &self,
        key: &CacheKey,
        policy: Policy,
    ) -> Option<(T, Instant)> {
        let l2 = self.inner.l2.as_ref()?;
        let remote = match l2.get(key.as_str()).await {
            Ok(remote) => remote?,
            Err(CacheError::Unavailable) => return None,
            Err(error) => {
                tracing::warn!(%error, %key, "remote cache read failed");
                return None;
            }
        };
        let stored_at = match policy {
            Policy::Immutable | Policy::Generation => Instant::now(),
            Policy::Ttl { fresh, .. } => {
                let age = policy.remote_ttl().checked_sub(remote.remaining?)?;
                // A stale shared value is reloaded by this caller.
                if age >= fresh {
                    return None;
                }
                Instant::now().checked_sub(age)?
            }
        };
        match serde_json::from_slice(&remote.bytes) {
            Ok(value) => Some((value, stored_at)),
            Err(error) => {
                tracing::warn!(%error, %key, "remote cache value could not be decoded");
                None
            }
        }
    }

    /// Writes `value` to the shared tier in the background, so a slow Redis
    /// never delays the request that loaded it.
    fn store_remote<T: Serialize>(&self, key: &CacheKey, value: &T, policy: Policy) {
        let Some(l2) = self.inner.l2.clone() else {
            return;
        };
        let bytes = match serde_json::to_vec(value) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                tracing::warn!(%error, %key, "cache value could not be encoded");
                return;
            }
        };
        let key = key.clone();
        tokio::spawn(async move {
            match l2.set(key.as_str(), bytes, policy.remote_ttl()).await {
                Ok(()) | Err(CacheError::Unavailable) => {}
                Err(error) => tracing::warn!(%error, %key, "remote cache write failed"),
            }
        });
    }
}

fn record(key: &CacheKey, outcome: &'static str) {
    metrics::counter!(
        "catalog_query_cache_requests_total",
        "namespace" => key.namespace().to_owned(),
        "outcome" => outcome
    )
    .increment(1);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn key(name: &str) -> CacheKey {
        CacheKey::new("test", &[&name])
    }

    /// A shared tier in process memory that records stored TTLs and can
    /// delay writes.
    #[derive(Default)]
    struct FakeRemote {
        entries: Mutex<HashMap<String, (Bytes, Instant, Duration)>>,
        write_delay: Duration,
    }

    #[async_trait]
    impl RemoteStore for FakeRemote {
        async fn get(&self, key: &str) -> Result<Option<RemoteValue>, CacheError> {
            let entries = self.entries.lock().unwrap();
            Ok(entries.get(key).and_then(|(bytes, stored_at, ttl)| {
                Some(RemoteValue {
                    bytes: bytes.clone(),
                    remaining: Some(ttl.checked_sub(stored_at.elapsed())?),
                })
            }))
        }

        async fn set(&self, key: &str, value: Bytes, ttl: Duration) -> Result<(), CacheError> {
            tokio::time::sleep(self.write_delay).await;
            self.entries
                .lock()
                .unwrap()
                .insert(key.to_owned(), (value, Instant::now(), ttl));
            Ok(())
        }
    }

    /// Behaviour every backend provides; `peer` shares `cache`'s remote tier
    /// (for memory, it is a separate process-local cache).
    async fn shared_behaviour(cache: QueryCache, peer: QueryCache, shared: bool) {
        let run = uuid_like();
        let key = CacheKey::new("behaviour", &[&run, &"value"]);
        let loads = Arc::new(AtomicUsize::new(0));
        let load = |value: u32| {
            let loads = loads.clone();
            move || async move {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok::<_, CacheError>(value)
            }
        };
        assert_eq!(
            *cache
                .fetch(key.clone(), &[], Policy::Generation, load(1))
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            *cache
                .fetch(key.clone(), &[], Policy::Generation, load(2))
                .await
                .unwrap(),
            1
        );
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        // A peer reads the shared tier instead of loading; a process-local
        // peer loads for itself. The shared write runs in the background.
        if let Some(remote) = &peer.inner.l2 {
            for _ in 0..50 {
                if remote.get(key.as_str()).await.unwrap().is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        let peer_value = *peer
            .fetch(key.clone(), &[], Policy::Generation, load(3))
            .await
            .unwrap();
        assert_eq!(peer_value, if shared { 1 } else { 3 });
    }

    fn uuid_like() -> String {
        format!(
            "{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[tokio::test]
    async fn memory_backend_behaviour() {
        shared_behaviour(QueryCache::default(), QueryCache::default(), false).await;
    }

    /// Runs only when `REDIS_URL` points at a Redis server.
    #[tokio::test]
    async fn redis_backend_behaviour() {
        let Some(url) = std::env::var("REDIS_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
        else {
            return;
        };
        let config = CacheConfig {
            backend: CacheBackend::Redis { url: url.clone() },
            max_entries: 100,
            ..CacheConfig::default()
        };
        let database = uuid_like();
        let redis = Redis::connect(&url, &format!("attricat-test:{database}"))
            .await
            .unwrap();
        redis.wait_until_connected().await;
        let remote: Arc<dyn RemoteStore> = Arc::new(redis.clone());
        let cache = QueryCache::with_remote(config.clone(), Some(remote.clone()));
        let peer = QueryCache::with_remote(config.clone(), Some(remote));
        shared_behaviour(cache, peer, true).await;

        // Another database identity does not see the entry.
        let other = Redis::connect(&url, &format!("attricat-test:{database}-other"))
            .await
            .unwrap();
        other.wait_until_connected().await;
        let key = CacheKey::new("behaviour", &[&"isolated"]);
        redis
            .set(
                key.as_str(),
                Bytes::from_static(b"1"),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert!(other.get(key.as_str()).await.unwrap().is_none());
        let stored = redis.get(key.as_str()).await.unwrap().unwrap();
        assert!(stored.remaining.unwrap() <= Duration::from_secs(5));

        // The shared sliding window.
        let limiter = RedisRateLimiter::new(&redis);
        let window = Duration::from_millis(300);
        let rate_key = uuid_like();
        assert!(limiter.allow(&rate_key, 2, window).await);
        assert!(limiter.allow(&rate_key, 2, window).await);
        assert!(!limiter.allow(&rate_key, 2, window).await);
        tokio::time::sleep(window).await;
        assert!(limiter.allow(&rate_key, 2, window).await);
    }

    #[tokio::test]
    async fn an_unreachable_redis_degrades_to_memory_and_keeps_retrying() {
        let config = CacheConfig {
            backend: CacheBackend::Redis {
                url: "redis://127.0.0.1:1".to_owned(),
            },
            max_entries: 100,
            ..CacheConfig::default()
        };
        let (cache, limiter) = QueryCache::from_config(config, "test").await.unwrap();
        // The shared tier stays configured and reconnects in the background.
        assert!(cache.inner.l2.is_some());
        let value = cache
            .fetch(key("degraded"), &[], Policy::Immutable, || async {
                Ok::<_, CacheError>(5_u32)
            })
            .await
            .unwrap();
        assert_eq!(*value, 5);
        assert!(limiter.allow("degraded", 1, Duration::from_secs(1)).await);
        assert!(!limiter.allow("degraded", 1, Duration::from_secs(1)).await);
    }

    #[test]
    fn configuration_defaults_to_memory_and_validates_redis() {
        let config = CacheConfig::parse(None, None, None, None).unwrap();
        assert_eq!(config.backend, CacheBackend::Memory);
        assert_eq!(config.key_prefix, "attricat");
        let redis = CacheConfig::parse(
            Some("redis"),
            Some("redis://cache"),
            Some("10"),
            Some("prod"),
        )
        .unwrap();
        assert_eq!(
            redis.backend,
            CacheBackend::Redis {
                url: "redis://cache".to_owned()
            }
        );
        assert_eq!(redis.key_prefix, "prod");
        assert!(CacheConfig::parse(Some("redis"), None, None, None).is_err());
        assert!(CacheConfig::parse(Some("disk"), None, None, None).is_err());
        assert!(CacheConfig::parse(None, None, Some("0"), None).is_err());
        assert!(CacheConfig::parse(None, None, None, Some("a b")).is_err());
    }

    #[tokio::test]
    async fn remote_writes_do_not_delay_the_loading_request() {
        let remote = Arc::new(FakeRemote {
            write_delay: Duration::from_secs(5),
            ..FakeRemote::default()
        });
        let cache = QueryCache::with_remote(CacheConfig::default(), Some(remote));
        let started = Instant::now();
        cache
            .fetch(key("slow"), &[], Policy::Immutable, || async {
                Ok::<_, CacheError>(1_u32)
            })
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn remote_ttl_hits_keep_their_shared_age() {
        let remote = Arc::new(FakeRemote::default());
        let policy = Policy::Ttl {
            fresh: Duration::from_millis(100),
            stale: Duration::from_millis(100),
        };
        let writer = QueryCache::with_remote(CacheConfig::default(), Some(remote.clone()));
        writer
            .fetch(key("aging"), &[], policy, || async {
                Ok::<_, CacheError>(1_u32)
            })
            .await
            .unwrap();
        for _ in 0..50 {
            if !remote.entries.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
        // A peer copies the shared value with its age...
        let peer = QueryCache::with_remote(CacheConfig::default(), Some(remote.clone()));
        let value = peer
            .fetch(key("aging"), &[], policy, || async {
                Ok::<_, CacheError>(2_u32)
            })
            .await
            .unwrap();
        assert_eq!(*value, 1);
        // ...so it turns stale when the original would, not a full TTL later.
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(peer.get::<u32>(&key("aging")).await.is_none());

        // A stale shared value is reloaded rather than copied.
        let late = QueryCache::with_remote(CacheConfig::default(), Some(remote));
        let value = late
            .fetch(key("aging"), &[], policy, || async {
                Ok::<_, CacheError>(3_u32)
            })
            .await
            .unwrap();
        assert_eq!(*value, 3);
    }

    #[tokio::test]
    async fn invalidating_a_tag_drops_only_its_local_entries() {
        let cache = QueryCache::default();
        let tag = Tag::new("workspace", &[&"a"]);
        let other = Tag::new("workspace", &[&"b"]);
        for (name, tag) in [("a", &tag), ("b", &other)] {
            cache
                .fetch(
                    key(name),
                    std::slice::from_ref(tag),
                    Policy::Generation,
                    || async { Ok::<_, CacheError>(name.to_owned()) },
                )
                .await
                .unwrap();
        }
        cache.invalidate_local(&tag);
        let reloaded = cache
            .fetch(key("a"), &[tag], Policy::Generation, || async {
                Ok::<_, CacheError>("reloaded".to_owned())
            })
            .await
            .unwrap();
        assert_eq!(*reloaded, "reloaded");
        let kept = cache
            .fetch(key("b"), &[other], Policy::Generation, || async {
                Ok::<_, CacheError>("reloaded".to_owned())
            })
            .await
            .unwrap();
        assert_eq!(*kept, "b");
    }

    #[tokio::test]
    async fn concurrent_misses_load_once() {
        let cache = QueryCache::default();
        let loads = Arc::new(AtomicUsize::new(0));
        let fetches = (0..16).map(|_| {
            let cache = cache.clone();
            let loads = loads.clone();
            async move {
                cache
                    .fetch(key("shared"), &[], Policy::Immutable, || async move {
                        loads.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok::<_, CacheError>(7_u32)
                    })
                    .await
            }
        });
        let values = futures_util::future::join_all(fetches).await;
        assert!(values.iter().all(|value| **value.as_ref().unwrap() == 7));
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_loads_are_not_cached() {
        let cache = QueryCache::default();
        let failed: Result<Arc<u32>, CacheError> = cache
            .fetch(key("flaky"), &[], Policy::Immutable, || async {
                Err(CacheError::Failed("down".into()))
            })
            .await;
        assert!(failed.is_err());
        let value = cache
            .fetch(key("flaky"), &[], Policy::Immutable, || async {
                Ok::<_, CacheError>(1_u32)
            })
            .await
            .unwrap();
        assert_eq!(*value, 1);
    }

    #[tokio::test]
    async fn ttl_entries_serve_stale_values_while_one_caller_reloads() {
        let cache = QueryCache::default();
        let policy = Policy::Ttl {
            fresh: Duration::from_millis(30),
            stale: Duration::from_secs(5),
        };
        let load = |value: u32| move || async move { Ok::<_, CacheError>(value) };
        assert_eq!(
            *cache.fetch(key("ttl"), &[], policy, load(1)).await.unwrap(),
            1
        );
        assert_eq!(
            *cache.fetch(key("ttl"), &[], policy, load(2)).await.unwrap(),
            1
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
        // Stale: the uncontended caller reloads.
        assert_eq!(
            *cache.fetch(key("ttl"), &[], policy, load(3)).await.unwrap(),
            3
        );

        tokio::time::sleep(Duration::from_millis(40)).await;
        let flight = cache.flight(&key("ttl"));
        let _held = flight.lock().await;
        // Another caller is reloading: the stale value is served.
        assert_eq!(
            *cache.fetch(key("ttl"), &[], policy, load(4)).await.unwrap(),
            3
        );
    }

    #[tokio::test]
    async fn expired_ttl_entries_are_reloaded() {
        let cache = QueryCache::default();
        let policy = Policy::Ttl {
            fresh: Duration::from_millis(10),
            stale: Duration::from_millis(10),
        };
        cache
            .fetch(key("short"), &[], policy, || async {
                Ok::<_, CacheError>(1_u32)
            })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let value = cache
            .fetch(key("short"), &[], policy, || async {
                Ok::<_, CacheError>(2_u32)
            })
            .await
            .unwrap();
        assert_eq!(*value, 2);
    }

    #[tokio::test]
    async fn a_key_reused_for_another_type_is_a_miss() {
        let cache = QueryCache::default();
        cache
            .fetch(key("typed"), &[], Policy::Immutable, || async {
                Ok::<_, CacheError>(1_u32)
            })
            .await
            .unwrap();
        let value = cache
            .fetch(key("typed"), &[], Policy::Immutable, || async {
                Ok::<_, CacheError>("text".to_owned())
            })
            .await
            .unwrap();
        assert_eq!(*value, "text");
    }
}
