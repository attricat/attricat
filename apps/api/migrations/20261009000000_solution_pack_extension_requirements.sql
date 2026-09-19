-- Immutable extension requirement evaluations attached to a solution-pack plan.
-- evaluation_template is private revalidation material and is never returned by
-- plan APIs. It contains only bounded, validated, explicitly non-secret JSON.
CREATE TABLE solution_pack_plan_extension_requirements (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    logical_key TEXT NOT NULL,
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    version_requirement TEXT NOT NULL,
    required BOOLEAN NOT NULL,
    configuration_template_path TEXT,
    configuration_template_sha256 TEXT CHECK (
        configuration_template_sha256 IS NULL
        OR configuration_template_sha256 ~ '^[0-9a-f]{64}$'
    ),
    status TEXT NOT NULL CHECK (status IN ('satisfied', 'blocked', 'skipped')),
    reason_code TEXT NOT NULL CHECK (
        reason_code IN ('satisfied', 'missing', 'incompatible_version', 'quarantined', 'configuration_mismatch')
    ),
    installed_release_id UUID,
    installed_version TEXT,
    installed_state TEXT CHECK (installed_state IS NULL OR installed_state IN ('disabled', 'enabled', 'quarantined')),
    configuration_matches BOOLEAN,
    evaluation_template JSONB NOT NULL CHECK (jsonb_typeof(evaluation_template) = 'object'),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, logical_key),
    UNIQUE (plan_id, extension_id),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    CHECK ((configuration_template_path IS NULL) = (configuration_template_sha256 IS NULL)),
    CHECK ((installed_release_id IS NULL) = (installed_version IS NULL)),
    CHECK ((installed_release_id IS NULL) = (installed_state IS NULL)),
    CHECK ((installed_release_id IS NULL) = (configuration_matches IS NULL)),
    CHECK ((status = 'satisfied') = (reason_code = 'satisfied')),
    CHECK ((status = 'blocked') = (required AND reason_code <> 'satisfied')),
    CHECK ((status = 'skipped') = (NOT required AND reason_code <> 'satisfied'))
);
