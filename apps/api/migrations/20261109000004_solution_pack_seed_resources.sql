-- Seed resources beyond blueprints: prerequisite seeds, contexts and their
-- publication channels, rules, workflows, and saved searches. Plan, apply,
-- and staging behavior stays in repository transactions; this migration only
-- widens the declarative evidence constraints and adds sample-file staging.
ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity', 'prerequisite', 'context', 'publication_channel', 'rule', 'workflow', 'saved_search')) NOT VALID;

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_actions_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity', 'prerequisite', 'context', 'publication_channel', 'rule', 'workflow', 'saved_search')) NOT VALID;

ALTER TABLE solution_pack_application_steps
    DROP CONSTRAINT solution_pack_application_steps_resource_kind_check,
    ADD CONSTRAINT solution_pack_application_steps_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting', 'presentation_asset', 'sample_entity', 'prerequisite', 'context', 'publication_channel', 'rule', 'workflow', 'saved_search')) NOT VALID;

ALTER TABLE solution_pack_plans
    DROP CONSTRAINT solution_pack_plans_resource_evidence_format_check,
    ADD CONSTRAINT solution_pack_plans_resource_evidence_format_check
        CHECK (resource_evidence_format IS NULL OR resource_evidence_format IN (2, 3, 4, 5));

-- Bundled sample files are staged in object storage while planning, under an
-- ordinary upload intent, and become ordinary files when a sample entity step
-- attaches them. The intent's cleanup deadline removes unclaimed objects.
CREATE TABLE solution_pack_plan_sample_files (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    path TEXT NOT NULL CHECK (path ~ '^sample-data/files/'),
    file_id UUID NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL CHECK (filename <> '' AND octet_length(filename) <= 255),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/webp', 'application/pdf', 'text/plain')),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0 AND byte_size <= 8388608),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY (plan_id, path),
    UNIQUE (plan_id, file_id),
    FOREIGN KEY (workspace_id, plan_id) REFERENCES solution_pack_plans (workspace_id, id)
);
