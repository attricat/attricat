-- Advanced by every change to a workspace's memberships, role grants and
-- workspace role permissions; request authorization caches a member's
-- workspace-wide permissions under it.
ALTER TABLE workspaces
    ADD COLUMN access_generation BIGINT NOT NULL DEFAULT 0 CHECK (access_generation >= 0);
