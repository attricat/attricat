ALTER TABLE attributes
    ADD COLUMN context_fallback TEXT NOT NULL DEFAULT 'default'
    CHECK (context_fallback IN ('default', 'none'));
