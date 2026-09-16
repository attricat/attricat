CREATE TABLE password_reset_rate_limits (
    key_digest BYTEA PRIMARY KEY,
    window_started_at TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts > 0)
);
