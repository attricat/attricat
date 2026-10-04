-- File reclamation checks every reference source for each candidate file.
CREATE INDEX blueprint_connector_jobs_input_file_idx
    ON blueprint_connector_jobs (workspace_id, input_file_id)
    WHERE input_file_id IS NOT NULL;
CREATE INDEX extension_operation_schedules_input_file_idx
    ON extension_operation_schedules (workspace_id, (source_reference->>'input_file_id'));
CREATE INDEX extension_operation_artifacts_input_object_idx
    ON extension_operation_artifacts (workspace_id, object_key)
    WHERE direction = 'input';
