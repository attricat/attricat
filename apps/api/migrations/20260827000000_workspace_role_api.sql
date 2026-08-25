-- Read entry points use the permission that owns each management operation:
-- custom-role lifecycle, member/invitation assignment, or personal tokens.
CREATE FUNCTION list_workspace_roles(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, code TEXT, is_system BOOLEAN, permissions TEXT[], created_at TIMESTAMPTZ)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace roles';
    END IF;
    RETURN QUERY
    SELECT role.id, role.code, role.is_system,
           coalesce(array_agg(permission.permission_code ORDER BY permission.permission_code)
                    FILTER (WHERE permission.permission_code IS NOT NULL), ARRAY[]::TEXT[]),
           role.created_at
    FROM roles role
    LEFT JOIN role_permissions permission ON permission.role_id = role.id
    WHERE role.is_system OR role.workspace_id = p_workspace_id
    GROUP BY role.id
    ORDER BY role.is_system DESC, role.code;
END;
$$;

CREATE FUNCTION list_workspace_assignable_roles(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, code TEXT, is_system BOOLEAN, permissions TEXT[], created_at TIMESTAMPTZ)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace members';
    END IF;
    RETURN QUERY
    SELECT role.id, role.code, role.is_system,
           coalesce(array_agg(permission.permission_code ORDER BY permission.permission_code)
                    FILTER (WHERE permission.permission_code IS NOT NULL), ARRAY[]::TEXT[]),
           role.created_at
    FROM roles role
    LEFT JOIN role_permissions permission ON permission.role_id = role.id
    WHERE role.is_system OR role.workspace_id = p_workspace_id
    GROUP BY role.id
    ORDER BY role.is_system DESC, role.code;
END;
$$;

CREATE FUNCTION list_workspace_permissions(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (code TEXT, description TEXT)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'roles.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace roles';
    END IF;
    RETURN QUERY SELECT permission.code, permission.description FROM permissions permission ORDER BY permission.code;
END;
$$;

CREATE FUNCTION list_workspace_token_permissions(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (code TEXT, description TEXT)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'tokens.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage personal API tokens';
    END IF;
    RETURN QUERY
    SELECT permission.code, permission.description
    FROM permissions permission
    WHERE EXISTS (
        SELECT 1
        FROM workspace_memberships membership
        JOIN role_grants grant ON grant.membership_id = membership.id
          AND grant.workspace_id = membership.workspace_id
        JOIN role_permissions role_permission ON role_permission.role_id = grant.role_id
        WHERE membership.user_id = p_actor_id
          AND membership.workspace_id = p_workspace_id
          AND membership.state = 'active'
          AND role_permission.permission_code = permission.code
    )
    ORDER BY permission.code;
END;
$$;

CREATE FUNCTION list_workspace_grant_targets(p_actor_id UUID, p_workspace_id UUID, p_scope_type TEXT)
RETURNS TABLE (id UUID, label TEXT)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace members';
    END IF;
    IF p_scope_type = 'workspace' THEN
        RETURN QUERY SELECT workspace.id, workspace.name FROM workspaces workspace WHERE workspace.id = p_workspace_id;
    ELSIF p_scope_type = 'blueprint_family' THEN
        RETURN QUERY SELECT blueprint.id, blueprint.code FROM blueprints blueprint WHERE blueprint.workspace_id = p_workspace_id ORDER BY blueprint.code;
    ELSIF p_scope_type = 'context_subtree' THEN
        RETURN QUERY SELECT context.id, context.code FROM attribute_contexts context WHERE context.workspace_id = p_workspace_id ORDER BY context.code;
    ELSIF p_scope_type = 'entity' THEN
        RETURN QUERY SELECT entity.id, entity.id::TEXT FROM entities entity WHERE entity.workspace_id = p_workspace_id ORDER BY entity.id;
    ELSE
        RAISE EXCEPTION 'invalid grant scope type';
    END IF;
END;
$$;

REVOKE ALL ON FUNCTION list_workspace_roles(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION list_workspace_assignable_roles(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION list_workspace_permissions(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION list_workspace_token_permissions(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION list_workspace_grant_targets(UUID, UUID, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION list_workspace_roles(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION list_workspace_assignable_roles(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION list_workspace_permissions(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION list_workspace_token_permissions(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION list_workspace_grant_targets(UUID, UUID, TEXT) TO catalog_api;
