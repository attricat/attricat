//! Durable, at-least-once dispatcher for internal domain-event handlers.
//!
//! Handlers must be idempotent: a lease can expire after a handler has made a
//! durable change but before its acknowledgement commits, and delivery order is
//! intentionally not guaranteed.
use std::{collections::HashSet, env, sync::Arc, time::Duration};

use async_trait::async_trait;
use catalog_repository::round_trips::measure;
use uuid::Uuid;

use crate::{
    domain_events::DomainEvent,
    repository::{CatalogRepository, RepositoryError, SystemRepository},
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

    /// The built-in handlers. There are none today; workflow and rule
    /// handlers are added by their runtimes.
    pub fn default_handlers() -> Self {
        Self::default()
    }

    pub fn with_handler(mut self, handler: Arc<dyn EventHandler>) -> Result<Self, &'static str> {
        if handler.name().is_empty()
            || handler.event_types().is_empty()
            || self
                .handlers
                .iter()
                .any(|existing| existing.name() == handler.name())
        {
            return Err("event handlers need unique names and at least one event type");
        }
        self.handlers.push(handler);
        Ok(self)
    }
}

/// Deliveries one handler processes per workspace before yielding to the
/// next workspace.
const MAX_DELIVERIES_PER_TICK: usize = 64;
/// How often queue-depth gauges are refreshed.
const HEALTH_INTERVAL: Duration = Duration::from_secs(5);

pub fn start(
    repository: impl Into<SystemRepository>,
    registry: EventHandlerRegistry,
    config: DispatcherConfig,
    shutdown: tokio::sync::watch::Receiver<()>,
) -> Vec<tokio::task::JoinHandle<()>> {
    let repository = repository.into();
    registry.handlers.into_iter().enumerate().map(|(index, handler)| {
        let repository = repository.clone();
        let config = config.clone();
        let mut shutdown = shutdown.clone();
        // One handler publishes the workspace-wide queue gauges.
        let publishes_health = index == 0;
        tokio::spawn(async move {
            let mut state = DispatchState::default();
            loop {
                let delay = match measure(
                    format!("worker:event_dispatcher:{}", handler.name()),
                    dispatch_handler(&repository, handler.as_ref(), &config, &mut state, publishes_health),
                )
                .await
                {
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

/// Per-handler loop state.
#[derive(Default)]
struct DispatchState {
    /// Workspaces whose consumer row exists; it is never removed afterwards.
    registered: HashSet<Uuid>,
    last_health: Option<std::time::Instant>,
}

async fn dispatch_handler(
    repository: &SystemRepository,
    handler: &dyn EventHandler,
    config: &DispatcherConfig,
    state: &mut DispatchState,
    publishes_health: bool,
) -> Result<(), RepositoryError> {
    let publish_health = publishes_health
        && state
            .last_health
            .is_none_or(|last| last.elapsed() >= HEALTH_INTERVAL);
    if publish_health {
        state.last_health = Some(std::time::Instant::now());
    }
    for workspace_id in repository.polled_workspace_ids().await?.as_ref().clone() {
        let repository = repository.for_workspace(workspace_id).await?;
        if !state.registered.contains(&workspace_id) {
            repository
                .ensure_event_consumer(handler.name(), handler.event_types())
                .await?;
            state.registered.insert(workspace_id);
        }
        if publish_health {
            record_delivery_health(&repository, workspace_id).await?;
        }
        for _ in 0..MAX_DELIVERIES_PER_TICK {
            let Some(delivery) = repository
                .claim_event_delivery(
                    handler.name(),
                    handler.event_types(),
                    &Uuid::new_v4().to_string(),
                    config.lease_duration,
                )
                .await?
            else {
                break;
            };
            deliver(&repository, handler, config, delivery).await?;
        }
    }
    Ok(())
}

async fn deliver(
    repository: &CatalogRepository,
    handler: &dyn EventHandler,
    config: &DispatcherConfig,
    delivery: crate::repository::EventDelivery,
) -> Result<(), RepositoryError> {
    metrics::counter!("catalog_event_deliveries_total", "outcome" => "claimed").increment(1);
    tracing::info!(handler = handler.name(), event_id = %delivery.event.id, attempt = delivery.attempts, "event delivery claimed");
    let context = EventHandlerCommandContext {
        repository: repository.for_event_handler(&delivery.event, handler.name()),
    };
    // Bound handler execution below the lease so a stalled dependency
    // cannot indefinitely block deliveries in every later workspace.
    // Handlers must be idempotent: cancellation can follow a durable write.
    let result = tokio::time::timeout(
        config.lease_duration / 2,
        handler.handle(delivery.event.clone(), context),
    )
    .await;
    match result {
        Ok(Ok(())) => {
            repository.complete_event_delivery(&delivery).await?;
            metrics::counter!("catalog_event_deliveries_total", "outcome" => "completed")
                .increment(1);
        }
        failure => {
            let error = match failure {
                Ok(Err(error)) => error.to_string(),
                Err(_) => "event handler timed out".to_owned(),
                Ok(Ok(())) => unreachable!(),
            };
            tracing::warn!(handler = handler.name(), event_id = %delivery.event.id, %error, "event handler failed");
            repository
                .retry_event_delivery(
                    &delivery,
                    &error,
                    config.retry_delay(delivery.attempts),
                    config.max_attempts,
                )
                .await?;
            metrics::counter!("catalog_event_deliveries_total", "outcome" => if delivery.attempts >= config.max_attempts { "dead_letter" } else { "retry" }).increment(1);
        }
    }
    Ok(())
}

/// Publishes queue depth per workspace, consumer and status. Statuses absent
/// from the aggregate are reset so a drained queue does not keep reporting its
/// last non-zero depth.
async fn record_delivery_health(
    repository: &CatalogRepository,
    workspace_id: Uuid,
) -> Result<(), RepositoryError> {
    const STATUSES: [&str; 4] = ["pending", "leased", "completed", "dead_letter"];
    let health = repository.event_delivery_health_by_consumer().await?;
    let workspace = workspace_id.to_string();
    let consumers = health
        .iter()
        .map(|(consumer, _, _)| consumer.as_str())
        .collect::<HashSet<_>>();
    for consumer in consumers {
        for status in STATUSES {
            let count = health
                .iter()
                .find(|(name, state, _)| name == consumer && state == status)
                .map_or(0, |(_, _, count)| *count);
            metrics::gauge!(
                "catalog_event_delivery_queue_depth",
                "workspace_id" => workspace.clone(),
                "consumer" => consumer.to_owned(),
                "status" => status
            )
            .set(count as f64);
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
