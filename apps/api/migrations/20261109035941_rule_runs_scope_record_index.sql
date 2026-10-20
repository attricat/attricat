-- Every record change now re-checks the record for each enabled rule, and the
-- record page lists that record's runs.
CREATE INDEX rule_runs_scope_record_idx ON rule_runs(workspace_id, scope_record_id, created_at DESC) WHERE scope_record_id IS NOT NULL;
