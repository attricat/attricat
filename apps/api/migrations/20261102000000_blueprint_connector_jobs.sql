CREATE TABLE blueprint_connector_jobs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    blueprint_id UUID NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('export', 'import')),
    extension_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    context_id UUID,
    interval_seconds INTEGER CHECK (interval_seconds BETWEEN 60 AND 2592000),
    next_at TIMESTAMPTZ,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, context_id) REFERENCES attribute_contexts(workspace_id, id),
    CHECK ((direction = 'export' AND context_id IS NULL) OR
           (direction = 'import' AND context_id IS NOT NULL)),
    CHECK ((interval_seconds IS NULL AND next_at IS NULL) OR
           (interval_seconds IS NOT NULL AND next_at IS NOT NULL))
);
CREATE INDEX blueprint_connector_jobs_due_idx ON blueprint_connector_jobs(next_at, id) WHERE enabled AND interval_seconds IS NOT NULL;
ALTER TABLE extension_operation_runs ADD COLUMN connector_job_id UUID REFERENCES blueprint_connector_jobs(id);
ALTER TABLE extension_operation_runs ADD COLUMN connector_channel_id UUID;
ALTER TABLE extension_operation_runs ADD COLUMN connector_context_id UUID;
ALTER TABLE extension_operation_runs ADD COLUMN connector_blueprint_id UUID;
ALTER TABLE extension_operation_runs ADD COLUMN connector_blueprint_version BIGINT;
