ALTER TABLE files
    ADD COLUMN attachment_expires_at TIMESTAMPTZ;

CREATE INDEX files_pending_attachment_expiry_idx
    ON files (workspace_id, attachment_expires_at)
    WHERE deleted_at IS NULL AND attachment_expires_at IS NOT NULL;
