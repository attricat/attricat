-- Workspace-owned custom extension registry configuration. Discovery results and
-- archives deliberately remain external and are never persisted as a catalogue.
CREATE TABLE extension_registry_sources (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    kind TEXT NOT NULL CHECK (kind IN ('github_repository')),
    owner TEXT NOT NULL CHECK (owner ~ '^[A-Za-z0-9][A-Za-z0-9-]{0,38}$'),
    repository TEXT NOT NULL CHECK (repository ~ '^[A-Za-z0-9_.-]{1,100}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, kind, owner, repository)
);
CREATE INDEX extension_registry_sources_workspace_idx
    ON extension_registry_sources (workspace_id, created_at, id);
