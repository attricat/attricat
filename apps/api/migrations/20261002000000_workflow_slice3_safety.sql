-- Slice 3 workflow execution state. Transitions and authorization remain in Rust.
ALTER TABLE workflow_runs
    ADD COLUMN cancelled_at TIMESTAMPTZ,
    ADD COLUMN root_trigger_event_id UUID REFERENCES domain_events(id),
    ADD COLUMN causal_depth INTEGER NOT NULL DEFAULT 0 CHECK (causal_depth >= 0 AND causal_depth <= 8);

UPDATE workflow_runs SET root_trigger_event_id = trigger_event_id WHERE root_trigger_event_id IS NULL;
ALTER TABLE workflow_runs ALTER COLUMN root_trigger_event_id SET NOT NULL;

ALTER TABLE workflow_runs DROP CONSTRAINT workflow_runs_status_check;
ALTER TABLE workflow_runs ADD CONSTRAINT workflow_runs_status_check
    CHECK (status IN ('pending','leased','completed','dead_letter','cancelled'));
