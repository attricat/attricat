-- External authentication providers identify an account by their immutable issuer
-- and subject pair. Provider claims (including email) are deliberately not an
-- account-linking key: a future adapter must explicitly choose the internal user
-- to link after it has verified the provider callback.
CREATE TABLE external_identities (
    issuer TEXT NOT NULL CHECK (issuer = btrim(issuer) AND issuer <> ''),
    subject TEXT NOT NULL CHECK (subject = btrim(subject) AND subject <> ''),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (issuer, subject)
);
CREATE INDEX external_identities_user_id_idx ON external_identities (user_id);

-- The only persistence entry point links a verified provider identifier to an
-- explicitly selected internal account. It intentionally accepts no email or
-- other mutable provider claim, and conflict is an error rather than a relink.
CREATE FUNCTION link_external_identity(
    p_user_id UUID,
    p_issuer TEXT,
    p_subject TEXT
) RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp AS $$
BEGIN
    INSERT INTO external_identities (issuer, subject, user_id)
    VALUES (p_issuer, p_subject, p_user_id);
    RETURN p_user_id;
END;
$$;

-- A callback adapter may resolve a previously linked identity by its verified
-- issuer and subject, then use this ordinary internal user ID for session and
-- RBAC evaluation. A missing row is intentionally not resolved by email.
CREATE FUNCTION find_external_identity_user(
    p_issuer TEXT,
    p_subject TEXT
) RETURNS UUID
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public, pg_temp AS $$
    SELECT user_id
    FROM external_identities
    WHERE issuer = p_issuer AND subject = p_subject
$$;

REVOKE ALL ON external_identities FROM PUBLIC;
REVOKE ALL ON external_identities FROM catalog_api;
REVOKE ALL ON FUNCTION link_external_identity(UUID, TEXT, TEXT) FROM PUBLIC;
REVOKE ALL ON FUNCTION find_external_identity_user(TEXT, TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION link_external_identity(UUID, TEXT, TEXT) TO catalog_api;
GRANT EXECUTE ON FUNCTION find_external_identity_user(TEXT, TEXT) TO catalog_api;
