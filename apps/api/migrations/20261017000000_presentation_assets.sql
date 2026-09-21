-- Immutable workspace presentation assets. Object keys remain private provider
-- identifiers and are never used as API or solution-pack logical identifiers.
CREATE TABLE presentation_assets (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    purpose TEXT NOT NULL CHECK (purpose IN ('logo', 'icon', 'illustration')),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/webp', 'image/svg+xml')),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0 AND byte_size <= 2097152),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    width INTEGER CHECK (width IS NULL OR (width > 0 AND width <= 4096)),
    height INTEGER CHECK (height IS NULL OR (height > 0 AND height <= 4096)),
    object_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, id),
    UNIQUE (object_key),
    CHECK (
        (media_type = 'image/svg+xml' AND width IS NULL AND height IS NULL)
        OR (media_type <> 'image/svg+xml' AND width IS NOT NULL AND height IS NOT NULL)
    ),
    CHECK (media_type <> 'image/jpeg' OR purpose = 'illustration'),
    CHECK (width IS NULL OR height IS NULL OR width::BIGINT * height::BIGINT <= 16000000)
);

CREATE INDEX presentation_assets_workspace_created_idx
    ON presentation_assets (workspace_id, created_at DESC, id DESC);

ALTER TABLE solution_pack_plans
    DROP CONSTRAINT solution_pack_plans_resource_evidence_format_check,
    ADD CONSTRAINT solution_pack_plans_resource_evidence_format_check
        CHECK (resource_evidence_format IS NULL OR resource_evidence_format IN (2, 3));

ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset')) NOT VALID,
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'workspace', 'existing')) NOT VALID;

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_actions_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset')) NOT VALID;

ALTER TABLE solution_pack_application_steps
    DROP CONSTRAINT solution_pack_application_steps_resource_kind_check,
    ADD CONSTRAINT solution_pack_application_steps_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset')) NOT VALID;

ALTER TABLE solution_pack_plan_release_changes
    DROP CONSTRAINT solution_pack_plan_release_changes_check,
    ADD CONSTRAINT solution_pack_plan_release_changes_check CHECK (
        (change_kind = 'added'
            AND prior_target_id IS NULL
            AND prior_target_code IS NULL
            AND prior_target_version IS NULL
            AND prior_canonical_definition_sha256 IS NULL
            AND current_canonical_definition_sha256 IS NOT NULL)
        OR
        (change_kind IN ('unchanged', 'changed')
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND ((logical_key LIKE 'assets/%' AND prior_target_version IS NULL)
                OR (logical_key NOT LIKE 'assets/%' AND prior_target_version IS NOT NULL))
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NOT NULL
            AND ((change_kind = 'unchanged' AND current_canonical_definition_sha256 = prior_canonical_definition_sha256)
                OR (change_kind = 'changed' AND current_canonical_definition_sha256 <> prior_canonical_definition_sha256)))
        OR
        (change_kind = 'removed'
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND ((logical_key LIKE 'assets/%' AND prior_target_version IS NULL)
                OR (logical_key NOT LIKE 'assets/%' AND prior_target_version IS NOT NULL))
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NULL)
    );

CREATE TABLE solution_pack_plan_asset_objects (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    logical_key TEXT NOT NULL,
    target_id UUID NOT NULL,
    object_key TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/webp', 'image/svg+xml')),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0 AND byte_size <= 2097152),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    state TEXT NOT NULL CHECK (state IN ('uploading', 'staged', 'claimed', 'cleanup_pending', 'cleaned')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_id, logical_key),
    UNIQUE (object_key),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id)
);

CREATE INDEX solution_pack_plan_asset_cleanup_idx
    ON solution_pack_plan_asset_objects (state, updated_at);
