# Catalog domain eventing

Catalog writes use a PostgreSQL transactional outbox. The outbox is the durable
source for asynchronous internal work; it is not an API feed and must never be
exposed directly to browser plugins.

## Contract

Every event has an immutable, versioned envelope with an ID, ordered outbox
watermark, workspace ID, timestamp, event type, aggregate kind/ID,
correlation/causation IDs, source kind/name, metadata, and payload. Routing and
operational facts are normalized columns; only metadata and event payload facts
are stored as JSONB.

Core event types are constants in `api::domain_events` and use the `*.v1`
version suffix. Plugin-produced event types must use the form
`plugin.<publisher>.<name>.v<version>`. The core namespaces (`entity`,
`attribute_value`, `relationship`, `blueprint`, and `context`) are reserved.
Consumers must ignore event versions they do not support rather than attempt to
reinterpret a payload.

Entity writes produce one event for each public logical mutation: entity create,
update, and delete use `entity.*`; scalar append and restore use
`attribute_value.*`; relationship replacement and removal use
`relationship.changed.v1`. Entity and value payloads identify the entity and,
where applicable, its blueprint. Their `facts` arrays contain only normalized
affected attribute facts (attribute/context IDs and codes, relationship target,
change hint, and before/after values); they never contain an entity snapshot.

## Delivery semantics

The startup dispatcher provides at-least-once delivery. Consumers must tolerate
duplicates and reordering. A consumer created through the repository begins at
the workspace's current outbox watermark, so newly registered internal handlers
receive future events only. PostgreSQL materializes matching deliveries and
claims them with `FOR UPDATE SKIP LOCKED`; completion and retries are protected
by the lease owner. Delivery attempts, leases, completion, and failure
diagnostics are durable `event_deliveries` facts.

Handlers have stable names and exact version filters. They receive the typed
`DomainEvent` envelope and a command context. Catalog writes made through that
context emit ordinary worker events retaining the triggering correlation ID and
using the triggering event ID as causation. Handlers must suppress events they
produce themselves when that would make a feedback loop; the initial
computed-field reservation handler demonstrates this rule.

A producer appends its event through the caller-owned SQL transaction. The
catalog mutation, audit evidence, and outbox row either all commit or all roll
back. External webhooks, dead-letter administration, and any client-safe event
bridge remain separate follow-up work.
