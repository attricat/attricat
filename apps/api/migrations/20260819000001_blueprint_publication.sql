ALTER TABLE blueprints
    ADD COLUMN status TEXT NOT NULL DEFAULT 'draft',
    ADD COLUMN published_at TIMESTAMPTZ;

UPDATE blueprints
SET status = 'published', published_at = created_at;

ALTER TABLE blueprints
    ADD CONSTRAINT blueprints_status_check
    CHECK (status IN ('draft', 'published'));

CREATE INDEX blueprints_published_version_idx
    ON blueprints (id, version DESC)
    WHERE status = 'published' AND deleted_at IS NULL;
