-- Workspace-scoped reusable attribute registry. Revisions are immutable once
-- published; entity attachments pin the selected published revision.
CREATE TABLE reusable_attribute_definitions (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    namespace TEXT NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (workspace_id, namespace, code)
);

CREATE TABLE reusable_attribute_revisions (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    definition_id UUID NOT NULL REFERENCES reusable_attribute_definitions (id),
    version BIGINT NOT NULL CHECK (version > 0),
    value_type TEXT NOT NULL,
    value_schema JSONB,
    default_value JSONB,
    file_policy JSONB,
    target_blueprint_code TEXT,
    cardinality TEXT,
    target_cardinality TEXT,
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    context_fallback TEXT NOT NULL DEFAULT 'default',
    context_editable TEXT NOT NULL DEFAULT 'all',
    readonly BOOLEAN NOT NULL DEFAULT false,
    searchable BOOLEAN NOT NULL DEFAULT false,
    facetable BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL CHECK (status IN ('draft', 'published')),
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, definition_id, version)
);
CREATE INDEX reusable_attribute_revisions_current_idx
    ON reusable_attribute_revisions (workspace_id, definition_id, version DESC);

CREATE TABLE reusable_attribute_groups (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    position BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, code)
);
CREATE TABLE reusable_attribute_group_members (
    group_id UUID NOT NULL REFERENCES reusable_attribute_groups (id) ON DELETE CASCADE,
    reusable_attribute_revision_id UUID NOT NULL REFERENCES reusable_attribute_revisions (id),
    position BIGINT NOT NULL,
    PRIMARY KEY (group_id, reusable_attribute_revision_id),
    UNIQUE (group_id, position)
);

CREATE TABLE entity_reusable_attribute_attachments (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    reusable_attribute_definition_id UUID NOT NULL REFERENCES reusable_attribute_definitions (id),
    reusable_attribute_revision_id UUID NOT NULL REFERENCES reusable_attribute_revisions (id),
    position BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, entity_id, reusable_attribute_definition_id),
    UNIQUE (workspace_id, entity_id, reusable_attribute_revision_id),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id)
);

-- Existing EAV values continue to own all typed storage. An attached reusable
-- revision materializes as an entity-owned attribute row, rather than a second
-- custom-value subsystem.
ALTER TABLE attributes DROP CONSTRAINT attributes_blueprint_version_fkey;
ALTER TABLE attributes DROP CONSTRAINT attributes_workspace_blueprint_fkey;
ALTER TABLE attributes DROP CONSTRAINT attributes_blueprint_version_code_key;
ALTER TABLE attributes ALTER COLUMN blueprint_id DROP NOT NULL;
ALTER TABLE attributes ALTER COLUMN blueprint_version DROP NOT NULL;
ALTER TABLE attributes ADD COLUMN entity_id UUID;
ALTER TABLE attributes ADD COLUMN reusable_attribute_revision_id UUID;
ALTER TABLE attributes ADD CONSTRAINT attributes_workspace_blueprint_fkey
    FOREIGN KEY (workspace_id, blueprint_id, blueprint_version)
    REFERENCES blueprints (workspace_id, id, version);
ALTER TABLE attributes ADD CONSTRAINT attributes_workspace_entity_fkey
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE attributes ADD CONSTRAINT attributes_reusable_revision_fkey
    FOREIGN KEY (reusable_attribute_revision_id) REFERENCES reusable_attribute_revisions (id);
ALTER TABLE attributes ADD CONSTRAINT attributes_owner_check CHECK (
    (blueprint_id IS NOT NULL AND blueprint_version IS NOT NULL AND entity_id IS NULL AND reusable_attribute_revision_id IS NULL)
    OR (blueprint_id IS NULL AND blueprint_version IS NULL AND entity_id IS NOT NULL AND reusable_attribute_revision_id IS NOT NULL)
);
CREATE UNIQUE INDEX attributes_workspace_blueprint_version_code_key
    ON attributes (workspace_id, blueprint_id, blueprint_version, code)
    WHERE blueprint_id IS NOT NULL;
CREATE UNIQUE INDEX attributes_workspace_entity_reusable_revision_key
    ON attributes (workspace_id, entity_id, reusable_attribute_revision_id)
    WHERE entity_id IS NOT NULL;
CREATE INDEX attributes_workspace_entity_idx ON attributes (workspace_id, entity_id)
    WHERE entity_id IS NOT NULL;
