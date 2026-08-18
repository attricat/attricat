ALTER TABLE blueprints
    ADD COLUMN views JSONB NOT NULL DEFAULT '{}'::jsonb;
