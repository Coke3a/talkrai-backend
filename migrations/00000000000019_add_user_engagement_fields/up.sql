-- Phase 1 (spec 2026-06-24 §C.2) — daily check-in / streak / re-engagement columns on users.
--
-- Day granularity in Asia/Bangkok: last_check_in_on doubles as "last day the user engaged", so we
-- do NOT add a high-churn last_active_at (that would rewrite the user row on every message).
-- longest_streak backs the /profile "สถิติสูงสุด X วัน" display (spec §B.4).
ALTER TABLE users
  ADD COLUMN check_in_streak       INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN longest_streak        INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN last_check_in_on      DATE,
  ADD COLUMN last_reminder_sent_on DATE;

-- Powers the daily re-engagement query (active users idle today, engaged recently).
-- Partial index keeps it small: only active users, only the range-scanned column.
CREATE INDEX idx_users_reengagement
  ON users (last_check_in_on)
  WHERE status = 'active';
