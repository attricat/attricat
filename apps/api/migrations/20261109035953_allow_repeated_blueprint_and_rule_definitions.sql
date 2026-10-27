-- A revision may return to an earlier definition, such as removing a field
-- that the previous revision added, and an embedded rule keeps its definition
-- across blueprint revisions that do not change it. Only a save identical to
-- the latest revision is refused, by the repository.
ALTER TABLE blueprints DROP CONSTRAINT blueprints_id_definition_hash_key;
ALTER TABLE rules DROP CONSTRAINT rules_id_definition_hash_key;
