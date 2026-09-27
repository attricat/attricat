CREATE TABLE extension_operation_http_inputs (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    transfer_key TEXT NOT NULL CHECK (char_length(transfer_key) BETWEEN 1 AND 128),
    artifact_id UUID NOT NULL REFERENCES extension_operation_artifacts(id),
    source_etag TEXT,
    request_digest TEXT NOT NULL CHECK (char_length(request_digest) = 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (operation_run_id, transfer_key)
);
CREATE TABLE extension_operation_http_deliveries (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    delivery_key TEXT NOT NULL CHECK (char_length(delivery_key) BETWEEN 1 AND 128),
    artifact_id UUID NOT NULL REFERENCES extension_operation_artifacts(id),
    destination_digest TEXT NOT NULL CHECK (char_length(destination_digest) = 64),
    state TEXT NOT NULL DEFAULT 'uncertain' CHECK (state IN ('uncertain','succeeded','failed')),
    http_status INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (operation_run_id, delivery_key)
);
CREATE INDEX extension_operation_http_deliveries_run_idx ON extension_operation_http_deliveries(workspace_id, operation_run_id, created_at);
