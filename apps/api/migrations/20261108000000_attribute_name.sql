-- Optional human-readable label declared on a blueprint attribute. Clients
-- fall back to the humanized code when it is absent.
ALTER TABLE attributes ADD COLUMN name TEXT CHECK (name IS NULL OR btrim(name) <> '');
