-- Every claim gets a fresh token so an expired worker cannot acknowledge
-- or fail a replacement worker's attempt, even with the same worker ID.
ALTER TABLE file_processing_jobs ADD COLUMN lease_token UUID;
