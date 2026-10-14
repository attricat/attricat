-- An approved bulk value edit reports each record it applies or stops at.
ALTER TABLE agent_run_events DROP CONSTRAINT agent_run_events_event_type_check;
ALTER TABLE agent_run_events ADD CONSTRAINT agent_run_events_event_type_check
    CHECK (event_type IN ('message_delta', 'message_completed', 'tool_call', 'approval_required', 'status', 'error', 'terminal', 'schedule_skipped', 'bulk_progress'));
