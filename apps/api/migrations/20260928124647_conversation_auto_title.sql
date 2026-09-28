ALTER TABLE conversations ADD COLUMN title_source TEXT NOT NULL DEFAULT 'manual'
    CHECK (title_source IN ('manual', 'pending', 'generated'));
