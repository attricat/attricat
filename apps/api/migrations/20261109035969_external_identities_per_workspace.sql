-- A link holds only in the workspace that made it. Workspace administrators can
-- point their provider at any issuer, so a link made in one workspace must not
-- be visible to, removable by, or usable from another. No release has created
-- these rows: until now nothing linked an identity outside tests.
ALTER TABLE external_identities
    ADD COLUMN workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    DROP CONSTRAINT external_identities_pkey,
    ADD PRIMARY KEY (workspace_id, issuer, subject);
