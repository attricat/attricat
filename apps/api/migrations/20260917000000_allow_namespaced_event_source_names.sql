-- Event source names distinguish extension origins with the documented
-- `extension:<extension-id>` form. Event type and aggregate identifiers remain
-- subject to their existing stricter validation.
ALTER TABLE domain_events
    DROP CONSTRAINT domain_events_source_name_check;
ALTER TABLE domain_events
    ADD CONSTRAINT domain_events_source_name_check
    CHECK (source_name ~ '^[A-Za-z][A-Za-z0-9._:-]{0,127}$');
