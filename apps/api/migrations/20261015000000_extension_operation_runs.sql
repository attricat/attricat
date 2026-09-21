ALTER TABLE tasks DROP CONSTRAINT tasks_kind_check;
ALTER TABLE tasks ADD CONSTRAINT tasks_kind_check CHECK (kind IN ('agent_run.v1','event_delivery.v1','workflow_run.v1','rule_run.v1','blueprint_migration_batch.v1','extension_operation_run.v1'));

CREATE TABLE extension_operation_runs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL,
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases(id),
    abi_version TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    actor_user_id UUID,
    actor_token_id UUID,
    configuration_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    source_reference JSONB NOT NULL DEFAULT '{}'::jsonb,
    destination_reference JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','leased','cancelled','completed','dead_letter')),
    cancellation_requested BOOLEAN NOT NULL DEFAULT false,
    -- The worker sets this only after the component's cooperative cancel hook
    -- returns under the live task lease. A requested cancellation can never
    -- terminalize a run before the hook has been delivered.
    cancellation_delivered BOOLEAN NOT NULL DEFAULT false,
    lifecycle_started BOOLEAN NOT NULL DEFAULT false,
    progress JSONB NOT NULL DEFAULT '{}'::jsonb,
    checkpoint JSONB NOT NULL DEFAULT '{}'::jsonb,
    batch_number INTEGER NOT NULL DEFAULT 0 CHECK (batch_number >= 0),
    idempotency_key TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    lease_token UUID,
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    last_error_code TEXT,
    last_error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, extension_id, installed_release_id, operation_id, idempotency_key),
    CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    CHECK (char_length(abi_version) BETWEEN 1 AND 32),
    CHECK (char_length(operation_id) BETWEEN 1 AND 128),
    CHECK (char_length(idempotency_key) BETWEEN 1 AND 128),
    CHECK (char_length(lease_owner) <= 128),
    CHECK (char_length(last_error_code) <= 128),
    CHECK (char_length(last_error_message) <= 1024),
    CHECK ((status = 'leased' AND lease_token IS NOT NULL AND lease_owner IS NOT NULL AND lease_until IS NOT NULL) OR (status <> 'leased'))
);
CREATE INDEX extension_operation_runs_active_idx ON extension_operation_runs(workspace_id, status, created_at) WHERE status IN ('pending','leased');
