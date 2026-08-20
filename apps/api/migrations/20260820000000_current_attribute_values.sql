-- Keep the hot EAV table to one row per current logical value. Superseded
-- values move to the separately retained history table in the same write
-- transaction.
CREATE TABLE attribute_value_history (
    id UUID NOT NULL,
    entity_id UUID NOT NULL REFERENCES entities (id),
    attribute_id UUID NOT NULL REFERENCES attributes (id),
    context_id UUID REFERENCES attribute_contexts (id),
    relationship_target_entity_id UUID REFERENCES entities (id),
    active BOOLEAN NOT NULL,
    value_text TEXT,
    value_number NUMERIC,
    value_integer BIGINT,
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_time TIME,
    value_time_zone TEXT,
    value_json JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT now()
) PARTITION BY RANGE (archived_at);

CREATE TABLE attribute_value_history_default
    PARTITION OF attribute_value_history DEFAULT;

CREATE INDEX attribute_value_history_entity_archived_idx
    ON attribute_value_history (entity_id, archived_at DESC, id DESC);
CREATE INDEX attribute_value_history_id_idx
    ON attribute_value_history (id);

-- A partition is created before each archive operation. The advisory lock
-- prevents concurrent writers from attempting the same monthly DDL.
CREATE FUNCTION ensure_attribute_value_history_partition(value_archived_at TIMESTAMPTZ)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    partition_start TIMESTAMPTZ := date_trunc('month', value_archived_at);
    partition_end TIMESTAMPTZ := partition_start + INTERVAL '1 month';
    partition_name TEXT := format('attribute_value_history_%s', to_char(partition_start, 'YYYY_MM'));
BEGIN
    PERFORM pg_advisory_xact_lock(847291);
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF attribute_value_history FOR VALUES FROM (%L) TO (%L)',
        partition_name,
        partition_start,
        partition_end
    );
END;
$$;

CREATE FUNCTION purge_attribute_value_history(retention INTERVAL)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    partition RECORD;
    cutoff_month DATE := date_trunc('month', now() - retention)::DATE;
    partition_month DATE;
BEGIN
    FOR partition IN
        SELECT child.relname
        FROM pg_inherits
        JOIN pg_class parent ON parent.oid = pg_inherits.inhparent
        JOIN pg_class child ON child.oid = pg_inherits.inhrelid
        WHERE parent.relname = 'attribute_value_history'
          AND child.relname ~ '^attribute_value_history_[0-9]{4}_[0-9]{2}$'
    LOOP
        partition_month := to_date(substring(partition.relname FROM '[0-9]{4}_[0-9]{2}$'), 'YYYY_MM');
        IF partition_month < cutoff_month THEN
            EXECUTE format('DROP TABLE %I', partition.relname);
        END IF;
    END LOOP;
END;
$$;

SELECT ensure_attribute_value_history_partition(now());

INSERT INTO attribute_value_history (
    id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
    value_time, value_time_zone, value_json, created_at
)
SELECT
    id, entity_id, attribute_id, context_id, relationship_target_entity_id, active,
    value_text, value_number, value_integer, value_boolean, value_date, value_datetime,
    value_time, value_time_zone, value_json, created_at
FROM attribute_values;

DELETE FROM attribute_values;

DROP INDEX attribute_values_latest_scalar_key;
DROP INDEX attribute_values_latest_relationship_key;
DROP INDEX attribute_values_latest_relationship_target_idx;
DROP INDEX attribute_values_latest_number_idx;
DROP INDEX attribute_values_latest_integer_idx;
DROP INDEX attribute_values_latest_date_idx;
DROP INDEX attribute_values_latest_datetime_idx;
DROP INDEX attribute_values_relationship_target_entity_idx;

ALTER TABLE attribute_values
    DROP CONSTRAINT attribute_values_native_value_shape_check,
    DROP COLUMN latest;

ALTER TABLE attribute_values
    ADD CONSTRAINT attribute_values_native_value_shape_check
    CHECK (
        (relationship_target_entity_id IS NOT NULL
            AND active
            AND value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
            AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
            AND value_time IS NULL AND value_time_zone IS NULL AND value_json IS NULL)
        OR
        (relationship_target_entity_id IS NULL AND active AND value_json IS NULL AND (
            (value_text IS NOT NULL)::INT + (value_number IS NOT NULL)::INT
            + (value_integer IS NOT NULL)::INT + (value_boolean IS NOT NULL)::INT
            + (value_date IS NOT NULL)::INT + (value_datetime IS NOT NULL)::INT
            + (value_time IS NOT NULL)::INT = 1
        ) AND ((value_time IS NULL AND value_time_zone IS NULL)
            OR (value_time IS NOT NULL AND value_time_zone IS NOT NULL AND value_time_zone <> '')))
    );

CREATE UNIQUE INDEX attribute_values_scalar_key
    ON attribute_values (entity_id, attribute_id, context_id) NULLS NOT DISTINCT
    WHERE relationship_target_entity_id IS NULL;
CREATE UNIQUE INDEX attribute_values_relationship_key
    ON attribute_values (entity_id, attribute_id, context_id, relationship_target_entity_id) NULLS NOT DISTINCT
    WHERE relationship_target_entity_id IS NOT NULL;
CREATE INDEX attribute_values_relationship_target_entity_idx
    ON attribute_values (relationship_target_entity_id, attribute_id, entity_id)
    WHERE relationship_target_entity_id IS NOT NULL;
CREATE INDEX attribute_values_number_idx
    ON attribute_values (attribute_id, value_number)
    WHERE relationship_target_entity_id IS NULL AND value_number IS NOT NULL;
CREATE INDEX attribute_values_integer_idx
    ON attribute_values (attribute_id, value_integer)
    WHERE relationship_target_entity_id IS NULL AND value_integer IS NOT NULL;
CREATE INDEX attribute_values_date_idx
    ON attribute_values (attribute_id, value_date)
    WHERE relationship_target_entity_id IS NULL AND value_date IS NOT NULL;
CREATE INDEX attribute_values_datetime_idx
    ON attribute_values (attribute_id, value_datetime)
    WHERE relationship_target_entity_id IS NULL AND value_datetime IS NOT NULL;
