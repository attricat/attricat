-- Context fallback looks for an active value of the same record attribute in a
-- nearer context. Without this index each probe scans every value of the
-- attribute across all records.
CREATE INDEX attribute_values_active_record_attribute_context_idx
    ON attribute_values (record_id, attribute_id, context_id)
    WHERE active;
