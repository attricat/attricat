//! Request rate limits shared by every replica when Redis is configured.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use fred::prelude::{Client, KeysInterface};

use crate::CacheError;
use crate::redis_backend::{RedisClients, bounded, error};

/// Allows at most `limit` events per `window` for each key.
#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn allow(&self, key: &str, limit: u32, window: Duration) -> bool;
}

/// Buckets idle for longer than this many windows are evicted.
const IDLE_WINDOWS: u32 = 2;
/// The bucket count above which idle buckets are evicted.
const EVICT_ABOVE: usize = 1_024;

/// A sliding-window limiter for one process. Idle buckets are evicted.
#[derive(Default)]
pub struct LocalRateLimiter {
    buckets: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl LocalRateLimiter {
    pub fn allow_now(&self, key: &str, limit: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if buckets.len() > EVICT_ABOVE {
            buckets.retain(|_, bucket| {
                bucket
                    .back()
                    .is_some_and(|last| now.duration_since(*last) < window * IDLE_WINDOWS)
            });
        }
        let bucket = buckets.entry(key.to_owned()).or_default();
        while bucket
            .front()
            .is_some_and(|at| now.duration_since(*at) >= window)
        {
            bucket.pop_front();
        }
        if bucket.len() >= limit as usize {
            return false;
        }
        bucket.push_back(now);
        true
    }

    pub fn bucket_count(&self) -> usize {
        self.buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

#[async_trait]
impl RateLimiter for LocalRateLimiter {
    async fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        self.allow_now(key, limit, window)
    }
}

/// A fixed-window counter in Redis, shared by every replica. When Redis is
/// unavailable the process falls back to its own limiter.
pub struct RedisRateLimiter {
    client: Client,
    fallback: LocalRateLimiter,
}

impl RedisRateLimiter {
    pub fn new(clients: &RedisClients) -> Self {
        Self {
            client: clients.commands.clone(),
            fallback: LocalRateLimiter::default(),
        }
    }

    async fn count(&self, counter: &str, window_ms: i64) -> Result<u64, CacheError> {
        let pipeline = self.client.pipeline();
        let _: () = pipeline.incr(counter).await.map_err(error)?;
        let _: () = pipeline
            .pexpire(counter, window_ms * 2, None)
            .await
            .map_err(error)?;
        let (count, _): (u64, i64) = bounded(pipeline.all()).await?;
        Ok(count)
    }
}

#[async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        let window_ms = window.as_millis().max(1) as i64;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let counter = format!("attricat:rate:{key}:{}", now_ms / window_ms);
        match self.count(&counter, window_ms).await {
            Ok(count) => count <= u64::from(limit),
            Err(error) => {
                tracing::warn!(%error, "shared rate limit unavailable; using the local limit");
                self.fallback.allow_now(key, limit, window)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_limits_per_key_and_evicts_idle_buckets() {
        let limiter = LocalRateLimiter::default();
        let window = Duration::from_millis(20);
        assert!(limiter.allow_now("a", 2, window));
        assert!(limiter.allow_now("a", 2, window));
        assert!(!limiter.allow_now("a", 2, window));
        assert!(limiter.allow_now("b", 2, window));
        std::thread::sleep(window);
        assert!(limiter.allow_now("a", 2, window));
        for index in 0..=EVICT_ABOVE {
            limiter.allow_now(&format!("idle-{index}"), 1, window);
        }
        std::thread::sleep(window * IDLE_WINDOWS);
        limiter.allow_now("fresh", 1, window);
        assert!(limiter.bucket_count() <= 2, "idle buckets were evicted");
    }
}
