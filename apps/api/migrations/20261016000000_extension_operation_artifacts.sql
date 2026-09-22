CREATE TABLE extension_operation_artifacts (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL,
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases(id),
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    direction TEXT NOT NULL CHECK (direction IN ('input','output')),
    state TEXT NOT NULL CHECK (state IN ('incomplete','completed','aborted')),
    content_length BIGINT NOT NULL DEFAULT 0 CHECK (content_length >= 0 AND content_length <= 1073741824),
    media_type TEXT NOT NULL DEFAULT 'application/octet-stream' CHECK (char_length(media_type) BETWEEN 1 AND 255),
    checksum_sha256 TEXT CHECK (checksum_sha256 ~ '^[a-f0-9]{64}$'),
    object_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    aborted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((state = 'completed' AND completed_at IS NOT NULL AND checksum_sha256 IS NOT NULL AND object_key IS NOT NULL) OR state <> 'completed'),
    CHECK ((state = 'aborted' AND aborted_at IS NOT NULL) OR state <> 'aborted')
);
CREATE INDEX extension_operation_artifacts_run_idx ON extension_operation_artifacts(workspace_id, operation_run_id, state);
CREATE INDEX extension_operation_artifacts_cleanup_idx ON extension_operation_artifacts(state, updated_at) WHERE state = 'incomplete';
