-- The unsaved record form values a run may propose changes to, sent with a
-- message from a record's agent panel beside its editable form.
ALTER TABLE agent_runs
    ADD COLUMN record_form_draft JSONB
    CHECK (record_form_draft IS NULL OR jsonb_typeof(record_form_draft) = 'object');
