-- Contexts are now a rooted tree. The fixed root ID lets existing NULL facts
-- become ordinary facts in the persisted default context.
ALTER TABLE attribute_contexts ADD COLUMN parent_id UUID;

INSERT INTO attribute_contexts (id, code, data, parent_id)
VALUES ('00000000-0000-4000-8000-000000000001', 'default', '{}'::jsonb, NULL);

UPDATE attribute_values
SET context_id = '00000000-0000-4000-8000-000000000001'
WHERE context_id IS NULL;

ALTER TABLE attribute_values ALTER COLUMN context_id SET NOT NULL;
ALTER TABLE attribute_contexts
    ADD CONSTRAINT attribute_contexts_parent_id_fkey
    FOREIGN KEY (parent_id) REFERENCES attribute_contexts (id);
CREATE INDEX attribute_contexts_parent_id_idx ON attribute_contexts (parent_id);
