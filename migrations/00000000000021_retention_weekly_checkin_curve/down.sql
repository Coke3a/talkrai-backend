-- Restore v1 exactly (spec 2026-06-25 §R6).
DELETE FROM app_config WHERE key = 'daily_checkin_weekly_credits';
INSERT INTO app_config (key, value, updated_at) VALUES
  ('daily_checkin_base_credits', '4', NOW()),
  ('daily_checkin_per_day_bonus', '1', NOW()),
  ('daily_checkin_max_streak_for_bonus', '10', NOW()),
  ('daily_checkin_milestone_bonuses', '7:20,14:30,30:60', NOW()),
  ('daily_checkin_daily_cap', '30', NOW())
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW();
