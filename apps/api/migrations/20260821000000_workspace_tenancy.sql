-- Catalog data is tenant-owned.  The initial installation is migrated into the
-- deterministic bootstrap workspace; later identity/bootstrap work can safely
-- refer to it without accepting a client-selected workspace ID.
CREATE TABLE workspaces (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE CHECK (slug ~ '^[A-Za-z0-9_-]+$'),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    bootstrap_owner_email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

INSERT INTO workspaces (id, slug, name)
VALUES ('00000000-0000-4000-8000-000000000002', 'default', 'Default workspace');

-- The API sets this trusted server configuration on each pooled connection.
-- Falling back to the bootstrap workspace keeps existing single-workspace
-- deployments working until authenticated active-workspace selection lands.
CREATE FUNCTION catalog_workspace_id() RETURNS UUID
LANGUAGE sql STABLE AS $$
    SELECT COALESCE(
        NULLIF(current_setting('catalog.workspace_id', true), '')::uuid,
        '00000000-0000-4000-8000-000000000002'::uuid
    )
$$;

-- Add and backfill ownership before making it mandatory.  This whole migration
-- is one transaction when run by SQLx, so a failed constraint change cannot
-- leave a partially-tenanted catalog behind.
ALTER TABLE blueprints ADD COLUMN workspace_id UUID;
ALTER TABLE attributes ADD COLUMN workspace_id UUID;
ALTER TABLE entities ADD COLUMN workspace_id UUID;
ALTER TABLE attribute_contexts ADD COLUMN workspace_id UUID;
ALTER TABLE attribute_values ADD COLUMN workspace_id UUID;
ALTER TABLE attribute_value_history ADD COLUMN workspace_id UUID;
ALTER TABLE blueprint_migration_batches ADD COLUMN workspace_id UUID;
ALTER TABLE entity_blueprint_migrations ADD COLUMN workspace_id UUID;

UPDATE blueprints SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE attributes SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE entities SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE attribute_contexts SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE attribute_values SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE attribute_value_history SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE blueprint_migration_batches SET workspace_id = '00000000-0000-4000-8000-000000000002';
UPDATE entity_blueprint_migrations SET workspace_id = '00000000-0000-4000-8000-000000000002';

ALTER TABLE blueprints ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE attributes ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE entities ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE attribute_contexts ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE attribute_values ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE attribute_value_history ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE blueprint_migration_batches ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE entity_blueprint_migrations ALTER COLUMN workspace_id SET NOT NULL;

-- New rows inherit the server-selected workspace. API payloads intentionally
-- have no workspace_id field, so callers cannot override this default.
ALTER TABLE blueprints ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE attributes ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE entities ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE attribute_contexts ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE attribute_values ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE attribute_value_history ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE blueprint_migration_batches ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();
ALTER TABLE entity_blueprint_migrations ALTER COLUMN workspace_id SET DEFAULT catalog_workspace_id();

ALTER TABLE blueprints ADD CONSTRAINT blueprints_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE attributes ADD CONSTRAINT attributes_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE entities ADD CONSTRAINT entities_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE attribute_contexts ADD CONSTRAINT attribute_contexts_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE attribute_value_history ADD CONSTRAINT attribute_value_history_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE blueprint_migration_batches ADD CONSTRAINT blueprint_migration_batches_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);
ALTER TABLE entity_blueprint_migrations ADD CONSTRAINT entity_blueprint_migrations_workspace_fkey FOREIGN KEY (workspace_id) REFERENCES workspaces (id);

ALTER TABLE attribute_contexts DROP CONSTRAINT attribute_contexts_code_key;
CREATE UNIQUE INDEX attribute_contexts_workspace_code_key ON attribute_contexts (workspace_id, code);
CREATE UNIQUE INDEX blueprints_workspace_revision_key ON blueprints (workspace_id, id, version);
CREATE UNIQUE INDEX attributes_workspace_id_key ON attributes (workspace_id, id);
CREATE UNIQUE INDEX entities_workspace_id_key ON entities (workspace_id, id);
CREATE UNIQUE INDEX attribute_contexts_workspace_id_key ON attribute_contexts (workspace_id, id);

ALTER TABLE attributes ADD CONSTRAINT attributes_workspace_blueprint_fkey
    FOREIGN KEY (workspace_id, blueprint_id, blueprint_version)
    REFERENCES blueprints (workspace_id, id, version);
ALTER TABLE entities ADD CONSTRAINT entities_workspace_blueprint_fkey
    FOREIGN KEY (workspace_id, blueprint_id, blueprint_version)
    REFERENCES blueprints (workspace_id, id, version);
ALTER TABLE attribute_contexts ADD CONSTRAINT attribute_contexts_workspace_parent_fkey
    FOREIGN KEY (workspace_id, parent_id)
    REFERENCES attribute_contexts (workspace_id, id);
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_workspace_entity_fkey
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_workspace_attribute_fkey
    FOREIGN KEY (workspace_id, attribute_id) REFERENCES attributes (workspace_id, id);
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_workspace_context_fkey
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts (workspace_id, id);
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_workspace_target_fkey
    FOREIGN KEY (workspace_id, relationship_target_entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE attribute_value_history ADD CONSTRAINT attribute_value_history_workspace_entity_fkey
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE attribute_value_history ADD CONSTRAINT attribute_value_history_workspace_attribute_fkey
    FOREIGN KEY (workspace_id, attribute_id) REFERENCES attributes (workspace_id, id);
ALTER TABLE attribute_value_history ADD CONSTRAINT attribute_value_history_workspace_context_fkey
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts (workspace_id, id);
ALTER TABLE attribute_value_history ADD CONSTRAINT attribute_value_history_workspace_target_fkey
    FOREIGN KEY (workspace_id, relationship_target_entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE blueprint_migration_batches ADD CONSTRAINT blueprint_migration_batches_workspace_blueprint_fkey
    FOREIGN KEY (workspace_id, blueprint_id, target_version) REFERENCES blueprints (workspace_id, id, version);
ALTER TABLE entity_blueprint_migrations ADD CONSTRAINT entity_blueprint_migrations_workspace_entity_fkey
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id);
ALTER TABLE entity_blueprint_migrations ADD CONSTRAINT entity_blueprint_migrations_workspace_blueprint_source_fkey
    FOREIGN KEY (workspace_id, blueprint_id, source_version) REFERENCES blueprints (workspace_id, id, version);
ALTER TABLE entity_blueprint_migrations ADD CONSTRAINT entity_blueprint_migrations_workspace_blueprint_target_fkey
    FOREIGN KEY (workspace_id, blueprint_id, target_version) REFERENCES blueprints (workspace_id, id, version);

ALTER TABLE blueprints ENABLE ROW LEVEL SECURITY;
ALTER TABLE attributes ENABLE ROW LEVEL SECURITY;
ALTER TABLE entities ENABLE ROW LEVEL SECURITY;
ALTER TABLE attribute_contexts ENABLE ROW LEVEL SECURITY;
ALTER TABLE attribute_values ENABLE ROW LEVEL SECURITY;
ALTER TABLE attribute_value_history ENABLE ROW LEVEL SECURITY;
ALTER TABLE blueprint_migration_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE entity_blueprint_migrations ENABLE ROW LEVEL SECURITY;

CREATE POLICY blueprints_workspace_policy ON blueprints USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY attributes_workspace_policy ON attributes USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY entities_workspace_policy ON entities USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY attribute_contexts_workspace_policy ON attribute_contexts USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY attribute_values_workspace_policy ON attribute_values USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY attribute_value_history_workspace_policy ON attribute_value_history USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY blueprint_migration_batches_workspace_policy ON blueprint_migration_batches USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY entity_blueprint_migrations_workspace_policy ON entity_blueprint_migrations USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
