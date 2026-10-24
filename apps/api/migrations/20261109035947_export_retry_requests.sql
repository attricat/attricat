-- Retry requests an operator sent with an idempotency key. The key is
-- unique, so a request sent again (even concurrently) finds the records
-- the first one retried instead of retrying them twice. Rows are swept
-- after a retention period.
CREATE TABLE extension_export_retry_requests (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    request_key TEXT NOT NULL CHECK (char_length(request_key) BETWEEN 1 AND 128),
    record_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, extension_id, request_key)
);
CREATE INDEX extension_export_retry_requests_created_idx
    ON extension_export_retry_requests (created_at);
