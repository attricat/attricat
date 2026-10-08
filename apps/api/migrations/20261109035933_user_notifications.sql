-- Per-user, per-workspace inbox. Rows are created by application code in the
-- transaction that causes them and are deleted outright by their recipient.
CREATE TABLE user_notifications (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    recipient_user_id UUID NOT NULL REFERENCES users (id),
    kind TEXT NOT NULL CHECK (kind ~ '^[a-z][a-z0-9_.]{0,99}$'),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 300),
    body TEXT CHECK (char_length(body) BETWEEN 1 AND 2000),
    actor_user_id UUID REFERENCES users (id),
    subject_kind TEXT CHECK (subject_kind IN ('entity', 'agent_conversation')),
    subject_id UUID,
    data JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(data) = 'object'),
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((subject_kind IS NULL) = (subject_id IS NULL))
);
CREATE INDEX user_notifications_inbox_idx
    ON user_notifications (workspace_id, recipient_user_id, created_at DESC, id DESC);
CREATE INDEX user_notifications_unread_idx
    ON user_notifications (workspace_id, recipient_user_id)
    WHERE read_at IS NULL;
