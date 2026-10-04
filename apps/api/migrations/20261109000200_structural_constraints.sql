-- Structural constraints declared by blueprint revisions. Repository
-- transactions decide which keys and hierarchies apply and maintain the key
-- index; the database only guarantees that two entities cannot hold the same
-- key value at once.

-- Compiled `[[unique_keys]]`. The latest published revision of a blueprint
-- family defines the keys enforced for every entity in that family.
ALTER TABLE blueprints
    ADD COLUMN unique_keys JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(unique_keys) = 'array');

-- `target_blueprints` lists every allowed target when there is more than
-- one; a single target stays in `target_blueprint_code`. `hierarchy` is the
-- compiled `acyclic`/`tree` declaration.
ALTER TABLE attributes
    ADD COLUMN target_blueprint_codes TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN hierarchy TEXT CHECK (hierarchy IN ('acyclic', 'tree'));

-- One row per entity, enforced key and context with the entity's normalized
-- key value. Workspace-scoped keys use the default context. The unique
-- constraint makes the second of two concurrent duplicate writers fail.
CREATE TABLE entity_unique_key_values (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    blueprint_id UUID NOT NULL,
    key_code TEXT NOT NULL CHECK (key_code ~ '^[A-Za-z0-9_-]+$'),
    context_id UUID NOT NULL,
    key_hash TEXT NOT NULL CHECK (key_hash ~ '^[0-9a-f]{64}$'),
    key_values JSONB NOT NULL CHECK (jsonb_typeof(key_values) = 'array'),
    entity_id UUID NOT NULL,
    PRIMARY KEY (entity_id, key_code, context_id),
    CONSTRAINT entity_unique_key_values_value_key
        UNIQUE (workspace_id, blueprint_id, key_code, context_id, key_hash),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (workspace_id, context_id)
        REFERENCES attribute_contexts (workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX entity_unique_key_values_family_idx
    ON entity_unique_key_values (workspace_id, blueprint_id, key_code);
