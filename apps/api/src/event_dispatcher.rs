//! Durable, at-least-once dispatcher for internal domain-event handlers.
//!
//! Handlers must be idempotent: a lease can expire after a handler has made a
//! durable change but before its acknowledgement commits, and delivery order is
//! intentionally not guaranteed.
use std::{collections::HashSet, env, sync::Arc, time::Duration};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    domain_events::DomainEvent,
    repository::{CatalogRepository, RepositoryError},
};

#[derive(Clone, Debug)]
pub struct DispatcherConfig {
    lease_duration: Duration,
    retry_initial_delay: Duration,
    retry_max_delay: Duration,
    max_attempts: i32,
    poll_interval: Duration,
}

impl DispatcherConfig {
    pub fn new(
        lease_duration: Duration,
        retry_initial_delay: Duration,
        retry_max_delay: Duration,
        max_attempts: i32,
        poll_interval: Duration,
    ) -> Result<Self, String> {
        if lease_duration.is_zero()
            || retry_initial_delay.is_zero()
            || retry_max_delay.is_zero()
            || poll_interval.is_zero()
            || max_attempts <= 0
        {
            return Err("dispatcher durations and max attempts must be positive".to_owned());
        }
        Ok(Self {
            lease_duration,
            retry_initial_delay,
            retry_max_delay,
            max_attempts,
            poll_interval,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        fn positive(name: &str, default: u64) -> Result<u64, String> {
            match env::var(name) {
                Ok(value) => value
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("{name} must be a positive integer")),
                Err(_) => Ok(default),
            }
        }
        Self::new(
            Duration::from_secs(positive("EVENT_DISPATCHER_LEASE_SECONDS", 30)?),
            Duration::from_secs(positive("EVENT_DISPATCHER_RETRY_INITIAL_SECONDS", 1)?),
            Duration::from_secs(positive("EVENT_DISPATCHER_RETRY_MAX_SECONDS", 60)?),
            positive("EVENT_DISPATCHER_MAX_ATTEMPTS", 5)?
                .try_into()
                .map_err(|_| "EVENT_DISPATCHER_MAX_ATTEMPTS is too large".to_owned())?,
            Duration::from_millis(positive("EVENT_DISPATCHER_POLL_MILLIS", 250)?),
        )
    }

    fn retry_delay(&self, attempts: i32) -> Duration {
        self.retry_initial_delay
            .saturating_mul(2_u32.saturating_pow((attempts.saturating_sub(1) as u32).min(16)))
            .min(self.retry_max_delay)
    }
}

/// The repository supplied to a handler emits worker-originated events which
/// retain the triggering correlation id and use the triggering event as their
/// causation id.
pub struct EventHandlerCommandContext {
    repository: CatalogRepository,
}

impl EventHandlerCommandContext {
    pub fn repository(&self) -> &CatalogRepository {
        &self.repository
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    /// A stable, deployment-independent consumer name.
    fn name(&self) -> &'static str;

    /// Exact event versions this handler understands. Unknown versions are not
    /// delivered to the handler.
    fn event_types(&self) -> &'static [&'static str];

    async fn handle(
        &self,
        event: DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone, Default)]
pub struct EventHandlerRegistry {
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl EventHandlerRegistry {
    pub fn new(handlers: Vec<Arc<dyn EventHandler>>) -> Result<Self, &'static str> {
        let mut names = HashSet::new();
        if handlers.iter().any(|handler| {
            handler.name().is_empty()
                || handler.event_types().is_empty()
                || !names.insert(handler.name())
        }) {
            return Err("event handlers need unique names and at least one event type");
        }
        Ok(Self { handlers })
    }

    pub fn default_handlers() -> Self {
        Self::new(vec![Arc::new(ComputedFieldHandler)]).expect("built-in handlers are valid")
    }
}

/// Reservation point for computed-field invalidation. It deliberately does no
/// writes yet; it also ignores its own worker events to prevent a future
/// computed-field mutation from recursively scheduling itself.
struct ComputedFieldHandler;

#[async_trait]
impl EventHandler for ComputedFieldHandler {
    fn name(&self) -> &'static str {
        "catalog.computed_fields"
    }

    fn event_types(&self) -> &'static [&'static str] {
        &["attribute_value.changed.v1", "relationship.changed.v1"]
    }

    async fn handle(
        &self,
        event: DomainEvent,
        _context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if event.source_kind == "worker" && event.source_name == self.name() {
            return Ok(());
        }
        // Computed-field definitions are intentionally a later feature.
        Ok(())
    }
}

pub fn start(
    repository: CatalogRepository,
    registry: EventHandlerRegistry,
    config: DispatcherConfig,
    shutdown: tokio::sync::watch::Receiver<()>,
) -> Vec<tokio::task::JoinHandle<()>> {
    registry.handlers.into_iter().map(|handler| {
        let repository = repository.clone();
        let config = config.clone();
        let mut shutdown = shutdown.clone();
        tokio::spawn(async move {
            loop {
                let delay = match dispatch_handler(&repository, handler.as_ref(), &config).await {
                    Ok(()) => config.poll_interval,
                    Err(error) => {
                        tracing::error!(handler = handler.name(), %error, "event handler poll failed");
                        config.retry_initial_delay
                    }
                };
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {},
                    _ = shutdown.changed() => {
                        tracing::info!(handler = handler.name(), "event handler stopped");
                        return;
                    }
                }
            }
        })
    }).collect()
}

async fn dispatch_handler(
    repository: &CatalogRepository,
    handler: &dyn EventHandler,
    config: &DispatcherConfig,
) -> Result<(), RepositoryError> {
    for workspace_id in repository.active_workspace_ids().await? {
        let repository = repository.for_workspace(workspace_id).await?;
        repository
            .ensure_event_consumer(handler.name(), handler.event_types())
            .await?;
        for (status, count) in repository.event_delivery_health().await? {
            metrics::gauge!("catalog_event_delivery_queue_depth", "status" => status)
                .set(count as f64);
        }
        let Some(delivery) = repository
            .claim_event_delivery(
                handler.name(),
                handler.event_types(),
                &Uuid::new_v4().to_string(),
                config.lease_duration,
            )
            .await?
        else {
            continue;
        };
        metrics::counter!("catalog_event_deliveries_total", "outcome" => "claimed").increment(1);
        tracing::info!(handler = handler.name(), event_id = %delivery.event.id, attempt = delivery.attempts, "event delivery claimed");
        let context = EventHandlerCommandContext {
            repository: repository.for_event_handler(&delivery.event, handler.name()),
        };
        match handler.handle(delivery.event.clone(), context).await {
            Ok(()) => {
                repository.complete_event_delivery(&delivery).await?;
                metrics::counter!("catalog_event_deliveries_total", "outcome" => "completed")
                    .increment(1);
            }
            Err(error) => {
                tracing::warn!(handler = handler.name(), event_id = %delivery.event.id, %error, "event handler failed");
                repository
                    .retry_event_delivery(
                        &delivery,
                        &error.to_string(),
                        config.retry_delay(delivery.attempts),
                        config.max_attempts,
                    )
                    .await?;
                metrics::counter!("catalog_event_deliveries_total", "outcome" => if delivery.attempts >= config.max_attempts { "dead_letter" } else { "retry" }).increment(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_is_bounded_exponential() {
        let config = DispatcherConfig::from_env().unwrap();
        assert_eq!(config.retry_delay(1), Duration::from_secs(1));
        assert_eq!(config.retry_delay(2), Duration::from_secs(2));
        assert_eq!(config.retry_delay(20), Duration::from_secs(60));
    }
}
