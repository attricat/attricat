-- Per-contract inter-extension event grants. Contract declarations remain in the
-- immutable installed manifest; this table records workspace operator consent.
ALTER TABLE extension_grants
    DROP CONSTRAINT extension_grants_grant_kind_check,
    ADD CONSTRAINT extension_grants_grant_kind_check
        CHECK (grant_kind IN ('capability', 'host_permission', 'event_publish', 'event_subscribe'));
