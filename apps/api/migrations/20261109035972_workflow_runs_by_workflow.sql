CREATE INDEX workflow_runs_workflow_idx ON workflow_runs (workspace_id, workflow_id, created_at DESC, id DESC);
