CREATE TABLE characters (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(100) NOT NULL,
    personality     TEXT NOT NULL,
    speaking_style  TEXT NOT NULL,
    background      TEXT NOT NULL,
    system_prompt   TEXT NOT NULL,
    avatar_url      TEXT,
    appearance_prompt TEXT,
    genre_tags      TEXT[] NOT NULL DEFAULT '{}',
    gender VARCHAR NOT NULL DEFAULT 'male' CHECK (gender IN ('male', 'female')),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_characters_is_active ON characters (is_active) WHERE is_active = TRUE;
