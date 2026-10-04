//! Request rate limits shared by every replica when Redis is configured.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use fred::prelude::LuaInterface;

use crate::{CacheError, redis_backend::Redis};

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

/// Atomically drops timestamps older than the window, rejects when `limit`
/// remain, and otherwise records this call. Time comes from the Redis server
/// so replicas with skewed clocks share one window. The member pairs the
/// timestamp with the count, which is unique within one microsecond because
/// scripts run one at a time.
const SLIDING_WINDOW: &str = r#"
local time = redis.call('TIME')
local now = tonumber(time[1]) * 1000000 + tonumber(time[2])
local window = tonumber(ARGV[1])
redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', now - window)
local count = redis.call('ZCARD', KEYS[1])
if count >= tonumber(ARGV[2]) then
  return 0
end
redis.call('ZADD', KEYS[1], now, string.format('%.0f:%d', now, count))
redis.call('PEXPIRE', KEYS[1], math.ceil(window / 1000))
return 1
"#;

/// The same sliding window as [`LocalRateLimiter`], kept in a Redis sorted
/// set that every replica shares. While Redis is unavailable each process
/// falls back to its own limiter, so the effective limit is per replica
/// until Redis recovers.
pub struct RedisRateLimiter {
    redis: Redis,
    fallback: LocalRateLimiter,
}

impl RedisRateLimiter {
    pub fn new(redis: &Redis) -> Self {
        Self {
            redis: redis.clone(),
            fallback: LocalRateLimiter::default(),
        }
    }

    async fn record(&self, key: &str, limit: u32, window: Duration) -> Result<bool, CacheError> {
        let window_us = window.as_micros().max(1).to_string();
        let allowed: i64 = self
            .redis
            .run(self.redis.client().eval(
                SLIDING_WINDOW,
                self.redis.key("rate", key),
                vec![window_us, limit.to_string()],
            ))
            .await?;
        Ok(allowed == 1)
    }
}

#[async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        match self.record(key, limit, window).await {
            Ok(allowed) => allowed,
            Err(CacheError::Unavailable) => self.fallback.allow_now(key, limit, window),
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
