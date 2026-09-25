CREATE TABLE saved_views (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    owner_user_id UUID NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL CHECK (kind = 'explorer_search'),
    name TEXT,
    description TEXT,
    visibility TEXT NOT NULL CHECK (visibility IN ('private', 'workspace', 'link')),
    state JSONB NOT NULL CHECK (jsonb_typeof(state) = 'object'),
    state_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CHECK ((visibility = 'link' AND name IS NULL) OR (visibility <> 'link' AND name IS NOT NULL))
);
CREATE INDEX saved_views_list_idx ON saved_views(workspace_id, updated_at DESC) WHERE deleted_at IS NULL AND visibility <> 'link';
CREATE INDEX saved_views_owner_idx ON saved_views(workspace_id, owner_user_id) WHERE deleted_at IS NULL;
CREATE INDEX saved_views_link_hash_idx ON saved_views(workspace_id, state_hash) WHERE deleted_at IS NULL AND visibility = 'link';
