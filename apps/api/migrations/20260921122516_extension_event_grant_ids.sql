-- Event-subscription grants use the stable <provider-id>:<contract-id> key.
ALTER TABLE extension_grants
    DROP CONSTRAINT extension_grants_grant_id_check,
    ADD CONSTRAINT extension_grants_grant_id_check
        CHECK (grant_id ~ '^[A-Za-z0-9._:-]{1,128}$');
