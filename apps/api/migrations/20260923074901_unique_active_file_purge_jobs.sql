-- Only one pending or completed purge is needed per file. Failed jobs may
-- remain as historical evidence while a later reconciliation retries the purge.
CREATE UNIQUE INDEX file_processing_jobs_one_active_purge_idx
    ON file_processing_jobs (file_id, kind)
    WHERE kind = 'purge' AND status IN ('queued', 'running', 'retryable', 'completed');
