-- Workspace-local roles are managed through SECURITY DEFINER entry points. This
-- keeps the central permission catalog immutable while making the authority
-- required to define or delegate a role explicit and scope-aware.
INSERT INTO permissions (code, description) VALUES
    ('roles.manage', 'Create, modify, duplicate, and retire workspace-local roles');

-- The original immutability trigger deliberately protects seeded rows even
-- from ordinary application writes. Temporarily suspend it for this catalog
-- migration, then immediately restore it.
ALTER TABLE role_permissions DISABLE TRIGGER role_permissions_protect_system;
INSERT INTO role_permissions (role_id, permission_code) VALUES
    ('00000000-0000-4000-8000-000000000101', 'roles.manage'),
    ('00000000-0000-4000-8000-000000000102', 'roles.manage');
ALTER TABLE role_permissions ENABLE TRIGGER role_permissions_protect_system;

-- Tenant-local role definitions must not leak through the otherwise global
-- role catalog. System roles remain visible in every workspace.
ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE roles FORCE ROW LEVEL SECURITY;
ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;
ALTER TABLE role_permissions FORCE ROW LEVEL SECURITY;

CREATE POLICY roles_workspace_or_system_policy ON roles
    USING (is_system OR workspace_id = catalog_workspace_id())
    WITH CHECK (NOT is_system AND workspace_id = catalog_workspace_id());
CREATE POLICY role_permissions_workspace_or_system_policy ON role_permissions
    USING (EXISTS (
        SELECT 1 FROM roles
        WHERE roles.id = role_permissions.role_id
          AND (roles.is_system OR roles.workspace_id = catalog_workspace_id())
    ))
    WITH CHECK (EXISTS (
        SELECT 1 FROM roles
        WHERE roles.id = role_permissions.role_id
          AND NOT roles.is_system
          AND roles.workspace_id = catalog_workspace_id()
    ));

CREATE FUNCTION catalog_workspace_role_permissions_are_delegable(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_role_id UUID,
    p_scope_target_id UUID
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
    SELECT NOT EXISTS (
        SELECT 1
        FROM role_permissions permission
        WHERE permission.role_id = p_role_id
          AND NOT catalog_authorize_request(
              p_actor_id,
              p_workspace_id,
              permission.permission_code,
              p_scope_target_id,
              NULL
          )
    )
$$;

CREATE FUNCTION catalog_create_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_role_id UUID,
    p_code TEXT,
    p_permission_codes TEXT[]
) RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace roles';
    END IF;
    IF p_code !~ '^[a-z][a-z0-9_-]*$' THEN
        RAISE EXCEPTION 'role code must start with a lowercase letter and contain only lowercase letters, numbers, hyphens, and underscores';
    END IF;
    IF EXISTS (
        SELECT 1 FROM unnest(p_permission_codes) permission_code
        LEFT JOIN permissions permission ON permission.code = permission_code
        WHERE permission.code IS NULL
    ) THEN
        RAISE EXCEPTION 'role contains an unknown permission';
    END IF;
    IF EXISTS (
        SELECT 1 FROM unnest(p_permission_codes) permission_code
        WHERE NOT catalog_authorize_request(p_actor_id, p_workspace_id, permission_code, NULL, NULL)
    ) THEN
        RAISE EXCEPTION 'role permissions exceed actor authority';
    END IF;

    INSERT INTO roles (id, code, workspace_id, is_system)
    VALUES (p_role_id, p_code, p_workspace_id, false);
    INSERT INTO role_permissions (role_id, permission_code)
    SELECT p_role_id, permission_code FROM unnest(p_permission_codes) permission_code;
    RETURN p_role_id;
END;
$$;

CREATE FUNCTION catalog_update_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_role_id UUID,
    p_code TEXT,
    p_permission_codes TEXT[]
) RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace roles';
    END IF;
    IF p_code !~ '^[a-z][a-z0-9_-]*$' THEN
        RAISE EXCEPTION 'role code must start with a lowercase letter and contain only lowercase letters, numbers, hyphens, and underscores';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE id = p_role_id AND workspace_id = p_workspace_id AND NOT is_system
    ) THEN
        RAISE EXCEPTION 'workspace-local role does not exist';
    END IF;
    IF EXISTS (
        SELECT 1 FROM unnest(p_permission_codes) permission_code
        LEFT JOIN permissions permission ON permission.code = permission_code
        WHERE permission.code IS NULL
    ) THEN
        RAISE EXCEPTION 'role contains an unknown permission';
    END IF;
    IF EXISTS (
        SELECT 1 FROM unnest(p_permission_codes) permission_code
        WHERE NOT catalog_authorize_request(p_actor_id, p_workspace_id, permission_code, NULL, NULL)
    ) THEN
        RAISE EXCEPTION 'role permissions exceed actor authority';
    END IF;

    UPDATE roles SET code = p_code WHERE id = p_role_id;
    DELETE FROM role_permissions WHERE role_id = p_role_id;
    INSERT INTO role_permissions (role_id, permission_code)
    SELECT p_role_id, permission_code FROM unnest(p_permission_codes) permission_code;
END;
$$;

CREATE FUNCTION catalog_duplicate_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_source_role_id UUID,
    p_new_role_id UUID,
    p_new_code TEXT
) RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    permission_codes TEXT[];
BEGIN
    SELECT coalesce(array_agg(permission_code ORDER BY permission_code), ARRAY[]::TEXT[])
    INTO permission_codes
    FROM role_permissions
    WHERE role_id = p_source_role_id;
    IF NOT EXISTS (
        SELECT 1 FROM roles
        WHERE id = p_source_role_id
          AND (is_system OR workspace_id = p_workspace_id)
    ) THEN
        RAISE EXCEPTION 'source role does not exist in workspace';
    END IF;
    RETURN catalog_create_workspace_role(
        p_actor_id, p_workspace_id, p_new_role_id, p_new_code, permission_codes
    );
END;
$$;

CREATE FUNCTION catalog_grant_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_grant_id UUID,
    p_membership_id UUID,
    p_role_id UUID,
    p_scope_type TEXT,
    p_scope_target_id UUID
) RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.grant', p_scope_target_id, NULL) THEN
        RAISE EXCEPTION 'actor may not grant roles at this scope';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM workspace_memberships
        WHERE id = p_membership_id AND workspace_id = p_workspace_id AND state = 'active'
    ) THEN
        RAISE EXCEPTION 'target membership is not active in workspace';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM roles
        WHERE id = p_role_id AND (is_system OR workspace_id = p_workspace_id)
    ) THEN
        RAISE EXCEPTION 'role does not exist in workspace';
    END IF;
    IF NOT catalog_workspace_role_permissions_are_delegable(
        p_actor_id, p_workspace_id, p_role_id, p_scope_target_id
    ) THEN
        RAISE EXCEPTION 'role permissions exceed actor authority at this scope';
    END IF;
    IF p_role_id = '00000000-0000-4000-8000-000000000101'::UUID
       AND NOT EXISTS (
           SELECT 1 FROM role_grants role_grant
           JOIN workspace_memberships membership ON membership.id = role_grant.membership_id
           WHERE role_grant.workspace_id = p_workspace_id
             AND membership.user_id = p_actor_id
             AND membership.state = 'active'
             AND role_grant.role_id = '00000000-0000-4000-8000-000000000101'::UUID
             AND role_grant.scope_type = 'workspace'
             AND role_grant.scope_target_id = p_workspace_id
       ) THEN
        RAISE EXCEPTION 'only a workspace owner may grant the owner role';
    END IF;

    INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id)
    VALUES (p_grant_id, p_workspace_id, p_membership_id, p_role_id, p_scope_type, p_scope_target_id);
    RETURN p_grant_id;
END;
$$;

CREATE FUNCTION catalog_revoke_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_grant_id UUID
) RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    grant_row role_grants%ROWTYPE;
BEGIN
    SELECT * INTO grant_row FROM role_grants
    WHERE id = p_grant_id AND workspace_id = p_workspace_id;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'role grant does not exist in workspace';
    END IF;
    IF NOT catalog_authorize_request(
        p_actor_id, p_workspace_id, 'roles.grant', grant_row.scope_target_id, NULL
    ) THEN
        RAISE EXCEPTION 'actor may not revoke roles at this scope';
    END IF;
    IF grant_row.role_id = '00000000-0000-4000-8000-000000000101'::UUID
       AND NOT EXISTS (
           SELECT 1 FROM role_grants role_grant
           JOIN workspace_memberships membership ON membership.id = role_grant.membership_id
           WHERE role_grant.workspace_id = p_workspace_id
             AND membership.user_id = p_actor_id
             AND membership.state = 'active'
             AND role_grant.role_id = '00000000-0000-4000-8000-000000000101'::UUID
             AND role_grant.scope_type = 'workspace'
             AND role_grant.scope_target_id = p_workspace_id
       ) THEN
        RAISE EXCEPTION 'only a workspace owner may revoke the owner role';
    END IF;
    DELETE FROM role_grants WHERE id = p_grant_id;
END;
$$;

CREATE FUNCTION catalog_retire_workspace_role(
    p_actor_id UUID,
    p_workspace_id UUID,
    p_role_id UUID,
    p_replacement_role_id UUID DEFAULT NULL
) RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    grant_row role_grants%ROWTYPE;
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace roles';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM roles WHERE id = p_role_id AND workspace_id = p_workspace_id AND NOT is_system
    ) THEN
        RAISE EXCEPTION 'only workspace-local roles may be retired';
    END IF;
    IF EXISTS (SELECT 1 FROM role_grants WHERE role_id = p_role_id)
       AND p_replacement_role_id IS NULL THEN
        RAISE EXCEPTION 'role has active grants; provide an explicit replacement role';
    END IF;
    IF p_replacement_role_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM roles
        WHERE id = p_replacement_role_id
          AND (is_system OR workspace_id = p_workspace_id)
    ) THEN
        RAISE EXCEPTION 'replacement role does not exist in workspace';
    END IF;

    FOR grant_row IN SELECT * FROM role_grants WHERE role_id = p_role_id LOOP
        IF NOT catalog_workspace_role_permissions_are_delegable(
            p_actor_id, p_workspace_id, p_replacement_role_id, grant_row.scope_target_id
        ) THEN
            RAISE EXCEPTION 'replacement role permissions exceed actor authority at an active grant scope';
        END IF;
    END LOOP;
    -- Remove pre-existing equivalent grants before changing role IDs so the
    -- uniqueness constraint is preserved without manufacturing grant IDs.
    DELETE FROM role_grants old_grant
    USING role_grants replacement_grant
    WHERE old_grant.role_id = p_role_id
      AND replacement_grant.role_id = p_replacement_role_id
      AND replacement_grant.workspace_id = old_grant.workspace_id
      AND replacement_grant.membership_id = old_grant.membership_id
      AND replacement_grant.scope_type = old_grant.scope_type
      AND replacement_grant.scope_target_id = old_grant.scope_target_id;
    UPDATE role_grants SET role_id = p_replacement_role_id WHERE role_id = p_role_id;
    DELETE FROM role_permissions WHERE role_id = p_role_id;
    DELETE FROM roles WHERE id = p_role_id;
END;
$$;

REVOKE ALL ON FUNCTION catalog_workspace_role_permissions_are_delegable(UUID, UUID, UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_create_workspace_role(UUID, UUID, UUID, TEXT, TEXT[]) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_update_workspace_role(UUID, UUID, UUID, TEXT, TEXT[]) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_duplicate_workspace_role(UUID, UUID, UUID, UUID, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_grant_workspace_role(UUID, UUID, UUID, UUID, UUID, TEXT, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_revoke_workspace_role(UUID, UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_retire_workspace_role(UUID, UUID, UUID, UUID) FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON role_grants FROM catalog_api;
GRANT EXECUTE ON FUNCTION catalog_workspace_role_permissions_are_delegable(UUID, UUID, UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_create_workspace_role(UUID, UUID, UUID, TEXT, TEXT[]) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_update_workspace_role(UUID, UUID, UUID, TEXT, TEXT[]) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_duplicate_workspace_role(UUID, UUID, UUID, UUID, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_grant_workspace_role(UUID, UUID, UUID, UUID, UUID, TEXT, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_revoke_workspace_role(UUID, UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_retire_workspace_role(UUID, UUID, UUID, UUID) TO catalog_api;
