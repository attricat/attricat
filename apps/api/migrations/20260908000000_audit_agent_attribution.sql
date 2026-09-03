-- Explicit provenance for mutations executed by the catalogue agent. These links
-- are normalized so audit readers never have to infer an agent executor from
-- token labels or parse JSON metadata.
ALTER TABLE audit_events
    ADD COLUMN executor_type TEXT NOT NULL DEFAULT 'human'
        CHECK (executor_type IN ('human', 'agent')),
    ADD COLUMN agent_run_id UUID REFERENCES agent_runs (id),
    ADD COLUMN agent_conversation_id UUID REFERENCES conversations (id),
    ADD COLUMN agent_tool_call_id UUID REFERENCES agent_tool_calls (id),
    ADD COLUMN agent_tool_name TEXT,
    ADD COLUMN approval_decision TEXT,
    ADD COLUMN approved_by_user_id UUID REFERENCES users (id),
    ADD CONSTRAINT audit_events_agent_attribution_check CHECK (
        (executor_type = 'human'
            AND agent_run_id IS NULL
            AND agent_conversation_id IS NULL
            AND agent_tool_call_id IS NULL
            AND agent_tool_name IS NULL
            AND approval_decision IS NULL
            AND approved_by_user_id IS NULL)
        OR
        (executor_type = 'agent'
            AND agent_run_id IS NOT NULL
            AND agent_conversation_id IS NOT NULL
            AND agent_tool_call_id IS NOT NULL
            AND length(btrim(agent_tool_name)) > 0
            AND ((approval_decision IS NULL AND approved_by_user_id IS NULL)
                OR (approval_decision = 'approved' AND approved_by_user_id IS NOT NULL)))
    );
CREATE INDEX audit_events_agent_run_occurred_at_idx ON audit_events (agent_run_id, occurred_at DESC)
    WHERE agent_run_id IS NOT NULL;
CREATE INDEX audit_events_agent_tool_call_idx ON audit_events (agent_tool_call_id)
    WHERE agent_tool_call_id IS NOT NULL;
