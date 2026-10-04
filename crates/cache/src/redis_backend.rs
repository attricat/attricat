//! Redis implementations of the shared cache tier and invalidation bus.
//!
//! Every operation has a short deadline and every failure is reported to the
//! caller as an error that [`crate::QueryCache`] logs and ignores, so an
//! unavailable Redis degrades to L1 and the database instead of failing
//! requests.

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{StreamExt, stream::BoxStream};
use redis::{AsyncCommands, aio::ConnectionManager};
use tokio::sync::mpsc;

use crate::{CacheError, InvalidationBus, RemoteStore, Tag};

/// Deadline for one Redis command; a slow Redis must not slow requests.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(250);
const KEY_PREFIX: &str = "attricat:cache:";
const TAG_PREFIX: &str = "attricat:tag:";
const CHANNEL: &str = "attricat:cache:invalidate";

fn error(error: impl std::fmt::Display) -> CacheError {
    CacheError(error.to_string())
}

async fn bounded<T>(
    future: impl std::future::Future<Output = redis::RedisResult<T>>,
) -> Result<T, CacheError> {
    tokio::time::timeout(COMMAND_TIMEOUT, future)
        .await
        .map_err(|_| CacheError("redis command timed out".into()))?
        .map_err(error)
}

/// Connects to `url` with an automatically reconnecting connection.
pub async fn connect(url: &str) -> Result<(redis::Client, ConnectionManager), CacheError> {
    let client = redis::Client::open(url).map_err(error)?;
    let manager = tokio::time::timeout(Duration::from_secs(5), client.get_connection_manager())
        .await
        .map_err(|_| CacheError("redis connection timed out".into()))?
        .map_err(error)?;
    Ok((client, manager))
}

/// Serialized cache values, with one set of keys per tag for invalidation.
#[derive(Clone)]
pub struct RedisStore {
    connection: ConnectionManager,
}

impl RedisStore {
    pub fn new(connection: ConnectionManager) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl RemoteStore for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<Bytes>, CacheError> {
        let mut connection = self.connection.clone();
        let value: Option<Vec<u8>> = bounded(connection.get(format!("{KEY_PREFIX}{key}"))).await?;
        Ok(value.map(Bytes::from))
    }

    async fn set(
        &self,
        key: &str,
        value: Bytes,
        ttl: Option<Duration>,
        tags: &[Tag],
    ) -> Result<(), CacheError> {
        let mut connection = self.connection.clone();
        let key = format!("{KEY_PREFIX}{key}");
        let mut pipeline = redis::pipe();
        match ttl {
            Some(ttl) => pipeline
                .cmd("SET")
                .arg(&key)
                .arg(value.as_ref())
                .arg("PX")
                .arg(ttl.as_millis().max(1) as u64)
                .ignore(),
            None => pipeline.cmd("SET").arg(&key).arg(value.as_ref()).ignore(),
        };
        for tag in tags {
            let tag_key = format!("{TAG_PREFIX}{tag}");
            pipeline.cmd("SADD").arg(&tag_key).arg(&key).ignore();
            // A tag's key set lives at least as long as its entries.
            if let Some(ttl) = ttl {
                pipeline
                    .cmd("PEXPIRE")
                    .arg(&tag_key)
                    .arg(ttl.as_millis().max(1) as u64)
                    .arg("GT")
                    .ignore();
            }
        }
        bounded(pipeline.query_async::<()>(&mut connection)).await
    }

    async fn delete_tag(&self, tag: &Tag) -> Result<(), CacheError> {
        let mut connection = self.connection.clone();
        let tag_key = format!("{TAG_PREFIX}{tag}");
        let keys: Vec<String> = bounded(connection.smembers(&tag_key)).await?;
        let mut pipeline = redis::pipe();
        for key in &keys {
            pipeline.cmd("DEL").arg(key).ignore();
        }
        pipeline.cmd("DEL").arg(&tag_key).ignore();
        bounded(pipeline.query_async::<()>(&mut connection)).await
    }
}

/// Invalidations broadcast with Redis pub/sub. Every replica, including the
/// publisher, drops its L1 entries for a received tag.
pub struct RedisBus {
    client: redis::Client,
    connection: ConnectionManager,
}

impl RedisBus {
    pub fn new(client: redis::Client, connection: ConnectionManager) -> Self {
        Self { client, connection }
    }
}

#[async_trait]
impl InvalidationBus for RedisBus {
    async fn publish(&self, tag: &Tag) -> Result<(), CacheError> {
        let mut connection = self.connection.clone();
        bounded(connection.publish::<_, _, ()>(CHANNEL, tag.as_str())).await
    }

    fn subscribe(&self) -> BoxStream<'static, Tag> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let client = self.client.clone();
        tokio::spawn(async move {
            // Reconnect until nobody listens any more.
            while !sender.is_closed() {
                match client.get_async_pubsub().await {
                    Ok(mut pubsub) => {
                        if let Err(error) = pubsub.subscribe(CHANNEL).await {
                            tracing::warn!(%error, "cache invalidation subscription failed");
                        } else {
                            let mut messages = pubsub.on_message();
                            while let Some(message) = messages.next().await {
                                if let Ok(tag) = message.get_payload::<String>()
                                    && sender.send(Tag::from_wire(&tag)).is_err()
                                {
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "cache invalidation connection failed");
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|tag| (tag, receiver))
        })
        .boxed()
    }
}
