-- Host platform for marketplace export extensions (#424).

-- Per-handler outcome of one event delivery. Completed handlers are skipped
-- when the delivery is retried for another handler; retrying handlers wait
-- for `next_attempt_at`; rejected and exhausted outcomes are kept for
-- operators and swept after their retention.
CREATE TABLE extension_handler_attempts (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    event_id UUID NOT NULL REFERENCES domain_events(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    handler_id TEXT NOT NULL CHECK (char_length(handler_id) BETWEEN 1 AND 128),
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases(id),
    status TEXT NOT NULL
        CHECK (status IN ('completed', 'retrying', 'debouncing', 'superseded', 'rejected', 'exhausted')),
    attempts INTEGER NOT NULL CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ,
    error_code TEXT CHECK (char_length(error_code) BETWEEN 1 AND 128),
    error_message TEXT CHECK (char_length(error_message) <= 4096),
    record_id UUID,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, event_id, extension_id, handler_id),
    CHECK (status NOT IN ('retrying', 'debouncing') OR next_attempt_at IS NOT NULL)
);
CREATE INDEX extension_handler_attempts_outcome_idx
    ON extension_handler_attempts (workspace_id, extension_id, status, updated_at DESC);

-- Host-managed destination credentials. Secret material is stored only as
-- AES-256-GCM ciphertext; it never leaves the host. A connection is named by
-- its extension-local key, which jobs, requests and mappings refer to.
CREATE TABLE extension_connections (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    connection TEXT NOT NULL CHECK (connection ~ '^[A-Za-z0-9._-]{1,128}$'),
    -- The manifest's connection kind.
    kind TEXT NOT NULL CHECK (char_length(kind) BETWEEN 1 AND 128),
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 200),
    grant_type TEXT NOT NULL
        CHECK (grant_type IN ('refresh_token', 'client_credentials', 'device_code', 'authorization_code')),
    host_permission_id TEXT NOT NULL CHECK (char_length(host_permission_id) BETWEEN 1 AND 128),
    token_url TEXT NOT NULL CHECK (char_length(token_url) BETWEEN 1 AND 2048),
    device_authorization_url TEXT CHECK (char_length(device_authorization_url) BETWEEN 1 AND 2048),
    client_id TEXT NOT NULL CHECK (char_length(client_id) BETWEEN 1 AND 512),
    scope TEXT CHECK (char_length(scope) <= 2048),
    -- Non-secret settings the extension reads, such as a shop domain or region.
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(parameters) = 'object'),
    client_secret_ciphertext BYTEA,
    refresh_token_ciphertext BYTEA,
    access_token_ciphertext BYTEA,
    access_token_expires_at TIMESTAMPTZ,
    device_code_ciphertext BYTEA,
    device_user_code TEXT CHECK (char_length(device_user_code) <= 128),
    device_verification_uri TEXT CHECK (char_length(device_verification_uri) <= 2048),
    device_expires_at TIMESTAMPTZ,
    device_interval_seconds INTEGER CHECK (device_interval_seconds BETWEEN 1 AND 3600),
    device_polled_at TIMESTAMPTZ,
    status TEXT NOT NULL
        CHECK (status IN ('pending_authorization', 'active', 'failed', 'revoked')),
    last_error TEXT CHECK (char_length(last_error) <= 1024),
    last_refreshed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, extension_id, connection)
);

-- Record -> destination mapping of an export extension, per connection (NULL
-- is the extension's default destination). Rows go with their record only;
-- withdrawing a record on the destination is the extension's job, so soft
-- deletion keeps the row. Rows outlive their connection so a connection
-- re-created under the same name keeps the destination's external IDs.
CREATE TABLE extension_export_mappings (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    connection TEXT CHECK (connection ~ '^[A-Za-z0-9._-]{1,128}$'),
    record_id UUID NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    external_id TEXT CHECK (char_length(external_id) BETWEEN 1 AND 512),
    external_ids JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(external_ids) = 'object'),
    content_hash TEXT CHECK (char_length(content_hash) BETWEEN 1 AND 128),
    record_revision BIGINT NOT NULL DEFAULT 0 CHECK (record_revision >= 0),
    status TEXT NOT NULL
        CHECK (status IN ('pending', 'synced', 'rejected', 'retrying', 'uncertain', 'withdrawn')),
    error JSONB CHECK (error IS NULL OR jsonb_typeof(error) = 'object'),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    lease_token UUID,
    lease_until TIMESTAMPTZ,
    leased_at TIMESTAMPTZ,
    retry_requested_at TIMESTAMPTZ,
    synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (workspace_id, extension_id, connection, record_id)
);
CREATE INDEX extension_export_mappings_external_idx
    ON extension_export_mappings (workspace_id, extension_id, connection, external_id)
    WHERE external_id IS NOT NULL;
CREATE INDEX extension_export_mappings_status_idx
    ON extension_export_mappings (workspace_id, extension_id, connection, status, updated_at, id);
CREATE INDEX extension_export_mappings_record_idx
    ON extension_export_mappings (workspace_id, record_id);

-- Short-lived public links an export extension issued for a record file so a
-- marketplace can fetch it. Only the token's SHA-256 is stored. A link serves
-- its file only while the record it was issued for still references it.
CREATE TABLE extension_file_links (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    record_id UUID NOT NULL REFERENCES records(id) ON DELETE CASCADE,
    token_sha256 TEXT NOT NULL UNIQUE CHECK (char_length(token_sha256) = 64),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX extension_file_links_expiry_idx ON extension_file_links (expires_at);

-- Operator-lowered request budgets of host-permission rules. The manifest's
-- declared budget stays the ceiling.
CREATE TABLE extension_host_permission_budgets (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    host_permission_id TEXT NOT NULL CHECK (char_length(host_permission_id) BETWEEN 1 AND 128),
    requests INTEGER NOT NULL CHECK (requests BETWEEN 1 AND 100000),
    per_seconds INTEGER NOT NULL CHECK (per_seconds BETWEEN 1 AND 86400),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, extension_id, host_permission_id)
);

-- A deferred batch's resumption time, for operators.
ALTER TABLE extension_operation_runs ADD COLUMN resume_at TIMESTAMPTZ;
-- The destination connection a connector job targets, and its optional
-- change trigger: record events after `trigger_watermark` start a run once
-- the newest of them is `trigger_debounce_seconds` old. A trigger whose run
-- could not be queued waits until `trigger_blocked_until` before it retries.
ALTER TABLE blueprint_connector_jobs
    ADD COLUMN connection TEXT CHECK (connection ~ '^[A-Za-z0-9._-]{1,128}$'),
    ADD COLUMN trigger_events TEXT[],
    ADD COLUMN trigger_debounce_seconds INTEGER CHECK (trigger_debounce_seconds BETWEEN 1 AND 3600),
    ADD COLUMN trigger_watermark BIGINT,
    ADD COLUMN trigger_blocked_until TIMESTAMPTZ,
    ADD CONSTRAINT blueprint_connector_jobs_trigger_check
        CHECK ((trigger_events IS NULL) = (trigger_debounce_seconds IS NULL));
-- A connector run's connection, mode and optional record filter (a resync
-- of selected records, or the records a change trigger saw).
ALTER TABLE extension_operation_runs
    ADD COLUMN connector_connection TEXT CHECK (connector_connection ~ '^[A-Za-z0-9._-]{1,128}$'),
    ADD COLUMN connector_mode TEXT CHECK (connector_mode IN ('incremental', 'resync', 'triggered')),
    ADD COLUMN connector_record_ids UUID[] CHECK (cardinality(connector_record_ids) <= 10000);
