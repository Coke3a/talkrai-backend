CREATE TABLE user_streaks (
    user_id            UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    current_streak     INTEGER NOT NULL DEFAULT 0,
    last_check_in_date DATE,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE daily_check_ins (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    checked_in_date DATE NOT NULL,
    streak_day      INTEGER NOT NULL,
    credits_earned  INTEGER NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_daily_check_ins_user_date UNIQUE(user_id, checked_in_date),
    CONSTRAINT daily_check_ins_streak_day_range CHECK (streak_day BETWEEN 1 AND 7),
    CONSTRAINT daily_check_ins_credits_positive CHECK (credits_earned > 0)
);

CREATE INDEX idx_daily_check_ins_user_id ON daily_check_ins(user_id);
CREATE INDEX idx_daily_check_ins_date ON daily_check_ins(user_id, checked_in_date DESC);
