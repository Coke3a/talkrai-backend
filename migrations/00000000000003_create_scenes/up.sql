CREATE TABLE scenes (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    character_id        UUID NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    name                VARCHAR(100) NOT NULL,
    location            VARCHAR(255) NOT NULL,
    time_of_day         VARCHAR(50) NOT NULL,
    atmosphere          TEXT NOT NULL,
    situation_prompt    TEXT NOT NULL,
    opening_narrator    TEXT NOT NULL,
    opening_dialogue    TEXT NOT NULL,
    is_default          BOOLEAN NOT NULL DEFAULT FALSE,
    is_active                   BOOLEAN NOT NULL DEFAULT TRUE,
    start_relationship_level    VARCHAR NOT NULL DEFAULT 'stranger'
                                CHECK (start_relationship_level IN ('stranger', 'acquaintance', 'friend', 'close_friend')),
    start_mood                  VARCHAR NOT NULL DEFAULT 'neutral'
                                CHECK (start_mood IN ('neutral', 'happy', 'sad', 'excited', 'angry', 'shy', 'playful', 'serious', 'worried')),
    image_url           TEXT,
    is_adult_content    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_scenes_character_id ON scenes (character_id);
CREATE UNIQUE INDEX idx_scenes_default_per_character
    ON scenes (character_id) WHERE is_default = TRUE AND is_active = TRUE;
