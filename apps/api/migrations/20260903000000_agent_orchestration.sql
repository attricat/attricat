-- Agent orchestration records the human principal whose permissions govern a
-- run. Provider credentials and authorization policy remain application code.
ALTER TABLE agent_runs ADD COLUMN initiated_by_user_id UUID REFERENCES users (id);
CREATE INDEX agent_runs_workspace_initiator_idx ON agent_runs (workspace_id, initiated_by_user_id, created_at DESC);

