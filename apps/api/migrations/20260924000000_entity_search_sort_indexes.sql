-- Ordered scans for current, default-context native scalar table values.
CREATE INDEX attribute_values_current_sort_text_idx
    ON attribute_values (attribute_id, value_text, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_text IS NOT NULL;
CREATE INDEX attribute_values_current_sort_number_idx
    ON attribute_values (attribute_id, value_number, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_number IS NOT NULL;
CREATE INDEX attribute_values_current_sort_integer_idx
    ON attribute_values (attribute_id, value_integer, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_integer IS NOT NULL;
CREATE INDEX attribute_values_current_sort_boolean_idx
    ON attribute_values (attribute_id, value_boolean, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_boolean IS NOT NULL;
CREATE INDEX attribute_values_current_sort_date_idx
    ON attribute_values (attribute_id, value_date, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_date IS NOT NULL;
CREATE INDEX attribute_values_current_sort_datetime_idx
    ON attribute_values (attribute_id, value_datetime, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_datetime IS NOT NULL;
CREATE INDEX attribute_values_current_sort_time_idx
    ON attribute_values (attribute_id, value_time, entity_id)
    WHERE active AND relationship_target_entity_id IS NULL AND value_time IS NOT NULL;
