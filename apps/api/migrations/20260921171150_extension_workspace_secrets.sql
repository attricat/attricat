-- Workspace-managed extension secrets. Values are never included in release,
-- configuration, operation, lifecycle, or audit snapshots; access is mediated
-- at invocation time by repository code.
CREATE TABLE workspace_extension_secrets (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    name TEXT NOT NULL CHECK (name ~ '^[A-Za-z0-9._-]{1,128}$'),
    value TEXT NOT NULL CHECK (length(value) <= 65536),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, name)
);
CREATE INDEX workspace_extension_secrets_workspace_name_idx
    ON workspace_extension_secrets (workspace_id, name);
