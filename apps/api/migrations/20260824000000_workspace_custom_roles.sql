INSERT INTO permissions (code, description) VALUES
    ('roles.manage', 'Create, modify, duplicate, and retire workspace-local roles');

INSERT INTO role_permissions (role_id, permission_code) VALUES
    ('00000000-0000-4000-8000-000000000101', 'roles.manage'),
    ('00000000-0000-4000-8000-000000000102', 'roles.manage');
