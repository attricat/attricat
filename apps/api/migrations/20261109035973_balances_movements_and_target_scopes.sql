-- Balances are number attributes that only movements write, such as a stock
-- level. A movement record (a goods receipt or issue) changes the balance of
-- the record its blueprint's `movements` find, inside the record's create
-- transaction. `target_scope` filters a relationship's record picker.
ALTER TABLE attributes ADD COLUMN balance JSONB;
ALTER TABLE attributes ADD COLUMN target_scope JSONB;
ALTER TABLE blueprints ADD COLUMN movements JSONB NOT NULL DEFAULT '[]'::jsonb;
