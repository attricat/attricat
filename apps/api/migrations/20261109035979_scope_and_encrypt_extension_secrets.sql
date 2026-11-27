-- Extension secrets belong to one extension installation and are stored only
-- as AES-256-GCM ciphertext under the extension_credential_key secret, bound
-- to their workspace, extension and name. Repository code encrypts values, so
-- the plaintext workspace-wide secrets cannot be carried over: operators set
-- them again for each extension that reads them.
DROP TABLE workspace_extension_secrets;

CREATE TABLE extension_secrets (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (name ~ '^[A-Za-z0-9._-]{1,128}$'),
    -- A 12-byte nonce, 1 to 65536 bytes of value and a 16-byte tag.
    value_ciphertext BYTEA NOT NULL
        CHECK (octet_length(value_ciphertext) BETWEEN 29 AND 65564),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, extension_id, name)
);
