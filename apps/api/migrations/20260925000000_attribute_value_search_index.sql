-- Accelerate substring and prefix search over active scalar string values.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX attribute_values_active_search_text_trgm_idx
    ON attribute_values USING GIN (value_text gin_trgm_ops)
    WHERE active AND relationship_target_entity_id IS NULL AND value_text IS NOT NULL;
