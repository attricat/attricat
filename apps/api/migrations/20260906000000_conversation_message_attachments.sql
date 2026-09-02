CREATE TABLE conversation_message_attachments (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    message_id UUID NOT NULL REFERENCES conversation_messages(id) ON DELETE CASCADE,
    file_id UUID NOT NULL REFERENCES files(id),
    position INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (message_id, position),
    UNIQUE (message_id, file_id)
);

CREATE INDEX conversation_message_attachments_file_idx
    ON conversation_message_attachments (workspace_id, file_id);
