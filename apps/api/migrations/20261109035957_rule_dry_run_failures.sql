-- The first records a dry run would flag, so its result can name them. A dry
-- run saves no findings; these rows go with the run.
CREATE TABLE rule_dry_run_failures (
    run_id UUID NOT NULL REFERENCES rule_runs(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    record_id UUID NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    evaluation_key TEXT NOT NULL,
    message TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    position BIGINT NOT NULL CHECK (position >= 0),
    PRIMARY KEY (run_id, record_id, evaluation_key)
);
CREATE INDEX rule_dry_run_failures_record_idx ON rule_dry_run_failures(record_id);
