CREATE TABLE audit_event_changes (
    id UUID PRIMARY KEY,
    audit_event_id UUID NOT NULL REFERENCES audit_events (id),
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    attribute_id UUID NOT NULL REFERENCES attributes (id),
    attribute_code TEXT NOT NULL,
    context_id UUID REFERENCES attribute_contexts (id),
    context_code TEXT,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('set', 'replace', 'remove', 'relationship_add', 'relationship_remove', 'restore')),
    before_value JSONB,
    after_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX audit_event_changes_entity_created_at_idx ON audit_event_changes (entity_id, created_at DESC);
CREATE INDEX audit_event_changes_audit_event_id_idx ON audit_event_changes (audit_event_id);
