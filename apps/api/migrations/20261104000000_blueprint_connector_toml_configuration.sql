ALTER TABLE blueprint_connector_jobs ADD COLUMN code TEXT;
ALTER TABLE blueprint_connector_jobs ADD COLUMN blueprint_version BIGINT;
CREATE UNIQUE INDEX blueprint_connector_jobs_definition_idx
    ON blueprint_connector_jobs(workspace_id, blueprint_id, code)
    WHERE code IS NOT NULL;
