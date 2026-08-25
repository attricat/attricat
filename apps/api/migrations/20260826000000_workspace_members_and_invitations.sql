-- Workspace administration is exposed through narrow database entry points.  In
-- particular, the API role cannot read the global users table or invitation
-- token digests directly.
CREATE TABLE workspace_invitations (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    invitee_email TEXT NOT NULL CHECK (invitee_email = lower(btrim(invitee_email)) AND position('@' IN invitee_email) > 1),
    inviter_user_id UUID NOT NULL REFERENCES users (id),
    role_id UUID NOT NULL REFERENCES roles (id) ON DELETE RESTRICT,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('workspace', 'blueprint_family', 'entity', 'context_subtree')),
    scope_target_id UUID NOT NULL,
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_by_user_id UUID REFERENCES users (id),
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (expires_at > created_at),
    CHECK (accepted_by_user_id IS NULL OR accepted_at IS NOT NULL)
);
CREATE INDEX workspace_invitations_workspace_active_idx
    ON workspace_invitations (workspace_id, created_at DESC)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE FUNCTION catalog_validate_invitation_scope(
    p_workspace_id UUID, p_role_id UUID, p_scope_type TEXT, p_scope_target_id UUID
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE role_workspace UUID;
BEGIN
    SELECT workspace_id INTO role_workspace FROM roles WHERE id = p_role_id;
    IF NOT FOUND OR (role_workspace IS NOT NULL AND role_workspace <> p_workspace_id) THEN
        RAISE EXCEPTION 'role does not belong to invitation workspace';
    END IF;
    IF p_scope_type = 'workspace' THEN
        IF p_scope_target_id <> p_workspace_id THEN RAISE EXCEPTION 'workspace scope must target the workspace'; END IF;
    ELSIF p_scope_type = 'blueprint_family' THEN
        PERFORM 1 FROM blueprints WHERE id = p_scope_target_id AND workspace_id = p_workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'blueprint family does not belong to invitation workspace'; END IF;
    ELSIF p_scope_type = 'entity' THEN
        PERFORM 1 FROM entities WHERE id = p_scope_target_id AND workspace_id = p_workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'entity does not belong to invitation workspace'; END IF;
    ELSIF p_scope_type = 'context_subtree' THEN
        PERFORM 1 FROM attribute_contexts WHERE id = p_scope_target_id AND workspace_id = p_workspace_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'context does not belong to invitation workspace'; END IF;
    ELSE
        RAISE EXCEPTION 'invalid invitation scope';
    END IF;
    IF p_role_id = '00000000-0000-4000-8000-000000000101'::uuid
       AND (p_scope_type <> 'workspace' OR p_scope_target_id <> p_workspace_id) THEN
        RAISE EXCEPTION 'owner invitations must be workspace scoped';
    END IF;
END;
$$;

CREATE FUNCTION catalog_create_workspace_invitation(
    p_id UUID, p_actor_id UUID, p_workspace_id UUID, p_email TEXT, p_role_id UUID,
    p_scope_type TEXT, p_scope_target_id UUID, p_token_digest BYTEA, p_expires_at TIMESTAMPTZ
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL) THEN
        RAISE EXCEPTION 'actor may not manage workspace members';
    END IF;
    IF p_email <> lower(btrim(p_email)) OR position('@' IN p_email) <= 1 THEN RAISE EXCEPTION 'invitee email must be normalized'; END IF;
    IF octet_length(p_token_digest) <> 32 OR p_expires_at <= clock_timestamp() THEN RAISE EXCEPTION 'invitation digest or expiry is invalid'; END IF;
    PERFORM catalog_validate_invitation_scope(p_workspace_id, p_role_id, p_scope_type, p_scope_target_id);
    IF NOT catalog_workspace_role_permissions_are_delegable(p_actor_id, p_workspace_id, p_role_id, p_scope_target_id) THEN
        RAISE EXCEPTION 'role permissions exceed actor authority at this scope';
    END IF;
    IF p_role_id = '00000000-0000-4000-8000-000000000101'::uuid
       AND NOT EXISTS (
           SELECT 1 FROM role_grants g JOIN workspace_memberships m ON m.id = g.membership_id
           WHERE g.workspace_id = p_workspace_id AND m.user_id = p_actor_id AND m.state = 'active'
             AND g.role_id = '00000000-0000-4000-8000-000000000101'::uuid
             AND g.scope_type = 'workspace' AND g.scope_target_id = p_workspace_id
       ) THEN RAISE EXCEPTION 'only an active owner may invite an owner'; END IF;
    INSERT INTO workspace_invitations (id, workspace_id, invitee_email, inviter_user_id, role_id, scope_type, scope_target_id, token_digest, expires_at)
    VALUES (p_id, p_workspace_id, p_email, p_actor_id, p_role_id, p_scope_type, p_scope_target_id, p_token_digest, p_expires_at);
END;
$$;

CREATE FUNCTION catalog_accept_workspace_invitation(
    p_token_digest BYTEA, p_user_id UUID, p_membership_id UUID, p_grant_id UUID
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE invitation workspace_invitations%ROWTYPE; membership UUID;
BEGIN
    IF octet_length(p_token_digest) <> 32 THEN RETURN NULL; END IF;
    SELECT * INTO invitation FROM workspace_invitations WHERE token_digest = p_token_digest FOR UPDATE;
    IF NOT FOUND OR invitation.accepted_at IS NOT NULL OR invitation.revoked_at IS NOT NULL OR invitation.expires_at <= clock_timestamp() THEN
        RETURN NULL;
    END IF;
    -- A matching address alone is insufficient: verification happens before a
    -- membership can be activated.
    PERFORM 1 FROM users WHERE id = p_user_id AND state = 'active'
        AND email = invitation.invitee_email AND email_verified_at IS NOT NULL FOR UPDATE;
    IF NOT FOUND THEN RETURN NULL; END IF;
    INSERT INTO workspace_memberships (id, workspace_id, user_id, state)
    VALUES (p_membership_id, invitation.workspace_id, p_user_id, 'active')
    ON CONFLICT (workspace_id, user_id) DO UPDATE SET state = 'active', updated_at = clock_timestamp()
    RETURNING id INTO membership;
    INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id)
    VALUES (p_grant_id, invitation.workspace_id, membership, invitation.role_id, invitation.scope_type, invitation.scope_target_id)
    ON CONFLICT (workspace_id, membership_id, role_id, scope_type, scope_target_id) DO NOTHING;
    UPDATE workspace_invitations SET accepted_at = clock_timestamp(), accepted_by_user_id = p_user_id WHERE id = invitation.id;
    RETURN membership;
END;
$$;

CREATE FUNCTION catalog_revoke_workspace_invitation(p_id UUID, p_actor_id UUID, p_workspace_id UUID)
RETURNS BOOLEAN LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL) THEN RAISE EXCEPTION 'actor may not manage workspace members'; END IF;
    UPDATE workspace_invitations SET revoked_at = clock_timestamp()
    WHERE id = p_id AND workspace_id = p_workspace_id AND accepted_at IS NULL AND revoked_at IS NULL;
    RETURN FOUND;
END;
$$;

CREATE FUNCTION catalog_set_workspace_membership_state(p_id UUID, p_actor_id UUID, p_workspace_id UUID, p_state TEXT)
RETURNS BOOLEAN LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    IF p_state NOT IN ('active', 'inactive') THEN RAISE EXCEPTION 'invalid membership state'; END IF;
    IF NOT catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL) THEN RAISE EXCEPTION 'actor may not manage workspace members'; END IF;
    UPDATE workspace_memberships SET state = p_state, updated_at = clock_timestamp() WHERE id = p_id AND workspace_id = p_workspace_id;
    RETURN FOUND;
END;
$$;

CREATE FUNCTION catalog_transfer_workspace_ownership(p_actor_id UUID, p_workspace_id UUID, p_target_membership_id UUID, p_grant_id UUID)
RETURNS VOID LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    PERFORM 1 FROM workspaces WHERE id = p_workspace_id FOR UPDATE;
    -- Only an active workspace owner may transfer ownership.
    IF NOT EXISTS (SELECT 1 FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id
        WHERE m.workspace_id = p_workspace_id AND m.user_id = p_actor_id AND m.state = 'active'
          AND g.workspace_id = p_workspace_id AND g.role_id = '00000000-0000-4000-8000-000000000101'::uuid
          AND g.scope_type = 'workspace' AND g.scope_target_id = p_workspace_id) THEN RAISE EXCEPTION 'only an active owner may transfer ownership'; END IF;
    PERFORM 1 FROM workspace_memberships WHERE id = p_target_membership_id AND workspace_id = p_workspace_id AND state = 'active';
    IF NOT FOUND THEN RAISE EXCEPTION 'ownership target must be an active workspace member'; END IF;
    -- Transferring to one of the caller's memberships is already satisfied and
    -- must not delete the sole owner grant.
    IF EXISTS (SELECT 1 FROM workspace_memberships WHERE id = p_target_membership_id AND user_id = p_actor_id) THEN RETURN; END IF;
    INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id)
    VALUES (p_grant_id, p_workspace_id, p_target_membership_id, '00000000-0000-4000-8000-000000000101', 'workspace', p_workspace_id)
    ON CONFLICT (workspace_id, membership_id, role_id, scope_type, scope_target_id) DO NOTHING;
    DELETE FROM role_grants WHERE workspace_id = p_workspace_id AND membership_id IN (
        SELECT id FROM workspace_memberships WHERE workspace_id = p_workspace_id AND user_id = p_actor_id
    ) AND role_id = '00000000-0000-4000-8000-000000000101'::uuid AND scope_type = 'workspace' AND scope_target_id = p_workspace_id;
END;
$$;

CREATE FUNCTION list_workspace_members(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, user_id UUID, email TEXT, display_name TEXT, state TEXT, created_at TIMESTAMPTZ, updated_at TIMESTAMPTZ, grants JSONB)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT m.id, m.user_id, u.email, u.display_name, m.state, m.created_at, m.updated_at,
      COALESCE(jsonb_agg(jsonb_build_object('id', g.id, 'role_id', g.role_id, 'role_code', r.code, 'scope_type', g.scope_type, 'scope_target_id', g.scope_target_id) ORDER BY g.created_at) FILTER (WHERE g.id IS NOT NULL), '[]'::jsonb)
    FROM workspace_memberships m JOIN users u ON u.id = m.user_id
    LEFT JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id
    LEFT JOIN roles r ON r.id = g.role_id
    WHERE m.workspace_id = p_workspace_id AND catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL)
    GROUP BY m.id, u.id ORDER BY u.email
$$;

CREATE FUNCTION list_workspace_invitations(p_actor_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, invitee_email TEXT, inviter_user_id UUID, inviter_email TEXT, role_id UUID, role_code TEXT, scope_type TEXT, scope_target_id UUID, expires_at TIMESTAMPTZ, accepted_at TIMESTAMPTZ, accepted_by_user_id UUID, revoked_at TIMESTAMPTZ, created_at TIMESTAMPTZ)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT i.id, i.invitee_email, i.inviter_user_id, u.email, i.role_id, r.code, i.scope_type, i.scope_target_id, i.expires_at, i.accepted_at, i.accepted_by_user_id, i.revoked_at, i.created_at
    FROM workspace_invitations i JOIN users u ON u.id = i.inviter_user_id JOIN roles r ON r.id = i.role_id
    WHERE i.workspace_id = p_workspace_id AND catalog_authorize_request(p_actor_id, p_workspace_id, 'members.manage', NULL, NULL)
    ORDER BY i.created_at DESC
$$;

ALTER TABLE workspace_invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_invitations FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_invitations_workspace_policy ON workspace_invitations USING (workspace_id = catalog_workspace_id()) WITH CHECK (workspace_id = catalog_workspace_id());
REVOKE ALL ON workspace_invitations FROM catalog_api;
REVOKE ALL ON FUNCTION catalog_validate_invitation_scope(UUID, UUID, TEXT, UUID), catalog_create_workspace_invitation(UUID, UUID, UUID, TEXT, UUID, TEXT, UUID, BYTEA, TIMESTAMPTZ), catalog_accept_workspace_invitation(BYTEA, UUID, UUID, UUID), catalog_revoke_workspace_invitation(UUID, UUID, UUID), catalog_set_workspace_membership_state(UUID, UUID, UUID, TEXT), catalog_transfer_workspace_ownership(UUID, UUID, UUID, UUID), list_workspace_members(UUID, UUID), list_workspace_invitations(UUID, UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION catalog_create_workspace_invitation(UUID, UUID, UUID, TEXT, UUID, TEXT, UUID, BYTEA, TIMESTAMPTZ), catalog_accept_workspace_invitation(BYTEA, UUID, UUID, UUID), catalog_revoke_workspace_invitation(UUID, UUID, UUID), catalog_set_workspace_membership_state(UUID, UUID, UUID, TEXT), catalog_transfer_workspace_ownership(UUID, UUID, UUID, UUID), list_workspace_members(UUID, UUID), list_workspace_invitations(UUID, UUID) TO catalog_api;
