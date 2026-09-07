ALTER TABLE workspaces ADD COLUMN settings JSONB NOT NULL DEFAULT '{}'::jsonb;

INSERT INTO permissions (code, description) VALUES
    ('workspace_navigation.manage', 'Configure workspace navigation shortcuts');

INSERT INTO role_permissions (role_id, permission_code) VALUES
    ('00000000-0000-4000-8000-000000000101', 'workspace_navigation.manage'),
    ('00000000-0000-4000-8000-000000000102', 'workspace_navigation.manage');
