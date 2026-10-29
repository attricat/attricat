-- What brought each status change about: the operation that wrote it and the
-- channel it arrived through. Rows written before this are NULL (not recorded).
-- An approval links to the transition that entered its status.
ALTER TABLE record_status_transitions
    ADD COLUMN operation text
        CHECK (operation IN ('edit', 'history_restore', 'blueprint_migration')),
    ADD COLUMN channel text
        CHECK (channel IN ('person', 'api_token', 'agent', 'workflow', 'extension', 'event_handler', 'system'));

ALTER TABLE record_approvals
    ADD COLUMN transition_id uuid REFERENCES record_status_transitions (id) ON DELETE SET NULL;
