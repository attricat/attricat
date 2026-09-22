-- Durable solution-pack application evidence. Application identity is unique per
-- immutable plan; each ordered step is committed with its catalog mutation.
CREATE TABLE solution_pack_applications (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    plan_id UUID NOT NULL,
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('local_archive')),
    source_metadata JSONB NOT NULL CHECK (jsonb_typeof(source_metadata) = 'object'),
    archive_sha256 TEXT NOT NULL CHECK (archive_sha256 ~ '^[0-9a-f]{64}$'),
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    blueprint_publication TEXT NOT NULL CHECK (blueprint_publication IN ('draft', 'publish')),
    state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'failed', 'invalid')),
    diagnostic_code TEXT CHECK (diagnostic_code IS NULL OR diagnostic_code ~ '^[a-z][a-z0-9_]*$'),
    diagnostic_message TEXT CHECK (diagnostic_message IS NULL OR octet_length(diagnostic_message) <= 4096),
    mapping_snapshot JSONB NOT NULL CHECK (jsonb_typeof(mapping_snapshot) = 'array'),
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ,
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    UNIQUE (plan_id),
    UNIQUE (workspace_id, id),
    UNIQUE (id, plan_id),
    CHECK ((state = 'completed') = (completed_at IS NOT NULL)),
    CHECK ((diagnostic_code IS NULL) = (diagnostic_message IS NULL))
);

CREATE INDEX solution_pack_applications_workspace_started_at_idx
    ON solution_pack_applications (workspace_id, started_at DESC, id DESC);

CREATE TABLE solution_pack_application_steps (
    application_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    plan_id UUID NOT NULL,
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('blueprint', 'context')),
    logical_key TEXT NOT NULL,
    target_id UUID NOT NULL,
    target_code TEXT NOT NULL,
    target_version BIGINT,
    state TEXT NOT NULL CHECK (state IN ('pending', 'completed', 'failed')),
    diagnostic_code TEXT CHECK (diagnostic_code IS NULL OR diagnostic_code ~ '^[a-z][a-z0-9_]*$'),
    diagnostic_message TEXT CHECK (diagnostic_message IS NULL OR octet_length(diagnostic_message) <= 4096),
    result_snapshot JSONB CHECK (result_snapshot IS NULL OR jsonb_typeof(result_snapshot) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    completed_at TIMESTAMPTZ,
    PRIMARY KEY (application_id, position),
    UNIQUE (application_id, logical_key),
    FOREIGN KEY (workspace_id, application_id)
        REFERENCES solution_pack_applications (workspace_id, id),
    FOREIGN KEY (application_id, plan_id)
        REFERENCES solution_pack_applications (id, plan_id),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    FOREIGN KEY (plan_id, position)
        REFERENCES solution_pack_plan_actions (plan_id, position),
    CHECK ((state = 'completed') = (completed_at IS NOT NULL)),
    CHECK ((state = 'completed') = (result_snapshot IS NOT NULL)),
    CHECK ((diagnostic_code IS NULL) = (diagnostic_message IS NULL)),
    CHECK ((state = 'failed') = (diagnostic_code IS NOT NULL))
);

CREATE INDEX solution_pack_application_steps_workspace_application_idx
    ON solution_pack_application_steps (workspace_id, application_id, position);
