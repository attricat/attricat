//! Redis implementations of the shared cache tier and invalidation bus.
//!
//! Every operation has a short deadline and every failure is reported to the
//! caller as an error that [`crate::QueryCache`] logs and ignores, so an
//! unavailable Redis degrades to L1 and the database instead of failing
//! requests. The clients reconnect in the background.

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use fred::prelude::{
    Builder, Client, ClientLike, Config, EventInterface, Expiration, KeysInterface,
    PubsubInterface, ReconnectPolicy, SetsInterface,
};
use fred::types::{ExpireOptions, Value};
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use tokio::sync::{broadcast::error::RecvError, mpsc};

use crate::{CacheError, InvalidationBus, RemoteStore, Tag};

/// Deadline for one Redis command; a slow Redis must not slow requests.
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_millis(250);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const KEY_PREFIX: &str = "attricat:cache:";
const TAG_PREFIX: &str = "attricat:tag:";
const CHANNEL: &str = "attricat:cache:invalidate";
/// Invalidations buffered for the listener before it is reported as lagging.
const MESSAGE_CAPACITY: usize = 1_024;

pub(crate) fn error(error: impl std::fmt::Display) -> CacheError {
    CacheError(error.to_string())
}

pub(crate) async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, fred::error::Error>>,
) -> Result<T, CacheError> {
    tokio::time::timeout(COMMAND_TIMEOUT, future)
        .await
        .map_err(|_| CacheError("redis command timed out".into()))?
        .map_err(error)
}

/// The command client and the subscriber client of one Redis server.
#[derive(Clone)]
pub struct RedisClients {
    pub(crate) commands: Client,
    subscriber: fred::clients::SubscriberClient,
}

/// Connects to `url`. The first connection must succeed; later disconnects
/// reconnect in the background with a capped exponential backoff.
pub async fn connect(url: &str) -> Result<RedisClients, CacheError> {
    let mut config = Config::from_url(url).map_err(error)?;
    config.fail_fast = true;
    let mut builder = Builder::from_config(config);
    builder
        .with_performance_config(|performance| {
            performance.default_command_timeout = COMMAND_TIMEOUT;
            performance.broadcast_channel_capacity = MESSAGE_CAPACITY;
        })
        .with_connection_config(|connection| {
            connection.connection_timeout = CONNECT_TIMEOUT;
            // A command interrupted by a disconnect fails instead of waiting
            // for the reconnection; callers fall back to the database.
            connection.max_command_attempts = 1;
        })
        // Zero attempts: keep reconnecting, between 100 ms and 5 s apart.
        .set_policy(ReconnectPolicy::new_exponential(0, 100, 5_000, 2));
    let commands = builder.build().map_err(error)?;
    let subscriber = builder.build_subscriber_client().map_err(error)?;
    initialize(&commands).await?;
    initialize(&subscriber).await?;
    // Resubscribes to the invalidation channel after every reconnection.
    subscriber.manage_subscriptions();
    Ok(RedisClients {
        commands,
        subscriber,
    })
}

async fn initialize(client: &impl ClientLike) -> Result<(), CacheError> {
    tokio::time::timeout(CONNECT_TIMEOUT, client.init())
        .await
        .map_err(|_| CacheError("redis connection timed out".into()))?
        .map_err(error)?;
    Ok(())
}

/// Serialized cache values, with one set of keys per tag for invalidation.
#[derive(Clone)]
pub struct RedisStore {
    client: Client,
}

impl RedisStore {
    pub fn new(clients: &RedisClients) -> Self {
        Self {
            client: clients.commands.clone(),
        }
    }
}

#[async_trait]
impl RemoteStore for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<Bytes>, CacheError> {
        bounded(
            self.client
                .get::<Option<Bytes>, _>(format!("{KEY_PREFIX}{key}")),
        )
        .await
    }

    async fn set(
        &self,
        key: &str,
        value: Bytes,
        ttl: Option<Duration>,
        tags: &[Tag],
    ) -> Result<(), CacheError> {
        let key = format!("{KEY_PREFIX}{key}");
        let ttl_ms = ttl.map(|ttl| ttl.as_millis().max(1) as i64);
        let pipeline = self.client.pipeline();
        let _: () = pipeline
            .set(&key, value, ttl_ms.map(Expiration::PX), None, false)
            .await
            .map_err(error)?;
        for tag in tags {
            let tag_key = format!("{TAG_PREFIX}{tag}");
            let _: () = pipeline.sadd(&tag_key, key.as_str()).await.map_err(error)?;
            // A tag's key set lives at least as long as its entries.
            if let Some(ttl_ms) = ttl_ms {
                let _: () = pipeline
                    .pexpire(&tag_key, ttl_ms, Some(ExpireOptions::GT))
                    .await
                    .map_err(error)?;
            }
        }
        bounded(pipeline.all::<Value>()).await.map(drop)
    }

    async fn delete_tag(&self, tag: &Tag) -> Result<(), CacheError> {
        let tag_key = format!("{TAG_PREFIX}{tag}");
        let mut keys: Vec<String> = bounded(self.client.smembers(&tag_key)).await?;
        keys.push(tag_key);
        bounded(self.client.del::<i64, _>(keys)).await.map(drop)
    }
}

/// Invalidations broadcast with Redis pub/sub. Every replica, including the
/// publisher, drops its L1 entries for a received tag.
pub struct RedisBus {
    clients: RedisClients,
}

impl RedisBus {
    pub fn new(clients: &RedisClients) -> Self {
        Self {
            clients: clients.clone(),
        }
    }
}

#[async_trait]
impl InvalidationBus for RedisBus {
    async fn publish(&self, tag: &Tag) -> Result<(), CacheError> {
        bounded(
            self.clients
                .commands
                .publish::<i64, _, _>(CHANNEL, tag.as_str()),
        )
        .await
        .map(drop)
    }

    fn subscribe(&self) -> BoxStream<'static, Tag> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let subscriber = self.clients.subscriber.clone();
        let mut messages = subscriber.message_rx();
        tokio::spawn(async move {
            if let Err(error) = subscriber.subscribe(CHANNEL).await {
                // `manage_subscriptions` retries after the next reconnection.
                tracing::warn!(%error, "cache invalidation subscription failed");
            }
            loop {
                match messages.recv().await {
                    Ok(message) if message.channel == CHANNEL => {
                        if let Some(tag) = message.value.as_string()
                            && sender.send(Tag::from_wire(&tag)).is_err()
                        {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "cache invalidation listener lagged");
                    }
                    Err(RecvError::Closed) => return,
                }
            }
        });
        futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|tag| (tag, receiver))
        })
        .boxed()
    }
}
