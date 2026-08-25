-- Links a workspace invitation to the account password-setup action. Opaque
-- secret digests live in the existing invitation/action tables; this table
-- contains no deliverable secret material.
CREATE TABLE workspace_invitation_onboarding (
    invitation_id UUID PRIMARY KEY REFERENCES workspace_invitations (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    action_token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(action_token_digest) = 32)
);
CREATE INDEX workspace_invitation_onboarding_workspace_idx
    ON workspace_invitation_onboarding (workspace_id);
