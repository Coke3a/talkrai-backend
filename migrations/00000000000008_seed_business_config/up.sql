INSERT INTO app_config (key, value) VALUES
    ('ai_max_tokens', '1024'),
    ('welcome_credits', '50'),
    ('relationship_threshold_acquaintance', '20'),
    ('relationship_threshold_friend', '40'),
    ('relationship_threshold_close_friend', '60')
ON CONFLICT (key) DO NOTHING;
