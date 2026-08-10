ALTER TABLE blueprints
    ADD COLUMN code TEXT,
    ADD COLUMN kind TEXT,
    ADD COLUMN includes JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE attributes
    ADD COLUMN position BIGINT;

ALTER TABLE blueprints
    ADD CONSTRAINT blueprints_kind_check
    CHECK (kind IS NULL OR kind IN ('entity', 'mixin'));

CREATE INDEX blueprints_code_version_idx
    ON blueprints (code, version);

CREATE INDEX blueprints_includes_gin_idx
    ON blueprints USING GIN (includes);
