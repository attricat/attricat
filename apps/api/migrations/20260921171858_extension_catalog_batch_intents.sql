CREATE TABLE extension_catalog_batch_intents (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    extension_id TEXT NOT NULL,
    batch_key TEXT NOT NULL,
    intent_key TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    outcome JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (workspace_id, extension_id, batch_key, intent_key),
    CHECK (length(extension_id) BETWEEN 1 AND 128),
    CHECK (length(batch_key) BETWEEN 1 AND 256),
    CHECK (length(intent_key) BETWEEN 1 AND 128),
    CHECK (length(input_hash) = 64)
);
