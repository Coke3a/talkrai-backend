CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    line_user_id    VARCHAR(64) NOT NULL UNIQUE,
    display_name    VARCHAR(255) NOT NULL,
    picture_url     TEXT,
    language        VARCHAR(10) NOT NULL DEFAULT 'th',
    terms_accepted_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_users_line_user_id ON users (line_user_id);
