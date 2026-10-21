-- The maintenance worker's retention sweep. Each index bounds one family's
-- expiry scan to rows that can already be deleted.

-- Event-delivery envelopes go with their delivery, so they are left out.
CREATE INDEX tasks_terminal_retention_idx ON tasks(created_at, id) WHERE status IN ('succeeded', 'cancelled') AND kind <> 'event_delivery.v1';

CREATE INDEX rule_runs_terminal_retention_idx ON rule_runs(created_at, id) WHERE status IN ('completed', 'cancelled', 'dead_letter');
-- "Is there a newer run of this rule version?" keeps the latest run.
CREATE INDEX rule_runs_version_created_idx ON rule_runs(workspace_id, rule_id, rule_version, created_at);
-- The enable gate's latest completed full dry run.
CREATE INDEX rule_runs_enable_gate_idx ON rule_runs(workspace_id, rule_id, rule_version, completed_at) WHERE dry_run AND scope_record_id IS NULL AND status = 'completed';
-- A finding keeps its resolution when the run that resolved it expires.
CREATE INDEX rule_findings_resolved_by_run_idx ON rule_findings(resolved_by_run_id) WHERE resolved_by_run_id IS NOT NULL;
ALTER TABLE rule_findings
    DROP CONSTRAINT rule_findings_resolved_by_run_id_fkey,
    ADD CONSTRAINT rule_findings_resolved_by_run_id_fkey FOREIGN KEY (resolved_by_run_id) REFERENCES rule_runs(id) ON DELETE SET NULL;

CREATE INDEX workflow_runs_terminal_retention_idx ON workflow_runs(created_at, id) WHERE status IN ('completed', 'cancelled', 'dead_letter');
-- A run keeps its copied `trigger_event` when the event itself expires.
CREATE INDEX workflow_runs_trigger_event_idx ON workflow_runs(trigger_event_id) WHERE trigger_event_id IS NOT NULL;
CREATE INDEX workflow_runs_root_trigger_event_idx ON workflow_runs(root_trigger_event_id) WHERE root_trigger_event_id IS NOT NULL;
ALTER TABLE workflow_runs
    DROP CONSTRAINT workflow_runs_trigger_event_id_fkey,
    ADD CONSTRAINT workflow_runs_trigger_event_id_fkey FOREIGN KEY (trigger_event_id) REFERENCES domain_events(id) ON DELETE SET NULL,
    DROP CONSTRAINT workflow_runs_root_trigger_event_id_fkey,
    ADD CONSTRAINT workflow_runs_root_trigger_event_id_fkey FOREIGN KEY (root_trigger_event_id) REFERENCES domain_events(id) ON DELETE SET NULL;

CREATE INDEX extension_operation_runs_terminal_retention_idx ON extension_operation_runs(created_at, id) WHERE outputs_expired AND status IN ('completed', 'cancelled', 'dead_letter');

CREATE INDEX agent_run_events_created_idx ON agent_run_events(created_at, id);

-- Purge jobs stay: a completed one stops a file from being queued for purging
-- again, and a failed one is evidence of a purge that cannot finish.
CREATE INDEX file_processing_jobs_terminal_retention_idx ON file_processing_jobs(created_at, id) WHERE kind <> 'purge' AND status IN ('completed', 'failed');

CREATE INDEX domain_events_occurred_at_idx ON domain_events(occurred_at, id);
-- The highest purged sequence per workspace. Change-feed cursors older than it
-- have lost events and must restart from a snapshot.
CREATE TABLE domain_event_retention (
    workspace_id UUID PRIMARY KEY REFERENCES workspaces(id),
    purged_through_sequence BIGINT NOT NULL CHECK (purged_through_sequence > 0)
);

CREATE INDEX solution_pack_check_runs_retention_idx ON solution_pack_check_runs(completed_at, id) WHERE trigger <> 'post_apply';

CREATE INDEX browser_sessions_expires_at_idx ON browser_sessions(expires_at);
CREATE INDEX browser_sessions_revoked_at_idx ON browser_sessions(revoked_at) WHERE revoked_at IS NOT NULL;
CREATE INDEX user_lifecycle_action_tokens_expires_at_idx ON user_lifecycle_action_tokens(expires_at, id);
CREATE INDEX workspace_invitations_unaccepted_expires_at_idx ON workspace_invitations(expires_at, id) WHERE accepted_at IS NULL;
CREATE INDEX browser_login_rate_limits_window_idx ON browser_login_rate_limits(window_started_at);
CREATE INDEX password_reset_rate_limits_window_idx ON password_reset_rate_limits(window_started_at);
CREATE INDEX workspace_discovery_rate_limits_window_idx ON workspace_discovery_rate_limits(window_started_at);

-- Opt-in history retention.
CREATE INDEX user_notifications_read_retention_idx ON user_notifications(read_at, id) WHERE read_at IS NOT NULL;
CREATE INDEX audit_events_occurred_at_idx ON audit_events(occurred_at, id);
