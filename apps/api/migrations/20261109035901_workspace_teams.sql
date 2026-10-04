-- Workspace teams: named groups of members that user-or-team assignment
-- attributes can reference. Deleted teams keep their row so stored
-- assignments still display a name.
CREATE TABLE teams (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    code TEXT NOT NULL CHECK (code ~ '^[A-Za-z0-9_-]{1,128}$'),
    name TEXT NOT NULL CHECK (btrim(name) <> '' AND char_length(name) <= 200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (workspace_id, id)
);
CREATE UNIQUE INDEX teams_workspace_code_key ON teams (workspace_id, code) WHERE deleted_at IS NULL;

CREATE TABLE team_members (
    workspace_id UUID NOT NULL,
    team_id UUID NOT NULL,
    membership_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, membership_id),
    FOREIGN KEY (workspace_id, team_id) REFERENCES teams (workspace_id, id) ON DELETE CASCADE,
    FOREIGN KEY (workspace_id, membership_id) REFERENCES workspace_memberships (workspace_id, id)
);
CREATE INDEX team_members_membership_idx ON team_members (workspace_id, membership_id);
