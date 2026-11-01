-- A file a person chose for an extension action, bound to them and to that
-- extension until a run claims it. It expires like other unclaimed uploads.
ALTER TABLE files
    ADD COLUMN extension_input_user_id UUID REFERENCES users (id),
    ADD COLUMN extension_input_extension_id TEXT,
    ADD CONSTRAINT files_extension_input_binding_check CHECK (
        (extension_input_user_id IS NULL AND extension_input_extension_id IS NULL)
        OR (extension_input_user_id IS NOT NULL AND extension_input_extension_id IS NOT NULL)
    );
