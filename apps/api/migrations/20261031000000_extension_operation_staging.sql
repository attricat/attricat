ALTER TABLE extension_operation_runs ADD COLUMN outputs_expired BOOLEAN NOT NULL DEFAULT false;

-- One immutable object per append, committed under a run/name/batch key.
-- Completion is separate: incomplete chunks are never downloadable.
CREATE TABLE extension_operation_output_chunks (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 128),
    media_type TEXT NOT NULL CHECK (char_length(media_type) BETWEEN 1 AND 255),
    batch_key TEXT NOT NULL CHECK (char_length(batch_key) BETWEEN 1 AND 128),
    content_length INTEGER NOT NULL CHECK (content_length BETWEEN 1 AND 65536),
    checksum_sha256 TEXT NOT NULL CHECK (char_length(checksum_sha256) = 64),
    object_key TEXT NOT NULL,
    uploaded BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (operation_run_id, name, batch_key)
);
CREATE INDEX extension_operation_output_chunks_run_idx ON extension_operation_output_chunks(workspace_id, operation_run_id, name, created_at);
CREATE TABLE extension_operation_object_cleanup (
    object_key TEXT PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX extension_operation_object_cleanup_workspace_idx ON extension_operation_object_cleanup(workspace_id, created_at);

CREATE TABLE extension_operation_output_staging (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 128),
    media_type TEXT NOT NULL CHECK (char_length(media_type) BETWEEN 1 AND 255),
    artifact_id UUID REFERENCES extension_operation_artifacts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (operation_run_id, name)
);
