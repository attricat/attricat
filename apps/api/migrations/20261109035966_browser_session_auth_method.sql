-- A session records how its user signed in and when. Only a password session
-- depends on a credential version; an external sign-in has no local credential.
ALTER TABLE browser_sessions
    ADD COLUMN auth_method TEXT NOT NULL DEFAULT 'local_password'
        CHECK (auth_method IN ('local_password', 'oidc')),
    ADD COLUMN authenticated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ALTER COLUMN issued_credential_version DROP NOT NULL,
    ADD CONSTRAINT browser_sessions_credential_version_check
        CHECK ((auth_method = 'local_password') = (issued_credential_version IS NOT NULL));
