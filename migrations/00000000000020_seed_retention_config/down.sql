DELETE FROM app_config WHERE key IN (
    'daily_checkin_base_credits',
    'daily_checkin_per_day_bonus',
    'daily_checkin_max_streak_for_bonus',
    'daily_checkin_milestone_bonuses',
    'daily_checkin_daily_cap',
    'reengagement_window_days',
    'reengagement_batch_cap'
);
