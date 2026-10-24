-- When a mapping last settled (synced, withdrawn or rejected). Unlike
-- updated_at, leases and releases do not move it, so a resync can tell a
-- record settled before it started from one it settled itself.
ALTER TABLE extension_export_mappings ADD COLUMN settled_at TIMESTAMPTZ;
