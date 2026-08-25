-- Workspace identifiers are normalized once at provisioning and never change. They
-- are routing hints, not DNS names: no ownership or resolution is attempted.
ALTER TABLE workspaces ADD COLUMN login_identifier TEXT;
WITH candidates AS (
    SELECT id, CASE
        WHEN lower(slug) ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'
            THEN lower(slug) || '.local'
        ELSE 'workspace-' || replace(id::TEXT, '-', '') || '.local'
    END AS identifier
    FROM workspaces
    WHERE login_identifier IS NULL
), identifiers AS (
    SELECT id, CASE
        WHEN count(*) OVER (PARTITION BY identifier) = 1 THEN identifier
        ELSE 'workspace-' || replace(id::TEXT, '-', '') || '.local'
    END AS identifier
    FROM candidates
)
UPDATE workspaces AS workspace
SET login_identifier = identifiers.identifier
FROM identifiers
WHERE workspace.id = identifiers.id;
ALTER TABLE workspaces ALTER COLUMN login_identifier SET NOT NULL;
ALTER TABLE workspaces ADD CONSTRAINT workspaces_login_identifier_key UNIQUE (login_identifier);
ALTER TABLE workspaces ADD CONSTRAINT workspaces_login_identifier_format_check CHECK (
    login_identifier = lower(trim(login_identifier))
    AND char_length(login_identifier) BETWEEN 3 AND 253
    AND login_identifier ~ '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$'
);

CREATE FUNCTION prevent_workspace_login_identifier_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.login_identifier IS DISTINCT FROM OLD.login_identifier THEN
        RAISE EXCEPTION 'workspace login identifier is immutable';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER workspaces_login_identifier_immutable
BEFORE UPDATE OF login_identifier ON workspaces
FOR EACH ROW EXECUTE FUNCTION prevent_workspace_login_identifier_change();

-- Browser sessions already carry a server-issued workspace; this overload makes
-- it the source of request tenancy rather than deployment configuration.
CREATE FUNCTION validate_browser_session(p_session_digest BYTEA)
RETURNS TABLE (user_id UUID, workspace_id UUID, csrf_digest BYTEA)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT session.user_id, session.workspace_id, session.csrf_digest
    FROM browser_sessions AS session
    JOIN users ON users.id = session.user_id AND users.state = 'active'
      AND users.security_version = session.issued_security_version
    JOIN local_password_credentials AS credential ON credential.user_id = session.user_id
      AND credential.credential_version = session.issued_credential_version
    JOIN workspace_memberships AS membership ON membership.user_id = session.user_id
      AND membership.workspace_id = session.workspace_id AND membership.state = 'active'
    WHERE session.session_digest = p_session_digest
      AND session.revoked_at IS NULL AND session.expires_at > clock_timestamp()
$$;
REVOKE ALL ON FUNCTION validate_browser_session(BYTEA) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION validate_browser_session(BYTEA) TO catalog_api;

CREATE TABLE workspace_discovery_rate_limits (
    key_digest BYTEA PRIMARY KEY CHECK (octet_length(key_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0)
);

CREATE FUNCTION reserve_workspace_discovery_attempt(p_key_digest BYTEA)
RETURNS BOOLEAN LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE attempts_count INTEGER;
BEGIN
    INSERT INTO workspace_discovery_rate_limits (key_digest, window_started_at, attempts)
    VALUES (p_key_digest, clock_timestamp(), 1)
    ON CONFLICT (key_digest) DO UPDATE SET
        attempts = CASE WHEN workspace_discovery_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes'
            THEN 1 ELSE workspace_discovery_rate_limits.attempts + 1 END,
        window_started_at = CASE WHEN workspace_discovery_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes'
            THEN clock_timestamp() ELSE workspace_discovery_rate_limits.window_started_at END
    RETURNING attempts INTO attempts_count;
    RETURN attempts_count <= 20;
END;
$$;

-- Only the identifier and the methods needed to continue sign-in are exposed.
CREATE FUNCTION discover_workspace_login(p_login_identifier TEXT)
RETURNS TABLE (id UUID, login_identifier TEXT)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT id, login_identifier FROM workspaces
    WHERE login_identifier = lower(trim(p_login_identifier)) AND deleted_at IS NULL
$$;

REVOKE ALL ON workspace_discovery_rate_limits FROM catalog_api;
REVOKE ALL ON FUNCTION reserve_workspace_discovery_attempt(BYTEA) FROM PUBLIC;
REVOKE ALL ON FUNCTION discover_workspace_login(TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION reserve_workspace_discovery_attempt(BYTEA) TO catalog_api;
GRANT EXECUTE ON FUNCTION discover_workspace_login(TEXT) TO catalog_api;
