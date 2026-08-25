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
