-- Task delivery now owns migration batch leasing. Existing pre-cutover rows
-- may contain duplicate batch/entity previews, so the explicit ownership bit
-- isolates the new stable task identity without data-changing migration SQL.
ALTER TABLE entity_blueprint_migrations
    ADD COLUMN task_owned BOOLEAN NOT NULL DEFAULT false;
CREATE UNIQUE INDEX entity_blueprint_migrations_task_batch_entity_key
    ON entity_blueprint_migrations (batch_id, entity_id)
    WHERE batch_id IS NOT NULL AND task_owned;

ALTER TABLE blueprint_migration_batches
    DROP CONSTRAINT blueprint_migration_batches_status_check;
ALTER TABLE blueprint_migration_batches
    ADD CONSTRAINT blueprint_migration_batches_status_check
    CHECK (status IN ('draft', 'queued', 'running', 'completed', 'failed', 'superseded'));
ALTER TABLE blueprint_migration_batches
    DROP COLUMN lease_owner,
    DROP COLUMN lease_until;
