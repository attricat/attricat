-- Channel publication is approval metadata for live entity data, not an export snapshot.
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

CREATE TABLE entity_channel_publications (
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    context_id UUID NOT NULL,
    published_at TIMESTAMPTZ,
    published_by_user_id UUID REFERENCES users (id),
    PRIMARY KEY (workspace_id, entity_id, context_id),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id),
    FOREIGN KEY (workspace_id, context_id)
        REFERENCES attribute_contexts (workspace_id, id),
    CHECK (
        (published_at IS NULL AND published_by_user_id IS NULL)
        OR (published_at IS NOT NULL AND published_by_user_id IS NOT NULL)
    )
);

CREATE INDEX entity_channel_publications_published_idx
    ON entity_channel_publications (workspace_id, context_id, entity_id)
    WHERE published_at IS NOT NULL;
