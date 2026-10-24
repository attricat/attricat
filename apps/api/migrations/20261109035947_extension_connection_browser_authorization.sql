-- Browser authorization of an authorization-code connection: the provider's
-- authorization endpoint, and the one pending request the public OAuth
-- callback completes. Only the state's SHA-256 is stored, the PKCE code
-- verifier only as ciphertext bound to the row, and the browser session that
-- started the request as a SHA-256 derived from its digest: the callback
-- completes the request only from that same signed-in browser.
ALTER TABLE extension_connections
    ADD COLUMN authorization_url TEXT CHECK (char_length(authorization_url) BETWEEN 1 AND 2048),
    ADD COLUMN authorization_state_sha256 TEXT UNIQUE
        CHECK (authorization_state_sha256 ~ '^[0-9a-f]{64}$'),
    ADD COLUMN authorization_verifier_ciphertext BYTEA,
    ADD COLUMN authorization_session_sha256 BYTEA
        CHECK (octet_length(authorization_session_sha256) = 32),
    ADD COLUMN authorization_expires_at TIMESTAMPTZ,
    ADD CONSTRAINT extension_connections_authorization_request_check CHECK (
        (authorization_state_sha256 IS NULL) = (authorization_verifier_ciphertext IS NULL)
        AND (authorization_state_sha256 IS NULL) = (authorization_session_sha256 IS NULL)
        AND (authorization_state_sha256 IS NULL) = (authorization_expires_at IS NULL)
    );
