-- Transactional outbox foundation. Delivery behavior remains in repository and
-- worker code; this migration declares only durable facts and their indexes.
CREATE TABLE domain_events (
    id UUID PRIMARY KEY,
    sequence BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE NOT NULL,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    event_type TEXT NOT NULL CHECK (event_type ~ '^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+\.v[0-9]+$'),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind ~ '^[a-z][a-z0-9_-]*$'),
    aggregate_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    causation_id UUID,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('api', 'worker', 'plugin', 'system')),
    source_name TEXT NOT NULL CHECK (source_name ~ '^[a-z][a-z0-9_-]*$'),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload) = 'object')
);
CREATE INDEX domain_events_workspace_sequence_idx ON domain_events (workspace_id, sequence);
CREATE INDEX domain_events_workspace_type_sequence_idx ON domain_events (workspace_id, event_type, sequence);
CREATE INDEX domain_events_workspace_aggregate_sequence_idx ON domain_events (workspace_id, aggregate_kind, aggregate_id, sequence);

CREATE TABLE event_consumers (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    name TEXT NOT NULL CHECK (name ~ '^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$'),
    watermark BIGINT NOT NULL DEFAULT 0 CHECK (watermark >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, name)
);

CREATE TABLE event_deliveries (
    consumer_id UUID NOT NULL REFERENCES event_consumers (id),
    event_id UUID NOT NULL REFERENCES domain_events (id),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'leased', 'completed', 'dead_letter')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    last_error TEXT,
    PRIMARY KEY (consumer_id, event_id),
    CHECK ((lease_owner IS NULL) = (lease_until IS NULL))
);
CREATE INDEX event_deliveries_poll_idx ON event_deliveries (status, next_attempt_at, lease_until);
CREATE INDEX event_deliveries_event_idx ON event_deliveries (event_id);
