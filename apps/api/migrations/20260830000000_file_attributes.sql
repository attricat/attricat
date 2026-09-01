-- File metadata is deliberately separate from object storage. Object keys are
-- internal-only and are never selected into normal entity value responses.
ALTER TABLE attributes ADD COLUMN file_policy JSONB;
ALTER TABLE attributes ADD CONSTRAINT attributes_file_policy_check
    CHECK ((value_type = 'file' AND file_policy IS NOT NULL) OR (value_type <> 'file' AND file_policy IS NULL));

-- File parents carry their values in attribute_file_references, so they have no
-- native scalar columns or relationship target.
ALTER TABLE attribute_values DROP CONSTRAINT attribute_values_native_value_shape_check;
ALTER TABLE attribute_values ADD CONSTRAINT attribute_values_native_value_shape_check
    CHECK (
        (relationship_target_entity_id IS NOT NULL
            AND value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
            AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
            AND value_time IS NULL AND value_time_zone IS NULL)
        OR
        (relationship_target_entity_id IS NULL AND (
            (value_text IS NOT NULL)::INT + (value_number IS NOT NULL)::INT
            + (value_integer IS NOT NULL)::INT + (value_boolean IS NOT NULL)::INT
            + (value_date IS NOT NULL)::INT + (value_datetime IS NOT NULL)::INT
            + (value_time IS NOT NULL)::INT = 1
            OR (value_text IS NULL AND value_number IS NULL AND value_integer IS NULL
                AND value_boolean IS NULL AND value_date IS NULL AND value_datetime IS NULL
                AND value_time IS NULL AND value_time_zone IS NULL)
        ) AND ((value_time IS NULL AND value_time_zone IS NULL)
            OR (value_time IS NOT NULL AND value_time_zone IS NOT NULL AND value_time_zone <> '')))
    );

CREATE TABLE files (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    original_filename TEXT NOT NULL,
    display_filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    original_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('uploading', 'queued', 'processing', 'ready', 'failed', 'deleted')),
    processing_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    purge_after TIMESTAMPTZ
);
CREATE UNIQUE INDEX files_workspace_id_key ON files (workspace_id, id);
CREATE INDEX files_purge_due_idx ON files (workspace_id, purge_after)
    WHERE deleted_at IS NOT NULL;

CREATE TABLE file_variants (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    file_id UUID NOT NULL,
    kind TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    width INTEGER CHECK (width IS NULL OR width > 0),
    height INTEGER CHECK (height IS NULL OR height > 0),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0),
    object_key TEXT NOT NULL,
    sha256 TEXT NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (file_id, kind),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id)
);

CREATE TABLE file_processing_jobs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    file_id UUID NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'retryable', 'completed', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_at TIMESTAMPTZ,
    worker_id TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id)
);
CREATE INDEX file_processing_jobs_poll_idx
    ON file_processing_jobs (workspace_id, available_at, id)
    WHERE status IN ('queued', 'retryable');

-- A file attribute has one EAV parent per attribute/context. Its ordered child
-- rows are the current set. History receives copied children in the same Rust
-- transaction that archives the parent; SQL triggers are intentionally absent.
-- Composite references must match their parent workspace. PostgreSQL requires
-- explicit unique constraints for each referenced key, even when an ID is
-- globally unique. The partitioned history table's key also includes its
-- partition key (`archived_at`).
ALTER TABLE attribute_values
    ADD CONSTRAINT attribute_values_workspace_id_key UNIQUE (workspace_id, id);
ALTER TABLE attribute_value_history
    ADD CONSTRAINT attribute_value_history_workspace_id_archived_at_key
    UNIQUE (workspace_id, id, archived_at);

CREATE TABLE attribute_file_references (
    attribute_value_id UUID NOT NULL REFERENCES attribute_values (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    file_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (attribute_value_id, position),
    UNIQUE (attribute_value_id, file_id),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id),
    FOREIGN KEY (workspace_id, attribute_value_id) REFERENCES attribute_values (workspace_id, id)
);
CREATE INDEX attribute_file_references_file_idx ON attribute_file_references (workspace_id, file_id);

CREATE TABLE attribute_file_reference_history (
    attribute_value_history_id UUID NOT NULL,
    attribute_value_history_archived_at TIMESTAMPTZ NOT NULL,
    workspace_id UUID NOT NULL,
    file_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (attribute_value_history_id, attribute_value_history_archived_at, position),
    UNIQUE (attribute_value_history_id, attribute_value_history_archived_at, file_id),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id),
    FOREIGN KEY (workspace_id, attribute_value_history_id, attribute_value_history_archived_at)
        REFERENCES attribute_value_history (workspace_id, id, archived_at) ON DELETE CASCADE
);
