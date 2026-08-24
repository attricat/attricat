-- Local credentials and lifecycle actions are deliberately separate from provider
-- identities. The application may handle plaintext secrets, but this database
-- stores only password hashes and one-way action-token digests.
CREATE FUNCTION is_valid_argon2id_phc(p_password_hash TEXT) RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT CASE
        WHEN p_password_hash ~ '^\$argon2id\$v=19\$m=[0-9]+,t=[0-9]+,p=[0-9]+\$[A-Za-z0-9+/]{8,}\$[A-Za-z0-9+/]{16,}$'
        THEN substring(split_part(split_part(p_password_hash, '$', 4), ',', 1) FROM 3)::INTEGER >= 8192
         AND substring(split_part(split_part(p_password_hash, '$', 4), ',', 2) FROM 3)::INTEGER >= 1
         AND substring(split_part(split_part(p_password_hash, '$', 4), ',', 3) FROM 3)::INTEGER >= 1
         AND length(split_part(p_password_hash, '$', 5)) % 4 <> 1
         AND length(split_part(p_password_hash, '$', 6)) % 4 <> 1
        ELSE false
    END
$$;

ALTER TABLE users
    ADD COLUMN email_verified_at TIMESTAMPTZ,
    ADD COLUMN security_version INTEGER NOT NULL DEFAULT 1 CHECK (security_version > 0);

CREATE TABLE local_password_credentials (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL CHECK (is_valid_argon2id_phc(password_hash)),
    credential_version INTEGER NOT NULL DEFAULT 1 CHECK (credential_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_lifecycle_action_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('email_verification', 'password_setup', 'password_reset')),
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    issued_security_version INTEGER NOT NULL CHECK (issued_security_version > 0),
    issued_credential_version INTEGER NOT NULL CHECK (issued_credential_version >= 0),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (expires_at > created_at),
    CHECK (consumed_at IS NULL OR consumed_at >= created_at),
    CHECK (revoked_at IS NULL OR revoked_at >= created_at)
);
CREATE INDEX user_lifecycle_action_tokens_active_user_purpose_idx
    ON user_lifecycle_action_tokens (user_id, purpose)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

-- Security-relevant user and credential changes make all outstanding lifecycle
-- links unusable, including links that would otherwise have matching versions.
CREATE FUNCTION revoke_lifecycle_tokens_after_user_security_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state IS DISTINCT FROM NEW.state
       OR OLD.email_verified_at IS DISTINCT FROM NEW.email_verified_at
       OR OLD.security_version <> NEW.security_version THEN
        UPDATE user_lifecycle_action_tokens
        SET revoked_at = now()
        WHERE user_id = NEW.id AND consumed_at IS NULL AND revoked_at IS NULL;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER users_revoke_lifecycle_tokens_on_security_change
AFTER UPDATE OF state, email_verified_at, security_version ON users
FOR EACH ROW EXECUTE FUNCTION revoke_lifecycle_tokens_after_user_security_change();

CREATE FUNCTION revoke_lifecycle_tokens_after_credential_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
    UPDATE user_lifecycle_action_tokens
    SET revoked_at = now()
    WHERE user_id = COALESCE(NEW.user_id, OLD.user_id)
      AND consumed_at IS NULL AND revoked_at IS NULL;
    RETURN COALESCE(NEW, OLD);
END;
$$;
CREATE TRIGGER local_password_credentials_revoke_lifecycle_tokens
AFTER INSERT OR UPDATE OR DELETE ON local_password_credentials
FOR EACH ROW EXECUTE FUNCTION revoke_lifecycle_tokens_after_credential_change();

-- The account layer is granted narrow SECURITY DEFINER entry points instead of
-- table access to the global users, credential, or token tables.
CREATE FUNCTION create_local_user(
    p_user_id UUID,
    p_email TEXT,
    p_display_name TEXT DEFAULT NULL
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    INSERT INTO users (id, email, display_name) VALUES (p_user_id, p_email, p_display_name);
    RETURN p_user_id;
END;
$$;

CREATE FUNCTION find_local_password_credential(p_email TEXT)
RETURNS TABLE (
    user_id UUID,
    password_hash TEXT,
    user_state TEXT,
    email_verified_at TIMESTAMPTZ,
    security_version INTEGER,
    credential_version INTEGER
)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = public, pg_temp AS $$
    SELECT users.id, credentials.password_hash, users.state, users.email_verified_at,
           users.security_version, credentials.credential_version
    FROM users
    JOIN local_password_credentials AS credentials ON credentials.user_id = users.id
    WHERE users.email = p_email
$$;

CREATE FUNCTION issue_user_lifecycle_action_token(
    p_token_id UUID,
    p_user_id UUID,
    p_purpose TEXT,
    p_token_digest BYTEA,
    p_expires_at TIMESTAMPTZ
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
    current_security_version INTEGER;
    current_credential_version INTEGER;
BEGIN
    IF p_purpose NOT IN ('email_verification', 'password_setup', 'password_reset') THEN
        RAISE EXCEPTION 'invalid lifecycle action token purpose';
    END IF;
    IF octet_length(p_token_digest) <> 32 THEN
        RAISE EXCEPTION 'lifecycle action token digest must be 32 bytes';
    END IF;
    IF p_expires_at <= clock_timestamp() THEN
        RAISE EXCEPTION 'lifecycle action token expiry must be in the future';
    END IF;

    SELECT security_version INTO current_security_version
    FROM users WHERE id = p_user_id AND state = 'active' FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'user is not active';
    END IF;
    IF p_purpose IN ('password_setup', 'password_reset')
       AND NOT EXISTS (
           SELECT 1 FROM users
           WHERE id = p_user_id AND email_verified_at IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'password lifecycle actions require a verified email';
    END IF;
    SELECT credential_version INTO current_credential_version
    FROM local_password_credentials WHERE user_id = p_user_id FOR SHARE;
    current_credential_version := COALESCE(current_credential_version, 0);

    IF (p_purpose = 'password_setup' AND current_credential_version <> 0)
       OR (p_purpose = 'password_reset' AND current_credential_version = 0) THEN
        RAISE EXCEPTION 'lifecycle action token purpose does not match credential state';
    END IF;

    -- Only the newest link for a purpose remains usable.
    UPDATE user_lifecycle_action_tokens
    SET revoked_at = now()
    WHERE user_id = p_user_id AND purpose = p_purpose
      AND consumed_at IS NULL AND revoked_at IS NULL;

    INSERT INTO user_lifecycle_action_tokens (
        id, user_id, purpose, token_digest, issued_security_version,
        issued_credential_version, expires_at
    ) VALUES (
        p_token_id, p_user_id, p_purpose, p_token_digest, current_security_version,
        current_credential_version, p_expires_at
    );
END;
$$;

CREATE FUNCTION consume_user_lifecycle_action_token(
    p_token_digest BYTEA,
    p_purpose TEXT
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
    matched_user_id UUID;
    current_security_version INTEGER;
BEGIN
    -- Lock the user before its token. User-state changes take the same order
    -- through their token-revocation trigger, avoiding a user/token deadlock.
    SELECT user_id INTO matched_user_id
    FROM user_lifecycle_action_tokens
    WHERE token_digest = p_token_digest AND purpose = p_purpose;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'lifecycle action token is invalid';
    END IF;

    SELECT security_version INTO current_security_version
    FROM users WHERE id = matched_user_id AND state = 'active' FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'lifecycle action token is invalid';
    END IF;

    SELECT token.user_id INTO matched_user_id
    FROM user_lifecycle_action_tokens AS token
    LEFT JOIN local_password_credentials AS credentials ON credentials.user_id = token.user_id
    WHERE token.token_digest = p_token_digest
      AND token.purpose = p_purpose
      AND token.consumed_at IS NULL
      AND token.revoked_at IS NULL
      AND token.expires_at > clock_timestamp()
      AND token.issued_security_version = current_security_version
      AND COALESCE(credentials.credential_version, 0) = token.issued_credential_version
    FOR UPDATE OF token;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'lifecycle action token is invalid';
    END IF;

    UPDATE user_lifecycle_action_tokens SET consumed_at = clock_timestamp()
    WHERE token_digest = p_token_digest AND consumed_at IS NULL;
    RETURN matched_user_id;
END;
$$;

CREATE FUNCTION consume_email_verification_token(p_token_digest BYTEA) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
    verified_user_id UUID;
BEGIN
    verified_user_id := consume_user_lifecycle_action_token(p_token_digest, 'email_verification');
    UPDATE users
    SET email_verified_at = COALESCE(email_verified_at, now()),
        security_version = security_version + 1,
        updated_at = now()
    WHERE id = verified_user_id;
    RETURN verified_user_id;
END;
$$;

CREATE FUNCTION consume_password_lifecycle_token(
    p_token_digest BYTEA,
    p_purpose TEXT,
    p_password_hash TEXT
) RETURNS UUID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
DECLARE
    changed_user_id UUID;
BEGIN
    IF p_purpose NOT IN ('password_setup', 'password_reset') THEN
        RAISE EXCEPTION 'password lifecycle token purpose is invalid';
    END IF;
    IF NOT is_valid_argon2id_phc(p_password_hash) THEN
        RAISE EXCEPTION 'password hash must be an Argon2id PHC value';
    END IF;

    changed_user_id := consume_user_lifecycle_action_token(p_token_digest, p_purpose);
    INSERT INTO local_password_credentials (user_id, password_hash, credential_version)
    VALUES (changed_user_id, p_password_hash, 1)
    ON CONFLICT (user_id) DO UPDATE
    SET password_hash = EXCLUDED.password_hash,
        credential_version = local_password_credentials.credential_version + 1,
        updated_at = now();

    UPDATE users SET security_version = security_version + 1, updated_at = now()
    WHERE id = changed_user_id;
    RETURN changed_user_id;
END;
$$;

CREATE FUNCTION revoke_user_lifecycle_action_tokens(
    p_user_id UUID,
    p_purpose TEXT DEFAULT NULL
) RETURNS VOID
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public, pg_temp AS $$
BEGIN
    IF p_purpose IS NOT NULL
       AND p_purpose NOT IN ('email_verification', 'password_setup', 'password_reset') THEN
        RAISE EXCEPTION 'invalid lifecycle action token purpose';
    END IF;
    UPDATE user_lifecycle_action_tokens
    SET revoked_at = now()
    WHERE user_id = p_user_id
      AND (p_purpose IS NULL OR purpose = p_purpose)
      AND consumed_at IS NULL AND revoked_at IS NULL;
END;
$$;

-- Startup must never undo an administrator's deliberate membership suspension.
CREATE OR REPLACE FUNCTION bootstrap_workspace_owner(
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
    ON CONFLICT (workspace_id, user_id) DO NOTHING;

    SELECT workspace_memberships.id INTO membership
    FROM workspace_memberships
    JOIN users ON users.id = workspace_memberships.user_id
    WHERE workspace_memberships.workspace_id = p_workspace_id AND users.email = p_email;

    INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id)
    VALUES (p_grant_id, p_workspace_id, membership, '00000000-0000-4000-8000-000000000101', 'workspace', p_workspace_id)
    ON CONFLICT (workspace_id, membership_id, role_id, scope_type, scope_target_id) DO NOTHING;
END;
$$;

REVOKE ALL ON users, local_password_credentials, user_lifecycle_action_tokens FROM catalog_api;
REVOKE ALL ON FUNCTION is_valid_argon2id_phc(TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION is_valid_argon2id_phc(TEXT) TO catalog_api;
REVOKE ALL ON FUNCTION create_local_user(UUID, TEXT, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION find_local_password_credential(TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION issue_user_lifecycle_action_token(UUID, UUID, TEXT, BYTEA, TIMESTAMPTZ) FROM PUBLIC;
REVOKE ALL ON FUNCTION consume_user_lifecycle_action_token(BYTEA, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION consume_email_verification_token(BYTEA) FROM PUBLIC;
REVOKE ALL ON FUNCTION consume_password_lifecycle_token(BYTEA, TEXT, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION revoke_user_lifecycle_action_tokens(UUID, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION create_local_user(UUID, TEXT, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION find_local_password_credential(TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION issue_user_lifecycle_action_token(UUID, UUID, TEXT, BYTEA, TIMESTAMPTZ) TO catalog_api;
GRANT EXECUTE ON FUNCTION consume_email_verification_token(BYTEA) TO catalog_api;
GRANT EXECUTE ON FUNCTION consume_password_lifecycle_token(BYTEA, TEXT, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION revoke_user_lifecycle_action_tokens(UUID, TEXT) TO catalog_api;
REVOKE ALL ON FUNCTION bootstrap_workspace_owner(UUID, UUID, UUID, UUID, TEXT) FROM PUBLIC;
