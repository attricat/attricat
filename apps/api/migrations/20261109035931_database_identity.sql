-- A random identity for this database, created once by the API at startup.
-- Shared Redis keys include it, so deployments or recreated databases that
-- use one Redis never read each other's cached entries.
CREATE TABLE database_identity (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
