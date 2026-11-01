-- A development build of an installed release's server component. With
-- ATTRICAT_DEVTOOLS on, `acli extension dev` replaces the component the runtime
-- loads for that release without a new upload or new grants; the revision names
-- the stored bytes and selects the compiled component.
CREATE TABLE extension_dev_server_artifacts (
    installed_release_id UUID PRIMARY KEY REFERENCES installed_extension_releases (id) ON DELETE CASCADE,
    revision TEXT NOT NULL CHECK (revision ~ '^[0-9a-f]{64}$'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
