-- Rules are independent, versioned blueprint-owned definitions. Runtime state is durable and
-- intentionally separate from workflow definitions/runs.
CREATE TABLE rules (
    id UUID NOT NULL,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL,
    context_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published')),
    definition TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    compiled_plan JSONB NOT NULL,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, version),
    UNIQUE (id, definition_hash),
    UNIQUE (workspace_id, id, version),
    FOREIGN KEY (blueprint_id, blueprint_version) REFERENCES blueprints(id, version),
    FOREIGN KEY (context_id) REFERENCES attribute_contexts(id)
);
CREATE INDEX rules_workspace_blueprint_idx ON rules(workspace_id, blueprint_id, blueprint_version, created_at DESC);
CREATE INDEX rules_workspace_code_idx ON rules(workspace_id, code);

CREATE TABLE rule_lifecycles (
    rule_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    enabled_version BIGINT,
    activation_sequence BIGINT,
    enabled_at TIMESTAMPTZ,
    disabled_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (workspace_id, rule_id, enabled_version) REFERENCES rules(workspace_id, id, version)
);
CREATE INDEX rule_lifecycles_enabled_idx ON rule_lifecycles(workspace_id, enabled_version) WHERE enabled_version IS NOT NULL;

-- One schedule cursor per immutable trigger. Runs carry a bounded candidate cursor rather than
-- creating entity timers or loading a workspace into memory.
CREATE TABLE rule_schedule_states (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    rule_id UUID NOT NULL,
    rule_version BIGINT NOT NULL,
    trigger_index INTEGER NOT NULL CHECK (trigger_index >= 0),
    next_run_at TIMESTAMPTZ NOT NULL,
    misfires BIGINT NOT NULL DEFAULT 0 CHECK (misfires >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, rule_id, rule_version, trigger_index),
    FOREIGN KEY (workspace_id, rule_id, rule_version) REFERENCES rules(workspace_id, id, version)
);
CREATE INDEX rule_schedule_states_due_idx ON rule_schedule_states(workspace_id, next_run_at);

CREATE TABLE rule_runs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    rule_id UUID NOT NULL,
    rule_version BIGINT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('manual', 'schedule', 'event', 'post_import')),
    dry_run BOOLEAN NOT NULL DEFAULT false,
    scope_entity_id UUID REFERENCES entities(id),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'leased', 'completed', 'dead_letter', 'cancelled')),
    idempotency_key TEXT NOT NULL,
    candidate_cursor UUID,
    candidates_evaluated BIGINT NOT NULL DEFAULT 0 CHECK (candidates_evaluated >= 0),
    findings_created BIGINT NOT NULL DEFAULT 0 CHECK (findings_created >= 0),
    findings_resolved BIGINT NOT NULL DEFAULT 0 CHECK (findings_resolved >= 0),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    last_error TEXT,
    completed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(workspace_id, rule_id, rule_version, source, idempotency_key),
    FOREIGN KEY (workspace_id, rule_id, rule_version) REFERENCES rules(workspace_id, id, version)
);
CREATE INDEX rule_runs_claim_idx ON rule_runs(workspace_id, status, next_attempt_at);

CREATE TABLE rule_findings (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    rule_id UUID NOT NULL,
    rule_version BIGINT NOT NULL,
    entity_id UUID NOT NULL REFERENCES entities(id),
    context_id UUID,
    evaluation_key TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error', 'critical')),
    message TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'acknowledged', 'resolved')),
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by_user_id UUID,
    resolved_at TIMESTAMPTZ,
    resolved_by_run_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(workspace_id, rule_id, entity_id, context_id, evaluation_key),
    FOREIGN KEY (workspace_id, rule_id, rule_version) REFERENCES rules(workspace_id, id, version),
    FOREIGN KEY (context_id) REFERENCES attribute_contexts(id),
    FOREIGN KEY (resolved_by_run_id) REFERENCES rule_runs(id)
);
CREATE INDEX rule_findings_active_idx ON rule_findings(workspace_id, state, severity, updated_at DESC) WHERE state <> 'resolved';
CREATE INDEX rule_findings_entity_idx ON rule_findings(workspace_id, entity_id, state);
