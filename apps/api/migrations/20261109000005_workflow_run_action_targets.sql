-- A referencing_entities_update action changes many entities, each in its own
-- transaction. One row per (run, action, target) is the idempotency key for a
-- target write and the operator-visible record of partial failures.
CREATE TABLE workflow_run_action_targets (
    run_id UUID NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    action_index INTEGER NOT NULL CHECK (action_index >= 0),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed', 'skipped')),
    attempts INTEGER NOT NULL DEFAULT 1 CHECK (attempts > 0),
    last_error TEXT CHECK (last_error IS NULL OR length(last_error) <= 1024),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, action_index, entity_id)
);
CREATE INDEX workflow_run_action_targets_workspace_idx
    ON workflow_run_action_targets (workspace_id, run_id);
