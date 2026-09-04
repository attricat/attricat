//! Durable, at-least-once dispatcher for internal domain-event handlers.
//!
//! Handlers must be idempotent: a lease can expire after a handler has made a
//! durable change but before its acknowledgement commits, and delivery order is
//! intentionally not guaranteed.
use std::{collections::HashSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    domain_events::DomainEvent,
    repository::{CatalogRepository, RepositoryError},
};

const LEASE_DURATION: Duration = Duration::from_secs(30);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

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

pub fn start(repository: CatalogRepository, registry: EventHandlerRegistry) {
    for handler in registry.handlers {
        let repository = repository.clone();
        tokio::spawn(async move {
            loop {
                if let Err(error) = dispatch_handler(&repository, handler.as_ref()).await {
                    tracing::error!(handler = handler.name(), %error, "event handler poll failed");
                    tokio::time::sleep(RETRY_DELAY).await;
                } else {
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
        });
    }
}

async fn dispatch_handler(
    repository: &CatalogRepository,
    handler: &dyn EventHandler,
) -> Result<(), RepositoryError> {
    for workspace_id in repository.active_workspace_ids().await? {
        let repository = repository.for_workspace(workspace_id).await?;
        repository
            .ensure_event_consumer(handler.name(), handler.event_types())
            .await?;
        let Some(delivery) = repository
            .claim_event_delivery(
                handler.name(),
                handler.event_types(),
                &Uuid::new_v4().to_string(),
                LEASE_DURATION,
            )
            .await?
        else {
            continue;
        };
        let context = EventHandlerCommandContext {
            repository: repository.for_event_handler(&delivery.event, handler.name()),
        };
        match handler.handle(delivery.event.clone(), context).await {
            Ok(()) => repository.complete_event_delivery(&delivery).await?,
            Err(error) => {
                tracing::warn!(handler = handler.name(), event_id = %delivery.event.id, %error, "event handler failed");
                repository
                    .retry_event_delivery(&delivery, &error.to_string(), RETRY_DELAY)
                    .await?;
            }
        }
    }
    Ok(())
}
