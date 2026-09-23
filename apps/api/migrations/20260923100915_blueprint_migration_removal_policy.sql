ALTER TABLE blueprint_migration_batches
    ADD COLUMN removal_policy JSONB NOT NULL DEFAULT '{}'::jsonb;
