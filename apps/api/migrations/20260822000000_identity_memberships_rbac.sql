-- Provider-neutral identities and the initial, immutable RBAC catalog. Credentials,
-- external identities, invitations, and request authorization are deliberately
-- introduced by later issues.
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE CHECK (email = lower(btrim(email)) AND position('@' IN email) > 1),
    display_name TEXT,
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'inactive')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE workspace_memberships (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    user_id UUID NOT NULL REFERENCES users (id),
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'inactive')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, user_id),
    UNIQUE (workspace_id, id)
);

CREATE TABLE permissions (
    code TEXT PRIMARY KEY CHECK (code ~ '^[a-z_]+\.[a-z_]+$'),
    description TEXT NOT NULL
);

CREATE TABLE roles (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    workspace_id UUID REFERENCES workspaces (id),
    is_system BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((is_system AND workspace_id IS NULL) OR (NOT is_system AND workspace_id IS NOT NULL))
);
CREATE UNIQUE INDEX roles_system_code_key ON roles (code) WHERE is_system;
CREATE UNIQUE INDEX roles_workspace_code_key ON roles (workspace_id, code) WHERE NOT is_system;

CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    permission_code TEXT NOT NULL REFERENCES permissions (code) ON DELETE RESTRICT,
    PRIMARY KEY (role_id, permission_code)
);

CREATE TABLE role_grants (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    membership_id UUID NOT NULL,
    role_id UUID NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('workspace', 'blueprint_family', 'entity', 'context_subtree')),
    scope_target_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, membership_id, role_id, scope_type, scope_target_id),
    FOREIGN KEY (workspace_id, membership_id)
        REFERENCES workspace_memberships (workspace_id, id)
);

INSERT INTO permissions (code, description) VALUES
    ('workspace.manage', 'Manage workspace lifecycle and ownership'),
    ('members.manage', 'Manage workspace memberships and invitations'),
    ('roles.grant', 'Grant and revoke workspace roles'),
    ('tokens.manage', 'Manage personal API tokens'),
    ('blueprints.read', 'Read blueprint definitions'),
    ('blueprints.write', 'Create and modify blueprint definitions'),
    ('blueprints.publish', 'Publish blueprint revisions'),
    ('entities.read', 'Read catalog entities'),
    ('entities.write', 'Create and modify catalog entities'),
    ('entities.delete', 'Delete catalog entities'),
    ('contexts.read', 'Read attribute contexts'),
    ('contexts.write', 'Create and modify attribute contexts'),
    ('data_health.read', 'Read catalog data-health reports');

INSERT INTO roles (id, code, workspace_id, is_system) VALUES
    ('00000000-0000-4000-8000-000000000101', 'owner', NULL, true),
    ('00000000-0000-4000-8000-000000000102', 'admin', NULL, true),
    ('00000000-0000-4000-8000-000000000103', 'editor', NULL, true),
    ('00000000-0000-4000-8000-000000000104', 'viewer', NULL, true);

-- Owner receives every catalog permission. Admin excludes ownership/lifecycle
-- control; editor cannot publish or administer a workspace; viewer is read-only.
INSERT INTO role_permissions (role_id, permission_code)
SELECT '00000000-0000-4000-8000-000000000101'::uuid, code FROM permissions;
INSERT INTO role_permissions (role_id, permission_code)
SELECT '00000000-0000-4000-8000-000000000102'::uuid, code
FROM permissions WHERE code <> 'workspace.manage';
INSERT INTO role_permissions (role_id, permission_code) VALUES
    ('00000000-0000-4000-8000-000000000103', 'blueprints.read'),
    ('00000000-0000-4000-8000-000000000103', 'blueprints.write'),
    ('00000000-0000-4000-8000-000000000103', 'entities.read'),
    ('00000000-0000-4000-8000-000000000103', 'entities.write'),
    ('00000000-0000-4000-8000-000000000103', 'entities.delete'),
    ('00000000-0000-4000-8000-000000000103', 'contexts.read'),
    ('00000000-0000-4000-8000-000000000103', 'contexts.write'),
    ('00000000-0000-4000-8000-000000000103', 'data_health.read'),
    ('00000000-0000-4000-8000-000000000104', 'blueprints.read'),
    ('00000000-0000-4000-8000-000000000104', 'entities.read'),
    ('00000000-0000-4000-8000-000000000104', 'contexts.read'),
    ('00000000-0000-4000-8000-000000000104', 'data_health.read');
