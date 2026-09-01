-- Agent- and operator-owned annotations. They are intentionally outside the
-- versioned blueprint/EAV data model and are not rendered in entity previews.
ALTER TABLE entities
    ADD COLUMN system_tags TEXT[] NOT NULL DEFAULT ARRAY[]::text[],
    ADD COLUMN system_metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX entities_system_tags_idx
    ON entities USING GIN (system_tags);
