ALTER TABLE conversations ADD COLUMN entity_id UUID REFERENCES entities(id);
ALTER TABLE conversations ADD COLUMN context_id UUID REFERENCES attribute_contexts(id);
ALTER TABLE conversations ADD CONSTRAINT conversations_context_requires_entity CHECK (context_id IS NULL OR entity_id IS NOT NULL);
CREATE INDEX conversations_entity_context_idx ON conversations (workspace_id, entity_id, context_id, updated_at DESC) WHERE entity_id IS NOT NULL AND archived_at IS NULL;
