DROP INDEX IF EXISTS idx_users_reengagement;

ALTER TABLE users
  DROP COLUMN check_in_streak,
  DROP COLUMN longest_streak,
  DROP COLUMN last_check_in_on,
  DROP COLUMN last_reminder_sent_on;
