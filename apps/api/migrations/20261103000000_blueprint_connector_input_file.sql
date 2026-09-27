ALTER TABLE blueprint_connector_jobs ADD COLUMN input_file_id UUID;
ALTER TABLE blueprint_connector_jobs ADD CONSTRAINT blueprint_connector_jobs_input_file_fkey
    FOREIGN KEY (workspace_id, input_file_id) REFERENCES files(workspace_id, id);
ALTER TABLE blueprint_connector_jobs ADD CONSTRAINT blueprint_connector_jobs_export_has_no_input_file
    CHECK (direction = 'import' OR input_file_id IS NULL);
