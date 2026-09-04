-- Installed extension snapshots and workspace-owned lifecycle state. These tables
-- retain only releases selected for installation, never a registry catalogue.
-- Runtime and registry transport are deliberately application concerns, not
-- database logic.
CREATE TABLE installed_extension_releases (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    version TEXT NOT NULL,
    manifest JSONB NOT NULL,
    manifest_sha256 TEXT NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'),
    source TEXT NOT NULL CHECK (length(trim(source)) > 0 AND length(source) <= 512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, extension_id, version)
);
CREATE INDEX installed_extension_releases_workspace_extension_version_idx
    ON installed_extension_releases (workspace_id, extension_id, version);

CREATE TABLE installed_extension_release_artifacts (
    id UUID PRIMARY KEY,
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases (id),
    artifact_id TEXT NOT NULL CHECK (artifact_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    artifact_kind TEXT NOT NULL CHECK (artifact_kind IN ('server_wasm', 'client_component')),
    artifact_path TEXT NOT NULL,
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    UNIQUE (installed_release_id, artifact_id),
    UNIQUE (installed_release_id, artifact_path)
);

CREATE TABLE extension_installations (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases (id),
    state TEXT NOT NULL CHECK (state IN ('disabled', 'enabled', 'quarantined')),
    configuration JSONB NOT NULL DEFAULT '{}'::jsonb,
    configuration_version INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, extension_id)
);
CREATE INDEX extension_installations_workspace_state_idx ON extension_installations (workspace_id, state);

CREATE TABLE extension_grants (
    id UUID PRIMARY KEY,
    installation_id UUID NOT NULL REFERENCES extension_installations (id) ON DELETE CASCADE,
    grant_kind TEXT NOT NULL CHECK (grant_kind IN ('capability', 'host_permission')),
    grant_id TEXT NOT NULL CHECK (grant_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (installation_id, grant_kind, grant_id)
);

CREATE TABLE extension_lifecycle_records (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    installation_id UUID,
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    installed_release_id UUID REFERENCES installed_extension_releases (id),
    operation TEXT NOT NULL CHECK (operation IN ('install', 'configure', 'grant', 'revoke', 'enable', 'disable', 'upgrade', 'quarantine', 'remove')),
    prior_state TEXT CHECK (prior_state IN ('disabled', 'enabled', 'quarantined')),
    new_state TEXT CHECK (new_state IN ('disabled', 'enabled', 'quarantined')),
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure')),
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    source TEXT,
    diagnostics JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX extension_lifecycle_records_workspace_created_at_idx ON extension_lifecycle_records (workspace_id, created_at DESC);
CREATE INDEX extension_lifecycle_records_installation_created_at_idx ON extension_lifecycle_records (installation_id, created_at DESC);
