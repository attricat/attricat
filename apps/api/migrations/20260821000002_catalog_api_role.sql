-- Run request handling as a non-owner role so FORCE ROW LEVEL SECURITY is
-- effective even when local development connects with the postgres superuser.
DO $$
BEGIN
    CREATE ROLE catalog_api NOLOGIN;
EXCEPTION WHEN duplicate_object THEN
    NULL;
END
$$;

GRANT USAGE ON SCHEMA public TO catalog_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON
    blueprints, attributes, entities, attribute_contexts, attribute_values,
    attribute_value_history, blueprint_migration_batches, entity_blueprint_migrations
TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_workspace_id() TO catalog_api;
GRANT EXECUTE ON FUNCTION ensure_attribute_value_history_partition(TIMESTAMPTZ) TO catalog_api;
