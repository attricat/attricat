-- Pinned declarative extension type metadata belongs to the immutable compiled
-- attribute revision. Repository code resolves, validates, and writes it.
ALTER TABLE attributes
    ADD COLUMN extension_type JSONB;

ALTER TABLE attribute_values
    DROP CONSTRAINT attribute_values_native_value_shape_check;

ALTER TABLE attribute_values
    ADD CONSTRAINT attribute_values_native_value_shape_check
    CHECK (
        (relationship_target_entity_id IS NOT NULL
            AND value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
            AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
            AND value_time IS NULL AND value_time_zone IS NULL AND value_json IS NULL)
        OR
        (relationship_target_entity_id IS NULL AND (
            (value_text IS NOT NULL)::INT + (value_number IS NOT NULL)::INT
            + (value_integer IS NOT NULL)::INT + (value_boolean IS NOT NULL)::INT
            + (value_date IS NOT NULL)::INT + (value_datetime IS NOT NULL)::INT
            + (value_time IS NOT NULL)::INT + (value_json IS NOT NULL)::INT = 1
            OR (value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
                AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
                AND value_time IS NULL AND value_time_zone IS NULL AND value_json IS NULL)
        ) AND ((value_time IS NULL AND value_time_zone IS NULL)
            OR (value_time IS NOT NULL AND value_time_zone IS NOT NULL AND value_time_zone <> '')))
    );
