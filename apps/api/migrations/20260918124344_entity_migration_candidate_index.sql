CREATE INDEX entities_active_migration_candidates_idx
    ON entities (workspace_id, blueprint_id, created_at DESC, id DESC)
    INCLUDE (blueprint_version)
    WHERE deleted_at IS NULL;
