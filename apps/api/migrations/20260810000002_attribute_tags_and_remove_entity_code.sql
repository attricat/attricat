ALTER TABLE attributes
    ADD COLUMN tags JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE attributes
    ADD CONSTRAINT attributes_tags_array_check
    CHECK (jsonb_typeof(tags) = 'array');

DROP INDEX entities_active_blueprint_code_idx;
ALTER TABLE entities DROP COLUMN code;
