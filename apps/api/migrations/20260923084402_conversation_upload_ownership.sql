-- Conversation uploads are private to the uploading user and conversation.
-- Existing files remain unowned and cannot be newly submitted to an agent.
ALTER TABLE files
    ADD COLUMN conversation_upload_conversation_id UUID REFERENCES conversations (id),
    ADD COLUMN conversation_upload_user_id UUID REFERENCES users (id);
CREATE INDEX files_conversation_upload_owner_idx
    ON files (workspace_id, conversation_upload_conversation_id, conversation_upload_user_id)
    WHERE conversation_upload_conversation_id IS NOT NULL;
