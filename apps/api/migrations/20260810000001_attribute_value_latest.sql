-- Persist the current-history decision so read paths do not need to rank every
-- value event. Event payloads remain historical; only this current-state marker
-- changes when a newer event is written.
ALTER TABLE attribute_values
    ADD COLUMN latest BOOLEAN;

WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY entity_id, attribute_id, context_id, relationship_target_entity_id
            ORDER BY created_at DESC, id DESC
        ) = 1 AS latest
    FROM attribute_values
)
UPDATE attribute_values
SET latest = ranked.latest
FROM ranked
WHERE attribute_values.id = ranked.id;

ALTER TABLE attribute_values
    ALTER COLUMN latest SET NOT NULL,
    ALTER COLUMN latest SET DEFAULT true;

CREATE UNIQUE INDEX attribute_values_latest_scalar_key
    ON attribute_values (entity_id, attribute_id, context_id) NULLS NOT DISTINCT
    WHERE relationship_target_entity_id IS NULL AND latest;

CREATE UNIQUE INDEX attribute_values_latest_relationship_key
    ON attribute_values (entity_id, attribute_id, context_id, relationship_target_entity_id) NULLS NOT DISTINCT
    WHERE relationship_target_entity_id IS NOT NULL AND latest;

CREATE INDEX attribute_values_latest_relationship_target_idx
    ON attribute_values (relationship_target_entity_id, attribute_id, entity_id)
    WHERE relationship_target_entity_id IS NOT NULL AND latest AND active;
