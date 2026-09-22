-- Explore navigation is a bounded workspace-setting application step. It uses
-- the existing immutable plan/application evidence without introducing generic
-- workspace-settings mutation semantics.
ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'context', 'workspace_setting')),
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'system', 'workspace'));

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_actions_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'context', 'workspace_setting')),
    DROP CONSTRAINT solution_pack_plan_actions_action_check,
    ADD CONSTRAINT solution_pack_plan_actions_action_check
        CHECK (action IN ('create', 'append', 'satisfied', 'skip', 'conflict', 'blocked')),
    DROP CONSTRAINT solution_pack_plan_actions_check,
    ADD CONSTRAINT solution_pack_plan_actions_payload_check
        CHECK ((action IN ('create', 'append', 'satisfied')) = (normalized_payload IS NOT NULL));

ALTER TABLE solution_pack_application_steps
    DROP CONSTRAINT solution_pack_application_steps_resource_kind_check,
    ADD CONSTRAINT solution_pack_application_steps_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'context', 'workspace_setting'));
