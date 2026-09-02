ALTER TABLE agent_tool_calls DROP CONSTRAINT agent_tool_calls_check;
ALTER TABLE agent_tool_calls ADD CONSTRAINT agent_tool_calls_decision_consistency_check
    CHECK (decided_by_user_id IS NULL OR decided_at IS NOT NULL);
