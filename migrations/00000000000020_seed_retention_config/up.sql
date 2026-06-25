-- Phase 1 (spec 2026-06-24 §C.8) — seed the runtime-tunable retention config keys.
-- The check-in hot path and the re-engagement batch path require these keys (they error if a key
-- is missing, exactly like the relationship_threshold_* keys). All values are strings (app_config
-- stores VARCHAR); the usecases parse them. milestone_bonuses is "day:bonus,day:bonus,...".
INSERT INTO app_config (key, value) VALUES
    ('daily_checkin_base_credits',         '4'),
    ('daily_checkin_per_day_bonus',        '1'),
    ('daily_checkin_max_streak_for_bonus', '10'),
    ('daily_checkin_milestone_bonuses',    '7:20,14:30,30:60'),
    ('daily_checkin_daily_cap',            '30'),
    ('reengagement_window_days',           '14'),
    ('reengagement_batch_cap',             '2000')
ON CONFLICT (key) DO NOTHING;
