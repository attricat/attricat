-- Explicit reuse pins one existing published blueprint revision. Historical
-- context rows remain intentionally unvalidated after the v1 resource narrowing.
ALTER TABLE solution_pack_plan_mappings
    DROP CONSTRAINT solution_pack_plan_mappings_mapping_kind_check,
    ADD CONSTRAINT solution_pack_plan_mappings_mapping_kind_check
        CHECK (mapping_kind IN ('create', 'workspace', 'existing')) NOT VALID;

ALTER TABLE solution_pack_plan_actions
    DROP CONSTRAINT solution_pack_plan_actions_action_check,
    ADD CONSTRAINT solution_pack_plan_actions_action_check
        CHECK (action IN ('create', 'append', 'satisfied', 'map', 'skip', 'conflict', 'blocked')) NOT VALID,
    DROP CONSTRAINT solution_pack_plan_actions_payload_check,
    ADD CONSTRAINT solution_pack_plan_actions_payload_check
        CHECK ((action IN ('create', 'append', 'satisfied')) = (normalized_payload IS NOT NULL)) NOT VALID;
