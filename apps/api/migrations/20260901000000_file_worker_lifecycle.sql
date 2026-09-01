-- Server-derived file metadata and durable worker lifecycle state.
ALTER TABLE files ADD COLUMN width INTEGER CHECK (width IS NULL OR width > 0);
ALTER TABLE files ADD COLUMN height INTEGER CHECK (height IS NULL OR height > 0);

CREATE INDEX file_processing_jobs_recovery_idx
    ON file_processing_jobs (locked_at, id)
    WHERE status = 'running';
CREATE INDEX files_unreferenced_idx
    ON files (workspace_id, created_at)
    WHERE deleted_at IS NULL;
