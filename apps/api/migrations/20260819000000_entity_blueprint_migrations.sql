CREATE TABLE blueprint_migration_batches (
    id UUID PRIMARY KEY,
    blueprint_id UUID NOT NULL,
    target_version BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CONSTRAINT blueprint_migration_batches_target_fkey
        FOREIGN KEY (blueprint_id, target_version)
        REFERENCES blueprints (id, version),
    CONSTRAINT blueprint_migration_batches_status_check
        CHECK (status IN ('draft', 'running', 'completed', 'superseded'))
);

CREATE TABLE entity_blueprint_migrations (
    id UUID PRIMARY KEY,
    batch_id UUID REFERENCES blueprint_migration_batches (id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities (id),
    blueprint_id UUID NOT NULL,
    source_version BIGINT NOT NULL,
    target_version BIGINT NOT NULL,
    status TEXT NOT NULL,
    issues JSONB NOT NULL DEFAULT '[]'::jsonb,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CONSTRAINT entity_blueprint_migrations_source_fkey
        FOREIGN KEY (blueprint_id, source_version)
        REFERENCES blueprints (id, version),
    CONSTRAINT entity_blueprint_migrations_target_fkey
        FOREIGN KEY (blueprint_id, target_version)
        REFERENCES blueprints (id, version),
    CONSTRAINT entity_blueprint_migrations_status_check
        CHECK (status IN (
            'ready', 'needs_input', 'blocked', 'pending', 'migrating',
            'migrated', 'failed', 'skipped', 'superseded'
        ))
);

CREATE INDEX entity_blueprint_migrations_entity_idx
    ON entity_blueprint_migrations (entity_id, created_at DESC);
CREATE INDEX entity_blueprint_migrations_batch_status_idx
    ON entity_blueprint_migrations (batch_id, status)
    WHERE batch_id IS NOT NULL;
