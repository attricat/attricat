-- Solution packs are one-time seeds, so later-release lineage is removed:
-- no plan derives from an earlier application and no release diff is stored.
DROP TABLE solution_pack_plan_release_changes;

ALTER TABLE solution_pack_plans
    DROP COLUMN prior_application_id;

ALTER TABLE solution_pack_applications
    DROP COLUMN prior_application_id,
    DROP COLUMN release_change_snapshot;

-- Only lineage reused an earlier application's sample records.
ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'workspace', 'existing')) NOT VALID;
