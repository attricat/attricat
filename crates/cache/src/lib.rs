//! Process-local query cache with an optional shared tier.
//!
//! [`QueryCache`] keeps decoded values in a bounded in-memory LRU (L1); a hit
//! is an `Arc` clone with no serialization. An optional [`RemoteStore`] (L2)
//! shares serialized values between replicas, and an [`InvalidationBus`]
//! tells every replica to drop L1 entries for an invalidated [`Tag`].
//!
//! Correctness never depends on the remote tier or the bus: callers either
//! cache immutable data, embed a generation number they read in the same
//! request into the key, or accept a bounded [`Policy::Ttl`]. Remote failures
//! degrade to L1 and the loader; they never fail a request.

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
use futures_util::{StreamExt, stream::BoxStream};
use serde::{Serialize, de::DeserializeOwned};

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

/// A group of entries invalidated together, such as one workspace's
/// extension set.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(Arc<str>);

impl Tag {
    pub fn new(namespace: &str, parts: &[&(dyn fmt::Display + Sync)]) -> Self {
        Self(CacheKey::new(namespace, parts).0)
    }

    pub fn from_wire(tag: &str) -> Self {
        Self(tag.into())
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
    fn remote_ttl(self) -> Option<Duration> {
        match self {
            // Remote memory is shared; let unused immutable entries age out.
            Self::Immutable | Self::Generation => Some(Duration::from_secs(24 * 60 * 60)),
            Self::Ttl { fresh, stale } => Some(fresh + stale),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CacheError(pub String);

/// A shared serialized tier, such as Redis.
#[async_trait]
pub trait RemoteStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Bytes>, CacheError>;
    /// Stores `value` and records the key under each tag for
    /// [`RemoteStore::delete_tag`].
    async fn set(
        &self,
        key: &str,
        value: Bytes,
        ttl: Option<Duration>,
        tags: &[Tag],
    ) -> Result<(), CacheError>;
    async fn delete_tag(&self, tag: &Tag) -> Result<(), CacheError>;
}

/// Tells other replicas which tags were invalidated.
#[async_trait]
pub trait InvalidationBus: Send + Sync {
    async fn publish(&self, tag: &Tag) -> Result<(), CacheError>;
    fn subscribe(&self) -> BoxStream<'static, Tag>;
}

/// The bus of a single process: invalidation already dropped the local
/// entries, so there is nothing to deliver.
pub struct LocalBus;

#[async_trait]
impl InvalidationBus for LocalBus {
    async fn publish(&self, _: &Tag) -> Result<(), CacheError> {
        Ok(())
    }

    fn subscribe(&self) -> BoxStream<'static, Tag> {
        futures_util::stream::empty().boxed()
    }
}

/// Where shared cache state lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheBackend {
    /// Process memory only; the default.
    Memory,
    /// Process memory in front of Redis, which also carries invalidations.
    Redis { url: String },
}

#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub backend: CacheBackend,
    /// Maximum number of L1 entries.
    pub max_entries: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            backend: CacheBackend::Memory,
            max_entries: 20_000,
        }
    }
}

impl CacheConfig {
    /// Reads `CACHE_BACKEND` (`memory` or `redis`), `REDIS_URL` and
    /// `CACHE_MAX_ENTRIES`.
    pub fn from_env() -> Result<Self, String> {
        Self::parse(
            std::env::var("CACHE_BACKEND").ok().as_deref(),
            std::env::var("REDIS_URL").ok().as_deref(),
            std::env::var("CACHE_MAX_ENTRIES").ok().as_deref(),
        )
    }

    fn parse(
        backend: Option<&str>,
        redis_url: Option<&str>,
        max_entries: Option<&str>,
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
        Ok(Self {
            backend,
            max_entries,
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
    bus: Arc<dyn InvalidationBus>,
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
    /// An in-memory cache with the process-local bus.
    pub fn new(config: CacheConfig) -> Self {
        Self::with_backends(config, None, Arc::new(LocalBus))
    }

    pub fn with_backends(
        config: CacheConfig,
        l2: Option<Arc<dyn RemoteStore>>,
        bus: Arc<dyn InvalidationBus>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                l1: moka::future::Cache::builder()
                    .max_capacity(config.max_entries)
                    .support_invalidation_closures()
                    .build(),
                l2,
                bus,
                flights: Mutex::new(HashMap::new()),
            }),
        }
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
        if let Some(value) = self.remote::<T>(&key).await {
            record(&key, "remote_hit");
            let value = Arc::new(value);
            self.store_local(&key, value.clone(), tags, policy).await;
            return Ok(value);
        }
        record(&key, "miss");
        let value = Arc::new(load().await?);
        self.store_local(&key, value.clone(), tags, policy).await;
        self.store_remote(&key, value.as_ref(), tags, policy).await;
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
        self.store_local(&key, value, tags, policy).await;
    }

    /// Drops every entry with `tag` here, in the remote tier, and (through
    /// the bus) on other replicas.
    pub async fn invalidate(&self, tag: &Tag) {
        self.invalidate_local(tag);
        if let Some(l2) = &self.inner.l2
            && let Err(error) = l2.delete_tag(tag).await
        {
            tracing::warn!(%error, %tag, "remote cache invalidation failed");
        }
        if let Err(error) = self.inner.bus.publish(tag).await {
            tracing::warn!(%error, %tag, "cache invalidation broadcast failed");
        }
    }

    /// Drops this replica's entries with `tag`.
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

    /// Applies invalidations other replicas publish until the bus closes.
    pub fn spawn_invalidation_listener(&self) -> tokio::task::JoinHandle<()> {
        let cache = self.clone();
        let mut tags = self.inner.bus.subscribe();
        tokio::spawn(async move {
            while let Some(tag) = tags.next().await {
                cache.invalidate_local(&tag);
            }
        })
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
    ) {
        self.inner
            .l1
            .insert(
                key.clone(),
                Entry {
                    value,
                    tags: tags.into(),
                    stored_at: Instant::now(),
                    policy,
                },
            )
            .await;
    }

    async fn remote<T: DeserializeOwned>(&self, key: &CacheKey) -> Option<T> {
        let l2 = self.inner.l2.as_ref()?;
        match l2.get(key.as_str()).await {
            Ok(Some(bytes)) => match serde_json::from_slice(&bytes) {
                Ok(value) => Some(value),
                Err(error) => {
                    tracing::warn!(%error, %key, "remote cache value could not be decoded");
                    None
                }
            },
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, %key, "remote cache read failed");
                None
            }
        }
    }

    async fn store_remote<T: Serialize>(
        &self,
        key: &CacheKey,
        value: &T,
        tags: &[Tag],
        policy: Policy,
    ) {
        let Some(l2) = self.inner.l2.as_ref() else {
            return;
        };
        let bytes = match serde_json::to_vec(value) {
            Ok(bytes) => Bytes::from(bytes),
            Err(error) => {
                tracing::warn!(%error, %key, "cache value could not be encoded");
                return;
            }
        };
        if let Err(error) = l2.set(key.as_str(), bytes, policy.remote_ttl(), tags).await {
            tracing::warn!(%error, %key, "remote cache write failed");
        }
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

    #[test]
    fn configuration_defaults_to_memory_and_validates_redis() {
        let config = CacheConfig::parse(None, None, None).unwrap();
        assert_eq!(config.backend, CacheBackend::Memory);
        assert_eq!(
            CacheConfig::parse(Some("redis"), Some("redis://cache"), Some("10"))
                .unwrap()
                .backend,
            CacheBackend::Redis {
                url: "redis://cache".to_owned()
            }
        );
        assert!(CacheConfig::parse(Some("redis"), None, None).is_err());
        assert!(CacheConfig::parse(Some("disk"), None, None).is_err());
        assert!(CacheConfig::parse(None, None, Some("0")).is_err());
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
                Err(CacheError("down".into()))
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
    async fn invalidating_a_tag_drops_only_its_entries() {
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
        cache.invalidate(&tag).await;
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
