-- A record conversation can apply its record's edits and comments without
-- asking, for runs started by the person who switched that on.
ALTER TABLE conversations
    ADD COLUMN auto_approve_user_id UUID REFERENCES users (id),
    ADD COLUMN auto_approve_enabled_at TIMESTAMPTZ,
    ADD CONSTRAINT conversations_auto_approval_check CHECK (
        (auto_approve_user_id IS NULL) = (auto_approve_enabled_at IS NULL)
        AND (auto_approve_user_id IS NULL OR record_id IS NOT NULL)
    );

-- A call approved by that switch rather than by a person deciding it.
ALTER TABLE agent_tool_calls
    ADD COLUMN auto_approved BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT agent_tool_calls_auto_approved_check CHECK (
        NOT auto_approved OR (state <> 'pending_approval' AND decided_by_user_id IS NOT NULL)
    );
