-- Dropping the primary key left this column NOT NULL, so blueprint-scoped values,
-- which have no attribute, could never be saved.
ALTER TABLE extension_scoped_configuration
    ALTER COLUMN attribute_id DROP NOT NULL;
