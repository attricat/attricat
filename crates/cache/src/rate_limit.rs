//! Request rate limits shared by every replica when Redis is configured.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use redis::aio::ConnectionManager;

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
    connection: ConnectionManager,
    fallback: LocalRateLimiter,
}

impl RedisRateLimiter {
    pub fn new(connection: ConnectionManager) -> Self {
        Self {
            connection,
            fallback: LocalRateLimiter::default(),
        }
    }
}

#[async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        let window_ms = window.as_millis().max(1) as u64;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let counter = format!("attricat:rate:{key}:{}", now_ms / window_ms);
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .cmd("INCR")
            .arg(&counter)
            .cmd("PEXPIRE")
            .arg(&counter)
            .arg(window_ms * 2)
            .ignore();
        let result = tokio::time::timeout(
            Duration::from_millis(250),
            pipeline.query_async::<(u64,)>(&mut connection),
        )
        .await;
        match result {
            Ok(Ok((count,))) => count <= u64::from(limit),
            Ok(Err(error)) => {
                tracing::warn!(%error, "shared rate limit unavailable; using the local limit");
                self.fallback.allow_now(key, limit, window)
            }
            Err(_) => {
                tracing::warn!("shared rate limit timed out; using the local limit");
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
