-- Attribute writes create monthly history partitions through this existing
-- controlled function. Request connections use catalog_api rather than the
-- table owner, so retain the operation as a narrowly scoped definer function
-- instead of granting schema DDL to the API role.
ALTER FUNCTION ensure_attribute_value_history_partition(TIMESTAMPTZ)
    SECURITY DEFINER
    SET search_path = public, pg_temp;
REVOKE ALL ON FUNCTION ensure_attribute_value_history_partition(TIMESTAMPTZ) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION ensure_attribute_value_history_partition(TIMESTAMPTZ) TO catalog_api;
