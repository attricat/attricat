-- Leaf-first relationship sorting scans one workspace/context in native value
-- order across every compatible pinned attribute revision. Entity ID provides
-- the stable leaf-target tie-breaker; reverse relationship indexes then expand
-- each leaf target to source entities.
DROP INDEX attribute_values_workspace_sort_text_idx;
DROP INDEX attribute_values_workspace_sort_number_idx;
DROP INDEX attribute_values_workspace_sort_integer_idx;
DROP INDEX attribute_values_workspace_sort_boolean_idx;
DROP INDEX attribute_values_workspace_sort_date_idx;
DROP INDEX attribute_values_workspace_sort_datetime_idx;
DROP INDEX attribute_values_workspace_sort_time_idx;

CREATE INDEX attribute_values_workspace_sort_text_idx
    ON attribute_values (workspace_id, context_id, value_text, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_text IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_number_idx
    ON attribute_values (workspace_id, context_id, value_number, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_number IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_integer_idx
    ON attribute_values (workspace_id, context_id, value_integer, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_integer IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_boolean_idx
    ON attribute_values (workspace_id, context_id, value_boolean, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_boolean IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_date_idx
    ON attribute_values (workspace_id, context_id, value_date, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_date IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_datetime_idx
    ON attribute_values (workspace_id, context_id, value_datetime, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_datetime IS NOT NULL;
CREATE INDEX attribute_values_workspace_sort_time_idx
    ON attribute_values (workspace_id, context_id, value_time, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_time IS NOT NULL;
