-- Immutable, workspace-scoped dry-run plans retain only validated metadata and
-- normalized resource payloads. Archive bytes are intentionally not persisted.
CREATE TABLE solution_pack_plans (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('local_archive')),
    source_metadata JSONB NOT NULL CHECK (jsonb_typeof(source_metadata) = 'object'),
    archive_sha256 TEXT NOT NULL CHECK (archive_sha256 ~ '^[0-9a-f]{64}$'),
    manifest_version BIGINT NOT NULL CHECK (manifest_version > 0),
    pack_id TEXT NOT NULL,
    pack_name TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    pack_description TEXT NOT NULL,
    host_api TEXT NOT NULL,
    prefix TEXT NOT NULL,
    blueprint_publication TEXT NOT NULL CHECK (blueprint_publication IN ('draft', 'publish')),
    ready BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > created_at),
    UNIQUE (workspace_id, id)
);

CREATE INDEX solution_pack_plans_workspace_created_at_idx
    ON solution_pack_plans (workspace_id, created_at DESC);
CREATE INDEX solution_pack_plans_expiry_idx
    ON solution_pack_plans (expires_at);

CREATE TABLE solution_pack_plan_mappings (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('blueprint', 'context')),
    logical_key TEXT NOT NULL,
    target_id UUID NOT NULL,
    target_code TEXT NOT NULL,
    target_version BIGINT,
    mapping_kind TEXT NOT NULL CHECK (mapping_kind IN ('create', 'system')),
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    PRIMARY KEY (plan_id, logical_key),
    UNIQUE (plan_id, position),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id)
);

CREATE INDEX solution_pack_plan_mappings_workspace_target_idx
    ON solution_pack_plan_mappings (workspace_id, resource_kind, target_code);

CREATE TABLE solution_pack_plan_actions (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('blueprint', 'context')),
    logical_key TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('create', 'skip', 'conflict', 'blocked')),
    reason_code TEXT NOT NULL CHECK (reason_code ~ '^[a-z][a-z0-9_]*$'),
    summary JSONB NOT NULL CHECK (jsonb_typeof(summary) = 'object'),
    normalized_payload JSONB,
    preconditions JSONB NOT NULL CHECK (jsonb_typeof(preconditions) = 'array'),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, logical_key),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    CHECK (normalized_payload IS NULL OR jsonb_typeof(normalized_payload) = 'object'),
    CHECK ((action = 'create') = (normalized_payload IS NOT NULL))
);

CREATE INDEX solution_pack_plan_actions_workspace_plan_idx
    ON solution_pack_plan_actions (workspace_id, plan_id, position);
