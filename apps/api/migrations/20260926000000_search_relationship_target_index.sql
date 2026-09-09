-- Reverse relationship traversal starts from matching target entities.
CREATE INDEX attribute_values_active_relationship_target_search_idx
    ON attribute_values (relationship_target_entity_id, entity_id, attribute_id)
    WHERE active AND relationship_target_entity_id IS NOT NULL;
