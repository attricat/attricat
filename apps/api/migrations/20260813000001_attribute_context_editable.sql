ALTER TABLE attributes
    ADD COLUMN context_editable TEXT NOT NULL DEFAULT 'all'
    CHECK (context_editable IN ('all', 'default'));
