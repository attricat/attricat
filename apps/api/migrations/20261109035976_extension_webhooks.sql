-- Public endpoints of extensions' manifest-declared inbound webhooks (#279).
-- An endpoint's URL carries a 256-bit token; only its SHA-256 is stored, so
-- the URL is shown once, when the endpoint is created or its token rotated.
-- The last request's time and reply status help operators troubleshoot a
-- sender without storing anything it sent.
CREATE TABLE extension_webhook_endpoints (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    -- The manifest webhook this endpoint receives.
    webhook TEXT NOT NULL CHECK (char_length(webhook) BETWEEN 1 AND 128),
    -- The operator's name, unique within the extension, which the component
    -- receives to tell accounts apart.
    endpoint TEXT NOT NULL CHECK (endpoint ~ '^[A-Za-z0-9._-]{1,128}$'),
    token_sha256 BYTEA NOT NULL UNIQUE CHECK (octet_length(token_sha256) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    token_rotated_at TIMESTAMPTZ,
    last_request_at TIMESTAMPTZ,
    last_reply_status SMALLINT CHECK (last_reply_status BETWEEN 100 AND 599),
    UNIQUE (workspace_id, extension_id, endpoint)
);

-- Payloads a component kept from webhook requests, delivered to its event
-- handlers as `webhook.received.v1` and read back by receipt ID. A receipt
-- outlives its endpoint, so deleting an endpoint does not lose payloads that
-- are still being delivered; the retention sweep removes old receipts.
CREATE TABLE extension_webhook_receipts (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    webhook TEXT NOT NULL CHECK (char_length(webhook) BETWEEN 1 AND 128),
    endpoint TEXT NOT NULL CHECK (char_length(endpoint) BETWEEN 1 AND 128),
    -- The component's key for the sender's delivery; a repeat is not stored.
    dedupe_key TEXT CHECK (char_length(dedupe_key) BETWEEN 1 AND 256),
    payload TEXT NOT NULL CHECK (octet_length(payload) <= 524288),
    received_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (workspace_id, extension_id, endpoint, dedupe_key)
);
-- The retention sweep's scan by age.
CREATE INDEX extension_webhook_receipts_received_idx
    ON extension_webhook_receipts (received_at);
