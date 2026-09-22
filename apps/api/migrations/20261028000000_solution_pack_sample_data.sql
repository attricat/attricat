-- Optional solution-pack sample data. All mutable lifecycle behavior is kept in
-- repository transactions; this migration defines only durable declarative state.
ALTER TABLE solution_pack_plans
    DROP CONSTRAINT solution_pack_plans_resource_evidence_format_check,
    ADD CONSTRAINT solution_pack_plans_resource_evidence_format_check
        CHECK (resource_evidence_format IS NULL OR resource_evidence_format IN (2, 3, 4)),
    ADD COLUMN sample_data_selected BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN sample_declaration_sha256 TEXT,
    ADD COLUMN sample_entity_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN sample_automation_warning TEXT,
    ADD CONSTRAINT solution_pack_plans_sample_selection_check CHECK (
        (sample_data_selected
            AND sample_declaration_sha256 ~ '^[0-9a-f]{64}$'
            AND sample_entity_count BETWEEN 1 AND 256
            AND sample_automation_warning IS NOT NULL)
        OR
        (NOT sample_data_selected
            AND sample_declaration_sha256 IS NULL
            AND sample_entity_count = 0
            AND sample_automation_warning IS NULL)
    );

CREATE TABLE solution_pack_sample_dataset_reservations (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    archive_sha256 TEXT NOT NULL CHECK (archive_sha256 ~ '^[0-9a-f]{64}$'),
    sample_declaration_sha256 TEXT NOT NULL CHECK (sample_declaration_sha256 ~ '^[0-9a-f]{64}$'),
    plan_id UUID NOT NULL UNIQUE,
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, pack_id, pack_version, archive_sha256, sample_declaration_sha256),
    FOREIGN KEY (workspace_id, plan_id) REFERENCES solution_pack_plans (workspace_id, id)
);

CREATE TABLE solution_pack_plan_sample_entities (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 255),
    logical_key TEXT NOT NULL CHECK (logical_key ~ '^sample-entities/[a-z][a-z0-9_-]*$'),
    target_id UUID NOT NULL,
    blueprint_logical_key TEXT NOT NULL,
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL CHECK (blueprint_version > 0),
    canonical_declaration_sha256 TEXT NOT NULL CHECK (canonical_declaration_sha256 ~ '^[0-9a-f]{64}$'),
    scalar_count INTEGER NOT NULL CHECK (scalar_count BETWEEN 0 AND 128),
    relationship_target_count INTEGER NOT NULL CHECK (relationship_target_count BETWEEN 0 AND 4096),
    canonical_input JSONB NOT NULL CHECK (jsonb_typeof(canonical_input) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, logical_key),
    UNIQUE (plan_id, target_id),
    FOREIGN KEY (workspace_id, plan_id) REFERENCES solution_pack_plans (workspace_id, id)
);

CREATE INDEX solution_pack_plan_sample_expiry_cleanup_idx
    ON solution_pack_plan_sample_entities (workspace_id, plan_id);

-- Non-value evidence survives staging scrubbing so an explicitly selected later
-- release can classify sample declarations without retaining private inputs.
CREATE TABLE solution_pack_plan_sample_evidence (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 255),
    logical_key TEXT NOT NULL CHECK (logical_key ~ '^sample-entities/[a-z][a-z0-9_-]*$'),
    target_id UUID NOT NULL,
    blueprint_logical_key TEXT NOT NULL,
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL CHECK (blueprint_version > 0),
    canonical_declaration_sha256 TEXT NOT NULL CHECK (canonical_declaration_sha256 ~ '^[0-9a-f]{64}$'),
    scalar_count INTEGER NOT NULL CHECK (scalar_count BETWEEN 0 AND 128),
    relationship_target_count INTEGER NOT NULL CHECK (relationship_target_count BETWEEN 0 AND 4096),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, logical_key),
    UNIQUE (plan_id, target_id),
    FOREIGN KEY (workspace_id, plan_id) REFERENCES solution_pack_plans (workspace_id, id)
);

ALTER TABLE solution_pack_applications
    ADD COLUMN resumable_until TIMESTAMPTZ,
    ADD COLUMN abandoned_at TIMESTAMPTZ,
    DROP CONSTRAINT solution_pack_applications_state_check,
    ADD CONSTRAINT solution_pack_applications_state_check
        CHECK (state IN ('running', 'completed', 'failed', 'invalid', 'abandoned')),
    ADD CONSTRAINT solution_pack_applications_resumability_check CHECK (
        (resumable_until IS NULL AND abandoned_at IS NULL)
        OR (resumable_until > started_at AND (state = 'abandoned') = (abandoned_at IS NOT NULL))
    );

ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity')) NOT VALID,
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'workspace', 'existing', 'reuse')) NOT VALID;

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_actions_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity')) NOT VALID;

ALTER TABLE solution_pack_application_steps
    DROP CONSTRAINT solution_pack_application_steps_resource_kind_check,
    ADD CONSTRAINT solution_pack_application_steps_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity')) NOT VALID;

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
            AND (((logical_key LIKE 'assets/%' OR logical_key LIKE 'sample-entities/%')
                    AND prior_target_version IS NULL)
                OR ((logical_key NOT LIKE 'assets/%' AND logical_key NOT LIKE 'sample-entities/%')
                    AND prior_target_version IS NOT NULL))
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NOT NULL
            AND ((change_kind = 'unchanged'
                    AND current_canonical_definition_sha256 = prior_canonical_definition_sha256)
                OR (change_kind = 'changed'
                    AND (current_canonical_definition_sha256 <> prior_canonical_definition_sha256
                        OR logical_key LIKE 'sample-entities/%'))))
        OR
        (change_kind = 'removed'
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND (((logical_key LIKE 'assets/%' OR logical_key LIKE 'sample-entities/%')
                    AND prior_target_version IS NULL)
                OR ((logical_key NOT LIKE 'assets/%' AND logical_key NOT LIKE 'sample-entities/%')
                    AND prior_target_version IS NOT NULL))
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NULL)
    );
