-- Phase 1 (spec 2026-06-25 §R6) — switch the daily check-in to a visible weekly 7-day cycle.
-- Data-only: seed the single weekly-credits key, then retire the v1 ramp/milestone/cap keys the
-- code no longer reads.
INSERT INTO app_config (key, value, updated_at)
VALUES ('daily_checkin_weekly_credits', '2,3,4,4,4,4,10', NOW())
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();

-- Retire the v1 curve keys (code no longer reads them).
DELETE FROM app_config WHERE key IN (
  'daily_checkin_base_credits',
  'daily_checkin_per_day_bonus',
  'daily_checkin_max_streak_for_bonus',
  'daily_checkin_milestone_bonuses',
  'daily_checkin_daily_cap'
);
