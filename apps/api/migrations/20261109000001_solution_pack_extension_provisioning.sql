-- A plan may install a missing extension from the official registry. Such a
-- requirement is recorded as `install` until apply provisions it.
ALTER TABLE solution_pack_plan_extension_requirements
    DROP CONSTRAINT solution_pack_plan_extension_requirements_status_check,
    ADD CONSTRAINT solution_pack_plan_extension_requirements_status_check
        CHECK (status IN ('satisfied', 'install', 'blocked', 'skipped')),
    DROP CONSTRAINT solution_pack_plan_extension_requirements_reason_code_check,
    ADD CONSTRAINT solution_pack_plan_extension_requirements_reason_code_check
        CHECK (reason_code IN (
            'satisfied',
            'install',
            'missing',
            'incompatible_version',
            'quarantined',
            'policy_incompatible',
            'configuration_mismatch'
        )),
    DROP CONSTRAINT solution_pack_plan_extension_requirements_check5,
    DROP CONSTRAINT solution_pack_plan_extension_requirements_check6,
    ADD CONSTRAINT solution_pack_plan_extension_requirements_install_check
        CHECK ((status = 'install') = (reason_code = 'install')),
    ADD CONSTRAINT solution_pack_plan_extension_requirements_blocked_check
        CHECK ((status = 'blocked') = (required AND reason_code NOT IN ('satisfied', 'install'))),
    ADD CONSTRAINT solution_pack_plan_extension_requirements_skipped_check
        CHECK ((status = 'skipped') = (NOT required AND reason_code NOT IN ('satisfied', 'install')));

-- The pinned official release for each `install` requirement. The installed
-- release ID is assigned while planning so reviewed contribution evidence names
-- the exact release that apply creates.
CREATE TABLE solution_pack_plan_extension_releases (
    plan_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    position BIGINT NOT NULL CHECK (position >= 0),
    logical_key TEXT NOT NULL,
    extension_id TEXT NOT NULL CHECK (extension_id ~ '^[A-Za-z0-9._-]{1,128}$'),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 256),
    installed_release_id UUID NOT NULL UNIQUE,
    repository TEXT NOT NULL CHECK (length(repository) BETWEEN 1 AND 512),
    release_id BIGINT NOT NULL CHECK (release_id >= 0),
    tag_name TEXT NOT NULL CHECK (length(tag_name) BETWEEN 1 AND 256),
    asset_id BIGINT NOT NULL CHECK (asset_id >= 0),
    asset_name TEXT NOT NULL CHECK (length(asset_name) BETWEEN 1 AND 256),
    download_url TEXT NOT NULL CHECK (length(download_url) BETWEEN 1 AND 2048),
    archive_sha256 TEXT NOT NULL CHECK (archive_sha256 ~ '^[0-9a-f]{64}$'),
    manifest_sha256 TEXT NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'),
    configuration JSONB NOT NULL CHECK (jsonb_typeof(configuration) = 'object'),
    required_grants JSONB NOT NULL CHECK (jsonb_typeof(required_grants) = 'array'),
    PRIMARY KEY (plan_id, position),
    UNIQUE (plan_id, extension_id),
    FOREIGN KEY (workspace_id, plan_id)
        REFERENCES solution_pack_plans (workspace_id, id),
    FOREIGN KEY (plan_id, logical_key)
        REFERENCES solution_pack_plan_extension_requirements (plan_id, logical_key)
);
