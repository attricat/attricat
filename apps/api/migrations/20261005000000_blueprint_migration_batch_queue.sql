-- Batch coordination is durable data. The API owns claiming and recovery; this
-- declarative migration only defines the states and singleton index.
ALTER TABLE blueprint_migration_batches
    DROP CONSTRAINT blueprint_migration_batches_status_check;
ALTER TABLE blueprint_migration_batches
    ADD CONSTRAINT blueprint_migration_batches_status_check
    CHECK (status IN ('draft', 'queued', 'running', 'completed', 'superseded'));

ALTER TABLE blueprint_migration_batches
    ADD COLUMN lease_owner UUID,
    ADD COLUMN lease_until TIMESTAMPTZ;

CREATE UNIQUE INDEX blueprint_migration_batches_active_target_key
    ON blueprint_migration_batches (workspace_id, blueprint_id, target_version)
    WHERE status IN ('queued', 'running');
