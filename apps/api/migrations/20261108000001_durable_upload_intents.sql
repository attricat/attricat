-- Durable ownership of object keys before an upload can reach S3.
CREATE TABLE file_upload_intents (
    object_key TEXT PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'cleaning')),
    cleanup_after TIMESTAMPTZ NOT NULL,
    lease_token UUID,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((state = 'pending' AND lease_token IS NULL) OR (state = 'cleaning' AND lease_token IS NOT NULL))
);
CREATE INDEX file_upload_intents_cleanup_idx ON file_upload_intents (cleanup_after, object_key);

CREATE INDEX attribute_value_history_retention_idx ON attribute_value_history (archived_at, id);
