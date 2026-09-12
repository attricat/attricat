-- Immutable workspace-scoped workflow revisions. Execution state is deliberately not modeled here.
CREATE TABLE workflows (
    id UUID NOT NULL,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published')),
    definition TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    compiled_plan JSONB NOT NULL,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, version),
    UNIQUE (id, definition_hash),
    UNIQUE (workspace_id, id, version)
);
CREATE INDEX workflows_workspace_code_idx ON workflows (workspace_id, code);
CREATE INDEX workflows_workspace_current_idx ON workflows (workspace_id, id, version DESC);

-- Exactly one enabled immutable revision per workflow; activation_event_id is the
-- dispatcher high-water boundary to be populated by a future dispatch slice.
CREATE TABLE workflow_lifecycles (
    workflow_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    enabled_version BIGINT,
    activation_sequence BIGINT,
    enabled_at TIMESTAMPTZ,
    disabled_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, workflow_id, enabled_version)
        REFERENCES workflows(workspace_id, id, version)
);
CREATE INDEX workflow_lifecycles_workspace_enabled_idx ON workflow_lifecycles (workspace_id, enabled_version) WHERE enabled_version IS NOT NULL;
