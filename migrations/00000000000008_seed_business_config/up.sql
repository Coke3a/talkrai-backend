INSERT INTO app_config (key, value) VALUES
    ('ai_max_tokens', '1024'),
    ('welcome_credits', '20'),
    ('relationship_threshold_acquaintance', '20'),
    ('relationship_threshold_friend', '40'),
    ('relationship_threshold_close_friend', '60'),
    ('summarize_interval', '10')
ON CONFLICT (key) DO NOTHING;
