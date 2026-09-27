CREATE TABLE extension_operation_schedules (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL,
    installed_release_id UUID NOT NULL REFERENCES installed_extension_releases(id),
    operation_id TEXT NOT NULL,
    input JSONB NOT NULL,
    configuration_snapshot JSONB NOT NULL,
    source_reference JSONB NOT NULL DEFAULT '{}'::jsonb,
    destination_reference JSONB NOT NULL DEFAULT '{}'::jsonb,
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds BETWEEN 60 AND 2592000),
    next_at TIMESTAMPTZ NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    paused_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    CHECK (char_length(operation_id) BETWEEN 1 AND 128),
    CHECK (paused_reason IS NULL OR char_length(paused_reason) <= 128)
);
CREATE INDEX extension_operation_schedules_due_idx ON extension_operation_schedules(next_at, id) WHERE enabled;
ALTER TABLE extension_operation_runs ADD COLUMN schedule_id UUID REFERENCES extension_operation_schedules(id);
CREATE INDEX extension_operation_runs_schedule_idx ON extension_operation_runs(schedule_id, created_at DESC);
