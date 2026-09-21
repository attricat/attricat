-- Explicit later-release lineage and bounded immutable release-diff evidence.
ALTER TABLE solution_pack_plans
    ADD COLUMN prior_application_id UUID,
    ADD COLUMN resource_evidence_format SMALLINT CHECK (resource_evidence_format IS NULL OR resource_evidence_format = 2),
    ADD CONSTRAINT solution_pack_plans_evidence_format_requires_digest
        CHECK (resource_evidence_format IS NULL OR resource_evidence_sha256 IS NOT NULL),
    ADD CONSTRAINT solution_pack_plans_prior_application_fk
        FOREIGN KEY (workspace_id, prior_application_id)
        REFERENCES solution_pack_applications (workspace_id, id);

CREATE INDEX solution_pack_plans_workspace_prior_application_idx
    ON solution_pack_plans (workspace_id, prior_application_id)
    WHERE prior_application_id IS NOT NULL;

CREATE TABLE solution_pack_plan_release_changes (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    logical_key TEXT NOT NULL,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('added', 'unchanged', 'changed', 'removed')),
    prior_target_id UUID,
    prior_target_code TEXT,
    prior_target_version BIGINT,
    prior_canonical_definition_sha256 TEXT CHECK (prior_canonical_definition_sha256 IS NULL OR prior_canonical_definition_sha256 ~ '^[0-9a-f]{64}$'),
    current_canonical_definition_sha256 TEXT CHECK (current_canonical_definition_sha256 IS NULL OR current_canonical_definition_sha256 ~ '^[0-9a-f]{64}$'),
    reason_code TEXT NOT NULL CHECK (reason_code ~ '^[a-z][a-z0-9_]*$'),
    evidence JSONB NOT NULL CHECK (jsonb_typeof(evidence) = 'object'),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, logical_key),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    CHECK (prior_target_version IS NULL OR prior_target_version > 0),
    CHECK (
        (change_kind = 'added'
            AND prior_target_id IS NULL
            AND prior_target_code IS NULL
            AND prior_target_version IS NULL
            AND prior_canonical_definition_sha256 IS NULL
            AND current_canonical_definition_sha256 IS NOT NULL)
        OR
        (change_kind = 'unchanged'
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND prior_target_version IS NOT NULL
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 = prior_canonical_definition_sha256)
        OR
        (change_kind = 'changed'
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND prior_target_version IS NOT NULL
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 <> prior_canonical_definition_sha256)
        OR
        (change_kind = 'removed'
            AND prior_target_id IS NOT NULL
            AND prior_target_code IS NOT NULL
            AND prior_target_version IS NOT NULL
            AND prior_canonical_definition_sha256 IS NOT NULL
            AND current_canonical_definition_sha256 IS NULL)
    )
);

ALTER TABLE solution_pack_applications
    ADD COLUMN prior_application_id UUID,
    ADD COLUMN release_change_snapshot JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(release_change_snapshot) = 'array'),
    ADD CONSTRAINT solution_pack_applications_prior_application_fk
        FOREIGN KEY (workspace_id, prior_application_id)
        REFERENCES solution_pack_applications (workspace_id, id);

CREATE INDEX solution_pack_applications_workspace_prior_application_idx
    ON solution_pack_applications (workspace_id, prior_application_id)
    WHERE prior_application_id IS NOT NULL;
