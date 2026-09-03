-- Read-path indexes for tenant-scoped audit-log filtering.
CREATE INDEX audit_events_workspace_actor_occurred_at_idx
    ON audit_events (workspace_id, actor_user_id, occurred_at DESC);
CREATE INDEX audit_events_workspace_executor_occurred_at_idx
    ON audit_events (workspace_id, executor_type, occurred_at DESC);
CREATE INDEX audit_events_workspace_action_occurred_at_idx
    ON audit_events (workspace_id, action, occurred_at DESC);
CREATE INDEX audit_events_workspace_target_type_occurred_at_idx
    ON audit_events (workspace_id, (target->>'type'), occurred_at DESC);
