-- Persist bounded public guidance and private declarative informational checks.
ALTER TABLE solution_pack_plans
    ADD COLUMN readme_markdown TEXT CHECK (readme_markdown IS NULL OR octet_length(readme_markdown) <= 65536),
    ADD COLUMN release_notes_markdown TEXT CHECK (release_notes_markdown IS NULL OR octet_length(release_notes_markdown) <= 32768),
    -- Rust enforces the 64 KiB canonical compact-JSON contract. JSONB's text
    -- rendering adds separator whitespace, so retain 4 KiB bounded overhead.
    ADD COLUMN setup_checklist JSONB CHECK (setup_checklist IS NULL OR (jsonb_typeof(setup_checklist) = 'object' AND octet_length(setup_checklist::text) <= 69632));

CREATE TABLE solution_pack_plan_check_definitions (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 64),
    check_key TEXT NOT NULL CHECK (check_key ~ '^checks/[a-z][a-z0-9_-]*$' AND octet_length(check_key) <= 128),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0 AND octet_length(title) <= 200),
    predicate JSONB NOT NULL CHECK (jsonb_typeof(predicate) = 'object' AND octet_length(predicate::text) <= 1024),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, check_key),
    FOREIGN KEY (workspace_id, plan_id) REFERENCES solution_pack_plans (workspace_id, id)
);

ALTER TABLE solution_pack_applications
    ADD COLUMN readme_markdown TEXT CHECK (readme_markdown IS NULL OR octet_length(readme_markdown) <= 65536),
    ADD COLUMN release_notes_markdown TEXT CHECK (release_notes_markdown IS NULL OR octet_length(release_notes_markdown) <= 32768),
    ADD COLUMN setup_checklist JSONB CHECK (setup_checklist IS NULL OR (jsonb_typeof(setup_checklist) = 'object' AND octet_length(setup_checklist::text) <= 69632));

CREATE TABLE solution_pack_application_check_definitions (
    application_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 64),
    check_key TEXT NOT NULL CHECK (check_key ~ '^checks/[a-z][a-z0-9_-]*$' AND octet_length(check_key) <= 128),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0 AND octet_length(title) <= 200),
    predicate JSONB NOT NULL CHECK (jsonb_typeof(predicate) = 'object' AND octet_length(predicate::text) <= 1024),
    PRIMARY KEY (application_id, position),
    UNIQUE (application_id, check_key),
    FOREIGN KEY (workspace_id, application_id) REFERENCES solution_pack_applications (workspace_id, id)
);

CREATE TABLE solution_pack_check_runs (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL,
    application_id UUID NOT NULL,
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    trigger TEXT NOT NULL CHECK (trigger IN ('post_apply', 'manual')),
    total_count BIGINT NOT NULL CHECK (total_count >= 0 AND total_count <= 64),
    passed_count BIGINT NOT NULL CHECK (passed_count >= 0 AND passed_count <= total_count),
    failed_count BIGINT NOT NULL CHECK (failed_count = total_count - passed_count),
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL CHECK (completed_at >= started_at),
    UNIQUE (workspace_id, id),
    FOREIGN KEY (workspace_id, application_id) REFERENCES solution_pack_applications (workspace_id, id)
);
CREATE UNIQUE INDEX solution_pack_check_runs_one_post_apply_idx
    ON solution_pack_check_runs (application_id) WHERE trigger = 'post_apply';
CREATE INDEX solution_pack_check_runs_application_history_idx
    ON solution_pack_check_runs (workspace_id, application_id, completed_at DESC, id DESC);

CREATE TABLE solution_pack_check_results (
    run_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0 AND position < 64),
    check_key TEXT NOT NULL CHECK (check_key ~ '^checks/[a-z][a-z0-9_-]*$' AND octet_length(check_key) <= 128),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0 AND octet_length(title) <= 200),
    predicate_type TEXT NOT NULL CHECK (predicate_type IN ('blueprint_published','extension_installed','extension_enabled','extension_configuration_matches','explore_navigation_entry_present','workspace_extension_layout_placement_present')),
    passed BOOLEAN NOT NULL,
    reason_code TEXT NOT NULL CHECK (reason_code ~ '^[a-z][a-z0-9_]*$'),
    summary TEXT NOT NULL CHECK (octet_length(summary) <= 4096),
    evidence JSONB NOT NULL CHECK (jsonb_typeof(evidence) = 'object' AND octet_length(evidence::text) <= 4096),
    evaluated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (run_id, position),
    UNIQUE (run_id, check_key),
    FOREIGN KEY (workspace_id, run_id) REFERENCES solution_pack_check_runs (workspace_id, id)
);
