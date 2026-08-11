ALTER TABLE blueprints
    ADD COLUMN display JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE blueprints
    ADD CONSTRAINT blueprints_display_object_check
    CHECK (jsonb_typeof(display) = 'object');
