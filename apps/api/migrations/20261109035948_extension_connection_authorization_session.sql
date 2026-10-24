-- The browser session that started a pending browser authorization, as a
-- SHA-256 derived from the session digest. The public OAuth callback
-- completes the request only from that same signed-in browser.
ALTER TABLE extension_connections
    ADD COLUMN authorization_session_sha256 BYTEA
        CHECK (octet_length(authorization_session_sha256) = 32),
    ADD CONSTRAINT extension_connections_authorization_session_check CHECK (
        (authorization_state_sha256 IS NULL) = (authorization_session_sha256 IS NULL)
    );
