INSERT INTO app_config (key, value) VALUES
    ('checkin_base_credits', '2'),
    ('checkin_bonus_day', '7'),
    ('checkin_bonus_credits', '6')
ON CONFLICT (key) DO NOTHING;
