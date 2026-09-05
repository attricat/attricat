-- Extension-owned configuration is deliberately separate from blueprint definitions.
-- Repository code validates scope ownership, manifest schema, grants, and lifecycle state.
CREATE TABLE extension_scoped_configuration (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases (id),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('blueprint', 'attribute')),
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL CHECK (blueprint_version > 0),
    attribute_id UUID,
    configuration JSONB NOT NULL,
    configuration_version INTEGER NOT NULL CHECK (configuration_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (workspace_id, extension_id, scope_kind, blueprint_id, blueprint_version, attribute_id),
    CHECK ((scope_kind = 'blueprint' AND attribute_id IS NULL) OR (scope_kind = 'attribute' AND attribute_id IS NOT NULL))
);
CREATE INDEX extension_scoped_configuration_workspace_extension_idx
    ON extension_scoped_configuration (workspace_id, extension_id, installed_release_id);
