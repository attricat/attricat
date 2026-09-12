# Catalog domain eventing

Catalog uses a PostgreSQL **transactional outbox** for durable internal domain
work. It is an internal, per-workspace integration mechanism—not an HTTP API,
a browser feed, or a plugin capability. Never expose `domain_events`, delivery
rows, or their payloads to a client.

## What is guaranteed

A catalog write appends its outbox event in the same SQL transaction as the
catalog mutation and its audit evidence. All three commit, or all roll back.
The database assigns each event an immutable UUID, `occurred_at`, and a
monotonically increasing `sequence`; `(workspace_id, sequence)` is useful for
storage inspection, but is **not** a delivery-order guarantee.

The dispatcher is durable and **at least once**:

- A handler can receive a duplicate, including after it has completed its own
  write but before Catalog persists the delivery acknowledgement.
- Events can be delivered out of sequence. Different handlers, workspaces, and
  recovered leases are independent; do not infer causal ordering from arrival
  order.
- A handler registered for the first time starts at that workspace's current
  high-water mark. It receives future matching events, not historical ones.
- Exact event-type filters determine which delivery rows exist. An unsupported
  version is skipped; it is never coerced into an older payload shape.

Each API process runs the dispatcher. A delivery is leased before execution, and
PostgreSQL's `FOR UPDATE SKIP LOCKED` prevents concurrent processes from
claiming the same active delivery. A lease that expires is eligible to be
claimed again. Completion and retry updates are conditional on the lease owner,
so a late worker cannot acknowledge a lease now owned by another worker.

## Envelope and producer contract

The durable envelope has these fields:

| Field | Meaning |
| --- | --- |
| `id` | Immutable event UUID. Use it as the deduplication key for one consumer. |
| `sequence` | Database-assigned outbox sequence; scoped inspection/order aid only. |
| `workspace_id`, `occurred_at` | Event tenancy and timestamp. |
| `event_type` | Versioned routing contract, such as `entity.updated.v1`. |
| `aggregate_kind`, `aggregate_id` | The affected aggregate's kind and UUID. |
| `correlation_id`, `causation_id` | Trace an operation and, for follow-on work, its direct triggering event. |
| `source_kind`, `source_name` | Producer identity (`api`, `worker`, `plugin`, or `system`) and stable name. |
| `metadata`, `payload` | JSON objects containing non-routing facts. Each is at most 64 KiB. |

Routing and operational facts are normalized columns, not payload JSON. Event
payloads describe the smallest useful affected facts; they are not entity
snapshots or projections. In particular, entity and value mutations identify
the entity (and, when applicable, its blueprint), while `facts` contains
attribute/context IDs and codes, relationship target IDs, a change kind, and
before/after values. A consumer needing current state must read and authorize
that state itself.

Core types are constants in `api::domain_events` and currently include:

- `entity.created.v1`, `entity.updated.v1`, `entity.deleted.v1`,
  `entity.migrated.v1`, `entity.published.v1`, and `entity.unpublished.v1`
- `attribute_value.changed.v1` and `attribute_value.restored.v1`
- `relationship.changed.v1`
- `blueprint.created.v1`, `blueprint.revision_created.v1`, and
  `blueprint.published.v1`
- `context.created.v1`, `context.updated.v1`, and `context.deleted.v1`

Core namespaces (`entity`, `attribute_value`, `relationship`, `blueprint`, and
`context`) are reserved. A plugin producer must use
`plugin.<publisher>.<name>.v<version>`—for example,
`plugin.acme.score_recomputed.v1`. Every dot-separated identifier must be
lowercase ASCII letters, digits, `_`, or `-`; routing fields are non-blank and
at most 128 bytes. Core event types are closed: do not invent a core-looking
name.

A payload change is a new contract. Keep an existing version immutable and emit
a new `.vN` event type for an incompatible change. Consumers must explicitly
filter the exact versions they understand and ignore other versions.

Use one correlation ID for the request or job that initiated a change. An
initial event normally has no `causation_id`. A worker event retains the
triggering event's `correlation_id` and sets `causation_id` to the triggering
event ID. Do not repurpose either ID as an entity ID or an arbitrary request
label.

The `EventPublisher::enqueue_event` boundary accepts a `NewDomainEvent` and the
**caller-owned SQL transaction**. It appends to the outbox; it does not publish
to a broker. This boundary intentionally leaves room for a future RabbitMQ (or
other) transport adapter without making broker availability part of a catalog
write transaction. There is no RabbitMQ configuration or external webhook
transport today.

## Backend handler author guide

A handler implements `api::event_dispatcher::EventHandler` and is installed in
an `EventHandlerRegistry`. Its `name()` is a durable consumer identity, so make
it stable across deploys (for example, `acme.search_index`). Renaming it creates
a new consumer at the current watermark and does not migrate its old delivery
history. Handler names must be unique in the registry and use the database's lowercase
dotted consumer-identifier format. Event `source_name` is broader: it accepts
letter-led names containing letters, digits, `.`, `_`, `:`, or `-` (for example,
`extension:attricat-extension-example`).

`event_types()` returns an exact, non-empty list of supported version strings.
Do not subscribe broadly and inspect an unknown payload in `handle`; register
the version deliberately. The handler receives a typed `DomainEvent` and an
`EventHandlerCommandContext`:

```rust
impl EventHandler for SearchIndexHandler {
    fn name(&self) -> &'static str { "acme.search_index" }

    fn event_types(&self) -> &'static [&'static str] {
        &["entity.updated.v1", "entity.deleted.v1"]
    }

    async fn handle(
        &self,
        event: DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Treat event.id as a durable idempotency key before making a side effect.
        // Use context.repository() for catalog writes so worker provenance,
        // correlation_id, and causation_id are derived from `event`.
        Ok(())
    }
}
```

Make all effects idempotent with a durable key such as `(handler name, event.id)`
or an equivalent unique upsert in the downstream system. In-memory deduplication
is insufficient across restarts and lease recovery. Return an error for a
retryable failure; do not acknowledge work by swallowing it. A handler may be
called again after success, and it must be safe when its external side effect
already happened.

Prevent feedback loops. If a handler writes catalog state, its derived event is
`source_kind = "worker"` and its `source_name` is the handler name. Ignore an
event with that source when processing it would recursively produce the same
work. The built-in `catalog.computed_fields` reservation handler follows this
rule. Apply a more specific provenance/causation guard as needed, but never
assume delivery order alone breaks a loop.

## Retry, dead letters, and retention

A failed attempt is recorded with an error and `failed_at`. The dispatcher uses
bounded exponential delay: by default 1 second, 2 seconds, 4 seconds, and so
on, capped at 60 seconds. The claim itself counts as an attempt. At the default
fifth failed attempt the delivery becomes `dead_letter`; its immutable
`domain_events` row is unchanged. Replaying a dead letter sets that delivery to
`pending` immediately, clears its failure/lease diagnostics, and **does not**
reset `attempts`. It can therefore become a dead letter again on its next
failure.

Catalog currently has no automatic outbox or delivery retention/pruning job and
no retention configuration. Keep PostgreSQL backups while events and delivery
records are retained. Do not manually delete `domain_events`, `event_consumers`,
or `event_deliveries`: their foreign keys and consumer watermarks are part of
the delivery contract. Establish a tested archival/purge procedure before
introducing retention.

## Operator runbook

The dispatcher emits Prometheus metrics on the normal `/metrics` endpoint
(`data_health.read`):

- `catalog_event_deliveries_total{outcome="claimed|completed|retry|dead_letter"}`
  counts lifecycle outcomes.
- `catalog_event_delivery_queue_depth{status="pending|leased|completed|dead_letter"}`
  is updated while the dispatcher polls each active workspace.

Alert on a growing `pending` queue, a lease that does not recover after the
lease period, or new/increasing dead letters. Queue-depth series are only
reported for statuses observed during polling, so use the dead-letter endpoint
for a definitive operator list.

1. Check API logs for `event delivery claimed`, handler failures, or dispatcher
   poll errors. Record the handler name, event ID, attempt count, and error.
2. Inspect terminal failures with a bearer token that has `data_health.read`:

   ```sh
   catalog --token "$CATALOG_TOKEN" event dead-letters
   # Equivalent HTTP: GET /event-deliveries/dead-letters
   ```

   The JSON array includes `consumer_id`, `event_id`, `consumer_name`,
   `event_type`, `attempts`, `failed_at`, and `last_error`.
3. Fix the handler, dependency, or data condition first. Verify that retrying is
   idempotent and will not recreate an external side effect.
4. Replay one terminal delivery with a token authorized for `roles.manage`:

   ```sh
   catalog --token "$CATALOG_TOKEN" event replay <consumer-id> <event-id>
   # Equivalent HTTP: POST /event-deliveries/{consumer_id}/{event_id}/replay
   ```

   Success returns `{"consumer_id":"…","event_id":"…","status":"pending"}`.
   A non-dead-letter, wrong-workspace, or unknown pair returns `404`; replay is
   not a general event republishing facility.
5. Watch the metrics and logs for a subsequent claim/completion. If it fails
   again, preserve the diagnostic and remediate rather than repeatedly replaying.

Tune only these positive-integer API-process environment variables:
`EVENT_DISPATCHER_LEASE_SECONDS` (default `30`),
`EVENT_DISPATCHER_RETRY_INITIAL_SECONDS` (`1`),
`EVENT_DISPATCHER_RETRY_MAX_SECONDS` (`60`),
`EVENT_DISPATCHER_MAX_ATTEMPTS` (`5`), and
`EVENT_DISPATCHER_POLL_MILLIS` (`250`). Invalid or zero values prevent API
startup. A shorter lease can increase duplicate execution; a higher attempt
limit delays dead-letter visibility.

## Client boundary

The [web-component event bridge](web-component-events.md) is deliberately a
separate, client-safe DOM protocol. It contains coarse IDs and change hints
only, validates strict detail schemas at the host boundary, and is not derived
from or connected to the outbox. It conveys neither authorization nor internal
event facts.
