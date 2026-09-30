-- Avatars are ordinary workspace files. `purpose` keeps avatar uploads out of
-- entity attributes (their originals may carry EXIF metadata) and tells the
-- file worker to produce the square `avatar` variant instead of attachment
-- variants.
ALTER TABLE files
    ADD COLUMN purpose TEXT NOT NULL DEFAULT 'attachment'
        CHECK (purpose IN ('attachment', 'avatar'));

-- A member's avatar in this workspace. Replaced avatars become unreferenced
-- and are reclaimed by file reconciliation.
ALTER TABLE workspace_memberships
    ADD COLUMN avatar_file_id UUID,
    ADD CONSTRAINT workspace_memberships_avatar_file_fkey
        FOREIGN KEY (workspace_id, avatar_file_id) REFERENCES files (workspace_id, id);

CREATE UNIQUE INDEX workspace_memberships_avatar_file_key
    ON workspace_memberships (avatar_file_id) WHERE avatar_file_id IS NOT NULL;
