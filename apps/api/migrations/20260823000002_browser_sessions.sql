-- Browser sessions retain only digests of opaque delivery secrets. The API role
-- reaches them through these narrowly scoped functions, never table access.
CREATE TABLE browser_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    session_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(session_digest) = 32),
    csrf_digest BYTEA NOT NULL CHECK (octet_length(csrf_digest) = 32),
    issued_security_version INTEGER NOT NULL CHECK (issued_security_version > 0),
    issued_credential_version INTEGER NOT NULL CHECK (issued_credential_version > 0),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at)
);
CREATE INDEX browser_sessions_active_user_workspace_idx
    ON browser_sessions (user_id, workspace_id)
    WHERE revoked_at IS NULL;

CREATE TABLE browser_login_rate_limits (
    key_digest BYTEA PRIMARY KEY CHECK (octet_length(key_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0)
);

CREATE FUNCTION revoke_browser_sessions(p_user_id UUID, p_workspace_id UUID DEFAULT NULL)
RETURNS VOID LANGUAGE sql SECURITY DEFINER SET search_path = public, pg_temp AS $$
    UPDATE browser_sessions SET revoked_at = clock_timestamp()
    WHERE user_id = p_user_id AND (p_workspace_id IS NULL OR workspace_id = p_workspace_id)
      AND revoked_at IS NULL
$$;

-- Login passes the versions observed while verifying the password. Locking the
-- user before its credential matches password-reset lock ordering and makes a
-- stale verification fail rather than creating a post-reset session.
CREATE FUNCTION issue_browser_login_session(
    p_id UUID, p_user_id UUID, p_workspace_id UUID,
    p_expected_security_version INTEGER, p_expected_credential_version INTEGER,
    p_session_digest BYTEA, p_csrf_digest BYTEA, p_expires_at TIMESTAMPTZ
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE current_security_version INTEGER; current_credential_version INTEGER;
BEGIN
    IF octet_length(p_session_digest) <> 32 OR octet_length(p_csrf_digest) <> 32 THEN
        RAISE EXCEPTION 'browser session digest must be 32 bytes';
    END IF;
    IF p_expires_at <= clock_timestamp() THEN
        RAISE EXCEPTION 'browser session expiry must be in the future';
    END IF;

    SELECT security_version INTO current_security_version
    FROM users WHERE id = p_user_id AND state = 'active' FOR UPDATE;
    IF NOT FOUND OR current_security_version <> p_expected_security_version THEN
        RAISE EXCEPTION 'browser session credential is stale';
    END IF;
    SELECT credential_version INTO current_credential_version
    FROM local_password_credentials WHERE user_id = p_user_id FOR SHARE;
    IF NOT FOUND OR current_credential_version <> p_expected_credential_version THEN
        RAISE EXCEPTION 'browser session credential is stale';
    END IF;
    PERFORM 1 FROM workspace_memberships
     WHERE user_id = p_user_id AND workspace_id = p_workspace_id AND state = 'active' FOR SHARE;
    IF NOT FOUND THEN RAISE EXCEPTION 'browser session membership is not active'; END IF;

    -- Revocation and creation are one transaction, so concurrent successful
    -- logins serialize on the user row and leave only the later session valid.
    PERFORM revoke_browser_sessions(p_user_id, p_workspace_id);
    INSERT INTO browser_sessions (
        id, user_id, workspace_id, session_digest, csrf_digest,
        issued_security_version, issued_credential_version, expires_at
    ) VALUES (
        p_id, p_user_id, p_workspace_id, p_session_digest, p_csrf_digest,
        current_security_version, current_credential_version, p_expires_at
    );
END;
$$;

CREATE FUNCTION validate_browser_session(p_session_digest BYTEA, p_workspace_id UUID)
RETURNS TABLE (user_id UUID, csrf_digest BYTEA, expires_at TIMESTAMPTZ)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT session.user_id, session.csrf_digest, session.expires_at
    FROM browser_sessions AS session
    JOIN users ON users.id = session.user_id AND users.state = 'active'
      AND users.security_version = session.issued_security_version
    JOIN local_password_credentials AS credential ON credential.user_id = session.user_id
      AND credential.credential_version = session.issued_credential_version
    JOIN workspace_memberships AS membership ON membership.user_id = session.user_id
      AND membership.workspace_id = session.workspace_id AND membership.state = 'active'
    WHERE session.session_digest = p_session_digest AND session.workspace_id = p_workspace_id
      AND session.revoked_at IS NULL AND session.expires_at > clock_timestamp()
$$;

CREATE FUNCTION rotate_browser_session(
    p_previous_digest BYTEA, p_id UUID, p_session_digest BYTEA, p_csrf_digest BYTEA,
    p_workspace_id UUID, p_expires_at TIMESTAMPTZ
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE p_user_id UUID; current_security_version INTEGER; current_credential_version INTEGER;
BEGIN
    SELECT session.user_id, users.security_version, credential.credential_version
    INTO p_user_id, current_security_version, current_credential_version
    FROM browser_sessions AS session
    JOIN users ON users.id = session.user_id AND users.state = 'active'
    JOIN local_password_credentials AS credential ON credential.user_id = session.user_id
    WHERE session.session_digest = p_previous_digest AND session.workspace_id = p_workspace_id
      AND session.revoked_at IS NULL AND session.expires_at > clock_timestamp()
      AND session.issued_security_version = users.security_version
      AND session.issued_credential_version = credential.credential_version
    FOR UPDATE OF session, users, credential;
    IF NOT FOUND THEN RAISE EXCEPTION 'browser session is invalid'; END IF;
    UPDATE browser_sessions SET revoked_at = clock_timestamp(), rotated_at = clock_timestamp()
    WHERE session_digest = p_previous_digest AND revoked_at IS NULL;
    INSERT INTO browser_sessions (
        id, user_id, workspace_id, session_digest, csrf_digest,
        issued_security_version, issued_credential_version, expires_at
    ) VALUES (
        p_id, p_user_id, p_workspace_id, p_session_digest, p_csrf_digest,
        current_security_version, current_credential_version, p_expires_at
    );
    RETURN p_user_id;
END;
$$;

CREATE FUNCTION revoke_browser_session(p_session_digest BYTEA, p_workspace_id UUID)
RETURNS VOID LANGUAGE sql SECURITY DEFINER SET search_path = public, pg_temp AS $$
    UPDATE browser_sessions SET revoked_at = clock_timestamp()
    WHERE session_digest = p_session_digest AND workspace_id = p_workspace_id AND revoked_at IS NULL
$$;

-- Each request reserves one attempt under the row lock. Invalid passwords keep
-- the reservation; a successful login clears it after password verification.
CREATE FUNCTION reserve_browser_login_attempt(p_key_digest BYTEA)
RETURNS BOOLEAN LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE attempt_failures INTEGER;
BEGIN
    INSERT INTO browser_login_rate_limits (key_digest, window_started_at, failures)
    VALUES (p_key_digest, clock_timestamp(), 1)
    ON CONFLICT (key_digest) DO UPDATE SET
      failures = CASE WHEN browser_login_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes'
                      THEN 1 ELSE browser_login_rate_limits.failures + 1 END,
      window_started_at = CASE WHEN browser_login_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes'
                               THEN clock_timestamp() ELSE browser_login_rate_limits.window_started_at END
    RETURNING failures INTO attempt_failures;
    RETURN attempt_failures <= 5;
END;
$$;
CREATE FUNCTION clear_browser_login_failures(p_key_digest BYTEA)
RETURNS VOID LANGUAGE sql SECURITY DEFINER SET search_path = public, pg_temp AS $$
    DELETE FROM browser_login_rate_limits WHERE key_digest = p_key_digest
$$;

CREATE FUNCTION revoke_browser_sessions_after_user_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state IS DISTINCT FROM NEW.state OR OLD.security_version <> NEW.security_version THEN
      PERFORM revoke_browser_sessions(NEW.id);
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER users_revoke_browser_sessions_on_security_change
AFTER UPDATE OF state, security_version ON users FOR EACH ROW
EXECUTE FUNCTION revoke_browser_sessions_after_user_change();
CREATE FUNCTION revoke_browser_sessions_after_credential_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM revoke_browser_sessions(COALESCE(NEW.user_id, OLD.user_id));
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER credentials_revoke_browser_sessions
AFTER INSERT OR UPDATE OR DELETE ON local_password_credentials FOR EACH ROW
EXECUTE FUNCTION revoke_browser_sessions_after_credential_change();
CREATE FUNCTION revoke_browser_sessions_after_membership_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR OLD.state IS DISTINCT FROM NEW.state THEN
      PERFORM revoke_browser_sessions(OLD.user_id, OLD.workspace_id);
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER memberships_revoke_browser_sessions
AFTER UPDATE OF state OR DELETE ON workspace_memberships FOR EACH ROW
EXECUTE FUNCTION revoke_browser_sessions_after_membership_change();
CREATE FUNCTION revoke_browser_sessions_after_role_grant_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
DECLARE affected_user UUID; affected_workspace UUID;
BEGIN
    SELECT user_id INTO affected_user FROM workspace_memberships WHERE id = COALESCE(NEW.membership_id, OLD.membership_id);
    affected_workspace := COALESCE(NEW.workspace_id, OLD.workspace_id);
    IF affected_user IS NOT NULL THEN PERFORM revoke_browser_sessions(affected_user, affected_workspace); END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER role_grants_revoke_browser_sessions
AFTER INSERT OR UPDATE OR DELETE ON role_grants FOR EACH ROW
EXECUTE FUNCTION revoke_browser_sessions_after_role_grant_change();

REVOKE ALL ON browser_sessions, browser_login_rate_limits FROM catalog_api;
REVOKE ALL ON FUNCTION issue_browser_login_session(UUID, UUID, UUID, INTEGER, INTEGER, BYTEA, BYTEA, TIMESTAMPTZ) FROM PUBLIC;
REVOKE ALL ON FUNCTION validate_browser_session(BYTEA, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION rotate_browser_session(BYTEA, UUID, BYTEA, BYTEA, UUID, TIMESTAMPTZ) FROM PUBLIC;
REVOKE ALL ON FUNCTION revoke_browser_session(BYTEA, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION revoke_browser_sessions(UUID, UUID) FROM PUBLIC;
REVOKE ALL ON FUNCTION reserve_browser_login_attempt(BYTEA) FROM PUBLIC;
REVOKE ALL ON FUNCTION clear_browser_login_failures(BYTEA) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION issue_browser_login_session(UUID, UUID, UUID, INTEGER, INTEGER, BYTEA, BYTEA, TIMESTAMPTZ) TO catalog_api;
GRANT EXECUTE ON FUNCTION validate_browser_session(BYTEA, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION rotate_browser_session(BYTEA, UUID, BYTEA, BYTEA, UUID, TIMESTAMPTZ) TO catalog_api;
GRANT EXECUTE ON FUNCTION revoke_browser_session(BYTEA, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION revoke_browser_sessions(UUID, UUID) TO catalog_api;
GRANT EXECUTE ON FUNCTION reserve_browser_login_attempt(BYTEA) TO catalog_api;
GRANT EXECUTE ON FUNCTION clear_browser_login_failures(BYTEA) TO catalog_api;
