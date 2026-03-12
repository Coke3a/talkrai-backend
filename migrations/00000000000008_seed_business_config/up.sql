INSERT INTO app_config (key, value) VALUES
    ('active_llm_provider', 'together'),
    ('ai_max_tokens', '600'),
    ('welcome_credits', '20'),
    ('relationship_threshold_acquaintance', '20'),
    ('relationship_threshold_friend', '40'),
    ('relationship_threshold_close_friend', '60'),
    ('summarize_interval', '10')
ON CONFLICT (key) DO NOTHING;
