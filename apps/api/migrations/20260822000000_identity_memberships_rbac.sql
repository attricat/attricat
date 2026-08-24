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

CREATE FUNCTION protect_system_role() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.is_system THEN
        RAISE EXCEPTION 'seeded system roles are immutable';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER roles_protect_system BEFORE UPDATE OR DELETE ON roles
FOR EACH ROW EXECUTE FUNCTION protect_system_role();

CREATE FUNCTION protect_system_role_permissions() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM roles WHERE id = COALESCE(NEW.role_id, OLD.role_id) AND is_system) THEN
        RAISE EXCEPTION 'seeded system role permissions are immutable';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER role_permissions_protect_system BEFORE INSERT OR UPDATE OR DELETE ON role_permissions
FOR EACH ROW EXECUTE FUNCTION protect_system_role_permissions();

CREATE FUNCTION validate_role_grant() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
DECLARE
    role_workspace UUID;
BEGIN
    SELECT workspace_id INTO role_workspace FROM roles WHERE id = NEW.role_id;
    IF NOT FOUND OR (role_workspace IS NOT NULL AND role_workspace <> NEW.workspace_id) THEN
        RAISE EXCEPTION 'role does not belong to grant workspace';
    END IF;

    IF NEW.scope_type = 'workspace' THEN
        IF NEW.scope_target_id <> NEW.workspace_id THEN
            RAISE EXCEPTION 'workspace grants must target their grant workspace';
        END IF;
    ELSIF NEW.scope_type = 'blueprint_family' THEN
        PERFORM 1 FROM blueprints WHERE id = NEW.scope_target_id AND workspace_id = NEW.workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'blueprint family does not belong to grant workspace'; END IF;
    ELSIF NEW.scope_type = 'entity' THEN
        PERFORM 1 FROM entities WHERE id = NEW.scope_target_id AND workspace_id = NEW.workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'entity does not belong to grant workspace'; END IF;
    ELSIF NEW.scope_type = 'context_subtree' THEN
        PERFORM 1 FROM attribute_contexts WHERE id = NEW.scope_target_id AND workspace_id = NEW.workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'context does not belong to grant workspace'; END IF;
    END IF;

    -- Ownership is intentionally a workspace-wide, not delegated, capability.
    IF NEW.role_id = '00000000-0000-4000-8000-000000000101'::uuid
       AND (NEW.scope_type <> 'workspace' OR NEW.scope_target_id <> NEW.workspace_id) THEN
        RAISE EXCEPTION 'owner grants must be workspace scoped';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER role_grants_validate BEFORE INSERT OR UPDATE ON role_grants
FOR EACH ROW EXECUTE FUNCTION validate_role_grant();

CREATE FUNCTION require_another_workspace_owner(p_workspace_id UUID, p_membership_id UUID) RETURNS VOID
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM 1 FROM workspaces WHERE id = p_workspace_id FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'workspace does not exist'; END IF;

    PERFORM 1
    FROM role_grants role_grant
    JOIN workspace_memberships membership ON membership.id = role_grant.membership_id
      AND membership.workspace_id = role_grant.workspace_id
    WHERE role_grant.workspace_id = p_workspace_id
      AND role_grant.membership_id <> p_membership_id
      AND role_grant.role_id = '00000000-0000-4000-8000-000000000101'::uuid
      AND role_grant.scope_type = 'workspace'
      AND role_grant.scope_target_id = p_workspace_id
      AND membership.state = 'active';
    IF NOT FOUND THEN
        RAISE EXCEPTION 'workspace must retain at least one active owner';
    END IF;
END;
$$;

CREATE FUNCTION protect_last_workspace_owner_grant() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.role_id = '00000000-0000-4000-8000-000000000101'::uuid
       AND OLD.scope_type = 'workspace' AND OLD.scope_target_id = OLD.workspace_id
       AND (TG_OP = 'DELETE' OR NEW.role_id <> OLD.role_id OR NEW.scope_type <> OLD.scope_type
            OR NEW.scope_target_id <> OLD.scope_target_id OR NEW.workspace_id <> OLD.workspace_id
            OR NEW.membership_id <> OLD.membership_id) THEN
        PERFORM require_another_workspace_owner(OLD.workspace_id, OLD.membership_id);
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER role_grants_protect_last_owner BEFORE UPDATE OR DELETE ON role_grants
FOR EACH ROW EXECUTE FUNCTION protect_last_workspace_owner_grant();

CREATE FUNCTION protect_last_workspace_owner_membership() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state = 'active' AND (TG_OP = 'DELETE' OR NEW.state <> 'active') THEN
        PERFORM 1 FROM role_grants
        WHERE workspace_id = OLD.workspace_id AND membership_id = OLD.id
          AND role_id = '00000000-0000-4000-8000-000000000101'::uuid
          AND scope_type = 'workspace' AND scope_target_id = OLD.workspace_id;
        IF FOUND THEN
            PERFORM require_another_workspace_owner(OLD.workspace_id, OLD.id);
        END IF;
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER workspace_memberships_protect_last_owner BEFORE UPDATE OR DELETE ON workspace_memberships
FOR EACH ROW EXECUTE FUNCTION protect_last_workspace_owner_membership();

-- Called by startup after it records the configured bootstrap-owner email.
CREATE FUNCTION bootstrap_workspace_owner(
    p_workspace_id UUID,
    p_user_id UUID,
    p_membership_id UUID,
    p_grant_id UUID,
    p_email TEXT
) RETURNS VOID
LANGUAGE plpgsql AS $$
DECLARE
    membership UUID;
BEGIN
    IF p_email <> lower(btrim(p_email)) THEN
        RAISE EXCEPTION 'bootstrap owner email must be normalized';
    END IF;
    PERFORM 1 FROM workspaces WHERE id = p_workspace_id AND deleted_at IS NULL FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'bootstrap workspace does not exist or is deleted'; END IF;

    INSERT INTO users (id, email) VALUES (p_user_id, p_email)
    ON CONFLICT (email) DO UPDATE SET updated_at = now();

    INSERT INTO workspace_memberships (id, workspace_id, user_id, state)
    SELECT p_membership_id, p_workspace_id, id, 'active' FROM users WHERE email = p_email
    ON CONFLICT (workspace_id, user_id) DO UPDATE SET state = 'active', updated_at = now()
    RETURNING id INTO membership;

    INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id)
    VALUES (p_grant_id, p_workspace_id, membership, '00000000-0000-4000-8000-000000000101', 'workspace', p_workspace_id)
    ON CONFLICT (workspace_id, membership_id, role_id, scope_type, scope_target_id) DO NOTHING;
END;
$$;

ALTER TABLE workspace_memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_memberships FORCE ROW LEVEL SECURITY;
ALTER TABLE role_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE role_grants FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_memberships_workspace_policy ON workspace_memberships
    USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
CREATE POLICY role_grants_workspace_policy ON role_grants
    USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());

GRANT SELECT ON permissions, roles, role_permissions TO catalog_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON workspace_memberships, role_grants TO catalog_api;
GRANT EXECUTE ON FUNCTION validate_role_grant() TO catalog_api;
REVOKE ALL ON FUNCTION bootstrap_workspace_owner(UUID, UUID, UUID, UUID, TEXT) FROM PUBLIC;
