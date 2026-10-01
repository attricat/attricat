CREATE TABLE entity_comments (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    entity_id UUID NOT NULL,
    author_user_id UUID NOT NULL REFERENCES users (id),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 10000),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, entity_id) REFERENCES entities (workspace_id, id)
);
CREATE INDEX entity_comments_page_idx
    ON entity_comments (workspace_id, entity_id, created_at DESC, id DESC);
