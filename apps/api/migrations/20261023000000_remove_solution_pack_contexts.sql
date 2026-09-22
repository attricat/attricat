-- Contexts are workspace operating structure and are not solution-pack resources.
-- Narrow the persisted plan/application evidence to the resource kinds and
-- mapping modes accepted by the solution-pack runtime.
ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting')) NOT VALID,
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'workspace')) NOT VALID;

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_resource_kind_check,
    ADD CONSTRAINT solution_pack_plan_actions_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting')) NOT VALID;

ALTER TABLE solution_pack_application_steps
    DROP CONSTRAINT solution_pack_application_steps_resource_kind_check,
    ADD CONSTRAINT solution_pack_application_steps_resource_kind_check
        CHECK (resource_kind IN ('blueprint', 'workspace_setting')) NOT VALID;
