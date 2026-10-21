-- Owners and administrators can unlock a locking status outside the declared
-- transitions; those rows are recorded with their own kind.
ALTER TABLE record_status_transitions
    DROP CONSTRAINT record_status_transitions_kind_check,
    ADD CONSTRAINT record_status_transitions_kind_check
        CHECK (kind IN ('transition', 'approval_void', 'override'));
