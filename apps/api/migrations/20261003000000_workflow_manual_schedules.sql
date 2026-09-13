-- Slice 5 workflow sources. Scheduling and lifecycle transitions remain Rust transactions.
ALTER TABLE workflow_runs
    ALTER COLUMN trigger_event_id DROP NOT NULL,
    ALTER COLUMN trigger_sequence DROP NOT NULL,
    ADD COLUMN source TEXT NOT NULL DEFAULT 'event' CHECK (source IN ('event', 'manual', 'schedule')),
    ADD COLUMN idempotency_key TEXT;

-- Existing event uniqueness remains in force. Non-event sources have an explicit stable key.
CREATE UNIQUE INDEX workflow_runs_source_idempotency_idx
    ON workflow_runs (workspace_id, workflow_id, workflow_version, source, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- One durable cursor per enabled revision and schedule trigger index. The worker owns all updates.
CREATE TABLE workflow_schedule_states (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    workflow_id UUID NOT NULL,
    workflow_version BIGINT NOT NULL,
    trigger_index INTEGER NOT NULL CHECK (trigger_index >= 0),
    next_run_at TIMESTAMPTZ NOT NULL,
    misfires BIGINT NOT NULL DEFAULT 0 CHECK (misfires >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, workflow_id, workflow_version, trigger_index),
    FOREIGN KEY (workspace_id, workflow_id, workflow_version)
        REFERENCES workflows(workspace_id, id, version)
);
CREATE INDEX workflow_schedule_states_due_idx ON workflow_schedule_states (workspace_id, next_run_at);
