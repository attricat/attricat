-- The outcome of extracting text from a PDF, Word or Excel file. Extracted text
-- is stored as the file's `text` variant; this row records which extractor
-- version produced it, or why the file has no text.
CREATE TABLE file_text_extractions (
    file_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    extractor_version INTEGER NOT NULL CHECK (extractor_version > 0),
    status TEXT NOT NULL CHECK (status IN ('ready', 'failed')),
    failure_reason TEXT CHECK (
        failure_reason IN ('encrypted', 'no_text_layer', 'too_large', 'malformed', 'unsupported', 'timed_out')
    ),
    section_count INTEGER NOT NULL DEFAULT 0 CHECK (section_count >= 0),
    char_count BIGINT NOT NULL DEFAULT 0 CHECK (char_count >= 0),
    truncated BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((status = 'failed') = (failure_reason IS NOT NULL)),
    FOREIGN KEY (workspace_id, file_id) REFERENCES files (workspace_id, id)
);

-- Upload and backfill both queue text extraction; one pending job per file is enough.
CREATE UNIQUE INDEX file_processing_jobs_one_active_text_extraction_idx
    ON file_processing_jobs (file_id, kind)
    WHERE kind = 'text_extraction' AND status IN ('queued', 'running', 'retryable');

-- Reconciliation looks for documents without current text on every pass.
CREATE INDEX files_text_extraction_candidates_idx
    ON files (mime_type, created_at, id)
    WHERE deleted_at IS NULL;
