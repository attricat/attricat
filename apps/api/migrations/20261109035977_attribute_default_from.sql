-- A relationship path whose leaf value is copied into an empty attribute
-- when the path's first relationship gains a target.
ALTER TABLE attributes
    ADD COLUMN default_from TEXT;

-- Why core, rather than the writer, supplied a changed value.
ALTER TABLE audit_event_changes
    ADD COLUMN origin TEXT CHECK (origin IN ('default_from'));
