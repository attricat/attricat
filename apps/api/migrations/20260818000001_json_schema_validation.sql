ALTER TABLE blueprints
    ADD COLUMN entity_schema JSONB;

ALTER TABLE attributes
    ADD COLUMN value_schema JSONB;
