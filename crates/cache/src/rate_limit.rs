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

    /// Takes one token from the key's bucket, which holds up to `capacity`
    /// tokens and refills `refill` tokens every `period`. When the bucket is
    /// empty, returns how long until a token is available.
    async fn take(
        &self,
        key: &str,
        capacity: u32,
        refill: u32,
        period: Duration,
    ) -> Result<(), Duration>;
}

/// One token bucket: its tokens when it was last updated.
struct Bucket {
    tokens: f64,
    updated: Instant,
}

/// Seconds per token of a bucket refilling `refill` tokens every `period`.
fn seconds_per_token(refill: u32, period: Duration) -> f64 {
    period.as_secs_f64() / f64::from(refill.max(1))
}

/// Buckets idle for longer than this many windows are evicted.
const IDLE_WINDOWS: u32 = 2;
/// The bucket count above which idle buckets are evicted.
const EVICT_ABOVE: usize = 1_024;

/// A sliding-window limiter for one process. Idle buckets are evicted.
#[derive(Default)]
pub struct LocalRateLimiter {
    buckets: Mutex<HashMap<String, VecDeque<Instant>>>,
    token_buckets: Mutex<HashMap<String, Bucket>>,
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

    pub fn take_now(
        &self,
        key: &str,
        capacity: u32,
        refill: u32,
        period: Duration,
    ) -> Result<(), Duration> {
        let now = Instant::now();
        let per_token = seconds_per_token(refill, period);
        let capacity = f64::from(capacity.max(1));
        let mut buckets = self
            .token_buckets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if buckets.len() > EVICT_ABOVE {
            // A bucket idle long enough to be full again holds no state.
            buckets.retain(|_, bucket| {
                now.duration_since(bucket.updated).as_secs_f64() < capacity * per_token
            });
        }
        let bucket = buckets.entry(key.to_owned()).or_insert(Bucket {
            tokens: capacity,
            updated: now,
        });
        let refilled = now.duration_since(bucket.updated).as_secs_f64() / per_token;
        bucket.tokens = (bucket.tokens + refilled).min(capacity);
        bucket.updated = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((1.0 - bucket.tokens) * per_token))
        }
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

    async fn take(
        &self,
        key: &str,
        capacity: u32,
        refill: u32,
        period: Duration,
    ) -> Result<(), Duration> {
        self.take_now(key, capacity, refill, period)
    }
}

/// Atomically refills the bucket from the Redis clock, then takes a token or
/// returns the microseconds until one is available. The hash holds the
/// tokens and the time they were counted; it expires once it would be full.
const TOKEN_BUCKET: &str = r#"
local time = redis.call('TIME')
local now = tonumber(time[1]) * 1000000 + tonumber(time[2])
local capacity = tonumber(ARGV[1])
local per_token = tonumber(ARGV[2])
local state = redis.call('HMGET', KEYS[1], 'tokens', 'at')
local tokens = tonumber(state[1]) or capacity
local at = tonumber(state[2]) or now
tokens = math.min(capacity, tokens + (now - at) / per_token)
local wait = 0
if tokens >= 1 then
  tokens = tokens - 1
else
  wait = math.ceil((1 - tokens) * per_token)
end
redis.call('HSET', KEYS[1], 'tokens', tostring(tokens), 'at', tostring(now))
redis.call('PEXPIRE', KEYS[1], math.ceil(capacity * per_token / 1000) + 1000)
return wait
"#;

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

    async fn record_take(
        &self,
        key: &str,
        capacity: u32,
        refill: u32,
        period: Duration,
    ) -> Result<Result<(), Duration>, CacheError> {
        let per_token_us = (seconds_per_token(refill, period) * 1_000_000.0).max(1.0);
        let wait_us: i64 = self
            .redis
            .run(self.redis.client().eval(
                TOKEN_BUCKET,
                self.redis.key("bucket", key),
                vec![capacity.max(1).to_string(), format!("{per_token_us:.0}")],
            ))
            .await?;
        Ok(if wait_us <= 0 {
            Ok(())
        } else {
            Err(Duration::from_micros(wait_us as u64))
        })
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

    async fn take(
        &self,
        key: &str,
        capacity: u32,
        refill: u32,
        period: Duration,
    ) -> Result<(), Duration> {
        match self.record_take(key, capacity, refill, period).await {
            Ok(result) => result,
            Err(CacheError::Unavailable) => self.fallback.take_now(key, capacity, refill, period),
            Err(error) => {
                tracing::warn!(%error, "shared rate limit unavailable; using the local limit");
                self.fallback.take_now(key, capacity, refill, period)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_buckets_allow_bursts_and_report_the_wait() {
        let limiter = LocalRateLimiter::default();
        let period = Duration::from_millis(100);
        assert!(limiter.take_now("a", 2, 1, period).is_ok());
        assert!(limiter.take_now("a", 2, 1, period).is_ok());
        let wait = limiter.take_now("a", 2, 1, period).unwrap_err();
        assert!(wait <= period && wait > Duration::ZERO);
        assert!(limiter.take_now("b", 2, 1, period).is_ok());
        std::thread::sleep(period);
        assert!(limiter.take_now("a", 2, 1, period).is_ok());
    }

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
