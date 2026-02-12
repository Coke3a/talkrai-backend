CREATE TABLE messages (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      UUID NOT NULL REFERENCES roleplay_sessions(id) ON DELETE CASCADE,
    role            VARCHAR NOT NULL
                    CHECK (role IN ('user', 'narrator', 'character')),
    message_type    VARCHAR NOT NULL
                    CHECK (message_type IN ('dialogue', 'action', 'mixed', 'narration', 'system')),
    content         TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_messages_session_id ON messages (session_id);
CREATE INDEX idx_messages_session_created ON messages (session_id, created_at);

CREATE TABLE jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      UUID REFERENCES roleplay_sessions(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    line_user_id    VARCHAR(64) NOT NULL,
    user_message    TEXT NOT NULL,
    mode            VARCHAR NOT NULL DEFAULT 'roleplay_message',
    status          VARCHAR NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    max_attempts    INTEGER NOT NULL DEFAULT 3,
    locked_at       TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    failed_reason   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_jobs_pending ON jobs (created_at) WHERE status = 'pending';
CREATE INDEX idx_jobs_processing ON jobs (locked_at) WHERE status = 'processing';
CREATE INDEX idx_jobs_user_id ON jobs (user_id);
CREATE INDEX idx_jobs_mode ON jobs (mode);
