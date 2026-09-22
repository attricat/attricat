-- Extension policy compatibility is durable requirement evidence. Workspace
-- extension layout uses the existing bounded workspace_setting action kind.
ALTER TABLE solution_pack_plan_extension_requirements
    DROP CONSTRAINT solution_pack_plan_extension_requirements_reason_code_check,
    ADD CONSTRAINT solution_pack_plan_extension_requirements_reason_code_check
        CHECK (reason_code IN (
            'satisfied',
            'missing',
            'incompatible_version',
            'quarantined',
            'policy_incompatible',
            'configuration_mismatch'
        ));
