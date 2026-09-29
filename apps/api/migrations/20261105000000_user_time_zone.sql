-- NULL follows the browser time zone. Values are IANA zone names validated by
-- the API; the database only rejects empty strings.
ALTER TABLE users ADD COLUMN time_zone TEXT CHECK (time_zone <> '');
