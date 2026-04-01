CREATE INDEX idx_jobs_session_active
    ON jobs (session_id)
    WHERE status IN ('pending', 'processing');
