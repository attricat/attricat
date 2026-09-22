-- The generic API task envelope is intentionally separate from domain state.
-- File processing remains on file_processing_jobs and is not represented here.
CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    kind TEXT NOT NULL CHECK (kind IN (
        'agent_run.v1',
        'event_delivery.v1',
        'workflow_run.v1',
        'rule_run.v1',
        'blueprint_migration_batch.v1'
    )),
    envelope_version INTEGER NOT NULL CHECK (envelope_version = 1),
    subject_id UUID NOT NULL,
    generation INTEGER NOT NULL CHECK (generation >= 0),
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'leased', 'succeeded', 'dead_letter', 'cancelled'
    )),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0),
    max_failures INTEGER NOT NULL CHECK (max_failures > 0),
    lease_owner TEXT,
    lease_token UUID,
    lease_until TIMESTAMPTZ,
    last_error_code TEXT,
    last_error_message TEXT,
    correlation_id UUID,
    causation_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, kind, subject_id, generation),
    CHECK (char_length(lease_owner) <= 128),
    CHECK (char_length(last_error_code) <= 128),
    CHECK (char_length(last_error_message) <= 1024),
    CHECK (
        (status = 'leased' AND lease_owner IS NOT NULL AND lease_token IS NOT NULL AND lease_until IS NOT NULL)
        OR (status <> 'leased' AND lease_owner IS NULL AND lease_token IS NULL AND lease_until IS NULL)
    )
);
CREATE INDEX tasks_due_claim_idx
    ON tasks (available_at, created_at, id)
    WHERE status = 'queued';
CREATE INDEX tasks_expired_lease_idx
    ON tasks (lease_until, created_at, id)
    WHERE status = 'leased';
CREATE INDEX tasks_workspace_kind_history_idx
    ON tasks (workspace_id, kind, created_at DESC);

-- Updated in the claim transaction to make cross-workspace service durable
-- and prevent a continuously busy workspace from starving another tenant.
CREATE TABLE task_workspace_service (
    workspace_id UUID PRIMARY KEY REFERENCES workspaces(id),
    last_served_at TIMESTAMPTZ NOT NULL DEFAULT '-infinity'::timestamptz,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX task_workspace_service_fairness_idx
    ON task_workspace_service (last_served_at, workspace_id);
