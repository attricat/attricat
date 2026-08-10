ALTER TABLE attributes
    ADD COLUMN target_blueprint_code TEXT;

ALTER TABLE attribute_values
    ADD COLUMN active BOOLEAN NOT NULL DEFAULT true;

CREATE INDEX attribute_values_relationship_current_idx
    ON attribute_values (
        entity_id,
        attribute_id,
        context_id,
        relationship_target_entity_id,
        created_at DESC,
        id DESC
    )
    WHERE relationship_target_entity_id IS NOT NULL;
