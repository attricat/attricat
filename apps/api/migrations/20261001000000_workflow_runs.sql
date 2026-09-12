-- Durable workflow intake/execution state. Business transitions are implemented in repository code.
CREATE TABLE workflow_runs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    workflow_id UUID NOT NULL,
    workflow_version BIGINT NOT NULL,
    trigger_event_id UUID NOT NULL REFERENCES domain_events(id),
    trigger_sequence BIGINT NOT NULL,
    trigger_event JSONB NOT NULL,
    compiled_plan JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','leased','completed','dead_letter')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    last_error TEXT,
    replayed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, workflow_id, workflow_version)
        REFERENCES workflows(workspace_id, id, version),
    UNIQUE (workspace_id, workflow_id, workflow_version, trigger_event_id)
);
CREATE INDEX workflow_runs_claim_idx ON workflow_runs (workspace_id, status, next_attempt_at);
CREATE INDEX workflow_runs_diagnostics_idx ON workflow_runs (workspace_id, created_at DESC);

-- Each action is committed once before a run may be retried. The action index is
-- immutable and identifies the action in the revision's stored compiled plan.
CREATE TABLE workflow_run_actions (
    run_id UUID NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    action_index INTEGER NOT NULL CHECK (action_index >= 0),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, action_index)
);
