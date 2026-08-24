-- Table owners otherwise bypass RLS, which would make the API process able to
-- accidentally read another workspace despite the policy. Force the policy for
-- the application role as well.
ALTER TABLE blueprints FORCE ROW LEVEL SECURITY;
ALTER TABLE attributes FORCE ROW LEVEL SECURITY;
ALTER TABLE entities FORCE ROW LEVEL SECURITY;
ALTER TABLE attribute_contexts FORCE ROW LEVEL SECURITY;
ALTER TABLE attribute_values FORCE ROW LEVEL SECURITY;
ALTER TABLE attribute_value_history FORCE ROW LEVEL SECURITY;
ALTER TABLE blueprint_migration_batches FORCE ROW LEVEL SECURITY;
ALTER TABLE entity_blueprint_migrations FORCE ROW LEVEL SECURITY;
