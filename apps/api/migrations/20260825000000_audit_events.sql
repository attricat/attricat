-- Immutable, tenant-scoped audit evidence. Event metadata is deliberately
-- application supplied and must be redacted before insertion; database triggers
-- below never copy credential hashes or token digests.
CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    workspace_id UUID REFERENCES workspaces (id),
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action ~ '^[a-z][a-z0-9_.-]+$'),
    authorization_scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    target JSONB NOT NULL DEFAULT '{}'::jsonb,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_events_workspace_occurred_at_idx ON audit_events (workspace_id, occurred_at DESC);
CREATE INDEX audit_events_actor_occurred_at_idx ON audit_events (actor_user_id, occurred_at DESC);
