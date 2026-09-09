-- Versioned one-to-one cardinality is enforced by repository transactions, not
-- a uniqueness constraint, so historical revisions remain representable.
CREATE INDEX attribute_values_active_relationship_source_target_idx
    ON attribute_values (entity_id, attribute_id, context_id, relationship_target_entity_id)
    WHERE active AND relationship_target_entity_id IS NOT NULL;

CREATE INDEX attribute_values_active_relationship_target_source_idx
    ON attribute_values (attribute_id, context_id, relationship_target_entity_id, entity_id)
    WHERE active AND relationship_target_entity_id IS NOT NULL;
