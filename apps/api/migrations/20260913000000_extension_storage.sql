-- Durable workspace-scoped state for enabled extensions. Authorization and quotas
-- are enforced in repository code; stored state is never configuration or secrets.
CREATE TABLE extension_storage_entries (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    key TEXT NOT NULL CHECK (octet_length(key) > 0 AND octet_length(key) <= 256),
    value JSONB NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0 AND byte_size <= 65536),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (workspace_id, extension_id, key)
);
CREATE INDEX extension_storage_entries_workspace_extension_updated_idx
    ON extension_storage_entries (workspace_id, extension_id, updated_at DESC);
