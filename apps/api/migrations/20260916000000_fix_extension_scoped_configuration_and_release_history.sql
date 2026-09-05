-- Nullable blueprint scopes require a nullable-aware key. Release snapshots are
-- immutable lifecycle history, so a reinstallation may record the same version.
ALTER TABLE extension_scoped_configuration
    DROP CONSTRAINT extension_scoped_configuration_pkey;
ALTER TABLE extension_scoped_configuration
    ADD COLUMN id UUID NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY;
ALTER TABLE extension_scoped_configuration
    ADD CONSTRAINT extension_scoped_configuration_scope_key
    UNIQUE NULLS NOT DISTINCT (workspace_id, extension_id, scope_kind, blueprint_id, blueprint_version, attribute_id);

ALTER TABLE installed_extension_releases
    -- PostgreSQL truncates implicit constraint names to 63 bytes while preserving
-- the `_key` suffix.
    DROP CONSTRAINT installed_extension_releases_workspace_id_extension_id_vers_key;
