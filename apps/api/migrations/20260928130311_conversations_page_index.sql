CREATE INDEX conversations_workspace_active_page_idx
    ON conversations (workspace_id, updated_at DESC, id DESC)
    WHERE archived_at IS NULL;
