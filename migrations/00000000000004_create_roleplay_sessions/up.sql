CREATE TABLE roleplay_sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    character_id        UUID NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    scene_id            UUID NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    status              VARCHAR NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'paused', 'ended')),
    mood                VARCHAR NOT NULL DEFAULT 'neutral'
                        CHECK (mood IN ('neutral', 'happy', 'sad', 'excited', 'angry', 'shy', 'playful', 'serious', 'worried')),
    relationship_level  VARCHAR NOT NULL DEFAULT 'stranger'
                        CHECK (relationship_level IN ('stranger', 'acquaintance', 'friend', 'close_friend')),
    message_count       INTEGER NOT NULL DEFAULT 0,
    current_location    VARCHAR(255),
    scene_time          VARCHAR(50),
    scene_summary       TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_roleplay_sessions_user_id ON roleplay_sessions (user_id);
CREATE INDEX idx_roleplay_sessions_character_id ON roleplay_sessions (character_id);
CREATE INDEX idx_roleplay_sessions_active
    ON roleplay_sessions (user_id) WHERE status = 'active';
