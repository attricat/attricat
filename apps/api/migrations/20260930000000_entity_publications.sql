-- Channel publication is deliberately modeled separately from mutable entity facts.
CREATE TABLE publication_channels (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    context_id UUID NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, context_id),
    FOREIGN KEY (workspace_id, context_id)
        REFERENCES attribute_contexts (workspace_id, id)
);

CREATE TABLE entity_publication_snapshots (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    context_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL,
    payload JSONB NOT NULL,
    payload_hash TEXT NOT NULL,
    published_by_user_id UUID REFERENCES users (id),
    published_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, entity_id, context_id, revision),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts (workspace_id, id),
    FOREIGN KEY (workspace_id, blueprint_id, blueprint_version)
        REFERENCES blueprints (workspace_id, id, version)
);

CREATE INDEX entity_publication_snapshots_entity_context_idx
    ON entity_publication_snapshots (workspace_id, entity_id, context_id, revision DESC);

CREATE TABLE entity_channel_publications (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    context_id UUID NOT NULL,
    active_snapshot_id UUID,
    unpublished_at TIMESTAMPTZ,
    unpublished_by_user_id UUID REFERENCES users (id),
    PRIMARY KEY (workspace_id, entity_id, context_id),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts (workspace_id, id),
    FOREIGN KEY (active_snapshot_id) REFERENCES entity_publication_snapshots (id)
);

CREATE TABLE entity_publication_dependencies (
    snapshot_id UUID NOT NULL REFERENCES entity_publication_snapshots (id),
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    target_entity_id UUID NOT NULL,
    context_id UUID NOT NULL,
    PRIMARY KEY (snapshot_id, target_entity_id, context_id),
    FOREIGN KEY (workspace_id, target_entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts (workspace_id, id)
);

CREATE INDEX entity_publication_dependencies_target_idx
    ON entity_publication_dependencies (workspace_id, target_entity_id, context_id);
