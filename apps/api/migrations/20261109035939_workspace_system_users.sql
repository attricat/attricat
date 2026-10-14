-- People sign in; a workspace's system user only authors automated notes,
-- such as comments that record a failed bulk edit. Application code creates
-- one per workspace and keeps it out of memberships, credentials and tokens.
ALTER TABLE users
    ADD COLUMN kind TEXT NOT NULL DEFAULT 'person' CHECK (kind IN ('person', 'system'));

ALTER TABLE workspaces
    ADD COLUMN system_user_id UUID UNIQUE REFERENCES users (id);
