-- Controlled records: status transition history (separation of duties and
-- unlock audit), approvals bound to a content digest, and file retention holds.
CREATE TABLE entity_status_transitions (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    attribute_code TEXT NOT NULL,
    context_id UUID NOT NULL,
    from_status TEXT,
    to_status TEXT,
    edge_code TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('transition', 'approval_void')),
    unlocked BOOLEAN NOT NULL DEFAULT false,
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (context_id) REFERENCES attribute_contexts (id) ON DELETE CASCADE,
    CHECK (from_status IS DISTINCT FROM to_status)
);
CREATE INDEX entity_status_transitions_lookup_idx
    ON entity_status_transitions (workspace_id, entity_id, attribute_code, context_id, occurred_at DESC);

CREATE TABLE entity_approvals (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    attribute_code TEXT NOT NULL,
    context_id UUID NOT NULL,
    status TEXT NOT NULL,
    covers_all BOOLEAN NOT NULL,
    covered_attributes TEXT[] NOT NULL,
    content_digest TEXT NOT NULL CHECK (content_digest ~ '^[0-9a-f]{64}$'),
    approved_by_user_id UUID REFERENCES users (id),
    approved_by_token_id UUID,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ,
    end_reason TEXT CHECK (end_reason IN ('content_changed', 'superseded')),
    ended_by_user_id UUID REFERENCES users (id),
    void_status TEXT,
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (context_id) REFERENCES attribute_contexts (id) ON DELETE CASCADE,
    CHECK ((ended_at IS NULL) = (end_reason IS NULL)),
    CHECK (covers_all OR cardinality(covered_attributes) > 0)
);
CREATE UNIQUE INDEX entity_approvals_active_key
    ON entity_approvals (workspace_id, entity_id, attribute_code, context_id)
    WHERE ended_at IS NULL;
CREATE INDEX entity_approvals_history_idx
    ON entity_approvals (workspace_id, entity_id, approved_at DESC, id DESC);

CREATE TABLE file_retention_holds (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    file_id UUID NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('status', 'explicit')),
    entity_id UUID,
    attribute_code TEXT,
    status TEXT,
    reason TEXT CHECK (reason IS NULL OR char_length(reason) BETWEEN 1 AND 1000),
    held_until TIMESTAMPTZ NOT NULL,
    created_by_user_id UUID REFERENCES users (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    released_by_user_id UUID REFERENCES users (id),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    CHECK (held_until > created_at),
    CHECK ((source = 'status') = (entity_id IS NOT NULL AND status IS NOT NULL)),
    CHECK (source = 'explicit' OR released_at IS NULL)
);
CREATE INDEX file_retention_holds_file_idx
    ON file_retention_holds (workspace_id, file_id, held_until DESC);
CREATE INDEX file_retention_holds_entity_idx
    ON file_retention_holds (workspace_id, entity_id) WHERE entity_id IS NOT NULL;
