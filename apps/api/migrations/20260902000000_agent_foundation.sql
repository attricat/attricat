-- Durable, provider-neutral state for conversational agents. Provider credentials
-- are process configuration and intentionally have no database representation.
CREATE TABLE conversations (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    created_by_user_id UUID REFERENCES users (id),
    title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);
CREATE INDEX conversations_workspace_created_idx ON conversations (workspace_id, created_at DESC);

CREATE TABLE agent_schedules (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    cron_expression TEXT NOT NULL CHECK (length(btrim(cron_expression)) > 0 AND length(cron_expression) <= 256),
    timezone TEXT NOT NULL DEFAULT 'UTC' CHECK (timezone = 'UTC'),
    enabled BOOLEAN NOT NULL DEFAULT true,
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX agent_schedules_due_idx ON agent_schedules (next_run_at, id) WHERE enabled;

CREATE TABLE agent_runs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    schedule_id UUID REFERENCES agent_schedules (id) ON DELETE SET NULL,
    origin TEXT NOT NULL CHECK (origin IN ('interactive', 'scheduled', 'manual')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'awaiting_approval', 'completed', 'failed', 'cancelled', 'skipped')),
    provider_base_url TEXT NOT NULL CHECK (length(provider_base_url) <= 2048),
    model TEXT NOT NULL CHECK (length(model) > 0 AND length(model) <= 512),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_code TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((status IN ('completed', 'failed', 'cancelled', 'skipped')) = (finished_at IS NOT NULL))
);
CREATE INDEX agent_runs_conversation_created_idx ON agent_runs (conversation_id, created_at);
CREATE INDEX agent_runs_schedule_created_idx ON agent_runs (schedule_id, created_at DESC) WHERE schedule_id IS NOT NULL;
-- A scheduled conversation cannot execute a second run while the prior one is
-- executing or waiting for a human decision.
CREATE UNIQUE INDEX agent_runs_one_active_schedule_idx ON agent_runs (schedule_id)
    WHERE schedule_id IS NOT NULL AND status IN ('queued', 'running', 'awaiting_approval');

CREATE TABLE conversation_messages (
    id UUID PRIMARY KEY,
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    run_id UUID REFERENCES agent_runs (id) ON DELETE SET NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (conversation_id, sequence)
);
CREATE INDEX conversation_messages_conversation_sequence_idx ON conversation_messages (conversation_id, sequence);

CREATE TABLE agent_tool_calls (
    id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES agent_runs (id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    provider_call_id TEXT,
    tool_name TEXT NOT NULL CHECK (length(btrim(tool_name)) > 0 AND length(tool_name) <= 256),
    arguments JSONB NOT NULL,
    change_summary TEXT,
    result JSONB,
    error JSONB,
    state TEXT NOT NULL CHECK (state IN ('pending_approval', 'approved', 'rejected', 'completed', 'failed')),
    decided_by_user_id UUID REFERENCES users (id),
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    UNIQUE (run_id, sequence),
    CHECK ((state IN ('approved', 'rejected')) = (decided_at IS NOT NULL)),
    CHECK (NOT (result IS NOT NULL AND error IS NOT NULL))
);
CREATE INDEX agent_tool_calls_pending_idx ON agent_tool_calls (run_id, sequence) WHERE state = 'pending_approval';

CREATE TABLE agent_run_events (
    id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES agent_runs (id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    event_type TEXT NOT NULL CHECK (event_type IN ('message_delta', 'message_completed', 'tool_call', 'approval_required', 'status', 'error', 'terminal', 'schedule_skipped')),
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (run_id, sequence)
);
CREATE INDEX agent_run_events_run_sequence_idx ON agent_run_events (run_id, sequence);
