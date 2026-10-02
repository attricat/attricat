-- Interactive extension operations are started by a signed-in user for an
-- explicit, immutable entity selection. Pre-existing and scheduled runs keep
-- the administrative contract; no initiating actor is invented for them.
ALTER TABLE extension_operation_runs
    ADD COLUMN invocation TEXT NOT NULL DEFAULT 'administrative'
        CHECK (invocation IN ('administrative', 'interactive')),
    ADD COLUMN contribution_id TEXT CHECK (char_length(contribution_id) BETWEEN 1 AND 128),
    ADD COLUMN selection_blueprint_id UUID,
    ADD COLUMN selection_blueprint_version BIGINT CHECK (selection_blueprint_version > 0),
    ADD COLUMN selection_context_id UUID,
    ADD COLUMN request_digest TEXT CHECK (char_length(request_digest) = 64),
    ADD CONSTRAINT extension_operation_runs_interactive_scope_check CHECK (
        invocation = 'administrative'
        OR (
            actor_user_id IS NOT NULL
            AND contribution_id IS NOT NULL
            AND selection_blueprint_id IS NOT NULL
            AND selection_blueprint_version IS NOT NULL
            AND request_digest IS NOT NULL
        )
    );

CREATE INDEX extension_operation_runs_initiator_idx
    ON extension_operation_runs (workspace_id, actor_user_id, created_at DESC, id DESC)
    WHERE invocation = 'interactive';

-- Ordered membership is frozen when the run is created. It is not an
-- authorization grant: every host call rechecks the initiator's access.
CREATE TABLE extension_operation_run_entities (
    operation_run_id UUID NOT NULL REFERENCES extension_operation_runs(id),
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    entity_id UUID NOT NULL REFERENCES entities(id),
    PRIMARY KEY (operation_run_id, position),
    UNIQUE (operation_run_id, entity_id)
);
CREATE INDEX extension_operation_run_entities_entity_idx
    ON extension_operation_run_entities (workspace_id, entity_id);

-- An extension owns `<extension-id>:<tag>` system tags and the
-- `system_metadata[<extension-id>]` object only after its namespace is claimed.
-- Claims survive disable, upgrade, and removal; they are never released
-- implicitly.
CREATE TABLE extension_annotation_namespaces (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    adopted_legacy BOOLEAN NOT NULL DEFAULT false,
    claimed_by_user_id UUID REFERENCES users(id),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, extension_id)
);

-- Monotonic per-entity namespace revision for conditional annotation patches.
CREATE TABLE entity_extension_annotation_revisions (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    entity_id UUID NOT NULL REFERENCES entities(id),
    extension_id TEXT NOT NULL CHECK (char_length(extension_id) BETWEEN 1 AND 128),
    revision BIGINT NOT NULL CHECK (revision > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, entity_id, extension_id),
    FOREIGN KEY (workspace_id, extension_id)
        REFERENCES extension_annotation_namespaces (workspace_id, extension_id)
);

-- Staging rows are swept an hour after completion; keep the extension-chosen
-- output name with the immutable artifact so user downloads remain named.
ALTER TABLE extension_operation_artifacts
    ADD COLUMN output_name TEXT CHECK (char_length(output_name) BETWEEN 1 AND 128);
