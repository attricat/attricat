ALTER TABLE attribute_values
    ADD COLUMN value_text TEXT,
    ADD COLUMN value_number NUMERIC,
    ADD COLUMN value_integer BIGINT,
    ADD COLUMN value_boolean BOOLEAN,
    ADD COLUMN value_date DATE,
    ADD COLUMN value_datetime TIMESTAMPTZ,
    ADD COLUMN value_time TIME,
    ADD COLUMN value_time_zone TEXT;

UPDATE attribute_values av
SET value_text = av.value #>> '{}'
FROM attributes a
WHERE a.id = av.attribute_id
  AND a.value_type = 'string'
  AND av.relationship_target_entity_id IS NULL;

UPDATE attribute_values av
SET value_number = (av.value #>> '{}')::NUMERIC
FROM attributes a
WHERE a.id = av.attribute_id
  AND a.value_type = 'number'
  AND av.relationship_target_entity_id IS NULL;

UPDATE attribute_values av
SET value_integer = (av.value #>> '{}')::BIGINT
FROM attributes a
WHERE a.id = av.attribute_id
  AND a.value_type = 'integer'
  AND av.relationship_target_entity_id IS NULL;

UPDATE attribute_values av
SET value_boolean = (av.value #>> '{}')::BOOLEAN
FROM attributes a
WHERE a.id = av.attribute_id
  AND a.value_type = 'boolean'
  AND av.relationship_target_entity_id IS NULL;

ALTER TABLE attribute_values DROP COLUMN value;

ALTER TABLE attribute_values
    ADD CONSTRAINT attribute_values_native_value_shape_check
    CHECK (
        (relationship_target_entity_id IS NOT NULL
            AND value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
            AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
            AND value_time IS NULL AND value_time_zone IS NULL)
        OR
        (relationship_target_entity_id IS NULL AND (
            (value_text IS NOT NULL)::INT + (value_number IS NOT NULL)::INT
            + (value_integer IS NOT NULL)::INT + (value_boolean IS NOT NULL)::INT
            + (value_date IS NOT NULL)::INT + (value_datetime IS NOT NULL)::INT
            + (value_time IS NOT NULL)::INT = 1
        ) AND ((value_time IS NULL AND value_time_zone IS NULL)
            OR (value_time IS NOT NULL AND value_time_zone IS NOT NULL AND value_time_zone <> '')))
    );

CREATE INDEX attribute_values_latest_number_idx
    ON attribute_values (attribute_id, value_number)
    WHERE latest AND relationship_target_entity_id IS NULL AND value_number IS NOT NULL;
CREATE INDEX attribute_values_latest_integer_idx
    ON attribute_values (attribute_id, value_integer)
    WHERE latest AND relationship_target_entity_id IS NULL AND value_integer IS NOT NULL;
CREATE INDEX attribute_values_latest_date_idx
    ON attribute_values (attribute_id, value_date)
    WHERE latest AND relationship_target_entity_id IS NULL AND value_date IS NOT NULL;
CREATE INDEX attribute_values_latest_datetime_idx
    ON attribute_values (attribute_id, value_datetime)
    WHERE latest AND relationship_target_entity_id IS NULL AND value_datetime IS NOT NULL;
