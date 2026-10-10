-- Files uploaded for a file attribute before their record exists. The binding
-- names the uploading user, the blueprint family, the attribute and the
-- context the file was validated for; creating a record claims the file and
-- clears the binding. Blueprints and contexts are matched by ID without a
-- foreign key so that an unclaimed upload never blocks deleting them.
ALTER TABLE files
    ADD COLUMN staged_upload_user_id UUID REFERENCES users (id),
    ADD COLUMN staged_upload_blueprint_id UUID,
    ADD COLUMN staged_upload_attribute_code TEXT,
    ADD COLUMN staged_upload_context_id UUID,
    ADD CONSTRAINT files_staged_upload_binding_check CHECK (
        (staged_upload_user_id IS NULL
            AND staged_upload_blueprint_id IS NULL
            AND staged_upload_attribute_code IS NULL
            AND staged_upload_context_id IS NULL)
        OR (staged_upload_user_id IS NOT NULL
            AND staged_upload_blueprint_id IS NOT NULL
            AND staged_upload_attribute_code IS NOT NULL
            AND staged_upload_context_id IS NOT NULL
            AND attachment_expires_at IS NOT NULL)
    );
