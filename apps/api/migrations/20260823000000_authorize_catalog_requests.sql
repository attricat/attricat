-- Authorization is evaluated in a SECURITY DEFINER function because the API
-- role is intentionally tenant-restricted by RLS. The caller must already have
-- selected the deployment workspace; the function only exposes a boolean.
CREATE FUNCTION catalog_request_principal_active(
    p_user_id UUID,
    p_workspace_id UUID
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM workspace_memberships membership
        JOIN users user_account ON user_account.id = membership.user_id
        JOIN workspaces workspace ON workspace.id = membership.workspace_id
        WHERE membership.user_id = p_user_id
          AND membership.workspace_id = p_workspace_id
          AND membership.state = 'active'
          AND user_account.state = 'active'
          AND workspace.deleted_at IS NULL
    )
$$;

CREATE FUNCTION catalog_authorize_request(
    p_user_id UUID,
    p_workspace_id UUID,
    p_permission TEXT,
    p_target_id UUID,
    p_target_code TEXT
) RETURNS BOOLEAN
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
    WITH RECURSIVE active_membership AS (
        SELECT membership.id
        FROM workspace_memberships membership
        JOIN users user_account ON user_account.id = membership.user_id
        JOIN workspaces workspace ON workspace.id = membership.workspace_id
        WHERE membership.user_id = p_user_id
          AND membership.workspace_id = p_workspace_id
          AND membership.state = 'active'
          AND user_account.state = 'active'
          AND workspace.deleted_at IS NULL
    ),
    permitted_grants AS (
        SELECT role_grant.scope_type, role_grant.scope_target_id
        FROM role_grants role_grant
        JOIN active_membership membership ON membership.id = role_grant.membership_id
        JOIN role_permissions permission ON permission.role_id = role_grant.role_id
        WHERE role_grant.workspace_id = p_workspace_id
          AND permission.permission_code = p_permission
    ),
    target AS (
        SELECT 'blueprint'::text AS kind, blueprint.id
        FROM blueprints blueprint
        WHERE blueprint.workspace_id = p_workspace_id
          AND (blueprint.id = p_target_id OR blueprint.code = p_target_code)
        UNION ALL
        SELECT 'entity', entity.id
        FROM entities entity
        WHERE entity.workspace_id = p_workspace_id AND entity.id = p_target_id
        UNION ALL
        SELECT 'context', context.id
        FROM attribute_contexts context
        WHERE context.workspace_id = p_workspace_id
          AND (context.id = p_target_id OR context.code = p_target_code)
    ),
    context_ancestors AS (
        SELECT context.id, context.parent_id
        FROM attribute_contexts context
        JOIN target ON target.kind = 'context' AND target.id = context.id
        UNION ALL
        SELECT parent.id, parent.parent_id
        FROM attribute_contexts parent
        JOIN context_ancestors child ON child.parent_id = parent.id
        WHERE parent.workspace_id = p_workspace_id
    )
    SELECT EXISTS (
        SELECT 1
        FROM permitted_grants role_grant
        WHERE (role_grant.scope_type = 'workspace'
          AND role_grant.scope_target_id = p_workspace_id)
          OR ((p_target_id IS NOT NULL OR p_target_code IS NOT NULL)
          AND (
              (role_grant.scope_type = 'blueprint_family' AND EXISTS (
                  SELECT 1 FROM target WHERE target.kind = 'blueprint' AND target.id = role_grant.scope_target_id
              ))
              OR (role_grant.scope_type = 'entity' AND EXISTS (
                  SELECT 1 FROM target WHERE target.kind = 'entity' AND target.id = role_grant.scope_target_id
              ))
              OR (role_grant.scope_type = 'blueprint_family' AND EXISTS (
                  SELECT 1 FROM entities entity JOIN target ON target.kind = 'entity' AND target.id = entity.id
                  WHERE entity.workspace_id = p_workspace_id AND entity.blueprint_id = role_grant.scope_target_id
              ))
              OR (role_grant.scope_type = 'context_subtree' AND EXISTS (
                  SELECT 1 FROM context_ancestors WHERE id = role_grant.scope_target_id
              ))
              OR (p_target_code = '__context_list__'
                  AND role_grant.scope_type IN ('workspace', 'context_subtree'))
          ))
    )
$$;

CREATE FUNCTION catalog_authorized_contexts(p_user_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, code TEXT, data JSONB, parent_id UUID)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
    SELECT context.id, context.code, context.data, context.parent_id
    FROM attribute_contexts context
    WHERE context.workspace_id = p_workspace_id
      AND catalog_authorize_request(p_user_id, p_workspace_id, 'contexts.read', context.id, NULL)
    ORDER BY context.code
$$;

REVOKE ALL ON FUNCTION catalog_request_principal_active(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_authorize_request(UUID, UUID, TEXT, UUID, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION catalog_authorized_contexts(UUID, UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION catalog_request_principal_active(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_authorize_request(UUID, UUID, TEXT, UUID, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION catalog_authorized_contexts(UUID, UUID) TO catalog_api;
