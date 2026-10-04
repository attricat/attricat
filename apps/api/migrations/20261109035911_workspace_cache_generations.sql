-- Monotonic counters that cached reads key on. Repository write
-- transactions advance the counter of what they change, so a reader that has
-- read a generation in its own query never uses data older than it.
ALTER TABLE workspaces
    ADD COLUMN catalog_generation BIGINT NOT NULL DEFAULT 0 CHECK (catalog_generation >= 0),
    ADD COLUMN contexts_generation BIGINT NOT NULL DEFAULT 0 CHECK (contexts_generation >= 0),
    ADD COLUMN extensions_generation BIGINT NOT NULL DEFAULT 0 CHECK (extensions_generation >= 0);
