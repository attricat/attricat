-- Opaque personal API tokens are scoped to one workspace. Their plaintext secret
-- never enters a table: only a SHA-256 digest is retained.
CREATE TABLE personal_api_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    label TEXT NOT NULL CHECK (label = btrim(label) AND char_length(label) BETWEEN 1 AND 120),
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at IS NULL OR expires_at > created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at),
    CHECK (last_used_at IS NULL OR last_used_at >= created_at)
);
CREATE INDEX personal_api_tokens_user_workspace_idx ON personal_api_tokens (user_id, workspace_id);

CREATE TABLE personal_api_token_permissions (
    token_id UUID NOT NULL REFERENCES personal_api_tokens (id) ON DELETE CASCADE,
    permission_code TEXT NOT NULL REFERENCES permissions (code) ON DELETE RESTRICT,
    PRIMARY KEY (token_id, permission_code)
);

CREATE FUNCTION issue_personal_api_token(
    p_token_id UUID, p_user_id UUID, p_workspace_id UUID, p_label TEXT,
    p_token_digest BYTEA, p_permissions TEXT[], p_expires_at TIMESTAMPTZ DEFAULT NULL
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    IF p_label IS NULL OR p_label <> btrim(p_label) OR char_length(p_label) NOT BETWEEN 1 AND 120 THEN
        RAISE EXCEPTION 'token label must be between 1 and 120 trimmed characters';
    END IF;
    IF octet_length(p_token_digest) <> 32 THEN RAISE EXCEPTION 'token digest must be 32 bytes'; END IF;
    IF p_expires_at IS NOT NULL AND p_expires_at <= clock_timestamp() THEN
        RAISE EXCEPTION 'token expiry must be in the future';
    END IF;
    IF cardinality(p_permissions) IS NULL OR cardinality(p_permissions) = 0
       OR cardinality(p_permissions) <> (SELECT count(DISTINCT permission_code) FROM unnest(p_permissions) permission_code) THEN
        RAISE EXCEPTION 'token permissions must be a non-empty unique subset';
    END IF;
    IF EXISTS (SELECT 1 FROM unnest(p_permissions) permission_code
               WHERE NOT EXISTS (SELECT 1 FROM permissions WHERE code = permission_code)) THEN
        RAISE EXCEPTION 'token permissions contain an unknown permission';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id = m.user_id
                   JOIN workspaces w ON w.id = m.workspace_id
                   WHERE m.user_id = p_user_id AND m.workspace_id = p_workspace_id
                     AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL) THEN
        RAISE EXCEPTION 'user is not an active workspace member';
    END IF;
    -- A token can further reduce its owner's authority, never add authority.
    IF EXISTS (
        SELECT 1 FROM unnest(p_permissions) requested(permission_code)
        WHERE NOT EXISTS (
            SELECT 1 FROM workspace_memberships membership
            JOIN role_grants grant ON grant.membership_id = membership.id
              AND grant.workspace_id = membership.workspace_id
            JOIN role_permissions permission ON permission.role_id = grant.role_id
            WHERE membership.user_id = p_user_id AND membership.workspace_id = p_workspace_id
              AND membership.state = 'active' AND permission.permission_code = requested.permission_code
        )
    ) THEN RAISE EXCEPTION 'token permissions must be a subset of the user permissions'; END IF;
    INSERT INTO personal_api_tokens (id, user_id, workspace_id, label, token_digest, expires_at)
    VALUES (p_token_id, p_user_id, p_workspace_id, p_label, p_token_digest, p_expires_at);
    INSERT INTO personal_api_token_permissions (token_id, permission_code)
    SELECT p_token_id, permission_code FROM unnest(p_permissions) permission_code;
END;
$$;

CREATE FUNCTION authenticate_personal_api_token(p_token_digest BYTEA)
RETURNS TABLE (token_id UUID, user_id UUID, workspace_id UUID)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    RETURN QUERY
    UPDATE personal_api_tokens token SET last_used_at = clock_timestamp()
    FROM workspace_memberships membership, users user_account, workspaces workspace
    WHERE token.token_digest = p_token_digest AND token.revoked_at IS NULL
      AND (token.expires_at IS NULL OR token.expires_at > clock_timestamp())
      AND membership.user_id = token.user_id AND membership.workspace_id = token.workspace_id
      AND membership.state = 'active' AND user_account.id = token.user_id
      AND user_account.state = 'active' AND workspace.id = token.workspace_id
      AND workspace.deleted_at IS NULL
    RETURNING token.id, token.user_id, token.workspace_id;
END;
$$;

CREATE FUNCTION personal_api_token_permits(p_token_id UUID, p_permission_code TEXT) RETURNS BOOLEAN
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT EXISTS (SELECT 1 FROM personal_api_token_permissions
                   WHERE token_id = p_token_id AND permission_code = p_permission_code)
$$;

CREATE FUNCTION list_personal_api_tokens(p_user_id UUID, p_workspace_id UUID)
RETURNS TABLE (id UUID, label TEXT, permissions TEXT[], expires_at TIMESTAMPTZ,
               revoked_at TIMESTAMPTZ, last_used_at TIMESTAMPTZ, created_at TIMESTAMPTZ)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT token.id, token.label, array_agg(permission.permission_code ORDER BY permission.permission_code),
           token.expires_at, token.revoked_at, token.last_used_at, token.created_at
    FROM personal_api_tokens token JOIN personal_api_token_permissions permission ON permission.token_id = token.id
    WHERE token.user_id = p_user_id AND token.workspace_id = p_workspace_id
    GROUP BY token.id ORDER BY token.created_at DESC
$$;

CREATE FUNCTION revoke_personal_api_token(p_token_id UUID, p_user_id UUID, p_workspace_id UUID) RETURNS BOOLEAN
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    UPDATE personal_api_tokens SET revoked_at = COALESCE(revoked_at, clock_timestamp())
    WHERE id = p_token_id AND user_id = p_user_id AND workspace_id = p_workspace_id;
    RETURN FOUND;
END;
$$;

REVOKE ALL ON personal_api_tokens, personal_api_token_permissions FROM catalog_api;
REVOKE ALL ON FUNCTION issue_personal_api_token(UUID, UUID, UUID, TEXT, BYTEA, TEXT[], TIMESTAMPTZ) FROM PUBLIC;
REVOKE ALL ON FUNCTION authenticate_personal_api_token(BYTEA) FROM PUBLIC;
REVOKE ALL ON FUNCTION personal_api_token_permits(UUID, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION list_personal_api_tokens(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION revoke_personal_api_token(UUID, UUID, UUID) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION issue_personal_api_token(UUID, UUID, UUID, TEXT, BYTEA, TEXT[], TIMESTAMPTZ) TO catalog_api;
GRANT EXECUTE ON FUNCTION authenticate_personal_api_token(BYTEA) TO catalog_api;
GRANT EXECUTE ON FUNCTION personal_api_token_permits(UUID, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION list_personal_api_tokens(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION revoke_personal_api_token(UUID, UUID, UUID) TO catalog_api;
