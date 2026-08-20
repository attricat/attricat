CREATE OR REPLACE FUNCTION ensure_attribute_value_history_partition(value_archived_at TIMESTAMPTZ)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    partition_start TIMESTAMPTZ := date_trunc('month', value_archived_at);
    partition_end TIMESTAMPTZ := partition_start + INTERVAL '1 month';
    partition_name TEXT := format('attribute_value_history_%s', to_char(partition_start, 'YYYY_MM'));
BEGIN
    IF to_regclass(partition_name) IS NOT NULL THEN
        RETURN;
    END IF;

    PERFORM pg_advisory_xact_lock(847291);
    IF to_regclass(partition_name) IS NULL THEN
        EXECUTE format(
            'CREATE TABLE %I PARTITION OF attribute_value_history FOR VALUES FROM (%L) TO (%L)',
            partition_name,
            partition_start,
            partition_end
        );
    END IF;
END;
$$;
